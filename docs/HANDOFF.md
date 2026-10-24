# NightKnight Sci-Fi Control Center — Handoff Checkpoint

## Repository
https://github.com/ilievi187/NightKnight-SciFi-Control-Panel

## Current State (2026-10-24)
- Latest commit: `a735602` (main)
- Latest release: v0.1.1-alpha (tag: a735602)
- Previous release: v0.1.0-alpha (tag: 65126fc1)

## Verified Working
- OpenCode: 50+ sessions, real models/tokens/costs (localhost:4098)
- Hermes: 50+ sessions, real models/tokens/costs (localhost:8642)
- Paperclip: 1 archived company, 0 agents (localhost:3100) — correctly shows as idle, no fake data
- All three providers return genuine data via /api/agents
- Host telemetry: CPU, RAM, disk, network, GPU, thermal, pressure, storage, disk I/O
- AMD + NVIDIA GPU monitoring
- Tailscale integration (read-only)
- Multi-host probing with auth

## Configuration
- .env file at project root (gitignored, chmod 600)
- .env.example template committed (no secrets)
- OPENCODE_ENDPOINT, HERMES_ENDPOINT, PAPERCLIP_ENDPOINT configured
- Auth tokens stored in .env only (never committed)

## Deployed Installation
- Location: ~/nightknight-sci-fi-control-center-0.1.1-alpha-linux-x86_64/
- Backup: ~/nightknight-sci-fi-control-center-backup-20261024-174237/
- Running on: http://127.0.0.1:8080
- Config: .env file in deployment directory (chmod 600)

## CI Status
- All checks green (Linux + Windows)
- 42 Rust tests pass
- Frontend build passes
- Clippy clean, fmt clean

## Remaining Work
1. Frontend agent tests (no test infrastructure yet)
2. Agent detail views (task history, etc.)
3. Token monitoring UI (separate counters for complimentary token groups)
4. GitHub PR/CI monitoring
5. Windows telemetry runtime validation (no Windows host)
6. Multi-host live verification (no second host)

## Next Agent Instructions
- Start OpenCode: `opencode serve --port 4098` (password in /tmp/oc.log)
- Start Hermes: already running on 8642
- Start Paperclip: `LD_LIBRARY_PATH=... paperclipai run --instance default`
- Set env vars from .env.example
- Run: `cargo run --manifest-path server/Cargo.toml`
- Dashboard: http://127.0.0.1:8080

## Security Notes
- API binds to 127.0.0.1:8080 (localhost only)
- Auth optional via NIGHTKNIGHT_API_TOKEN
- .env file is gitignored (chmod 600)
- No secrets in source code or commits
- Paperclip uses X-API-Key header (not Bearer)

## Verified Agent Data (2026-10-24)
- OpenCode: 50 sessions, real models (beam-free, kimi-k3, etc.), tokens, costs
- Hermes: 50 sessions, real models (gpt-6-luna-900k), tokens, costs
- Paperclip: 1 archived company (Ilievi), 0 agents — correctly shows as idle, no fake data
- All providers return genuine data via /api/agents