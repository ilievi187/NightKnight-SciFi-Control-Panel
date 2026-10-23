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

The API also reports the Linux 1, 5, and 15 minute load averages from `/proc/loadavg`. These values are not normalized by CPU core count and are unavailable on other operating systems.

The `/api/metrics` response also includes `storage_volumes`, which reports capacity for mounted filesystems that expose usable space. Virtual filesystems such as procfs, sysfs, and tmpfs are omitted. The command center refreshes these readings with the rest of host telemetry and shows each mount point, filesystem type, device, and used capacity.

When the NVIDIA driver tool `nvidia-smi` is available, `/api/metrics` also reports GPU utilization, video memory use, and temperature for each NVIDIA GPU. The API runs a fixed read-only query with a two-second timeout. Missing tools, unsupported devices, or incomplete readings are reported as unavailable; the application does not send GPU commands or control the device.

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
