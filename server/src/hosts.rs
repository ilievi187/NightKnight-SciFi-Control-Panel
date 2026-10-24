use serde::Deserialize;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

/// Read-only host registry entry.
#[derive(Serialize, Clone)]
pub struct HostEntry {
    pub id: String,
    pub hostname: String,
    pub operating_system: String,
    pub connection_status: String,
    pub telemetry_source: String,
    pub is_local: bool,
    pub last_heartbeat_unix: Option<u64>,
    pub cpu_percent: Option<f64>,
    pub memory_percent: Option<f64>,
    pub gpu_name: Option<String>,
    pub gpu_utilization: Option<u8>,
    pub gpu_temperature: Option<f64>,
    pub vram_used_bytes: Option<u64>,
    pub vram_total_bytes: Option<u64>,
    pub error: Option<String>,
}

/// Configuration for a remote host.
#[derive(Clone)]
pub struct HostConfig {
    pub id: String,
    pub address: String,
    pub port: u16,
    pub is_local: bool,
    /// Optional auth token for remote hosts. Never logged or exposed.
    pub auth_token: Option<String>,
}

impl std::fmt::Debug for HostConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostConfig")
            .field("id", &self.id)
            .field("address", &self.address)
            .field("port", &self.port)
            .field("is_local", &self.is_local)
            .field("auth_token", &"[REDACTED]")
            .finish()
    }
}
/// Format: "host1:port,host2:port" or just "host1,host2" (default port 8080)
/// Auth token can be set via NIGHTKNIGHT_HOST_TOKEN env var (applied to all remote hosts).
pub fn parse_host_config() -> Vec<HostConfig> {
    let mut hosts = Vec::new();

    // Always include local host
    hosts.push(HostConfig {
        id: "local".to_string(),
        address: "127.0.0.1".to_string(),
        port: 8080,
        is_local: true,
        auth_token: None,
    });

    // Parse additional hosts from NIGHTKNIGHT_HOSTS env var
    if let Ok(hosts_str) = std::env::var("NIGHTKNIGHT_HOSTS") {
        for (i, host) in hosts_str.split(',').enumerate() {
            let host = host.trim();
            if host.is_empty() {
                continue;
            }
            let (address, port) = if let Some((addr, port_str)) = host.split_once(':') {
                let port = port_str.parse().unwrap_or(8080);
                (addr.to_string(), port)
            } else {
                (host.to_string(), 8080)
            };
            hosts.push(HostConfig {
                id: format!("remote-{}", i + 1),
                address,
                port,
                is_local: false,
                auth_token: std::env::var("NIGHTKNIGHT_HOST_TOKEN").ok(),
            });
        }
    }

    hosts
}

/// Remote metrics response (subset of HostMetrics for parsing).
#[derive(Deserialize, Debug)]
struct RemoteMetrics {
    hostname: Option<RemoteMetric<String>>,
    operating_system: Option<RemoteMetric<String>>,
    cpu_usage_percent: Option<RemoteMetric<f64>>,
    memory: Option<RemoteMetric<RemoteMemory>>,
    gpu_metrics: Option<RemoteMetric<Vec<RemoteGpu>>>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
#[serde(tag = "status", rename_all = "snake_case")]
enum RemoteMetric<T> {
    Available { value: T },
    Unavailable { reason: String },
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct RemoteMemory {
    used_percent: f64,
    used_bytes: u64,
    total_bytes: u64,
}

#[derive(Deserialize, Debug)]
struct RemoteGpu {
    name: String,
    utilization_percent: Option<u8>,
    memory_total_bytes: u64,
    memory_used_bytes: u64,
    temperature_celsius: Option<u16>,
}

/// Probe a host for connectivity and collect telemetry.
pub async fn probe_host(config: &HostConfig) -> HostEntry {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if config.is_local {
        return collect_local_host_entry(config, now).await;
    }

    // Remote host - probe via HTTP
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let health_url = format!("http://{}:{}/api/health", config.address, config.port);

    // Build request with optional auth token
    let mut health_req = client.get(&health_url);
    if let Some(token) = &config.auth_token {
        health_req = health_req.header("Authorization", format!("Bearer {}", token));
    }

    match health_req.send().await {
        Ok(response) if response.status().is_success() => {
            // Connected - now fetch metrics
            let metrics_url = format!("http://{}:{}/api/metrics", config.address, config.port);
            let mut metrics_req = client.get(&metrics_url);
            if let Some(token) = &config.auth_token {
                metrics_req = metrics_req.header("Authorization", format!("Bearer {}", token));
            }

            match metrics_req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<RemoteMetrics>().await {
                        Ok(metrics) => {
                            let hostname = match &metrics.hostname {
                                Some(RemoteMetric::Available { value }) => value.clone(),
                                _ => config.address.clone(),
                            };
                            let os = match &metrics.operating_system {
                                Some(RemoteMetric::Available { value }) => value.clone(),
                                _ => "Remote host".to_string(),
                            };
                            let cpu = match &metrics.cpu_usage_percent {
                                Some(RemoteMetric::Available { value }) => Some(*value),
                                _ => None,
                            };
                            let memory = match &metrics.memory {
                                Some(RemoteMetric::Available { value }) => Some(value.used_percent),
                                _ => None,
                            };
                            let (gpu_name, gpu_util, gpu_temp, vram_used, vram_total) =
                                match &metrics.gpu_metrics {
                                    Some(RemoteMetric::Available { value })
                                        if !value.is_empty() =>
                                    {
                                        let g = &value[0];
                                        (
                                            Some(g.name.clone()),
                                            g.utilization_percent,
                                            g.temperature_celsius.map(|t| t as f64),
                                            Some(g.memory_used_bytes),
                                            Some(g.memory_total_bytes),
                                        )
                                    }
                                    _ => (None, None, None, None, None),
                                };

                            HostEntry {
                                id: config.id.clone(),
                                hostname,
                                operating_system: os,
                                connection_status: "connected".to_string(),
                                telemetry_source: "remote".to_string(),
                                is_local: false,
                                last_heartbeat_unix: Some(now),
                                cpu_percent: cpu,
                                memory_percent: memory,
                                gpu_name,
                                gpu_utilization: gpu_util,
                                gpu_temperature: gpu_temp,
                                vram_used_bytes: vram_used,
                                vram_total_bytes: vram_total,
                                error: None,
                            }
                        }
                        Err(e) => HostEntry {
                            id: config.id.clone(),
                            hostname: config.address.clone(),
                            operating_system: "Unknown".to_string(),
                            connection_status: "connected".to_string(),
                            telemetry_source: "remote".to_string(),
                            is_local: false,
                            last_heartbeat_unix: Some(now),
                            cpu_percent: None,
                            memory_percent: None,
                            gpu_name: None,
                            gpu_utilization: None,
                            gpu_temperature: None,
                            vram_used_bytes: None,
                            vram_total_bytes: None,
                            error: Some(format!("Invalid metrics response: {}", e)),
                        },
                    }
                }
                _ => HostEntry {
                    id: config.id.clone(),
                    hostname: config.address.clone(),
                    operating_system: "Unknown".to_string(),
                    connection_status: "connected".to_string(),
                    telemetry_source: "remote".to_string(),
                    is_local: false,
                    last_heartbeat_unix: Some(now),
                    cpu_percent: None,
                    memory_percent: None,
                    gpu_name: None,
                    gpu_utilization: None,
                    gpu_temperature: None,
                    vram_used_bytes: None,
                    vram_total_bytes: None,
                    error: Some("Metrics endpoint unavailable".to_string()),
                },
            }
        }
        Ok(response) => HostEntry {
            id: config.id.clone(),
            hostname: config.address.clone(),
            operating_system: "Unknown".to_string(),
            connection_status: "disconnected".to_string(),
            telemetry_source: "none".to_string(),
            is_local: false,
            last_heartbeat_unix: None,
            cpu_percent: None,
            memory_percent: None,
            gpu_name: None,
            gpu_utilization: None,
            gpu_temperature: None,
            vram_used_bytes: None,
            vram_total_bytes: None,
            error: Some(format!("HTTP {}", response.status())),
        },
        Err(e) => HostEntry {
            id: config.id.clone(),
            hostname: config.address.clone(),
            operating_system: "Unknown".to_string(),
            connection_status: "unavailable".to_string(),
            telemetry_source: "none".to_string(),
            is_local: false,
            last_heartbeat_unix: None,
            cpu_percent: None,
            memory_percent: None,
            gpu_name: None,
            gpu_utilization: None,
            gpu_temperature: None,
            vram_used_bytes: None,
            vram_total_bytes: None,
            error: Some(if e.is_timeout() {
                "Connection timed out".to_string()
            } else if e.is_connect() {
                "Connection refused".to_string()
            } else {
                "Network error".to_string()
            }),
        },
    }
}

async fn collect_local_host_entry(config: &HostConfig, now: u64) -> HostEntry {
    let hostname = std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let os = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                line.strip_prefix("PRETTY_NAME=")
                    .map(|value| value.trim_matches('"').to_string())
            })
        })
        .unwrap_or_else(|| "Unknown".to_string());

    // Collect real local metrics
    let metrics = crate::metrics::collect_host_metrics().await;

    let cpu_percent = match &metrics.cpu_usage_percent {
        crate::metrics::Metric::Available { value } => Some(*value),
        _ => None,
    };

    let memory_percent = match &metrics.memory {
        crate::metrics::Metric::Available { value } => Some(value.used_percent),
        _ => None,
    };

    let (gpu_name, gpu_util, gpu_temp, vram_used, vram_total) = match &metrics.gpu_metrics {
        crate::metrics::Metric::Available { value } if !value.is_empty() => {
            let g = &value[0];
            (
                Some(g.name.clone()),
                g.utilization_percent,
                g.temperature_celsius.map(|t| t as f64),
                Some(g.memory_used_bytes),
                Some(g.memory_total_bytes),
            )
        }
        _ => (None, None, None, None, None),
    };

    HostEntry {
        id: config.id.clone(),
        hostname,
        operating_system: os,
        connection_status: "connected".to_string(),
        telemetry_source: "local".to_string(),
        is_local: true,
        last_heartbeat_unix: Some(now),
        cpu_percent,
        memory_percent,
        gpu_name,
        gpu_utilization: gpu_util,
        gpu_temperature: gpu_temp,
        vram_used_bytes: vram_used,
        vram_total_bytes: vram_total,
        error: None,
    }
}

/// List all configured hosts with their status.
pub async fn list_hosts() -> Vec<HostEntry> {
    let configs = parse_host_config();
    let mut hosts = Vec::new();
    for config in &configs {
        hosts.push(probe_host(config).await);
    }
    hosts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_host_config_includes_local() {
        let configs = parse_host_config();
        assert!(!configs.is_empty());
        assert!(configs[0].is_local);
        assert_eq!(configs[0].id, "local");
    }

    #[test]
    fn parse_host_config_handles_empty() {
        // Should always have at least local host
        let configs = parse_host_config();
        assert!(!configs.is_empty());
    }

    #[test]
    fn remote_metric_available_parses() {
        let json = r#"{"status":"available","value":42.5}"#;
        let metric: RemoteMetric<f64> = serde_json::from_str(json).unwrap();
        match metric {
            RemoteMetric::Available { value } => assert_eq!(value, 42.5),
            _ => panic!("Expected Available"),
        }
    }

    #[test]
    fn remote_metric_unavailable_parses() {
        let json = r#"{"status":"unavailable","reason":"No data"}"#;
        let metric: RemoteMetric<f64> = serde_json::from_str(json).unwrap();
        match metric {
            RemoteMetric::Unavailable { reason } => assert_eq!(reason, "No data"),
            _ => panic!("Expected Unavailable"),
        }
    }

    #[test]
    fn remote_metrics_full_parses() {
        let json = r#"{
            "hostname": {"status":"available","value":"test-host"},
            "operating_system": {"status":"available","value":"Ubuntu 24.04"},
            "cpu_usage_percent": {"status":"available","value":45.2},
            "memory": {"status":"available","value":{"used_percent":60.5,"used_bytes":1000,"total_bytes":2000}},
            "gpu_metrics": {"status":"available","value":[{"name":"RTX 4090","utilization_percent":80,"memory_total_bytes":24576,"memory_used_bytes":8192,"temperature_celsius":65}]}
        }"#;
        let metrics: RemoteMetrics = serde_json::from_str(json).unwrap();
        assert!(matches!(
            metrics.hostname,
            Some(RemoteMetric::Available { .. })
        ));
        assert!(matches!(
            metrics.cpu_usage_percent,
            Some(RemoteMetric::Available { .. })
        ));
        if let Some(RemoteMetric::Available { value }) = &metrics.gpu_metrics {
            assert_eq!(value[0].name, "RTX 4090");
            assert_eq!(value[0].utilization_percent, Some(80));
        } else {
            panic!("Expected GPU available");
        }
    }

    #[test]
    fn remote_metrics_missing_fields_handled() {
        let json = r#"{}"#;
        let metrics: RemoteMetrics = serde_json::from_str(json).unwrap();
        assert!(metrics.hostname.is_none());
        assert!(metrics.cpu_usage_percent.is_none());
        assert!(metrics.gpu_metrics.is_none());
    }

    #[test]
    fn remote_metrics_malformed_json_fails() {
        let json = r#"{invalid json}"#;
        let result: Result<RemoteMetrics, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
