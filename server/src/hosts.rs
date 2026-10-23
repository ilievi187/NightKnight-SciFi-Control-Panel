use serde::Serialize;

/// Read-only host registry entry.
#[derive(Serialize, Clone)]
pub struct HostEntry {
    pub id: String,
    pub hostname: String,
    pub operating_system: String,
    pub connection_status: &'static str,
    pub telemetry_source: &'static str,
    pub is_local: bool,
}

/// Registry of monitored hosts. Starts with the local host only.
pub fn local_host_entry(hostname: String, os: String) -> HostEntry {
    HostEntry {
        id: "local".to_string(),
        hostname,
        operating_system: os,
        connection_status: "connected",
        telemetry_source: "local",
        is_local: true,
    }
}

/// List of registered hosts. Currently only the local host.
pub async fn list_hosts() -> Vec<HostEntry> {
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

    vec![local_host_entry(hostname, os)]
}