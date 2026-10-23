# NightKnight Sci-Fi Control Center — Development Roadmap

## Phase A — Foundation ✅
- [x] Rust + Axum backend with /api/health and /api/metrics
- [x] React + TypeScript frontend with host telemetry
- [x] Linux host metrics (CPU, RAM, disk, network, GPU, thermal, pressure, storage, disk I/O)
- [x] Tailscale integration (read-only)
- [x] GitHub Actions CI
- [x] Sci-fi animated background
- [x] System overview panel
- [x] AI agent status panel (foundation)

## Phase B — Sci-Fi Interface ✅
- [x] Dark futuristic UI with cyan/blue/violet accents
- [x] Animated telemetry panels and sparkline charts
- [x] Responsive desktop/tablet/mobile layouts
- [x] Reduced-motion support
- [x] Real-time polling (5s interval)

## Phase C — Multi-Computer Monitoring 🔄
- [ ] Host registry (hostname, OS, connection status, heartbeat)
- [ ] Authenticated host connectivity (Tailscale, SSH read-only)
- [ ] Linux host telemetry adapter
- [ ] Windows host telemetry adapter
- [ ] AMD GPU telemetry
- [ ] Multi-host UI with device cards
- [ ] Host detail pages

## Phase D — AI Agent Control Center 🔄
- [x] AI agent status panel (foundation, all unavailable)
- [ ] OpenCode adapter
- [ ] Hermes Agent adapter
- [ ] Paperclip adapter
- [ ] Agent detail view with task history
- [ ] Token usage and cost tracking

## Phase E — Jobs and Operations
- [ ] Task queue monitoring
- [ ] GitHub PR/CI monitoring
- [ ] Error/warning dashboards
- [ ] Job progress tracking
- [ ] Notifications

## Phase F — Polish and Optimization
- [ ] Frontend tests
- [ ] Performance optimization
- [ ] Security audit
- [ ] Documentation

## Completed PRs
| PR | Title | Commit |
|----|-------|--------|
| #14 | Show read-only Tailscale connection status | f4e6413 |
| #15 | fix: collapse collapsible if in pressure parsing | 7ad63b1 |
| #16 | feat(ui): add sci-fi animated background | 3a4be81 |
| #17 | feat(ui): add system overview panel | 47928d6 |
| #18 | feat(ui): add AI agent status panel foundation | 1a4af7c |

## Current Status
- All CI checks passing
- 17 Rust tests, frontend build passes
- Local-first, no external dependencies required

## Next Priority
Phase C: Multi-computer monitoring foundation (host registry + UI)