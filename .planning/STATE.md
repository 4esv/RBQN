# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-23)

**Core value:** Correct BQN execution with identical behavior to CBQN
**Current focus:** Phase 1 — Runtime Bypass and Working Pipeline

## Current Position

Phase: 1 of 6 (Runtime Bypass and Working Pipeline)
Plan: 0 of ? in current phase
Status: Ready to plan
Last activity: 2026-02-23 — Roadmap v2.0 created; milestone replanned from scratch with Option B strategy

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**
- Total plans completed: 0
- Average duration: —
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| - | - | - | - |

**Recent Trend:**
- Last 5 plans: —
- Trend: —

*Updated after each plan completion*

## Accumulated Context

### Decisions

- [Pre-planning]: Option B (bypass runtime0 overrides) chosen after 19 failed debug iterations — native Rust primitives already correct
- [Pre-planning]: Scrapped M1 roadmap entirely; replanned for v2.0 with GPU scope included
- [Pre-planning]: Full CBQN test suite (13 files) required; GPU integration in-scope this milestone
- [Pre-planning]: Runtime bypass already implemented in bootstrap.rs — Phase 1 is verification, not new implementation

### Pending Todos

None yet.

### Blockers/Concerns

- PrimInd regression risk: `•PrimInd "+"` must return 0 not ¯1 after bypass; assert immediately in Phase 1
- `setInv`/`setPrims` wrapped in `catch_unwind` in bootstrap.rs — panics are silently swallowed; remove before Phase 1 complete
- GPU f32 precision: all GPU kernels use f32, BQN semantics are f64 — precision guard required before any GPU arithmetic dispatch in Phase 5
- GPU staging buffers: current `GpuBuffer::storage()` lacks `MAP_READ`; pool staging buffers before any Phase 5 kernel wiring

## Session Continuity

Last session: 2026-02-23
Stopped at: Roadmap created; ready to plan Phase 1
Resume file: None
