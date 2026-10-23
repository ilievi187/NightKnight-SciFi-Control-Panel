export type Metric<T> =
  | { status: "available"; value: T }
  | { status: "unavailable"; reason: string };

export interface MemoryMetrics {
  total_bytes: number;
  available_bytes: number;
  used_bytes: number;
  used_percent: number;
}

export interface SwapMetrics {
  total_bytes: number;
  free_bytes: number;
  used_bytes: number;
  used_percent: number;
}

export interface DiskMetrics {
  total_bytes: number;
  available_bytes: number;
  used_bytes: number;
  used_percent: number;
  mount_point: string;
  inode_usage?: InodeMetrics;
}

export interface InodeMetrics {
  total: number;
  free: number;
  used: number;
  used_percent: number;
}

export interface StorageVolumeMetrics {
  device: string;
  filesystem: string;
  mount_point: string;
  total_bytes: number;
  available_bytes: number;
  used_bytes: number;
  used_percent: number;
  inode_usage?: InodeMetrics;
}

export interface NetworkInterfaceMetrics {
  name: string;
  received_bytes: number;
  transmitted_bytes: number;
}

export interface DiskIoMetrics {
  device: string;
  sectors_read: number;
  sectors_written: number;
}

export interface GpuMetrics {
  name: string;
  utilization_percent: number;
  memory_total_bytes: number;
  memory_used_bytes: number;
  temperature_celsius: number;
}

export interface LoadAverageMetrics {
  one_minute: number;
  five_minutes: number;
  fifteen_minutes: number;
  runnable_tasks: number;
  total_tasks: number;
}

export interface CpuCoreMetrics {
  core_id: number;
  utilization_percent: number;
}

export interface PressureAverages {
  avg10_percent: number;
  avg60_percent: number;
  avg300_percent: number;
  total_microseconds: number;
}

export interface ResourcePressureMetrics {
  some: PressureAverages;
  full: PressureAverages | null;
}

export interface SystemPressureMetrics {
  cpu: ResourcePressureMetrics;
  memory: ResourcePressureMetrics;
  io: ResourcePressureMetrics;
}

export interface ThermalZoneMetrics {
  name: string;
  temperature_celsius: number;
}

export interface IntegrationStatus {
  name: string;
  status: "unavailable";
  reason: string;
}

export interface TailscaleMetrics {
  backend_state: string;
  connected: boolean;
}

export interface HostMetrics {
  generated_at_unix: number;
  hostname: Metric<string>;
  operating_system: Metric<string>;
  cpu_usage_percent: Metric<number>;
  cpu_cores: Metric<CpuCoreMetrics[]>;
  system_pressure: Metric<SystemPressureMetrics>;
  load_average: Metric<LoadAverageMetrics>;
  thermal_zones: Metric<ThermalZoneMetrics[]>;
  memory: Metric<MemoryMetrics>;
  swap: Metric<SwapMetrics>;
  uptime_seconds: Metric<number>;
  root_disk: Metric<DiskMetrics>;
  storage_volumes: Metric<StorageVolumeMetrics[]>;
  network_interfaces: Metric<NetworkInterfaceMetrics[]>;
  disk_io: Metric<DiskIoMetrics[]>;
  gpu_metrics: Metric<GpuMetrics[]>;
  tailscale: Metric<TailscaleMetrics>;
  integrations: IntegrationStatus[];
}
