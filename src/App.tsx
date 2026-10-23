import { useCallback, useEffect, useRef, useState } from "react";
import type { HostMetrics, Metric, NetworkInterfaceMetrics } from "./types";

type MetricCardProps<T> = {
  label: string;
  metric: Metric<T>;
  format: (value: T) => string;
  detail?: (value: T) => string;
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

const MAX_HISTORY_SAMPLES = 24;

function MetricCard<T>({
  label,
  metric,
  format,
  detail,
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
  const refreshInFlight = useRef(false);
  const previousNetworkSample = useRef<NetworkSample | null>(null);

  const refresh = useCallback(async () => {
    if (refreshInFlight.current) return;
    refreshInFlight.current = true;
    setLoading(true);
    try {
      const response = await fetch("/api/metrics", { cache: "no-store" });
      if (!response.ok) throw new Error("API returned HTTP " + response.status);
      const payload = (await response.json()) as HostMetrics;
      setMetrics(payload);
      if (payload.network_interfaces.status === "available") {
        const sampledAt = performance.now();
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

      <section className="section-heading">
        <div>
          <p className="eyebrow">HOST TELEMETRY</p>
          <h2>Current readings</h2>
        </div>
        <span className="refresh-note">AUTO REFRESH / 5 SEC</span>
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
              history={
                history.length
                  ? history.map((sample) => sample.disk)
                  : undefined
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

      <section className="lower-grid">
        <article className="panel network-panel">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">LIVE THROUGHPUT</p>
              <h2>Network interfaces</h2>
            </div>
            <span className="panel-index">05</span>
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
            <span className="panel-index">06</span>
          </div>
          {metrics ? (
            <div className="integration-list">
              {metrics.integrations.map((integration) => (
                <div className="integration-row" key={integration.name}>
                  <span className="integration-symbol">—</span>
                  <div className="integration-copy">
                    <strong>{integration.name}</strong>
                    <p>{integration.reason}</p>
                  </div>
                  <span className="unavailable-badge">UNAVAILABLE</span>
                </div>
              ))}
            </div>
          ) : (
            <Unavailable reason="Integration sources are not yet available." />
          )}
        </article>
      </section>

      <footer className="footer">
        <span>NIGHTKNIGHT / LOCAL OBSERVABILITY</span>
        <span>DATA STAYS ON THIS HOST</span>
      </footer>
    </main>
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
