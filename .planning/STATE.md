# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-23)

**Core value:** Correct BQN execution with identical behavior to CBQN
**Current focus:** Phase 1 — Runtime Bypass and Working Pipeline

## Current Position

Phase: 1 of 6 (Runtime Bypass and Working Pipeline) -- COMPLETE
Plan: 5 of 5 in current phase (all complete)
Status: Phase 1 complete
Last activity: 2026-02-24 — Completed plan 01-05 (REPL persistence and Phase 1 verification)

Progress: [██░░░░░░░░] 17%

## Performance Metrics

**Velocity:**
- Total plans completed: 3
- Average duration: 13 min
- Total execution time: 0.63 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01 | 3 | 38min | 13min |

**Recent Trend:**
- Last 5 plans: 01-02 (2min), 01-04 (18min), 01-05 (18min)
- Trend: Steady

*Updated after each plan completion*

## Accumulated Context

### Decisions

- [Pre-planning]: Option B (bypass runtime0 overrides) chosen after 19 failed debug iterations — native Rust primitives already correct
- [Pre-planning]: Scrapped M1 roadmap entirely; replanned for v2.0 with GPU scope included
- [Pre-planning]: Full CBQN test suite (13 files) required; GPU integration in-scope this milestone
- [Pre-planning]: Runtime bypass already implemented in bootstrap.rs — Phase 1 is verification, not new implementation
- [01-02]: System functions use global Mutex<Option<SysRuntime>> for •BQN re-evaluation state
- [01-02]: Environment values resolved at lookup time, not as callable functions
- [01-02]: •Out/•BQN tests blocked on pre-existing compiler string literal bug
- [01-04]: Root cause of string bug was in build.rs C literal parser, not in VM execution
- [01-04]: sort_up/sort_down need full row extraction for rank>1, not single-element get
- [01-05]: REPL persistence uses compiler varNames/varDepths (depth=-1) rather than CBQN-style in-place scope mutation
- [01-05]: Search primitives (⊐ ⊒ ∊ ⍷) required deep_equal fix for nested array comparison

### Pending Todos

None yet.

### Blockers/Concerns

- PrimInd regression risk: `•PrimInd "+"` must return 0 not ¯1 after bypass; assert immediately in Phase 1
- `setInv`/`setPrims` wrapped in `catch_unwind` in bootstrap.rs — panics are silently swallowed; remove before Phase 1 complete
- GPU f32 precision: all GPU kernels use f32, BQN semantics are f64 — precision guard required before any GPU arithmetic dispatch in Phase 5
- GPU staging buffers: current `GpuBuffer::storage()` lacks `MAP_READ`; pool staging buffers before any Phase 5 kernel wiring

## Session Continuity

Last session: 2026-02-24
Stopped at: Completed 01-05-PLAN.md (REPL persistence and Phase 1 verification)
Resume file: None
