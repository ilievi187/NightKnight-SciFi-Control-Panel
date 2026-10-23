# NightKnight Command Center

A local-first command surface for monitoring the host that runs it. The React + TypeScript UI reads current host metrics from a small Rust API. Metrics that have no configured source are shown as unavailable; the application does not seed example values.

## Requirements

- Node.js 20.19 or later in the 20.x line, or 22.12 or later, and npm
- Rust stable toolchain with Cargo
- Linux for live host metrics; on other operating systems, the API reports Linux-only metrics as unavailable

## Development

Install the web dependencies once:

    npm install

Start the API in one terminal:

    cargo run --manifest-path server/Cargo.toml

Start the web client in another terminal:

    npm run dev

Open the local URL printed by Vite. The development server forwards /api requests to the API at 127.0.0.1:8080.

The API binds to loopback only. It exposes:

- GET /api/health for a process health check
- GET /api/metrics for current host metrics and integration availability

On Linux, the API reads the hostname and OS release files, CPU and memory counters in /proc, root filesystem capacity through statvfs, and interface counters in /proc/net/dev. Remote probe and cloud API integrations are marked unavailable until a local source or endpoint is configured.

The API also reports the Linux 1, 5, and 15 minute load averages from `/proc/loadavg`, plus the current runnable and total task counts. These averages are not normalized by CPU core count and are unavailable on other operating systems.

Per-core CPU utilization is sampled from Linux `/proc/stat` counter changes. New or reset cores are omitted until two valid samples are available; on other operating systems this reading is unavailable.

System pressure stall information is read from Linux `/proc/pressure/cpu`, `/proc/pressure/memory`, and `/proc/pressure/io`. The dashboard shows `some` and (when available) `full` stall percentages over the 10, 60, and 300 second windows. System-wide CPU `full` pressure is not defined. Pressure readings are unavailable when the kernel does not expose a valid source.

Linux swap capacity is read from `/proc/meminfo` and shown as used and free space. If swap is disabled, the command center reports it as disabled; missing or invalid source data is reported as unavailable.

When Linux exposes readable thermal zones under `/sys/class/thermal`, the API reports each zone's host-provided label and temperature in Celsius. The command center shows these as generic system thermal readings because zone labels and sensor coverage vary by device. Thermal readings are unavailable when the source cannot be read or no usable zones are exposed.

The `/api/metrics` response also includes `storage_volumes`, which reports capacity for mounted filesystems that expose usable space. When a filesystem reports inode capacity, the API includes total, free, and used inode counts. The command center refreshes these readings with the rest of host telemetry and shows each mount point, filesystem type, device, space use, and inode use. Inode readings are omitted when a filesystem does not report inode capacity. Virtual filesystems such as procfs, sysfs, and tmpfs are omitted.

When the NVIDIA driver tool `nvidia-smi` is available, `/api/metrics` also reports GPU utilization, video memory use, and temperature for each NVIDIA GPU. The API runs a fixed read-only query with a two-second timeout. Missing tools, unsupported devices, or incomplete readings are reported as unavailable; the application does not send GPU commands or control the device.

On Linux, the integrations panel reads the local Tailscale backend state with `tailscale status --json --peers=false`. It reports whether the daemon is running and does not collect peer details or change Tailscale configuration. Missing tools, timeouts, or invalid output are reported as unavailable.

## Checks

Run Rust unit tests and the frontend typecheck/build:

    cargo test --manifest-path server/Cargo.toml
    npm run build

## Build and preview

Build the frontend and serve the production build locally:

    npm run build
    npm run preview

The preview server also forwards /api requests to the local Rust API.

The UI polls the local API every five seconds. Metric cards show up to 24 recent
CPU, memory, and root filesystem samples in browser memory. History resets when
the page reloads. Unavailable samples create gaps in the chart. The UI does not
connect to external services.

Network interface rates are calculated from local byte-counter changes between
polls. A newly seen or reset interface needs two samples before its receive and
transmit rates are available.

Block-device read and write rates use the cumulative sector counters in Linux
[`/proc/diskstats`](https://docs.kernel.org/admin-guide/iostats.html). The
dashboard converts standard 512-byte sectors to bytes per second from local
counter changes. Device and partition rows remain separate; new or reset devices
need two samples before rates are available. On other operating systems,
block-device I/O is unavailable.
