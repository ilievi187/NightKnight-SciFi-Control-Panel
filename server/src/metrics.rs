use serde::Serialize;
#[cfg(target_os = "linux")]
use std::collections::{BTreeMap, BTreeSet};
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
pub struct SwapMetrics {
    pub total_bytes: u64,
    pub free_bytes: u64,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inode_usage: Option<InodeMetrics>,
}

#[derive(Serialize)]
pub struct InodeMetrics {
    pub total: u64,
    pub free: u64,
    pub used: u64,
    pub used_percent: f64,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inode_usage: Option<InodeMetrics>,
}

#[derive(Serialize)]
pub struct NetworkInterfaceMetrics {
    pub name: String,
    pub received_bytes: u64,
    pub transmitted_bytes: u64,
}

#[derive(Serialize)]
pub struct DiskIoMetrics {
    pub device: String,
    pub sectors_read: u64,
    pub sectors_written: u64,
}

#[derive(Serialize)]
pub struct GpuMetrics {
    pub name: String,
    pub utilization_percent: Option<u8>,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub temperature_celsius: Option<u16>,
}

#[derive(Serialize)]
pub struct LoadAverageMetrics {
    pub one_minute: f64,
    pub five_minutes: f64,
    pub fifteen_minutes: f64,
    pub runnable_tasks: u64,
    pub total_tasks: u64,
}

#[derive(Serialize)]
pub struct CpuCoreMetrics {
    pub core_id: u32,
    pub utilization_percent: f64,
}

#[derive(Serialize)]
pub struct PressureAverages {
    pub avg10_percent: f64,
    pub avg60_percent: f64,
    pub avg300_percent: f64,
    pub total_microseconds: u64,
}

#[derive(Serialize)]
pub struct ResourcePressureMetrics {
    pub some: PressureAverages,
    pub full: Option<PressureAverages>,
}

#[derive(Serialize)]
pub struct SystemPressureMetrics {
    pub cpu: ResourcePressureMetrics,
    pub memory: ResourcePressureMetrics,
    pub io: ResourcePressureMetrics,
}

#[derive(Serialize)]
pub struct ThermalZoneMetrics {
    pub name: String,
    pub temperature_celsius: f64,
}

#[derive(Serialize)]
pub struct IntegrationStatus {
    pub name: String,
    pub status: &'static str,
    pub reason: &'static str,
}

#[derive(Serialize)]
pub struct TailscaleMetrics {
    pub backend_state: String,
    pub connected: bool,
}

#[derive(Serialize)]
pub struct HostMetrics {
    pub generated_at_unix: u64,
    pub hostname: Metric<String>,
    pub operating_system: Metric<String>,
    pub cpu_usage_percent: Metric<f64>,
    pub cpu_cores: Metric<Vec<CpuCoreMetrics>>,
    pub system_pressure: Metric<SystemPressureMetrics>,
    pub load_average: Metric<LoadAverageMetrics>,
    pub thermal_zones: Metric<Vec<ThermalZoneMetrics>>,
    pub memory: Metric<MemoryMetrics>,
    pub swap: Metric<SwapMetrics>,
    pub uptime_seconds: Metric<f64>,
    pub root_disk: Metric<DiskMetrics>,
    pub storage_volumes: Metric<Vec<StorageVolumeMetrics>>,
    pub network_interfaces: Metric<Vec<NetworkInterfaceMetrics>>,
    pub disk_io: Metric<Vec<DiskIoMetrics>>,
    pub gpu_metrics: Metric<Vec<GpuMetrics>>,
    pub tailscale: Metric<TailscaleMetrics>,
    pub integrations: Vec<IntegrationStatus>,
}

pub async fn collect_host_metrics() -> HostMetrics {
    let generated_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let (cpu_usage_percent, cpu_cores, gpu_metrics, tailscale) = tokio::join!(
        cpu_usage_percent(),
        cpu_cores(),
        gpu_metrics(),
        tailscale_status()
    );

    HostMetrics {
        generated_at_unix,
        hostname: hostname(),
        operating_system: operating_system(),
        cpu_usage_percent,
        cpu_cores,
        system_pressure: system_pressure(),
        load_average: load_average(),
        thermal_zones: thermal_zones(),
        memory: memory(),
        swap: swap(),
        uptime_seconds: uptime(),
        root_disk: root_disk(),
        storage_volumes: storage_volumes(),
        network_interfaces: network_interfaces(),
        disk_io: disk_io(),
        gpu_metrics,
        tailscale,
        integrations: unavailable_integrations(),
    }
}

fn unavailable_integrations() -> Vec<IntegrationStatus> {
    ["Container runtime", "Remote probes", "Cloud provider APIs"]
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

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
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

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
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

#[cfg(target_os = "linux")]
async fn cpu_cores() -> Metric<Vec<CpuCoreMetrics>> {
    let first = match read_cpu_core_counters() {
        Some(counters) => counters,
        None => {
            return Metric::unavailable(
                "Linux per-core CPU counters in /proc/stat are unavailable.",
            )
        }
    };
    tokio::time::sleep(std::time::Duration::from_millis(160)).await;
    let second = match read_cpu_core_counters() {
        Some(counters) => counters,
        None => {
            return Metric::unavailable(
                "Linux per-core CPU counters in /proc/stat are unavailable.",
            )
        }
    };
    let cores = calculate_cpu_core_usage(&first, &second);
    if cores.is_empty() {
        return Metric::unavailable(
            "Linux per-core CPU counters did not advance during the sample.",
        );
    }
    Metric::available(cores)
}

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
async fn cpu_cores() -> Metric<Vec<CpuCoreMetrics>> {
    Metric::unavailable("Per-core CPU utilization is only collected on Linux.")
}

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
async fn cpu_usage_percent() -> Metric<f64> {
    Metric::unavailable("CPU utilization collection is only implemented for Linux.")
}

#[cfg(target_os = "linux")]
fn load_average() -> Metric<LoadAverageMetrics> {
    match std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|contents| parse_load_average(&contents))
    {
        Some(values) => Metric::available(values),
        None => Metric::unavailable("Linux load averages in /proc/loadavg are unavailable."),
    }
}

#[cfg(target_os = "linux")]
fn system_pressure() -> Metric<SystemPressureMetrics> {
    let Some(mut cpu) = std::fs::read_to_string("/proc/pressure/cpu")
        .ok()
        .and_then(|contents| parse_pressure_resource(&contents))
    else {
        return Metric::unavailable(
            "Linux CPU pressure data in /proc/pressure/cpu is unavailable.",
        );
    };
    let Some(memory) = std::fs::read_to_string("/proc/pressure/memory")
        .ok()
        .and_then(|contents| parse_pressure_resource(&contents))
    else {
        return Metric::unavailable(
            "Linux memory pressure data in /proc/pressure/memory is unavailable.",
        );
    };
    let Some(io) = std::fs::read_to_string("/proc/pressure/io")
        .ok()
        .and_then(|contents| parse_pressure_resource(&contents))
    else {
        return Metric::unavailable("Linux I/O pressure data in /proc/pressure/io is unavailable.");
    };

    // System-wide CPU full pressure is undefined, even on kernels that emit a
    // compatibility line for it.
    cpu.full = None;
    Metric::available(SystemPressureMetrics { cpu, memory, io })
}

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
fn system_pressure() -> Metric<SystemPressureMetrics> {
    Metric::unavailable("System pressure stall information is only collected on Linux.")
}

#[cfg(target_os = "linux")]
fn parse_pressure_resource(contents: &str) -> Option<ResourcePressureMetrics> {
    let mut some = None;
    let mut full = None;
    let mut full_seen = false;
    for line in contents.lines().filter(|line| !line.trim().is_empty()) {
        let mut fields = line.split_whitespace();
        let kind = fields.next()?;
        match kind {
            "some" => {
                let averages = parse_pressure_averages(fields)?;
                if some.replace(averages).is_some() {
                    return None;
                }
            }
            "full" => {
                if full_seen {
                    full = None;
                } else {
                    full_seen = true;
                    full = parse_pressure_averages(fields);
                }
            }
            _ => {}
        }
    }
    Some(ResourcePressureMetrics { some: some?, full })
}

#[cfg(target_os = "linux")]
fn parse_pressure_averages<'a>(fields: impl Iterator<Item = &'a str>) -> Option<PressureAverages> {
    let mut avg10_percent = None;
    let mut avg60_percent = None;
    let mut avg300_percent = None;
    let mut total_microseconds = None;

    for field in fields {
        let Some((key, value)) = field.split_once('=') else {
            continue;
        };
        match key {
            "avg10" => {
                if avg10_percent
                    .replace(parse_pressure_percent(value)?)
                    .is_some()
                {
                    return None;
                }
            }
            "avg60" => {
                if avg60_percent
                    .replace(parse_pressure_percent(value)?)
                    .is_some()
                {
                    return None;
                }
            }
            "avg300" => {
                if avg300_percent
                    .replace(parse_pressure_percent(value)?)
                    .is_some()
                {
                    return None;
                }
            }
            "total"
                if total_microseconds
                    .replace(value.parse::<u64>().ok()?)
                    .is_some() =>
            {
                return None;
            }
            _ => {}
        }
    }

    Some(PressureAverages {
        avg10_percent: avg10_percent?,
        avg60_percent: avg60_percent?,
        avg300_percent: avg300_percent?,
        total_microseconds: total_microseconds?,
    })
}

#[cfg(target_os = "linux")]
fn parse_pressure_percent(value: &str) -> Option<f64> {
    let percent = value.parse::<f64>().ok()?;
    (percent.is_finite() && (0.0..=100.0).contains(&percent)).then_some(percent)
}

#[cfg(target_os = "linux")]
fn thermal_zones() -> Metric<Vec<ThermalZoneMetrics>> {
    let entries = match std::fs::read_dir("/sys/class/thermal") {
        Ok(entries) => entries,
        Err(_) => {
            return Metric::unavailable(
                "Linux thermal zone data in /sys/class/thermal is unavailable.",
            )
        }
    };

    let mut zones = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // /sys/class entries are commonly symlinks to their device directories.
        if !name.starts_with("thermal_zone") || !entry.path().is_dir() {
            continue;
        }

        let directory = entry.path();
        let label = std::fs::read_to_string(directory.join("type"))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(name);
        let temperature_celsius = std::fs::read_to_string(directory.join("temp"))
            .ok()
            .and_then(|value| parse_thermal_millidegrees(&value));

        if let Some(temperature_celsius) = temperature_celsius {
            zones.push(ThermalZoneMetrics {
                name: label,
                temperature_celsius,
            });
        }
    }

    zones.sort_by(|left, right| left.name.cmp(&right.name));
    if zones.is_empty() {
        return Metric::unavailable(
            "No readable Linux thermal zones are exposed by /sys/class/thermal.",
        );
    }
    Metric::available(zones)
}

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
fn thermal_zones() -> Metric<Vec<ThermalZoneMetrics>> {
    Metric::unavailable("Thermal zone collection is only implemented for Linux.")
}

#[cfg(target_os = "linux")]
fn parse_thermal_millidegrees(contents: &str) -> Option<f64> {
    let millidegrees = contents.trim().parse::<i64>().ok()?;
    if !(-273_150..=1_000_000).contains(&millidegrees) {
        return None;
    }
    Some(millidegrees as f64 / 1_000.0)
}

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
fn load_average() -> Metric<LoadAverageMetrics> {
    Metric::unavailable("System load averages are only collected on Linux.")
}

#[cfg(target_os = "linux")]
fn parse_load_average(contents: &str) -> Option<LoadAverageMetrics> {
    let mut fields = contents.split_whitespace();
    let one_minute = fields.next()?.parse::<f64>().ok()?;
    let five_minutes = fields.next()?.parse::<f64>().ok()?;
    let fifteen_minutes = fields.next()?.parse::<f64>().ok()?;
    let (runnable_tasks, total_tasks) = fields.next()?.split_once('/')?;
    let runnable_tasks = runnable_tasks.parse::<u64>().ok()?;
    let total_tasks = total_tasks.parse::<u64>().ok()?;
    if [one_minute, five_minutes, fifteen_minutes]
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
        || runnable_tasks > total_tasks
    {
        return None;
    }
    Some(LoadAverageMetrics {
        one_minute,
        five_minutes,
        fifteen_minutes,
        runnable_tasks,
        total_tasks,
    })
}

#[cfg(target_os = "linux")]
fn read_cpu_counters() -> Option<(u64, u64)> {
    let contents = std::fs::read_to_string("/proc/stat").ok()?;
    parse_cpu_counters(contents.lines().next()?)
}

#[cfg(target_os = "linux")]
fn read_cpu_core_counters() -> Option<BTreeMap<u32, (u64, u64)>> {
    let contents = std::fs::read_to_string("/proc/stat").ok()?;
    parse_cpu_core_counters(&contents)
}

#[cfg(target_os = "linux")]
fn parse_cpu_counters(line: &str) -> Option<(u64, u64)> {
    parse_cpu_counters_for_name(line, "cpu")
}

#[cfg(target_os = "linux")]
fn parse_cpu_core_counters(contents: &str) -> Option<BTreeMap<u32, (u64, u64)>> {
    let mut cores = BTreeMap::new();
    for line in contents.lines() {
        let Some(name) = line.split_whitespace().next() else {
            continue;
        };
        let Some(core_id) = name
            .strip_prefix("cpu")
            .filter(|suffix| !suffix.is_empty())
            .and_then(|suffix| suffix.parse::<u32>().ok())
        else {
            continue;
        };
        let counters = parse_cpu_counters_for_name(line, name)?;
        if cores.insert(core_id, counters).is_some() {
            return None;
        }
    }
    (!cores.is_empty()).then_some(cores)
}

#[cfg(target_os = "linux")]
fn parse_cpu_counters_for_name(line: &str, expected_name: &str) -> Option<(u64, u64)> {
    let mut fields = line.split_whitespace();
    if fields.next()? != expected_name {
        return None;
    }
    let counters: Vec<u64> = fields.map(str::parse).collect::<Result<_, _>>().ok()?;
    if counters.len() < 4 {
        return None;
    }
    // Linux guest and guest_nice are already included in user and nice.
    let total = counters
        .iter()
        .take(8)
        .try_fold(0u64, |sum, counter| sum.checked_add(*counter))?;
    let idle = counters[3].checked_add(counters.get(4).copied().unwrap_or(0))?;
    Some((total, idle))
}

#[cfg(target_os = "linux")]
fn calculate_cpu_core_usage(
    previous: &BTreeMap<u32, (u64, u64)>,
    current: &BTreeMap<u32, (u64, u64)>,
) -> Vec<CpuCoreMetrics> {
    current
        .iter()
        .filter_map(|(core_id, (total, idle))| {
            let (previous_total, previous_idle) = previous.get(core_id)?;
            let total_delta = total.checked_sub(*previous_total)?;
            let idle_delta = idle.checked_sub(*previous_idle)?;
            if total_delta == 0 || idle_delta > total_delta {
                return None;
            }
            let busy_delta = total_delta - idle_delta;
            let utilization_percent =
                (busy_delta as f64 * 100.0 / total_delta as f64).clamp(0.0, 100.0);
            Some(CpuCoreMetrics {
                core_id: *core_id,
                utilization_percent: (utilization_percent * 10.0).round() / 10.0,
            })
        })
        .collect()
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

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
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
fn swap() -> Metric<SwapMetrics> {
    let data = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|contents| parse_swap_kib(&contents));
    match data {
        Some((total_kib, free_kib)) => Metric::available(build_swap_metrics(total_kib, free_kib)),
        None => Metric::unavailable("Linux swap data in /proc/meminfo is unavailable."),
    }
}

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
fn swap() -> Metric<SwapMetrics> {
    Metric::unavailable("Swap collection is only implemented for Linux.")
}

#[cfg(target_os = "linux")]
fn parse_swap_kib(contents: &str) -> Option<(u64, u64)> {
    let mut total = None;
    let mut free = None;
    for line in contents.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if !matches!(key, "SwapTotal" | "SwapFree") {
            continue;
        }
        let mut fields = value.split_whitespace();
        let kib = fields.next()?.parse::<u64>().ok()?;
        if fields.next() != Some("kB") || fields.next().is_some() {
            return None;
        }
        match key {
            "SwapTotal" => total = Some(kib),
            "SwapFree" => free = Some(kib),
            _ => unreachable!(),
        }
    }
    Some((total?, free?))
}

fn build_swap_metrics(total_kib: u64, free_kib: u64) -> SwapMetrics {
    let total_bytes = total_kib.saturating_mul(1024);
    let free_bytes = free_kib.min(total_kib).saturating_mul(1024);
    let used_bytes = total_bytes.saturating_sub(free_bytes);
    let used_percent = if total_bytes == 0 {
        0.0
    } else {
        used_bytes as f64 * 100.0 / total_bytes as f64
    };
    SwapMetrics {
        total_bytes,
        free_bytes,
        used_bytes,
        used_percent,
    }
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

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
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
        inode_usage: inode_usage(stats.f_files as u64, stats.f_ffree as u64),
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
        let inode_usage = inode_usage(stats.f_files as u64, stats.f_ffree as u64);
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
                inode_usage,
            },
        );
    }

    Metric::available(volumes.into_values().collect())
}

fn inode_usage(total: u64, free: u64) -> Option<InodeMetrics> {
    if total == 0 {
        return None;
    }
    let free = free.min(total);
    let used = total - free;
    Some(InodeMetrics {
        total,
        free,
        used,
        used_percent: used as f64 * 100.0 / total as f64,
    })
}

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
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

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
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

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
fn network_interfaces() -> Metric<Vec<NetworkInterfaceMetrics>> {
    Metric::unavailable("Network counters are only implemented for Linux.")
}

#[cfg(target_os = "linux")]
fn disk_io() -> Metric<Vec<DiskIoMetrics>> {
    match std::fs::read_to_string("/proc/diskstats")
        .ok()
        .and_then(|contents| parse_disk_io(&contents))
    {
        Some(devices) => Metric::available(devices),
        None => {
            Metric::unavailable("Linux block-device counters in /proc/diskstats are unavailable.")
        }
    }
}

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
fn disk_io() -> Metric<Vec<DiskIoMetrics>> {
    Metric::unavailable("Block-device I/O counters are only implemented for Linux.")
}

#[cfg(target_os = "linux")]
async fn gpu_metrics() -> Metric<Vec<GpuMetrics>> {
    use std::time::Duration;
    use tokio::process::Command;

    let mut gpus = Vec::new();

    // NVIDIA GPUs
    let nvidia_result = tokio::time::timeout(
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

    if let Ok(Ok(output)) = nvidia_result {
        if output.status.success() {
            if let Some(nvidia_gpus) = String::from_utf8(output.stdout)
                .ok()
                .and_then(|contents| parse_nvidia_smi_output(&contents))
            {
                gpus.extend(nvidia_gpus);
            }
        }
    }

    // AMD GPUs (via sysfs)
    if let Some(amd_gpus) = collect_amd_gpu_metrics() {
        gpus.extend(amd_gpus);
    }

    if gpus.is_empty() {
        Metric::unavailable("No supported GPU telemetry sources are available.")
    } else {
        Metric::available(gpus)
    }
}

#[cfg(target_os = "linux")]
async fn tailscale_status() -> Metric<TailscaleMetrics> {
    use std::time::Duration;
    use tokio::process::Command;

    // Peers are not needed for the dashboard, so keep the local status query
    // limited to the daemon's own backend state.
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        Command::new("tailscale")
            .args(["status", "--json", "--peers=false"])
            .kill_on_drop(true)
            .output(),
    )
    .await;

    match result {
        Err(_) => Metric::unavailable("Tailscale status did not respond within 2 seconds."),
        Ok(Err(_)) => Metric::unavailable("The Tailscale CLI is unavailable."),
        Ok(Ok(output)) if !output.status.success() => {
            Metric::unavailable("The Tailscale CLI could not read local status.")
        }
        Ok(Ok(output)) => match String::from_utf8(output.stdout)
            .ok()
            .and_then(|contents| parse_tailscale_status(&contents))
        {
            Some(status) => Metric::available(status),
            None => Metric::unavailable("Tailscale returned invalid local status data."),
        },
    }
}

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
async fn tailscale_status() -> Metric<TailscaleMetrics> {
    Metric::unavailable("Tailscale status collection is only implemented for Linux.")
}

#[cfg(target_os = "linux")]
fn parse_tailscale_status(contents: &str) -> Option<TailscaleMetrics> {
    let value: serde_json::Value = serde_json::from_str(contents).ok()?;
    let backend_state = value.get("BackendState")?.as_str()?.trim();
    if backend_state.is_empty()
        || backend_state.len() > 64
        || !backend_state
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return None;
    }

    Some(TailscaleMetrics {
        backend_state: backend_state.to_string(),
        connected: backend_state == "Running",
    })
}

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
async fn gpu_metrics() -> Metric<Vec<GpuMetrics>> {
    Metric::unavailable("GPU telemetry is only implemented for Linux NVIDIA drivers.")
}

#[cfg(target_os = "linux")]
fn collect_amd_gpu_metrics() -> Option<Vec<GpuMetrics>> {
    let mut gpus = Vec::new();
    let drm_path = std::path::Path::new("/sys/class/drm");

    if let Ok(entries) = std::fs::read_dir(drm_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Only process cardN directories (not cardN-DP-1 etc.)
            if !name.starts_with("card") || name.contains('-') {
                continue;
            }
            let device_path = entry.path().join("device");
            let driver_path = device_path.join("driver");

            // Check if this is an AMD GPU
            let driver = std::fs::read_link(&driver_path)
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
            if driver.as_deref() != Some("amdgpu") {
                continue;
            }

            // Read GPU utilization
            let utilization = std::fs::read_to_string(device_path.join("gpu_busy_percent"))
                .ok()
                .and_then(|s| s.trim().parse::<f64>().ok())
                .map(|v| v as u8);

            // Read VRAM
            let vram_total = std::fs::read_to_string(device_path.join("mem_info_vram_total"))
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0);
            let vram_used = std::fs::read_to_string(device_path.join("mem_info_vram_used"))
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0);

            // Read temperature from hwmon
            let mut temperature = None;
            if let Ok(hwmon_entries) = std::fs::read_dir(device_path.join("hwmon")) {
                for hwmon in hwmon_entries.flatten() {
                    let temp_path = hwmon.path().join("temp1_input");
                    if let Ok(temp_str) = std::fs::read_to_string(&temp_path) {
                        if let Ok(temp_millidegrees) = temp_str.trim().parse::<i64>() {
                            temperature = Some((temp_millidegrees / 1000) as u16);
                            break;
                        }
                    }
                }
            }

            // Only include if we have at least VRAM info
            if vram_total == 0 {
                continue;
            }

            // Get GPU name
            let name = std::fs::read_to_string(device_path.join("product_name"))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| {
                    std::fs::read_to_string(device_path.join("uevent"))
                        .ok()
                        .and_then(|s| {
                            s.lines()
                                .find(|l| l.starts_with("PCI_ID="))
                                .map(|l| format!("AMD GPU ({})", l.trim_start_matches("PCI_ID=")))
                        })
                        .unwrap_or_else(|| "AMD GPU".to_string())
                });

            gpus.push(GpuMetrics {
                name: if name.is_empty() {
                    "AMD GPU".to_string()
                } else {
                    name
                },
                utilization_percent: utilization,
                memory_total_bytes: vram_total,
                memory_used_bytes: vram_used,
                temperature_celsius: temperature,
            });
        }
    }

    if gpus.is_empty() {
        None
    } else {
        Some(gpus)
    }
}

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
fn collect_amd_gpu_metrics() -> Option<Vec<GpuMetrics>> {
    None
}

#[cfg(target_os = "linux")]
fn parse_nvidia_smi_output(contents: &str) -> Option<Vec<GpuMetrics>> {
    let rows: Vec<&str> = contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if rows.is_empty() {
        return None;
    }

    rows.into_iter()
        .map(|row| {
            let fields: Vec<&str> = row.split(',').map(str::trim).collect();
            if fields.len() != 5 || fields[0].is_empty() {
                return None;
            }

            let utilization_percent = fields[1].parse::<u8>().ok();
            if let Some(u) = utilization_percent {
                if u > 100 {
                    return None;
                }
            }
            let memory_total_mib = fields[2].parse::<u64>().ok()?;
            let memory_used_mib = fields[3].parse::<u64>().ok()?;
            if memory_total_mib == 0 || memory_used_mib > memory_total_mib {
                return None;
            }
            let temperature_celsius = fields[4].parse::<u16>().ok();

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

#[cfg(target_os = "linux")]
fn parse_disk_io(contents: &str) -> Option<Vec<DiskIoMetrics>> {
    let mut devices = Vec::new();
    let mut names = BTreeSet::new();
    for line in contents.lines().filter(|line| !line.trim().is_empty()) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 14 {
            return None;
        }
        let _major = fields[0].parse::<u32>().ok()?;
        let _minor = fields[1].parse::<u32>().ok()?;
        let device = fields[2];
        if !names.insert(device) {
            return None;
        }
        devices.push(DiskIoMetrics {
            device: device.to_string(),
            sectors_read: fields[5].parse::<u64>().ok()?,
            sectors_written: fields[9].parse::<u64>().ok()?,
        });
    }
    Some(devices)
}

// Windows stubs for functions not yet implemented
#[cfg(target_os = "windows")]
async fn cpu_cores() -> Metric<Vec<CpuCoreMetrics>> {
    Metric::unavailable("Per-core CPU is not yet implemented for Windows.")
}

#[cfg(target_os = "windows")]
fn system_pressure() -> Metric<SystemPressureMetrics> {
    Metric::unavailable("System pressure is Linux-only.")
}

#[cfg(target_os = "windows")]
fn load_average() -> Metric<LoadAverageMetrics> {
    Metric::unavailable("Load average is Linux-only.")
}

#[cfg(target_os = "windows")]
fn thermal_zones() -> Metric<Vec<ThermalZoneMetrics>> {
    Metric::unavailable("Thermal zones are Linux-only.")
}

#[cfg(target_os = "windows")]
fn swap() -> Metric<SwapMetrics> {
    Metric::unavailable("Swap metrics are not yet implemented for Windows.")
}

#[cfg(target_os = "windows")]
fn storage_volumes() -> Metric<Vec<StorageVolumeMetrics>> {
    Metric::unavailable("Storage volumes are not yet implemented for Windows.")
}

#[cfg(target_os = "windows")]
fn disk_io() -> Metric<Vec<DiskIoMetrics>> {
    Metric::unavailable("Block-device I/O is Linux-only.")
}

#[cfg(target_os = "windows")]
async fn tailscale_status() -> Metric<TailscaleMetrics> {
    Metric::unavailable("Tailscale status is not yet implemented for Windows.")
}

#[cfg(target_os = "windows")]
async fn gpu_metrics() -> Metric<Vec<GpuMetrics>> {
    // Try NVIDIA via nvidia-smi
    use std::process::Command;
    let result = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,utilization.gpu,memory.total,memory.used,temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output();

    if let Ok(output) = result {
        if output.status.success() {
            if let Some(gpus) = String::from_utf8(output.stdout)
                .ok()
                .and_then(|contents| parse_nvidia_smi_output(&contents))
            {
                return Metric::available(gpus);
            }
        }
    }

    // Try AMD via PowerShell (WMI)
    match run_powershell("(Get-CimInstance Win32_VideoController | Where-Object { $_.Name -like '*AMD*' -or $_.Name -like '*Radeon*' } | Select-Object -First 1).Name") {
        Some(name) if !name.is_empty() => {
            // AMD GPU detected but detailed metrics require vendor tools
            Metric::available(vec![GpuMetrics {
                name,
                utilization_percent: None,
                memory_total_bytes: 0,
                memory_used_bytes: 0,
                temperature_celsius: None,
            }])
        }
        _ => Metric::unavailable("No supported GPU telemetry sources are available."),
    }
}

// --- Windows telemetry (PowerShell-based) ---

#[cfg(target_os = "windows")]
fn hostname() -> Metric<String> {
    run_powershell("hostname")
        .filter(|s| !s.is_empty())
        .map(Metric::available)
        .unwrap_or_else(|| Metric::unavailable("Hostname could not be determined."))
}

#[cfg(target_os = "windows")]
fn operating_system() -> Metric<String> {
    run_powershell("(Get-CimInstance Win32_OperatingSystem).Caption")
        .filter(|s| !s.is_empty())
        .map(Metric::available)
        .unwrap_or_else(|| Metric::unavailable("Operating system could not be determined."))
}

#[cfg(target_os = "windows")]
async fn cpu_usage_percent() -> Metric<f64> {
    match run_powershell("(Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage -Average).Average") {
        Some(val) => match val.parse::<f64>() {
            Ok(v) if v.is_finite() => Metric::available((v * 10.0).round() / 10.0),
            _ => Metric::unavailable("CPU utilization could not be parsed."),
        },
        None => Metric::unavailable("CPU utilization collection requires PowerShell."),
    }
}

#[cfg(target_os = "windows")]
fn memory() -> Metric<MemoryMetrics> {
    let total = run_powershell("((Get-CimInstance Win32_OperatingSystem).TotalVisibleMemorySize * 1024)");
    let free = run_powershell("((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory * 1024)");
    match (total, free) {
        (Some(t), Some(f)) => {
            let total_bytes = t.parse::<u64>().unwrap_or(0);
            let free_bytes = f.parse::<u64>().unwrap_or(0).min(total_bytes);
            let used_bytes = total_bytes.saturating_sub(free_bytes);
            if total_bytes > 0 {
                Metric::available(MemoryMetrics {
                    total_bytes,
                    available_bytes: free_bytes,
                    used_bytes,
                    used_percent: used_bytes as f64 * 100.0 / total_bytes as f64,
                })
            } else {
                Metric::unavailable("Memory data is unavailable.")
            }
        }
        _ => Metric::unavailable("Memory collection requires PowerShell."),
    }
}

#[cfg(target_os = "windows")]
fn uptime() -> Metric<f64> {
    match run_powershell(
        "((Get-Date) - (Get-CimInstance Win32_OperatingSystem).LastBootUpTime).TotalSeconds",
    ) {
        Some(val) => match val.parse::<f64>() {
            Ok(v) if v.is_finite() && v >= 0.0 => Metric::available(v),
            _ => Metric::unavailable("Uptime could not be parsed."),
        },
        None => Metric::unavailable("Uptime collection requires PowerShell."),
    }
}

#[cfg(target_os = "windows")]
fn root_disk() -> Metric<DiskMetrics> {
    match run_powershell("(Get-PSDrive -Name C).Used + (Get-PSDrive -Name C).Free") {
        Some(total_str) => {
            let total: u64 = total_str.parse().unwrap_or(0);
            let used_str = run_powershell("(Get-PSDrive -Name C).Used").unwrap_or_default();
            let used: u64 = used_str.parse().unwrap_or(0);
            let available = total.saturating_sub(used);
            if total > 0 {
                Metric::available(DiskMetrics {
                    total_bytes: total,
                    available_bytes: available,
                    used_bytes: used,
                    used_percent: used as f64 * 100.0 / total as f64,
                    mount_point: "C:\\".to_string(),
                    inode_usage: None,
                })
            } else {
                Metric::unavailable("Root disk capacity could not be read.")
            }
        }
        None => Metric::unavailable("Root disk collection requires PowerShell."),
    }
}

#[cfg(target_os = "windows")]
fn network_interfaces() -> Metric<Vec<NetworkInterfaceMetrics>> {
    Metric::unavailable("Network interface collection is not yet implemented for Windows.")
}

#[cfg(target_os = "windows")]
fn run_powershell(command: &str) -> Option<String> {
    use std::process::Command;
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", command])
        .output()
        .ok()?;
    if output.status.success() {
        let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_inode_usage_and_omits_unknown_capacity() {
        let usage = inode_usage(1_000, 250).unwrap();
        assert_eq!(usage.total, 1_000);
        assert_eq!(usage.free, 250);
        assert_eq!(usage.used, 750);
        assert_eq!(usage.used_percent, 75.0);

        let full = inode_usage(4, 9).unwrap();
        assert_eq!(full.free, 4);
        assert_eq!(full.used, 0);
        assert_eq!(full.used_percent, 0.0);
        assert!(inode_usage(0, 0).is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_memory_totals_from_linux_meminfo() {
        let source = "MemTotal:       8192 kB\nMemAvailable:   4096 kB\n";
        assert_eq!(parse_memory_kib(source), Some((8192, 4096)));
        assert_eq!(parse_memory_kib("MemTotal: 100 kB\n"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_linux_swap_totals_and_free_capacity() {
        let source = "MemTotal: 8192 kB\nSwapTotal: 2048 kB\nSwapFree: 512 kB\n";
        assert_eq!(parse_swap_kib(source), Some((2048, 512)));
        assert_eq!(parse_swap_kib("SwapTotal: 2048 kB\n"), None);
        assert_eq!(parse_swap_kib("SwapTotal: 2 MB\nSwapFree: 1 MB\n"), None);
    }

    #[test]
    fn calculates_swap_usage_and_handles_disabled_or_inconsistent_counts() {
        let usage = build_swap_metrics(2_048, 512);
        assert_eq!(usage.total_bytes, 2_097_152);
        assert_eq!(usage.free_bytes, 524_288);
        assert_eq!(usage.used_bytes, 1_572_864);
        assert_eq!(usage.used_percent, 75.0);

        let disabled = build_swap_metrics(0, 0);
        assert_eq!(disabled.total_bytes, 0);
        assert_eq!(disabled.used_percent, 0.0);

        let inconsistent = build_swap_metrics(512, 1_024);
        assert_eq!(inconsistent.free_bytes, inconsistent.total_bytes);
        assert_eq!(inconsistent.used_bytes, 0);
        assert_eq!(inconsistent.used_percent, 0.0);
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
        assert_eq!(parse_cpu_counters("cpu 18446744073709551615 1 0 0"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_per_core_cpu_counters_and_calculates_usage_from_matching_samples() {
        let first = parse_cpu_core_counters(
            "cpu 300 0 150 1100 30 0 0 0\ncpu0 100 0 50 400 10 0 0 0\ncpu1 200 0 100 700 20 0 0 0\nintr 42\n",
        )
        .unwrap();
        assert_eq!(first.get(&0), Some(&(560, 410)));
        assert_eq!(first.get(&1), Some(&(1_020, 720)));

        let second = parse_cpu_core_counters(
            "cpu 500 0 250 1300 50 0 0 0\ncpu0 140 0 70 440 10 0 0 0\ncpu1 240 0 120 740 20 0 0 0\ncpu3 100 0 50 200 10 0 0 0\n",
        )
        .unwrap();
        let cores = calculate_cpu_core_usage(&first, &second);

        assert_eq!(cores.len(), 2);
        assert_eq!(cores[0].core_id, 0);
        assert_eq!(cores[0].utilization_percent, 60.0);
        assert_eq!(cores[1].core_id, 1);
        assert_eq!(cores[1].utilization_percent, 60.0);
        assert!(parse_cpu_core_counters("cpu 300 0 150 1100\n").is_none());
        assert!(parse_cpu_core_counters("cpu0 100 0 nope 400\n").is_none());
        assert!(parse_cpu_core_counters("cpu0 100 0 50 400\ncpu0 101 0 50 400\n").is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_linux_pressure_windows_and_optional_full_stall_lines() {
        let cpu = parse_pressure_resource("some avg10=1.25 avg60=2.50 avg300=3.75 total=12345\n")
            .unwrap();
        assert_eq!(cpu.some.avg10_percent, 1.25);
        assert_eq!(cpu.some.avg60_percent, 2.5);
        assert_eq!(cpu.some.avg300_percent, 3.75);
        assert_eq!(cpu.some.total_microseconds, 12_345);
        assert!(cpu.full.is_none());

        let memory = parse_pressure_resource(
            "some total=23456 avg300=4.50 avg10=2.00 avg60=3.00\nfull avg10=0.50 avg60=0.75 avg300=1.00 total=4567\n",
        )
        .unwrap();
        assert_eq!(memory.some.total_microseconds, 23_456);
        assert_eq!(memory.full.as_ref().unwrap().avg10_percent, 0.5);
        assert_eq!(memory.full.unwrap().total_microseconds, 4_567);

        let malformed_full = parse_pressure_resource(
            "some avg10=1 avg60=2 avg300=3 total=123\nfull avg10=bad avg60=2 avg300=3 total=456\n",
        )
        .unwrap();
        assert_eq!(malformed_full.some.total_microseconds, 123);
        assert!(malformed_full.full.is_none());

        let future_field =
            parse_pressure_resource("some avg10=1 avg60=2 avg300=3 total=789 newfield\n").unwrap();
        assert_eq!(future_field.some.total_microseconds, 789);

        assert!(parse_pressure_resource("").is_none());
        assert!(parse_pressure_resource("full avg10=0 avg60=0 avg300=0 total=0\n").is_none());
        assert!(parse_pressure_resource("some avg10=101 avg60=0 avg300=0 total=0\n").is_none());
        assert!(parse_pressure_resource("some avg10=NaN avg60=0 avg300=0 total=0\n").is_none());
        assert!(parse_pressure_resource("some avg10=0 avg60=0 avg300=0\n").is_none());
        assert!(parse_pressure_resource("some avg10=0 avg60=0 avg300=0 total=nope\n").is_none());
        assert!(parse_pressure_resource(
            "some avg10=0 avg60=0 avg300=0 total=0\nsome avg10=0 avg60=0 avg300=0 total=0\n"
        )
        .is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn skips_cpu_cores_with_reset_or_non_advancing_counters() {
        let first = BTreeMap::from([(0, (100, 50)), (1, (200, 100)), (2, (300, 150))]);
        let second = BTreeMap::from([(0, (120, 60)), (1, (200, 100)), (2, (50, 20))]);

        let cores = calculate_cpu_core_usage(&first, &second);
        assert_eq!(cores.len(), 1);
        assert_eq!(cores[0].core_id, 0);
        assert_eq!(cores[0].utilization_percent, 50.0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_linux_load_averages_and_rejects_invalid_values() {
        let values = parse_load_average("0.42 1.25 2.75 2/105 12345").unwrap();
        assert_eq!(values.one_minute, 0.42);
        assert_eq!(values.five_minutes, 1.25);
        assert_eq!(values.fifteen_minutes, 2.75);
        assert_eq!(values.runnable_tasks, 2);
        assert_eq!(values.total_tasks, 105);

        assert!(parse_load_average("0.42 1.25").is_none());
        assert!(parse_load_average("0.42 bad 2.75").is_none());
        assert!(parse_load_average("0.42 -1 2.75").is_none());
        assert!(parse_load_average("NaN 1 2").is_none());
        assert!(parse_load_average("0.42 1.25 2.75 3/2 12345").is_none());
        assert!(parse_load_average("0.42 1.25 2.75 2/tasks 12345").is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_linux_thermal_zone_millidegrees() {
        assert_eq!(parse_thermal_millidegrees("42500\n"), Some(42.5));
        assert_eq!(parse_thermal_millidegrees("-12500"), Some(-12.5));
        assert_eq!(parse_thermal_millidegrees(""), None);
        assert_eq!(parse_thermal_millidegrees("not-a-temperature"), None);
        assert_eq!(parse_thermal_millidegrees("-273151"), None);
        assert_eq!(parse_thermal_millidegrees("1000001"), None);
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
    fn parses_linux_disk_read_and_write_sector_counters() {
        let source = concat!(
            "  8 0 sda 100 2 2048 40 90 3 4096 50 0 61 91\n",
            "  8 1 sda1 10 1 512 10 20 0 1024 20 0 30 40 0 0 0 0\n",
        );
        let devices = parse_disk_io(source).unwrap();

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].device, "sda");
        assert_eq!(devices[0].sectors_read, 2_048);
        assert_eq!(devices[0].sectors_written, 4_096);
        assert_eq!(devices[1].device, "sda1");
        assert_eq!(devices[1].sectors_read, 512);
        assert_eq!(devices[1].sectors_written, 1_024);
        assert!(matches!(parse_disk_io(""), Some(devices) if devices.is_empty()));
        assert!(parse_disk_io("8 0 sda 100 2 2048 40 90 3 4096 50 0 61").is_none());
        assert!(parse_disk_io("8 0 sda 100 2 2048 40 90 3 nope 50 0 61 91").is_none());
        assert!(parse_disk_io(
            "8 0 sda 100 2 2048 40 90 3 4096 50 0 61 91\n8 0 sda 1 0 1 0 1 0 1 0 0 0 0\n"
        )
        .is_none());
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
        assert_eq!(gpus[0].utilization_percent, Some(37));
        assert_eq!(gpus[0].memory_total_bytes, 49_140 * 1024 * 1024);
        assert_eq!(gpus[0].memory_used_bytes, 12_288 * 1024 * 1024);
        assert_eq!(gpus[0].temperature_celsius, Some(64));
        assert_eq!(gpus[1].memory_used_bytes, 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_incomplete_or_out_of_range_nvidia_gpu_data() {
        assert!(parse_nvidia_smi_output("").is_none());
        assert!(parse_nvidia_smi_output("NVIDIA GPU, 101, 8192, 1024, 54").is_none());
        assert!(parse_nvidia_smi_output("NVIDIA GPU, 50, 8192, 8193, 54").is_none());
        // N/A utilization is now acceptable (optional field)
        let result = parse_nvidia_smi_output("NVIDIA GPU, N/A, 8192, 1024, 54");
        assert!(result.is_some());
        assert_eq!(result.unwrap()[0].utilization_percent, None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_tailscale_backend_state_without_peer_data() {
        let connected = parse_tailscale_status(
            r#"{"BackendState":"Running","Peer":{"ignored":{"HostName":"private-peer"}}}"#,
        )
        .unwrap();
        assert_eq!(connected.backend_state, "Running");
        assert!(connected.connected);

        let stopped = parse_tailscale_status(r#"{"BackendState":"Stopped"}"#).unwrap();
        assert_eq!(stopped.backend_state, "Stopped");
        assert!(!stopped.connected);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_invalid_tailscale_status_payloads() {
        assert!(parse_tailscale_status("").is_none());
        assert!(parse_tailscale_status("[]").is_none());
        assert!(parse_tailscale_status(r#"{"Peer":{}}"#).is_none());
        assert!(parse_tailscale_status(r#"{"BackendState":3}"#).is_none());
        assert!(parse_tailscale_status(r#"{"BackendState":"<script>"}"#).is_none());
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
