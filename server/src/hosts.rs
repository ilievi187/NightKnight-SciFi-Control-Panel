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
    pub error: Option<String>,
}

/// Configuration for a remote host.
#[derive(Clone)]
pub struct HostConfig {
    pub id: String,
    pub address: String,
    pub port: u16,
    pub is_local: bool,
}

/// Parse host configuration from environment variable.
/// Format: "host1:port,host2:port" or just "host1,host2" (default port 8080)
pub fn parse_host_config() -> Vec<HostConfig> {
    let mut hosts = Vec::new();

    // Always include local host
    hosts.push(HostConfig {
        id: "local".to_string(),
        address: "127.0.0.1".to_string(),
        port: 8080,
        is_local: true,
    });

    // Parse additional hosts from HOSTS env var
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
            });
        }
    }

    hosts
}

/// Probe a host for connectivity and collect basic info.
pub async fn probe_host(config: &HostConfig) -> HostEntry {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if config.is_local {
        // Local host - read directly
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

        return HostEntry {
            id: config.id.clone(),
            hostname,
            operating_system: os,
            connection_status: "connected".to_string(),
            telemetry_source: "local".to_string(),
            is_local: true,
            last_heartbeat_unix: Some(now),
            cpu_percent: None,
            memory_percent: None,
            gpu_name: None,
            gpu_utilization: None,
            error: None,
        };
    }

    // Remote host - try HTTP probe
    let url = format!("http://{}:{}/api/health", config.address, config.port);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    match client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            // Try to get metrics
            let metrics_url = format!("http://{}:{}/api/metrics", config.address, config.port);
            let (cpu, memory, gpu_name, gpu_util) = match client.get(&metrics_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    // Parse metrics (simplified - just check availability)
                    (None, None, None, None)
                }
                _ => (None, None, None, None),
            };

            HostEntry {
                id: config.id.clone(),
                hostname: config.address.clone(),
                operating_system: "Remote host".to_string(),
                connection_status: "connected".to_string(),
                telemetry_source: "remote".to_string(),
                is_local: false,
                last_heartbeat_unix: Some(now),
                cpu_percent: cpu,
                memory_percent: memory,
                gpu_name,
                gpu_utilization: gpu_util,
                error: None,
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

/// List all configured hosts with their status.
pub async fn list_hosts() -> Vec<HostEntry> {
    let configs = parse_host_config();
    let mut hosts = Vec::new();
    for config in &configs {
        hosts.push(probe_host(config).await);
    }
    hosts
}