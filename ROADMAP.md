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
- [x] Secure multi-host integration with auth (PR #23)
- [x] AMD GPU telemetry via sysfs (PR #20)
- [x] Windows telemetry adapter (PR #25)
- [x] Fix local CPU telemetry (PR #24)
- [ ] Windows CI validation (PR pending)
- [ ] Windows telemetry runtime validation
- [ ] Authenticated Tailscale connectivity (full implementation)
- [ ] Host detail pages with historical charts

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
| #23 | feat: secure multi-host integration with auth | 19294f7 |
| #24 | feat: fix local CPU telemetry, real remote metrics parsing | 46a9393 |
| #25 | feat: add Windows telemetry adapter with PowerShell-based metrics | 2e64682 |

## Current Status
- All CI checks passing (Linux)
- Windows CI added (pending validation)
- 24 Rust tests, frontend build passes
- Local-first, AMD + NVIDIA GPU support
- Multi-host probing with 5s timeout
- Real remote metrics parsing (CPU, RAM, GPU, VRAM)
- Windows telemetry adapter (unverified)

## Limitations
- Windows telemetry untested (no Windows host available)
- Multi-host not verified (no second host available)
- AI agents shown as unavailable (no adapters yet)
- Frontend has no automated tests

## Next Priority
Phase C: Windows CI validation + Phase D: AI agent adapters