use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Metric<T> {
    Available { value: T },
    Unavailable { reason: String },
}

impl<T> Metric<T> {
    fn available(value: T) -> Self {
        Self::Available { value }
    }

    fn unavailable(reason: impl Into<String>) -> Self {
        Self::Unavailable {
            reason: reason.into(),
        }
    }
}

#[derive(Serialize)]
pub struct MemoryMetrics {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub used_percent: f64,
}

#[derive(Serialize)]
pub struct DiskMetrics {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub used_percent: f64,
    pub mount_point: String,
}

#[derive(Serialize)]
pub struct NetworkInterfaceMetrics {
    pub name: String,
    pub received_bytes: u64,
    pub transmitted_bytes: u64,
}

#[derive(Serialize)]
pub struct IntegrationStatus {
    pub name: String,
    pub status: &'static str,
    pub reason: &'static str,
}

#[derive(Serialize)]
pub struct HostMetrics {
    pub generated_at_unix: u64,
    pub hostname: Metric<String>,
    pub operating_system: Metric<String>,
    pub cpu_usage_percent: Metric<f64>,
    pub memory: Metric<MemoryMetrics>,
    pub uptime_seconds: Metric<f64>,
    pub root_disk: Metric<DiskMetrics>,
    pub network_interfaces: Metric<Vec<NetworkInterfaceMetrics>>,
    pub integrations: Vec<IntegrationStatus>,
}

pub async fn collect_host_metrics() -> HostMetrics {
    let generated_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    HostMetrics {
        generated_at_unix,
        hostname: hostname(),
        operating_system: operating_system(),
        cpu_usage_percent: cpu_usage_percent().await,
        memory: memory(),
        uptime_seconds: uptime(),
        root_disk: root_disk(),
        network_interfaces: network_interfaces(),
        integrations: unavailable_integrations(),
    }
}

fn unavailable_integrations() -> Vec<IntegrationStatus> {
    [
        "Container runtime",
        "GPU telemetry",
        "Remote probes",
        "Cloud provider APIs",
    ]
    .into_iter()
    .map(|name| IntegrationStatus {
        name: name.to_string(),
        status: "unavailable",
        reason: "No local source or endpoint is configured.",
    })
    .collect()
}

#[cfg(target_os = "linux")]
fn hostname() -> Metric<String> {
    match std::fs::read_to_string("/etc/hostname") {
        Ok(name) if !name.trim().is_empty() => Metric::available(name.trim().to_string()),
        _ => Metric::unavailable("The host name source /etc/hostname is not readable."),
    }
}

#[cfg(not(target_os = "linux"))]
fn hostname() -> Metric<String> {
    Metric::unavailable("Host name collection is only implemented for Linux.")
}

#[cfg(target_os = "linux")]
fn operating_system() -> Metric<String> {
    let result = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                line.strip_prefix("PRETTY_NAME=")
                    .map(|value| value.trim_matches('"').to_string())
            })
        });
    match result {
        Some(name) if !name.is_empty() => Metric::available(name),
        _ => Metric::unavailable("The operating system source /etc/os-release is not readable."),
    }
}

#[cfg(not(target_os = "linux"))]
fn operating_system() -> Metric<String> {
    Metric::unavailable("Operating system collection is only implemented for Linux.")
}

#[cfg(target_os = "linux")]
async fn cpu_usage_percent() -> Metric<f64> {
    let first = match read_cpu_counters() {
        Some(counters) => counters,
        None => return Metric::unavailable("Linux CPU counters in /proc/stat are unavailable."),
    };
    tokio::time::sleep(std::time::Duration::from_millis(160)).await;
    let second = match read_cpu_counters() {
        Some(counters) => counters,
        None => return Metric::unavailable("Linux CPU counters in /proc/stat are unavailable."),
    };
    let total_delta = second.0.saturating_sub(first.0);
    let idle_delta = second.1.saturating_sub(first.1);
    if total_delta == 0 {
        return Metric::unavailable("Linux CPU counters did not advance during the sample.");
    }
    let busy_delta = total_delta.saturating_sub(idle_delta);
    let percent = (busy_delta as f64 * 100.0 / total_delta as f64).clamp(0.0, 100.0);
    Metric::available((percent * 10.0).round() / 10.0)
}

#[cfg(not(target_os = "linux"))]
async fn cpu_usage_percent() -> Metric<f64> {
    Metric::unavailable("CPU utilization collection is only implemented for Linux.")
}

#[cfg(target_os = "linux")]
fn read_cpu_counters() -> Option<(u64, u64)> {
    let contents = std::fs::read_to_string("/proc/stat").ok()?;
    parse_cpu_counters(contents.lines().next()?)
}

#[cfg(target_os = "linux")]
fn parse_cpu_counters(line: &str) -> Option<(u64, u64)> {
    let mut fields = line.split_whitespace();
    if fields.next()? != "cpu" {
        return None;
    }
    let counters: Vec<u64> = fields.map(str::parse).collect::<Result<_, _>>().ok()?;
    if counters.len() < 4 {
        return None;
    }
    // Linux guest and guest_nice are already included in user and nice.
    let total = counters.iter().take(8).copied().sum();
    let idle = counters[3].saturating_add(counters.get(4).copied().unwrap_or(0));
    Some((total, idle))
}

#[cfg(target_os = "linux")]
fn memory() -> Metric<MemoryMetrics> {
    let data = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|contents| parse_memory_kib(&contents));
    match data {
        Some((total_kib, available_kib)) if total_kib > 0 => {
            let total_bytes = total_kib.saturating_mul(1024);
            let available_bytes = available_kib.min(total_kib).saturating_mul(1024);
            let used_bytes = total_bytes.saturating_sub(available_bytes);
            Metric::available(MemoryMetrics {
                total_bytes,
                available_bytes,
                used_bytes,
                used_percent: used_bytes as f64 * 100.0 / total_bytes as f64,
            })
        }
        _ => Metric::unavailable("Linux memory data in /proc/meminfo is unavailable."),
    }
}

#[cfg(not(target_os = "linux"))]
fn memory() -> Metric<MemoryMetrics> {
    Metric::unavailable("Memory collection is only implemented for Linux.")
}

#[cfg(target_os = "linux")]
fn parse_memory_kib(contents: &str) -> Option<(u64, u64)> {
    let mut total = None;
    let mut available = None;
    for line in contents.lines() {
        let (key, value) = line.split_once(':')?;
        let kib = value.split_whitespace().next()?.parse::<u64>().ok()?;
        match key {
            "MemTotal" => total = Some(kib),
            "MemAvailable" => available = Some(kib),
            _ => {}
        }
    }
    Some((total?, available?))
}

#[cfg(target_os = "linux")]
fn uptime() -> Metric<f64> {
    let value = std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|contents| contents.split_whitespace().next()?.parse::<f64>().ok());
    match value {
        Some(seconds) if seconds.is_finite() && seconds >= 0.0 => Metric::available(seconds),
        _ => Metric::unavailable("Linux uptime data in /proc/uptime is unavailable."),
    }
}

#[cfg(not(target_os = "linux"))]
fn uptime() -> Metric<f64> {
    Metric::unavailable("Uptime collection is only implemented for Linux.")
}

#[cfg(target_os = "linux")]
fn root_disk() -> Metric<DiskMetrics> {
    use std::ffi::CString;

    let path = CString::new("/").expect("root path has no nul byte");
    let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
    let result = unsafe { libc::statvfs(path.as_ptr(), &mut stats) };
    if result != 0 {
        return Metric::unavailable("Root filesystem capacity could not be read.");
    }
    let block_size = if stats.f_frsize == 0 {
        stats.f_bsize as u64
    } else {
        stats.f_frsize as u64
    };
    let total_bytes = (stats.f_blocks as u64).saturating_mul(block_size);
    let available_bytes = (stats.f_bavail as u64)
        .saturating_mul(block_size)
        .min(total_bytes);
    let used_bytes = total_bytes.saturating_sub(
        (stats.f_bfree as u64)
            .saturating_mul(block_size)
            .min(total_bytes),
    );
    if total_bytes == 0 {
        return Metric::unavailable("Root filesystem reported zero capacity.");
    }
    Metric::available(DiskMetrics {
        total_bytes,
        available_bytes,
        used_bytes,
        used_percent: used_bytes as f64 * 100.0 / total_bytes as f64,
        mount_point: "/".to_string(),
    })
}

#[cfg(not(target_os = "linux"))]
fn root_disk() -> Metric<DiskMetrics> {
    Metric::unavailable("Root filesystem collection is only implemented for Linux.")
}

#[cfg(target_os = "linux")]
fn network_interfaces() -> Metric<Vec<NetworkInterfaceMetrics>> {
    match std::fs::read_to_string("/proc/net/dev")
        .ok()
        .and_then(|contents| parse_network_interfaces(&contents))
    {
        Some(interfaces) => Metric::available(interfaces),
        None => Metric::unavailable("Linux network counters in /proc/net/dev are unavailable."),
    }
}

#[cfg(not(target_os = "linux"))]
fn network_interfaces() -> Metric<Vec<NetworkInterfaceMetrics>> {
    Metric::unavailable("Network counters are only implemented for Linux.")
}

#[cfg(target_os = "linux")]
fn parse_network_interfaces(contents: &str) -> Option<Vec<NetworkInterfaceMetrics>> {
    let mut interfaces = Vec::new();
    for line in contents.lines().skip(2) {
        let (name, counters) = line.split_once(':')?;
        let mut values = counters.split_whitespace().map(str::parse::<u64>);
        let received_bytes = values.next()?.ok()?;
        for _ in 0..7 {
            values.next()?.ok()?;
        }
        let transmitted_bytes = values.next()?.ok()?;
        interfaces.push(NetworkInterfaceMetrics {
            name: name.trim().to_string(),
            received_bytes,
            transmitted_bytes,
        });
    }
    Some(interfaces)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_memory_totals_from_linux_meminfo() {
        let source = "MemTotal:       8192 kB\nMemAvailable:   4096 kB\n";
        assert_eq!(parse_memory_kib(source), Some((8192, 4096)));
        assert_eq!(parse_memory_kib("MemTotal: 100 kB\n"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_cpu_total_and_idle_counters() {
        assert_eq!(
            parse_cpu_counters("cpu  100 20 30 400 50 0 0 0"),
            Some((600, 450))
        );
        assert_eq!(
            parse_cpu_counters("cpu 100 20 30 400 50 10 20 5 5000 6000"),
            Some((635, 450))
        );
        assert_eq!(parse_cpu_counters("cpu 10 20"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_network_receive_and_transmit_counters() {
        let source = "Inter-| Receive | Transmit\n face |bytes packets errs drop fifo frame compressed multicast|bytes packets errs drop fifo colls carrier compressed\n eth0: 1024 10 0 0 0 0 0 0 2048 20 0 0 0 0 0 0\n";
        let interfaces = parse_network_interfaces(source).unwrap();
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].name, "eth0");
        assert_eq!(interfaces[0].received_bytes, 1024);
        assert_eq!(interfaces[0].transmitted_bytes, 2048);
    }
}
