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

On Linux, the API reads the hostname and OS release files, CPU and memory counters in /proc, root filesystem capacity through statvfs, and interface counters in /proc/net/dev. Container runtime, GPU, remote probe, and cloud API integrations are marked unavailable until a local source or endpoint is configured.

## Checks

Run Rust unit tests and the frontend typecheck/build:

    cargo test --manifest-path server/Cargo.toml
    npm run build

## Build and preview

Build the frontend and serve the production build locally:

    npm run build
    npm run preview

The preview server also forwards /api requests to the local Rust API.

The UI polls the local API every five seconds. It does not connect to external services.
