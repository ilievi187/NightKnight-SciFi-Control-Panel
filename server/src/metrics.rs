use serde::Serialize;
#[cfg(target_os = "linux")]
use std::collections::BTreeMap;
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
pub struct StorageVolumeMetrics {
    pub device: String,
    pub filesystem: String,
    pub mount_point: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub used_percent: f64,
}

#[derive(Serialize)]
pub struct NetworkInterfaceMetrics {
    pub name: String,
    pub received_bytes: u64,
    pub transmitted_bytes: u64,
}

#[derive(Serialize)]
pub struct GpuMetrics {
    pub name: String,
    pub utilization_percent: u8,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub temperature_celsius: u16,
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
    pub storage_volumes: Metric<Vec<StorageVolumeMetrics>>,
    pub network_interfaces: Metric<Vec<NetworkInterfaceMetrics>>,
    pub gpu_metrics: Metric<Vec<GpuMetrics>>,
    pub integrations: Vec<IntegrationStatus>,
}

pub async fn collect_host_metrics() -> HostMetrics {
    let generated_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let (cpu_usage_percent, gpu_metrics) = tokio::join!(cpu_usage_percent(), gpu_metrics());

    HostMetrics {
        generated_at_unix,
        hostname: hostname(),
        operating_system: operating_system(),
        cpu_usage_percent,
        memory: memory(),
        uptime_seconds: uptime(),
        root_disk: root_disk(),
        storage_volumes: storage_volumes(),
        network_interfaces: network_interfaces(),
        gpu_metrics,
        integrations: unavailable_integrations(),
    }
}

fn unavailable_integrations() -> Vec<IntegrationStatus> {
    [
        "Container runtime",
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

#[cfg(target_os = "linux")]
fn storage_volumes() -> Metric<Vec<StorageVolumeMetrics>> {
    use std::ffi::CString;

    let contents = match std::fs::read_to_string("/proc/self/mountinfo") {
        Ok(contents) => contents,
        Err(_) => {
            return Metric::unavailable("Linux mount data in /proc/self/mountinfo is unavailable.")
        }
    };

    let mut mounts_by_point: BTreeMap<String, Vec<MountRecord>> = BTreeMap::new();
    for mount in parse_mount_records(&contents) {
        mounts_by_point
            .entry(mount.mount_point.clone())
            .or_default()
            .push(mount);
    }

    let mut volumes = BTreeMap::new();
    for (mount_point, mounts) in mounts_by_point {
        if mount_point.is_empty() {
            continue;
        }
        let Ok(path) = CString::new(mount_point.as_bytes()) else {
            continue;
        };
        let Some(active_mount_id) = mount_id_for_path(&path) else {
            // Without an active mount ID, the mount table cannot reliably tell
            // which record at this path supplies the capacity reported below.
            continue;
        };
        let Some(mount) = select_mount_record(mounts, active_mount_id) else {
            // A mount change between reading mountinfo and statx can leave the
            // active mount absent from this snapshot. Skip the inconsistent row.
            continue;
        };
        if is_virtual_filesystem(&mount.filesystem) {
            continue;
        }

        let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(path.as_ptr(), &mut stats) } != 0 {
            continue;
        }

        let block_size = if stats.f_frsize == 0 {
            stats.f_bsize as u64
        } else {
            stats.f_frsize as u64
        };
        let total_bytes = (stats.f_blocks as u64).saturating_mul(block_size);
        if total_bytes == 0 {
            continue;
        }
        let available_bytes = (stats.f_bavail as u64)
            .saturating_mul(block_size)
            .min(total_bytes);
        let free_bytes = (stats.f_bfree as u64)
            .saturating_mul(block_size)
            .min(total_bytes);
        let used_bytes = total_bytes.saturating_sub(free_bytes);

        volumes.insert(
            mount_point.clone(),
            StorageVolumeMetrics {
                device: mount.device,
                filesystem: mount.filesystem,
                mount_point,
                total_bytes,
                available_bytes,
                used_bytes,
                used_percent: used_bytes as f64 * 100.0 / total_bytes as f64,
            },
        );
    }

    Metric::available(volumes.into_values().collect())
}

#[cfg(not(target_os = "linux"))]
fn storage_volumes() -> Metric<Vec<StorageVolumeMetrics>> {
    Metric::unavailable("Mounted filesystem collection is only implemented for Linux.")
}

#[cfg(target_os = "linux")]
struct MountRecord {
    mount_id: u64,
    device: String,
    mount_point: String,
    filesystem: String,
}

#[cfg(target_os = "linux")]
fn parse_mount_records(contents: &str) -> Vec<MountRecord> {
    contents
        .lines()
        .filter_map(|line| {
            let (mount_fields, filesystem_fields) = line.split_once(" - ")?;
            let mut mount_fields = mount_fields.split_whitespace();
            let mount_id = mount_fields.next()?.parse().ok()?;
            // Parent ID, major:minor, and filesystem root precede the mount point.
            mount_fields.next()?;
            mount_fields.next()?;
            mount_fields.next()?;
            let mount_point = decode_mount_field(mount_fields.next()?);
            let mut filesystem_fields = filesystem_fields.split_whitespace();
            Some(MountRecord {
                mount_id,
                filesystem: decode_mount_field(filesystem_fields.next()?),
                device: decode_mount_field(filesystem_fields.next()?),
                mount_point,
            })
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn select_mount_record(mounts: Vec<MountRecord>, active_mount_id: u64) -> Option<MountRecord> {
    mounts
        .into_iter()
        .find(|mount| mount.mount_id == active_mount_id)
}

#[cfg(target_os = "linux")]
fn mount_id_for_path(path: &std::ffi::CString) -> Option<u64> {
    let mut stats: libc::statx = unsafe { std::mem::zeroed() };
    let result = unsafe {
        libc::statx(
            libc::AT_FDCWD,
            path.as_ptr(),
            libc::AT_STATX_SYNC_AS_STAT,
            libc::STATX_MNT_ID,
            &mut stats,
        )
    };
    if result != 0 || stats.stx_mask & libc::STATX_MNT_ID == 0 {
        return None;
    }
    Some(stats.stx_mnt_id)
}

#[cfg(target_os = "linux")]
fn decode_mount_field(field: &str) -> String {
    field
        .replace(r"\040", " ")
        .replace(r"\011", "\t")
        .replace(r"\012", "\n")
        .replace(r"\134", "\\")
}

#[cfg(target_os = "linux")]
fn is_virtual_filesystem(filesystem: &str) -> bool {
    matches!(
        filesystem,
        "proc"
            | "sysfs"
            | "devtmpfs"
            | "devpts"
            | "tmpfs"
            | "ramfs"
            | "cgroup"
            | "cgroup2"
            | "securityfs"
            | "debugfs"
            | "tracefs"
            | "pstore"
            | "configfs"
            | "fusectl"
            | "mqueue"
            | "hugetlbfs"
            | "bpf"
            | "rpc_pipefs"
            | "binfmt_misc"
            | "efivarfs"
            | "autofs"
            | "nsfs"
    )
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
async fn gpu_metrics() -> Metric<Vec<GpuMetrics>> {
    use std::time::Duration;
    use tokio::process::Command;

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,utilization.gpu,memory.total,memory.used,temperature.gpu",
                "--format=csv,noheader,nounits",
            ])
            .kill_on_drop(true)
            .output(),
    )
    .await;

    match result {
        Err(_) => Metric::unavailable("NVIDIA GPU telemetry did not respond within 2 seconds."),
        Ok(Err(_)) => {
            Metric::unavailable("The NVIDIA telemetry tool nvidia-smi is unavailable.")
        }
        Ok(Ok(output)) if !output.status.success() => {
            Metric::unavailable("nvidia-smi could not read GPU telemetry.")
        }
        Ok(Ok(output)) => match String::from_utf8(output.stdout)
            .ok()
            .and_then(|contents| parse_nvidia_smi_output(&contents))
        {
            Some(gpus) => Metric::available(gpus),
            None => Metric::unavailable(
                "nvidia-smi returned incomplete or invalid GPU telemetry.",
            ),
        },
    }
}

#[cfg(not(target_os = "linux"))]
async fn gpu_metrics() -> Metric<Vec<GpuMetrics>> {
    Metric::unavailable("GPU telemetry is only implemented for Linux NVIDIA drivers.")
}

#[cfg(target_os = "linux")]
fn parse_nvidia_smi_output(contents: &str) -> Option<Vec<GpuMetrics>> {
    let rows: Vec<&str> = contents.lines().filter(|line| !line.trim().is_empty()).collect();
    if rows.is_empty() {
        return None;
    }

    rows.into_iter()
        .map(|row| {
            let fields: Vec<&str> = row.split(',').map(str::trim).collect();
            if fields.len() != 5 || fields[0].is_empty() {
                return None;
            }

            let utilization_percent = fields[1].parse::<u8>().ok()?;
            if utilization_percent > 100 {
                return None;
            }
            let memory_total_mib = fields[2].parse::<u64>().ok()?;
            let memory_used_mib = fields[3].parse::<u64>().ok()?;
            if memory_total_mib == 0 || memory_used_mib > memory_total_mib {
                return None;
            }
            let temperature_celsius = fields[4].parse::<u16>().ok()?;

            Some(GpuMetrics {
                name: fields[0].to_string(),
                utilization_percent,
                memory_total_bytes: memory_total_mib.saturating_mul(1024 * 1024),
                memory_used_bytes: memory_used_mib.saturating_mul(1024 * 1024),
                temperature_celsius,
            })
        })
        .collect()
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

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_nvidia_gpu_utilization_memory_and_temperature() {
        let gpus = parse_nvidia_smi_output(
            "NVIDIA RTX 6000 Ada Generation, 37, 49140, 12288, 64\nNVIDIA A10, 0, 23028, 0, 38\n",
        )
        .unwrap();

        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].name, "NVIDIA RTX 6000 Ada Generation");
        assert_eq!(gpus[0].utilization_percent, 37);
        assert_eq!(gpus[0].memory_total_bytes, 49_140 * 1024 * 1024);
        assert_eq!(gpus[0].memory_used_bytes, 12_288 * 1024 * 1024);
        assert_eq!(gpus[0].temperature_celsius, 64);
        assert_eq!(gpus[1].memory_used_bytes, 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_incomplete_or_out_of_range_nvidia_gpu_data() {
        assert!(parse_nvidia_smi_output("").is_none());
        assert!(parse_nvidia_smi_output("NVIDIA GPU, N/A, 8192, 1024, 54").is_none());
        assert!(parse_nvidia_smi_output("NVIDIA GPU, 101, 8192, 1024, 54").is_none());
        assert!(parse_nvidia_smi_output("NVIDIA GPU, 50, 8192, 8193, 54").is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn selects_active_mount_when_mountinfo_has_duplicate_mount_points() {
        let records = parse_mount_records(
            "37 35 8:1 / /mnt/team\\040files rw - ext4 /dev/visible rw\n36 35 0:42 / /mnt/team\\040files rw - tmpfs tmpfs rw\n38 36 0:42 /proc /proc rw - proc proc rw\n",
        );
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].mount_point, "/mnt/team files");
        assert_eq!(records[0].filesystem, "ext4");
        assert_eq!(records[0].device, "/dev/visible");
        let active_mount = select_mount_record(records, 37).unwrap();
        assert_eq!(active_mount.mount_id, 37);
        assert_eq!(active_mount.device, "/dev/visible");
        assert_eq!(active_mount.filesystem, "ext4");
        assert!(is_virtual_filesystem("proc"));
        assert!(!is_virtual_filesystem("nfs4"));
    }
}
