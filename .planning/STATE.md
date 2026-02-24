# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-23)

**Core value:** Correct BQN execution with identical behavior to CBQN
**Current focus:** Phase 3 — Language Completeness

## Current Position

Phase: 3 of 6 (Language Completeness) -- COMPLETE
Plan: 4 of 4 in current phase (03-04 complete)
Status: Phase 3 complete — ready for Phase 4
Last activity: 2026-02-24 — Completed plan 03-04 (modifier gap closure: sys 203 c2, dyadic +⁼, functional-k Under)

Progress: [███████░░░] 60%

## Performance Metrics

**Velocity:**
- Total plans completed: 7
- Average duration: 10 min
- Total execution time: ~1.55 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01 | 3 | 38min | 13min |
| 02 | 3 | 20min | 7min |
| 03 | 3 | ~37min | ~12min |

**Recent Trend:**
- Last 5 plans: 02-01 (8min), 02-02 (4min), 02-03 (8min), 03-02 (~15min), 03-03 (~35min)
- Trend: Steady (averaging ~11min)

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
- [02-02]: v_get and v_get_move need array handling for SETM with list targets (destructuring modify-assign)
- [02-02]: syntax.bqn improved from 152 to 153 as collateral benefit of v_get array fix
- [02-03]: ARMM merge targets use bit-0 marker in array payload to distinguish from LSTM list targets
- [02-03]: Rank-1 merge-destructuring wraps elements as rank-0 unit arrays (CBQN m_unit semantics)
- [03-02]: Dyadic •FChars/•FBytes write variants share sys_idx with monadic reads (52/53) — arity distinguishes
- [03-02]: •ImportCache uses B::SENTINEL as circular-import sentinel; canonical path as cache key via fs::canonicalize
- [03-02]: •ParseFloat normalizes ¯→- then handles ∞ and π as special string cases before f64 parse
- [03-03]: CBQN compiler strips underscores from modifier names — "_while_" → "while", "_fillBy_" → "fillby"
- [03-03]: Math functions use sys indices 1100-1130 to avoid collision with existing sys 100 (system resolver)
- [03-03]: BQN namespace field names are lowercased by compiler: "PI" → "pi" at access time
- [Phase 03]: native_inverse_reg removes + shortcut so BQN runtime handles dyadic +⁼ char arithmetic: 3+⁼'d'='a' via runtime -˜ inverse
- [Phase 03]: sys 203 c2 dispatch: w√⁼x = x^w implemented via pow_c2(x, xa, w, wa) with swapped args
- [Phase 03]: Functional-k Under: detect left_op.is_fun() in try_structural_under, evaluate c1(left_op,x) to get numeric k before take/drop

### Pending Todos

None yet.

### Blockers/Concerns

- PrimInd regression risk: `•PrimInd "+"` must return 0 not ¯1 after bypass; assert immediately in Phase 1
- `setInv`/`setPrims` wrapped in `catch_unwind` in bootstrap.rs — panics are silently swallowed; remove before Phase 1 complete
- GPU f32 precision: all GPU kernels use f32, BQN semantics are f64 — precision guard required before any GPU arithmetic dispatch in Phase 5
- GPU staging buffers: current `GpuBuffer::storage()` lacks `MAP_READ`; pool staging buffers before any Phase 5 kernel wiring

## Session Continuity

Last session: 2026-02-24
Stopped at: Completed 03-04-PLAN.md (modifier gap closure: sys 203 c2, dyadic +⁼ char fallthrough, functional-k Under)
Resume file: None
