import { useCallback, useEffect, useState } from "react";
import type { HostMetrics, Metric } from "./types";

type MetricCardProps<T> = {
  label: string;
  metric: Metric<T>;
  format: (value: T) => string;
  detail?: (value: T) => string;
  index: string;
};

function MetricCard<T>({
  label,
  metric,
  format,
  detail,
  index,
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

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const response = await fetch("/api/metrics", { cache: "no-store" });
      if (!response.ok) throw new Error("API returned HTTP " + response.status);
      const payload = (await response.json()) as HostMetrics;
      setMetrics(payload);
      setApiState("online");
      setError("");
    } catch (cause) {
      setMetrics(null);
      setApiState("offline");
      setError(
        cause instanceof Error ? cause.message : "Could not reach the local API.",
      );
    } finally {
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
              <p className="eyebrow">TRAFFIC COUNTERS</p>
              <h2>Network interfaces</h2>
            </div>
            <span className="panel-index">05</span>
          </div>
          {metrics?.network_interfaces.status === "available" ? (
            metrics.network_interfaces.value.length > 0 ? (
              <table className="network-table">
                <caption className="sr-only">Network traffic by interface</caption>
                <thead>
                  <tr>
                    <th scope="col">INTERFACE</th>
                    <th scope="col">RECEIVED</th>
                    <th scope="col">TRANSMITTED</th>
                  </tr>
                </thead>
                <tbody>
                  {metrics.network_interfaces.value.map((network) => (
                    <tr key={network.name}>
                      <th scope="row">{network.name}</th>
                      <td>{formatBytes(network.received_bytes)}</td>
                      <td>{formatBytes(network.transmitted_bytes)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
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
