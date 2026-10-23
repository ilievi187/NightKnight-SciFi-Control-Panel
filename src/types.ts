export type Metric<T> =
  | { status: "available"; value: T }
  | { status: "unavailable"; reason: string };

export interface MemoryMetrics {
  total_bytes: number;
  available_bytes: number;
  used_bytes: number;
  used_percent: number;
}

export interface DiskMetrics {
  total_bytes: number;
  available_bytes: number;
  used_bytes: number;
  used_percent: number;
  mount_point: string;
}

export interface StorageVolumeMetrics {
  device: string;
  filesystem: string;
  mount_point: string;
  total_bytes: number;
  available_bytes: number;
  used_bytes: number;
  used_percent: number;
}

export interface NetworkInterfaceMetrics {
  name: string;
  received_bytes: number;
  transmitted_bytes: number;
}

export interface IntegrationStatus {
  name: string;
  status: "unavailable";
  reason: string;
}

export interface HostMetrics {
  generated_at_unix: number;
  hostname: Metric<string>;
  operating_system: Metric<string>;
  cpu_usage_percent: Metric<number>;
  memory: Metric<MemoryMetrics>;
  uptime_seconds: Metric<number>;
  root_disk: Metric<DiskMetrics>;
  storage_volumes: Metric<StorageVolumeMetrics[]>;
  network_interfaces: Metric<NetworkInterfaceMetrics[]>;
  integrations: IntegrationStatus[];
}
