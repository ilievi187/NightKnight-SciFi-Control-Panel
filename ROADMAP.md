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
- [x] Host registry foundation (PR #19)
- [x] Multi-host connectivity with authenticated probing (PR #21)
- [x] Real multi-host telemetry with JSON parsing (PR #22)
- [x] AMD GPU telemetry via sysfs (PR #20)
- [ ] Windows host telemetry adapter
- [ ] AMD GPU error handling improved (optional fields)
- [ ] Host detail pages with historical charts
- [ ] Authenticated Tailscale connectivity (full implementation)

## Phase D — AI Agent Control Center 🔄
- [x] AI agent status panel foundation (PR #18)
- [ ] OpenCode adapter (read-only)
- [ ] Hermes Agent adapter (read-only)
- [ ] Paperclip adapter (read-only)
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
| #19 | feat: add host registry foundation with /api/hosts | c22fc14 |
| #20 | feat: add AMD GPU telemetry via sysfs | a43fe37 |
| #21 | feat: multi-host connectivity with authenticated probing | 988219e |
| #22 | feat: real multi-host telemetry with JSON parsing | c2477ee |

## Current Status
- All CI checks passing
- 24 Rust tests, frontend build passes
- Local-first, AMD + NVIDIA GPU support
- Multi-host probing with 5s timeout
- Real remote metrics parsing (CPU, RAM, GPU, VRAM)

## Limitations
- Remote host telemetry requires NIGHTKNIGHT_HOSTS env var
- Auth token via NIGHTKNIGHT_HOST_TOKEN (Bearer header)
- Windows telemetry not yet implemented
- AI agents shown as unavailable (no adapters yet)
- No live multi-host testing (single host environment)

## Next Priority
Phase C: Windows host telemetry adapter + Phase D: AI agent adapters
