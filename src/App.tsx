import { useCallback, useEffect, useRef, useState } from "react";
import SciFiBackground from "./SciFiBackground";
import type {
  CpuCoreMetrics,
  DiskIoMetrics,
  GpuMetrics,
  HostMetrics,
  Metric,
  NetworkInterfaceMetrics,
  PressureAverages,
  ResourcePressureMetrics,
  StorageVolumeMetrics,
  TailscaleMetrics,
  ThermalZoneMetrics,
} from "./types";

type MetricCardProps<T> = {
  label: string;
  metric: Metric<T>;
  format: (value: T) => string;
  detail?: (value: T) => string;
  secondaryDetail?: (value: T) => string;
  index: string;
  history?: Array<number | null>;
};

type TelemetrySample = {
  cpu: number | null;
  memory: number | null;
  disk: number | null;
};

type NetworkRate = {
  name: string;
  receivedBytesPerSecond: number | null;
  transmittedBytesPerSecond: number | null;
};

type NetworkSample = {
  sampledAt: number;
  counters: Map<string, NetworkInterfaceMetrics>;
};

type DiskIoRate = {
  device: string;
  readBytesPerSecond: number | null;
  writeBytesPerSecond: number | null;
};

type DiskIoSample = {
  sampledAt: number;
  counters: Map<string, DiskIoMetrics>;
};

const MAX_HISTORY_SAMPLES = 24;

function MetricCard<T>({
  label,
  metric,
  format,
  detail,
  secondaryDetail,
  index,
  history,
}: MetricCardProps<T>) {
  return (
    <article className="metric-card">
      <div className="metric-card-top">
        <span className="metric-index">{index}</span>
        <span className="metric-source">HOST</span>
      </div>
      <h3>{label}</h3>
      {metric.status === "available" ? (
        <>
          <p className="metric-value">{format(metric.value)}</p>
          {detail && <p className="metric-detail">{detail(metric.value)}</p>}
          {secondaryDetail && (
            <p className="metric-secondary-detail">
              {secondaryDetail(metric.value)}
            </p>
          )}
          {history && (
            <div className="metric-history">
              <div className="metric-history-label">
                <span>RECENT ACTIVITY</span>
                <span>~2 MIN</span>
              </div>
              <Sparkline values={history} label={label} />
            </div>
          )}
        </>
      ) : (
        <div className="unavailable-value">
          <span>UNAVAILABLE</span>
          <p>{metric.reason}</p>
        </div>
      )}
    </article>
  );
}

function StorageVolumeCard({ volume }: { volume: StorageVolumeMetrics }) {
  const usedPercent = Math.min(100, Math.max(0, volume.used_percent));
  const inodeUsage = volume.inode_usage;
  const inodeUsedPercent = inodeUsage
    ? Math.min(100, Math.max(0, inodeUsage.used_percent))
    : null;

  return (
    <article className="storage-volume-card">
      <div className="storage-volume-heading">
        <strong title={volume.mount_point}>{volume.mount_point}</strong>
        <span>{volume.filesystem}</span>
      </div>
      <p className="storage-volume-device" title={volume.device}>
        {volume.device}
      </p>
      <div
        className="storage-volume-meter"
        role="meter"
        aria-label={`${volume.mount_point} storage used`}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={usedPercent}
      >
        <span style={{ width: `${usedPercent}%` }} />
      </div>
      <div className="storage-volume-meta">
        <strong>{usedPercent.toFixed(1)}% USED</strong>
        <span>
          {formatBytes(volume.used_bytes)} / {formatBytes(volume.total_bytes)}
        </span>
      </div>
      {inodeUsage && inodeUsedPercent !== null ? (
        <div className="storage-volume-inodes">
          <div className="storage-volume-inode-heading">
            <span>INODES USED</span>
            <strong>{inodeUsedPercent.toFixed(1)}%</strong>
          </div>
          <div
            className="storage-volume-meter storage-volume-inode-meter"
            role="meter"
            aria-label={`${volume.mount_point} inodes used`}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={inodeUsedPercent}
          >
            <span style={{ width: `${inodeUsedPercent}%` }} />
          </div>
          <p className="storage-volume-inode-count">
            {formatCount(inodeUsage.used)} / {formatCount(inodeUsage.total)} used
          </p>
        </div>
      ) : (
        <p className="storage-volume-inode-note">INODE CAPACITY NOT REPORTED</p>
      )}
    </article>
  );
}

function GpuCard({ gpu }: { gpu: GpuMetrics }) {
  const utilization = Math.min(100, Math.max(0, gpu.utilization_percent));
  const memoryUsed =
    gpu.memory_total_bytes > 0
      ? Math.min(
          100,
          Math.max(0, (gpu.memory_used_bytes / gpu.memory_total_bytes) * 100),
        )
      : 0;

  return (
    <article className="gpu-card">
      <div className="gpu-card-heading">
        <strong title={gpu.name}>{gpu.name}</strong>
        <span>{gpu.temperature_celsius}°C</span>
      </div>
      <div className="gpu-meter-block">
        <div className="gpu-meter-label">
          <span>CORE UTILIZATION</span>
          <strong>{utilization}%</strong>
        </div>
        <div
          className="gpu-meter"
          role="meter"
          aria-label={`${gpu.name} core utilization`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={utilization}
        >
          <span style={{ width: `${utilization}%` }} />
        </div>
      </div>
      <div className="gpu-memory-block">
        <div className="gpu-meter-label">
          <span>VIDEO MEMORY</span>
          <strong>
            {formatBytes(gpu.memory_used_bytes)} / {formatBytes(gpu.memory_total_bytes)}
          </strong>
        </div>
        <div
          className="gpu-meter gpu-memory-meter"
          role="meter"
          aria-label={`${gpu.name} video memory used`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={memoryUsed}
        >
          <span style={{ width: `${memoryUsed}%` }} />
        </div>
      </div>
    </article>
  );
}

function CpuCoreMeter({ core }: { core: CpuCoreMetrics }) {
  const utilization = Math.min(100, Math.max(0, core.utilization_percent));

  return (
    <div className="cpu-core-row">
      <div className="cpu-core-heading">
        <strong>CORE {core.core_id}</strong>
        <span>{utilization.toFixed(1)}%</span>
      </div>
      <div
        className="gpu-meter cpu-core-meter"
        role="meter"
        aria-label={`CPU core ${core.core_id} utilization`}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={utilization}
      >
        <span style={{ width: `${utilization}%` }} />
      </div>
    </div>
  );
}

function PressureReading({
  label,
  averages,
}: {
  label: string;
  averages: PressureAverages;
}) {
  const avg10 = Math.min(100, Math.max(0, averages.avg10_percent));

  return (
    <div className="pressure-reading">
      <div className="pressure-reading-heading">
        <strong>{label}</strong>
        <span>{avg10.toFixed(2)}%</span>
      </div>
      <div
        className="gpu-meter pressure-meter"
        role="meter"
        aria-label={`${label} pressure over the last 10 seconds`}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={avg10}
      >
        <span style={{ width: `${avg10}%` }} />
      </div>
      <p className="pressure-windows">
        10s {averages.avg10_percent.toFixed(2)}% · 60s {averages.avg60_percent.toFixed(2)}% · 300s {averages.avg300_percent.toFixed(2)}%
      </p>
    </div>
  );
}

function ResourcePressureCard({
  label,
  pressure,
}: {
  label: string;
  pressure: ResourcePressureMetrics;
}) {
  return (
    <article className="pressure-card">
      <h3>{label}</h3>
      <PressureReading label={`${label} some`} averages={pressure.some} />
      {pressure.full && (
        <PressureReading label={`${label} full`} averages={pressure.full} />
      )}
    </article>
  );
}

function ThermalZoneCard({ zone }: { zone: ThermalZoneMetrics }) {
  return (
    <article className="thermal-zone-card">
      <span className="thermal-zone-symbol" aria-hidden="true">◈</span>
      <div className="thermal-zone-copy">
        <strong title={zone.name}>{zone.name}</strong>
        <span>{zone.temperature_celsius.toFixed(1)} °C</span>
      </div>
    </article>
  );
}

function IntegrationRow({
  name,
  status,
  reason,
}: {
  name: string;
  status: "connected" | "disconnected" | "unavailable";
  reason: string;
}) {
  return (
    <div className="integration-row">
      <span className={`integration-symbol ${status}`} aria-hidden="true">
        {status === "connected" ? "✓" : "—"}
      </span>
      <div className="integration-copy">
        <strong>{name}</strong>
        <p>{reason}</p>
      </div>
      <span className={`unavailable-badge integration-badge ${status}`}>
        {status.toUpperCase()}
      </span>
    </div>
  );
}

function tailscaleReason(metric: Metric<TailscaleMetrics>): string {
  if (metric.status === "unavailable") return metric.reason;
  return `Local daemon state: ${metric.value.backend_state}. Peer details are not collected.`;
}

function Sparkline({
  values,
  label,
}: {
  values: Array<number | null>;
  label: string;
}) {
  const segments: Array<Array<{ index: number; x: number; y: number }>> = [];
  let segment: Array<{ index: number; x: number; y: number }> = [];
  const lastIndex = Math.max(values.length - 1, 1);

  values.forEach((value, index) => {
    if (value === null || !Number.isFinite(value)) {
      if (segment.length > 0) segments.push(segment);
      segment = [];
      return;
    }

    const x = (index / lastIndex) * 160;
    const y = 38 - (Math.min(100, Math.max(0, value)) / 100) * 34;
    segment.push({ index, x, y });
  });
  if (segment.length > 0) segments.push(segment);

  let latest: { index: number; x: number; y: number } | null = null;
  for (let index = values.length - 1; index >= 0; index -= 1) {
    const value = values[index];
    if (value !== null && Number.isFinite(value)) {
      latest = {
        index,
        x: (index / lastIndex) * 160,
        y: 38 - (Math.min(100, Math.max(0, value)) / 100) * 34,
      };
      break;
    }
  }

  return (
    <svg
      className="sparkline"
      viewBox="0 0 160 42"
      preserveAspectRatio="none"
      role="img"
      aria-label={`${label} history from recent host polling samples`}
    >
      <line className="sparkline-grid" x1="0" y1="4" x2="160" y2="4" />
      <line className="sparkline-grid" x1="0" y1="21" x2="160" y2="21" />
      <line className="sparkline-grid" x1="0" y1="38" x2="160" y2="38" />
      {segments.map((points, index) =>
        points.length === 1 ? (
          points[0].index === latest?.index ? null : (
            <circle
              className="sparkline-point"
              key={index}
              cx={points[0].x}
              cy={points[0].y}
              r="1.8"
            />
          )
        ) : (
          <polyline
            className="sparkline-line"
            key={index}
            points={points.map(({ x, y }) => `${x},${y}`).join(" ")}
          />
        ),
      )}
      {latest && (
        <circle
          className="sparkline-dot"
          cx={latest.x}
          cy={latest.y}
          r="2.5"
        />
      )}
    </svg>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return bytes + " B";
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return value.toFixed(value >= 10 ? 0 : 1) + " " + units[unit];
}

function formatCount(value: number): string {
  return new Intl.NumberFormat().format(value);
}

function formatRate(bytesPerSecond: number | null | undefined): string {
  if (bytesPerSecond === null || bytesPerSecond === undefined) {
    return "Sampling…";
  }
  return formatBytes(bytesPerSecond) + "/s";
}

function formatDuration(seconds: number): string {
  const wholeSeconds = Math.floor(seconds);
  const days = Math.floor(wholeSeconds / 86_400);
  const hours = Math.floor((wholeSeconds % 86_400) / 3_600);
  const minutes = Math.floor((wholeSeconds % 3_600) / 60);
  if (days > 0) return days + "d " + hours + "h";
  if (hours > 0) return hours + "h " + minutes + "m";
  return minutes + "m " + (wholeSeconds % 60) + "s";
}

function App() {
  const [metrics, setMetrics] = useState<HostMetrics | null>(null);
  const [apiState, setApiState] = useState<"connecting" | "online" | "offline">(
    "connecting",
  );
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [history, setHistory] = useState<TelemetrySample[]>([]);
  const [networkRates, setNetworkRates] = useState<NetworkRate[] | null>(null);
  const [diskIoRates, setDiskIoRates] = useState<DiskIoRate[] | null>(null);
  const refreshInFlight = useRef(false);
  const previousNetworkSample = useRef<NetworkSample | null>(null);
  const previousDiskIoSample = useRef<DiskIoSample | null>(null);

  const refresh = useCallback(async () => {
    if (refreshInFlight.current) return;
    refreshInFlight.current = true;
    setLoading(true);
    try {
      const response = await fetch("/api/metrics", { cache: "no-store" });
      if (!response.ok) throw new Error("API returned HTTP " + response.status);
      const payload = (await response.json()) as HostMetrics;
      const sampledAt = performance.now();
      setMetrics(payload);
      if (payload.network_interfaces.status === "available") {
        const interfaces = payload.network_interfaces.value;
        const previous = previousNetworkSample.current;
        const elapsedSeconds = previous
          ? (sampledAt - previous.sampledAt) / 1_000
          : 0;
        setNetworkRates(
          interfaces.map((network) => {
            const prior = previous?.counters.get(network.name);
            const hasElapsedSample = prior && elapsedSeconds > 0;
            return {
              name: network.name,
              receivedBytesPerSecond:
                hasElapsedSample && network.received_bytes >= prior.received_bytes
                  ? (network.received_bytes - prior.received_bytes) / elapsedSeconds
                  : null,
              transmittedBytesPerSecond:
                hasElapsedSample &&
                network.transmitted_bytes >= prior.transmitted_bytes
                  ? (network.transmitted_bytes - prior.transmitted_bytes) / elapsedSeconds
                  : null,
            };
          }),
        );
        previousNetworkSample.current = {
          sampledAt,
          counters: new Map(interfaces.map((network) => [network.name, network])),
        };
      } else {
        setNetworkRates(null);
        previousNetworkSample.current = null;
      }
      if (payload.disk_io.status === "available") {
        const devices = payload.disk_io.value;
        const previous = previousDiskIoSample.current;
        const elapsedSeconds = previous
          ? (sampledAt - previous.sampledAt) / 1_000
          : 0;
        setDiskIoRates(
          devices.map((device) => {
            const prior = previous?.counters.get(device.device);
            const hasElapsedSample = prior && elapsedSeconds > 0;
            return {
              device: device.device,
              readBytesPerSecond:
                hasElapsedSample && device.sectors_read >= prior.sectors_read
                  ? ((device.sectors_read - prior.sectors_read) * 512) / elapsedSeconds
                  : null,
              writeBytesPerSecond:
                hasElapsedSample && device.sectors_written >= prior.sectors_written
                  ? ((device.sectors_written - prior.sectors_written) * 512) / elapsedSeconds
                  : null,
            };
          }),
        );
        previousDiskIoSample.current = {
          sampledAt,
          counters: new Map(devices.map((device) => [device.device, device])),
        };
      } else {
        setDiskIoRates(null);
        previousDiskIoSample.current = null;
      }
      setHistory((current) =>
        [
          ...current,
          {
            cpu:
              payload.cpu_usage_percent.status === "available"
                ? payload.cpu_usage_percent.value
                : null,
            memory:
              payload.memory.status === "available"
                ? payload.memory.value.used_percent
                : null,
            disk:
              payload.root_disk.status === "available"
                ? payload.root_disk.value.used_percent
                : null,
          },
        ].slice(-MAX_HISTORY_SAMPLES),
      );
      setApiState("online");
      setError("");
    } catch (cause) {
      setMetrics(null);
      setNetworkRates(null);
      previousNetworkSample.current = null;
      setDiskIoRates(null);
      previousDiskIoSample.current = null;
      setHistory((current) =>
        [...current, { cpu: null, memory: null, disk: null }].slice(
          -MAX_HISTORY_SAMPLES,
        ),
      );
      setApiState("offline");
      setError(
        cause instanceof Error ? cause.message : "Could not reach the local API.",
      );
    } finally {
      refreshInFlight.current = false;
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 5_000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const timestamp = metrics
    ? new Date(metrics.generated_at_unix * 1_000)
    : null;

  return (
    <main className="app-shell">
      <SciFiBackground />
      <header className="topbar">
        <a className="brand" href="/" aria-label="NightKnight Command Center">
          <span className="brand-mark">NK</span>
          <span className="brand-name">
            <strong>NightKnight</strong>
            <span>COMMAND CENTER</span>
          </span>
        </a>
        <div className={"api-indicator " + apiState} role="status">
          <span className="status-dot" />
          <span>
            {apiState === "online"
              ? "LOCAL API CONNECTED"
              : apiState === "offline"
                ? "LOCAL API UNAVAILABLE"
                : "CONNECTING TO LOCAL API"}
          </span>
        </div>
      </header>

      <section className="hero">
        <div className="hero-copy">
          <p className="eyebrow">INFRASTRUCTURE / 01</p>
          <h1>Local system overview</h1>
          <p className="hero-description">
            Live telemetry from this host, collected by the local Rust API.
          </p>
        </div>
        <div className="hero-meta">
          <span className="meta-label">LAST UPDATED</span>
          {timestamp ? (
            <time dateTime={timestamp.toISOString()}>
              {timestamp.toLocaleTimeString()}
            </time>
          ) : (
            <span className="meta-value">Waiting for host data</span>
          )}
          <button
            className="refresh-button"
            type="button"
            onClick={() => void refresh()}
            disabled={loading}
          >
            <span className={loading ? "refresh-icon spinning" : "refresh-icon"}>
              ↻
            </span>
            Refresh
          </button>
        </div>
      </section>

      {error && (
        <section className="connection-notice" role="alert">
          <span className="notice-mark">!</span>
          <div>
            <strong>Host data unavailable</strong>
            <p>{error} Start the Rust API and reload this page.</p>
          </div>
        </section>
      )}

      <section className="host-strip" aria-label="Host identity">
        <div>
          <span className="meta-label">HOSTNAME</span>
          <strong>
            {metrics?.hostname.status === "available"
              ? metrics.hostname.value
              : "unavailable"}
          </strong>
        </div>
        <div>
          <span className="meta-label">OPERATING SYSTEM</span>
          <strong>
            {metrics?.operating_system.status === "available"
              ? metrics.operating_system.value
              : "unavailable"}
          </strong>
        </div>
        <div className="host-scope">
          <span className="scope-icon">⌂</span>
          <span>LOCAL HOST</span>
        </div>
      </section>

      <section className="system-overview" aria-label="System overview">
        {metrics ? (
          <div className="overview-grid">
            <div className="overview-item">
              <span className="overview-label">CPU</span>
              <span className="overview-value">
                {metrics.cpu_usage_percent.status === "available"
                  ? metrics.cpu_usage_percent.value.toFixed(0) + "%"
                  : "N/A"}
              </span>
              <div className="overview-bar">
                <span
                  style={{
                    width:
                      metrics.cpu_usage_percent.status === "available"
                        ? metrics.cpu_usage_percent.value + "%"
                        : "0%",
                  }}
                />
              </div>
            </div>
            <div className="overview-item">
              <span className="overview-label">RAM</span>
              <span className="overview-value">
                {metrics.memory.status === "available"
                  ? metrics.memory.value.used_percent.toFixed(0) + "%"
                  : "N/A"}
              </span>
              <div className="overview-bar">
                <span
                  style={{
                    width:
                      metrics.memory.status === "available"
                        ? metrics.memory.value.used_percent + "%"
                        : "0%",
                  }}
                />
              </div>
            </div>
            <div className="overview-item">
              <span className="overview-label">DISK</span>
              <span className="overview-value">
                {metrics.root_disk.status === "available"
                  ? metrics.root_disk.value.used_percent.toFixed(0) + "%"
                  : "N/A"}
              </span>
              <div className="overview-bar">
                <span
                  style={{
                    width:
                      metrics.root_disk.status === "available"
                        ? metrics.root_disk.value.used_percent + "%"
                        : "0%",
                  }}
                />
              </div>
            </div>
            <div className="overview-item">
              <span className="overview-label">GPU</span>
              <span className="overview-value">
                {metrics.gpu_metrics.status === "available" &&
                metrics.gpu_metrics.value.length > 0
                  ? metrics.gpu_metrics.value[0].utilization_percent + "%"
                  : "N/A"}
              </span>
              <div className="overview-bar">
                <span
                  style={{
                    width:
                      metrics.gpu_metrics.status === "available" &&
                      metrics.gpu_metrics.value.length > 0
                        ? metrics.gpu_metrics.value[0].utilization_percent + "%"
                        : "0%",
                  }}
                />
              </div>
            </div>
            <div className="overview-item">
              <span className="overview-label">LOAD</span>
              <span className="overview-value">
                {metrics.load_average.status === "available"
                  ? metrics.load_average.value.one_minute.toFixed(2)
                  : "N/A"}
              </span>
            </div>
            <div className="overview-item">
              <span className="overview-label">UPTIME</span>
              <span className="overview-value">
                {metrics.uptime_seconds.status === "available"
                  ? formatDuration(metrics.uptime_seconds.value)
                  : "N/A"}
              </span>
            </div>
          </div>
        ) : (
          <div className="overview-loading">Waiting for telemetry…</div>
        )}
      </section>

      <section className="metrics-grid" aria-label="Host metrics">
        {metrics ? (
          <>
            <MetricCard
              index="01"
              label="CPU utilization"
              metric={metrics.cpu_usage_percent}
              format={(value) => value.toFixed(1) + "%"}
              detail={() => "Measured from Linux CPU counters"}
              history={
                history.length
                  ? history.map((sample) => sample.cpu)
                  : undefined
              }
            />
            <MetricCard
              index="02"
              label="Memory"
              metric={metrics.memory}
              format={(value) => value.used_percent.toFixed(1) + "%"}
              detail={(value) =>
                formatBytes(value.used_bytes) +
                " used of " +
                formatBytes(value.total_bytes)
              }
              history={
                history.length
                  ? history.map((sample) => sample.memory)
                  : undefined
              }
            />
            <MetricCard
              index="03"
              label="System uptime"
              metric={metrics.uptime_seconds}
              format={formatDuration}
              detail={() => "Since the host started"}
            />
            <MetricCard
              index="04"
              label="Root filesystem"
              metric={metrics.root_disk}
              format={(value) => value.used_percent.toFixed(1) + "%"}
              detail={(value) =>
                formatBytes(value.used_bytes) +
                " used of " +
                formatBytes(value.total_bytes)
              }
              secondaryDetail={(value) =>
                value.inode_usage
                  ? `Inodes ${value.inode_usage.used_percent.toFixed(1)}% used (${formatCount(value.inode_usage.used)} / ${formatCount(value.inode_usage.total)})`
                  : "Inode capacity is not reported by this filesystem"
              }
              history={
                history.length
                  ? history.map((sample) => sample.disk)
                  : undefined
              }
            />
            <MetricCard
              index="05"
              label="Swap usage"
              metric={metrics.swap}
              format={(value) =>
                value.total_bytes > 0 ? value.used_percent.toFixed(1) + "%" : "DISABLED"
              }
              detail={(value) =>
                value.total_bytes > 0
                  ? `${formatBytes(value.used_bytes)} used · ${formatBytes(value.free_bytes)} free`
                  : "No swap space is configured on this host"
              }
            />
            <MetricCard
              index="06"
              label="System load average"
              metric={metrics.load_average}
              format={(value) =>
                [value.one_minute, value.five_minutes, value.fifteen_minutes]
                  .map((average) => average.toFixed(2))
                  .join(" / ")
              }
              detail={() => "1m / 5m / 15m · not CPU-normalized"}
              secondaryDetail={(value) =>
                `${formatCount(value.runnable_tasks)} runnable / ${formatCount(value.total_tasks)} total tasks`
              }
            />
          </>
        ) : (
          <div className="empty-metrics">
            <span className="empty-pulse" />
            <div>
              <strong>Waiting for local telemetry</strong>
              <p>
                No host values are shown until the local API returns real data.
              </p>
            </div>
          </div>
        )}
      </section>

      <section className="panel cpu-cores-panel" aria-label="CPU core activity">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">PROCESSOR DETAIL</p>
            <h2>Per-core utilization</h2>
          </div>
          <span className="panel-index">07</span>
        </div>
        {metrics?.cpu_cores.status === "available" ? (
          metrics.cpu_cores.value.length > 0 ? (
            <div className="cpu-core-grid">
              {metrics.cpu_cores.value.map((core) => (
                <CpuCoreMeter key={core.core_id} core={core} />
              ))}
            </div>
          ) : (
            <p className="empty-copy">No CPU core counters were reported.</p>
          )
        ) : (
          <Unavailable
            reason={
              metrics?.cpu_cores.status === "unavailable"
                ? metrics.cpu_cores.reason
                : "Waiting for host data."
            }
          />
        )}
        <p className="cpu-core-note">
          Read-only Linux counter deltas. New or reset cores appear after two valid samples.
        </p>
      </section>

      <section className="panel pressure-panel" aria-label="System pressure stall information">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">RESOURCE CONTENTION</p>
            <h2>System pressure</h2>
          </div>
          <span className="panel-index">08</span>
        </div>
        {metrics?.system_pressure.status === "available" ? (
          <div className="pressure-grid">
            <ResourcePressureCard label="CPU" pressure={metrics.system_pressure.value.cpu} />
            <ResourcePressureCard label="MEMORY" pressure={metrics.system_pressure.value.memory} />
            <ResourcePressureCard label="I/O" pressure={metrics.system_pressure.value.io} />
          </div>
        ) : (
          <Unavailable
            reason={
              metrics?.system_pressure.status === "unavailable"
                ? metrics.system_pressure.reason
                : "Waiting for host data."
            }
          />
        )}
        <p className="pressure-source-note">
          Rolling stall-time percentages over 10, 60, and 300 seconds. System-wide CPU full pressure is not defined.
        </p>
      </section>

      <section className="lower-grid">
        <article className="panel network-panel">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">LIVE THROUGHPUT</p>
              <h2>Network interfaces</h2>
            </div>
            <span className="panel-index">09</span>
          </div>
          {metrics?.network_interfaces.status === "available" ? (
            metrics.network_interfaces.value.length > 0 ? (
              <>
                <table className="network-table">
                  <caption className="sr-only">
                    Current network throughput rates by interface
                  </caption>
                  <thead>
                    <tr>
                      <th scope="col">INTERFACE</th>
                      <th scope="col">RECEIVE / SEC</th>
                      <th scope="col">TRANSMIT / SEC</th>
                    </tr>
                  </thead>
                  <tbody>
                    {metrics.network_interfaces.value.map((network) => {
                      const rate = networkRates?.find(
                        (sample) => sample.name === network.name,
                      );
                      return (
                        <tr key={network.name}>
                          <th scope="row">{network.name}</th>
                          <td>{formatRate(rate?.receivedBytesPerSecond)}</td>
                          <td>{formatRate(rate?.transmittedBytesPerSecond)}</td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
                <p className="network-rate-note">
                  Rates use local counter changes. New or reset interfaces need
                  two samples before a rate is available.
                </p>
              </>
            ) : (
              <p className="empty-copy">No network interfaces were reported.</p>
            )
          ) : (
            <Unavailable
              reason={
                metrics?.network_interfaces.status === "unavailable"
                  ? metrics.network_interfaces.reason
                  : "Waiting for host data."
              }
            />
          )}
        </article>

        <article className="panel integrations-panel">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">OPTIONAL SOURCES</p>
              <h2>Integrations</h2>
            </div>
            <span className="panel-index">10</span>
          </div>
          {metrics ? (
            <div className="integration-list">
              <IntegrationRow
                name="Tailscale"
                status={
                  metrics.tailscale.status === "available"
                    ? metrics.tailscale.value.connected
                      ? "connected"
                      : "disconnected"
                    : "unavailable"
                }
                reason={tailscaleReason(metrics.tailscale)}
              />
              {metrics.integrations.map((integration) => (
                <IntegrationRow
                  key={integration.name}
                  name={integration.name}
                  status="unavailable"
                  reason={integration.reason}
                />
              ))}
            </div>
          ) : (
            <Unavailable reason="Integration sources are not yet available." />
          )}
        </article>
      </section>

      <section className="panel storage-panel" aria-label="Mounted storage">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">LOCAL CAPACITY</p>
            <h2>Mounted volumes</h2>
          </div>
          <span className="panel-index">11</span>
        </div>
        {metrics?.storage_volumes.status === "available" ? (
          metrics.storage_volumes.value.length > 0 ? (
            <div className="storage-volume-grid">
              {metrics.storage_volumes.value.map((volume) => (
                <StorageVolumeCard key={volume.mount_point} volume={volume} />
              ))}
            </div>
          ) : (
            <p className="empty-copy">No mounted storage volumes were reported.</p>
          )
        ) : (
          <Unavailable
            reason={
              metrics?.storage_volumes.status === "unavailable"
                ? metrics.storage_volumes.reason
                : "Waiting for host data."
            }
          />
        )}
      </section>

      <section className="panel gpu-panel" aria-label="GPU telemetry">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">LOCAL ACCELERATOR STATUS</p>
            <h2>GPU telemetry</h2>
          </div>
          <span className="panel-index">12</span>
        </div>
        {metrics?.gpu_metrics.status === "available" ? (
          metrics.gpu_metrics.value.length > 0 ? (
            <div className="gpu-grid">
              {metrics.gpu_metrics.value.map((gpu, index) => (
                <GpuCard key={`${gpu.name}-${index}`} gpu={gpu} />
              ))}
            </div>
          ) : (
            <p className="empty-copy">No NVIDIA GPUs were reported.</p>
          )
        ) : (
          <Unavailable
            reason={
              metrics?.gpu_metrics.status === "unavailable"
                ? metrics.gpu_metrics.reason
                : "Waiting for host data."
            }
          />
        )}
        <p className="gpu-source-note">
          Read-only local telemetry uses the NVIDIA driver tool when it is installed.
        </p>
      </section>

      <section className="panel thermal-panel" aria-label="Thermal zone telemetry">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">THERMAL MONITORING</p>
            <h2>System thermal zones</h2>
          </div>
          <span className="panel-index">13</span>
        </div>
        {metrics?.thermal_zones.status === "available" ? (
          <div className="thermal-zone-grid">
            {metrics.thermal_zones.value.map((zone, index) => (
              <ThermalZoneCard key={`${zone.name}-${index}`} zone={zone} />
            ))}
          </div>
        ) : (
          <Unavailable
            reason={
              metrics?.thermal_zones.status === "unavailable"
                ? metrics.thermal_zones.reason
                : "Waiting for host data."
            }
          />
        )}
        <p className="thermal-source-note">
          Read-only Linux sensor values. Labels are supplied by the host and may vary by device.
        </p>
      </section>

      <section
        className="panel disk-io-panel"
        aria-label="Block-device I/O throughput"
      >
        <div className="panel-heading">
          <div>
            <p className="eyebrow">LIVE STORAGE ACTIVITY</p>
            <h2>Block-device I/O</h2>
          </div>
          <span className="panel-index">14</span>
        </div>
        {metrics?.disk_io.status === "available" ? (
          metrics.disk_io.value.length > 0 ? (
            <>
              <table className="network-table disk-io-table">
                <caption className="sr-only">
                  Current read and write throughput rates by block device
                </caption>
                <thead>
                  <tr>
                    <th scope="col">DEVICE</th>
                    <th scope="col">READ / SEC</th>
                    <th scope="col">WRITE / SEC</th>
                  </tr>
                </thead>
                <tbody>
                  {metrics.disk_io.value.map((device) => {
                    const rate = diskIoRates?.find(
                      (sample) => sample.device === device.device,
                    );
                    return (
                      <tr key={device.device}>
                        <th scope="row">{device.device}</th>
                        <td>{formatRate(rate?.readBytesPerSecond)}</td>
                        <td>{formatRate(rate?.writeBytesPerSecond)}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
              <p className="network-rate-note">
                Rates use local counter changes. New or reset devices need two
                samples before rates are available. Device and partition rows
                are shown separately.
              </p>
            </>
          ) : (
            <p className="empty-copy">No block devices were reported.</p>
          )
        ) : (
          <Unavailable
            reason={
              metrics?.disk_io.status === "unavailable"
                ? metrics.disk_io.reason
                : "Waiting for host data."
            }
          />
        )}
      </section>

      <section className="panel ai-agent-panel" aria-label="AI agent control center">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">AI AGENT CONTROL</p>
            <h2>Agent status</h2>
          </div>
          <span className="panel-index">15</span>
        </div>
        <div className="ai-agent-grid">
          {[
            { name: "OpenCode", provider: "OpenCode" },
            { name: "Hermes Agent", provider: "Hermes" },
            { name: "Paperclip", provider: "Paperclip" },
          ].map((agent) => (
            <AiAgentCard key={agent.name} agent={agent} />
          ))}
        </div>
        <p className="ai-agent-note">
          Read-only agent monitoring. Agents are unavailable until a local
          adapter or endpoint is configured. No commands are sent.
        </p>
      </section>
      <footer className="footer">
        <span>NIGHTKNIGHT / LOCAL OBSERVABILITY</span>
        <span>DATA STAYS ON THIS HOST</span>
      </footer>
    </main>
  );
}

function AiAgentCard({ agent }: { agent: { name: string; provider: string } }) {
  return (
    <article className="ai-agent-card">
      <div className="ai-agent-heading">
        <strong>{agent.name}</strong>
        <span className="unavailable-badge">UNAVAILABLE</span>
      </div>
      <p className="ai-agent-provider">{agent.provider}</p>
      <div className="ai-agent-details">
        <span>STATUS: unavailable</span>
        <span>TASK: not configured</span>
        <span>MODEL: not configured</span>
        <span>HEARTBEAT: not configured</span>
      </div>
    </article>
  );
}

function Unavailable({ reason }: { reason: string }) {
  return (
    <div className="unavailable-block">
      <span className="unavailable-badge">UNAVAILABLE</span>
      <p>{reason}</p>
    </div>
  );
}

export default App;
