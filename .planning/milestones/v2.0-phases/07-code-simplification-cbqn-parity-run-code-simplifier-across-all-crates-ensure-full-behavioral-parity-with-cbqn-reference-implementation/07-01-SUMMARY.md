---
phase: 07-code-simplification-cbqn-parity
plan: 01
subsystem: testing
tags: [clippy, warnings, code-quality, rust]

# Dependency graph
requires: []
provides:
  - "Zero-warning workspace build (0 clippy + 0 compiler warnings)"
  - "Clean baseline for Phase 07 structural simplification"
affects: [07-02, 07-03]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Use _ prefix for unused function params in dispatch signatures"
    - "Use iter().enumerate() / iter_mut().enumerate() over indexed loops"
    - "#[allow(clippy::too_many_arguments)] for fixed-signature public API functions"

key-files:
  created: []
  modified:
    - crates/rbqn-prim/src/fold.rs
    - crates/rbqn-prim/src/group.rs
    - crates/rbqn-prim/src/search.rs
    - crates/rbqn-prim/src/select.rs
    - crates/rbqn-prim/src/slash.rs
    - crates/rbqn-prim/src/structural.rs
    - crates/rbqn-prim/src/sysfn.rs
    - crates/rbqn-vm/src/bytecode.rs
    - crates/rbqn-vm/src/compiler.rs
    - crates/rbqn-vm/src/derive.rs
    - crates/rbqn-vm/src/modifiers.rs
    - crates/rbqn/src/bootstrap.rs
    - crates/rbqn/src/repl.rs

key-decisions:
  - "Removed dead functions deep_pick_one/deep_pick/group_multi_axis_scalar_x/md2d_inverse_reg rather than suppressing — they were unreachable"
  - "Removed unused result_size fields from DimSpec enums — result_shape Vec tracks the data separately"
  - "Used #[allow(clippy::too_many_arguments)] for compile_all/compile_block — restructuring is out of scope for this plan"

patterns-established:
  - "Diverging error::throw() calls must NOT use return — return error::throw(...) triggers unreachable_code and diverging_sub_expression warnings simultaneously"

requirements-completed: [SIMP-01, SIMP-02]

# Metrics
duration: 35min
completed: 2026-03-02
---

# Phase 7 Plan 01: Code Simplification — Warning Elimination Summary

**Zero-warning workspace: eliminated 209 clippy + 39 compiler warnings via auto-fix and manual cleanup while keeping 1316/1316 BQN tests green**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-03-02T16:03Z (Task 1 commit timestamp)
- **Completed:** 2026-03-02
- **Tasks:** 2
- **Files modified:** 13

## Accomplishments
- `cargo clippy --workspace` produces 0 warnings (was 209)
- `cargo build --workspace` produces 0 warnings (was 39)
- All 1316 BQN test cases still pass (13/13 files)
- test_cbqn_compat.sh still passes 21/21
- Removed 4 dead functions that clippy flagged as never-used

## Task Commits

1. **Task 1: Apply cargo clippy --fix auto-corrections** - `6a22343` (chore) — previous session
2. **Task 2: Manually fix remaining warnings** - `94e5558` (chore)

## Files Created/Modified
- `crates/rbqn-prim/src/fold.rs` - Remove Err() wrapper around throw_nyi (never type)
- `crates/rbqn-prim/src/group.rs` - Remove dead group_multi_axis_scalar_x; drop unused result_size DimSpec fields; prefix unused params
- `crates/rbqn-prim/src/search.rs` - Use enumerate() on class_map/used; zip for result_shape loop
- `crates/rbqn-prim/src/select.rs` - Remove dead deep_pick_one/deep_pick functions
- `crates/rbqn-prim/src/slash.rs` - Use iter_mut().enumerate() for xi/counts loop
- `crates/rbqn-prim/src/structural.rs` - Prefix unused vars; use enumerate() for loop vars; simplify stride product
- `crates/rbqn-prim/src/sysfn.rs` - Prefix _xa on unused param
- `crates/rbqn-vm/src/bytecode.rs` - Add transmute type annotations
- `crates/rbqn-vm/src/compiler.rs` - Add #[allow] for too_many_arguments; use enumerate/slice iter for loop vars
- `crates/rbqn-vm/src/derive.rs` - Remove dead md2d_inverse_reg; merge identical if/else blocks; prefix unused params; add #[allow] for too_many_arguments and type_complexity
- `crates/rbqn-vm/src/modifiers.rs` - Remove return from diverging throw() calls in scan_c1/c2/inv_c1/inv_c2
- `crates/rbqn/src/bootstrap.rs` - Remove redundant let binding; fix doc list indentation
- `crates/rbqn/src/repl.rs` - Use strip_prefix for manual_strip warning

## Decisions Made
- Removed dead functions rather than suppressing warnings — they were truly unreachable and removing them reduced code size by ~270 lines
- Used `#[allow(clippy::too_many_arguments)]` for compile_all/compile_block/rbqn_vm_compile_all — these are internal VM functions matching CBQN's calling conventions; restructuring would be out of scope
- `return error::throw(...)` pattern removed: since `error::throw()` returns `!`, the `return` keyword triggers both `unreachable_code` and `diverging_sub_expression` simultaneously — dropping `return` fixes both

## Deviations from Plan

None — plan executed exactly as written. Both auto-fix (Task 1, done in prior session) and manual cleanup (Task 2) completed without incident.

## Issues Encountered

None — all warnings were straightforward to resolve. The `return error::throw(...)` pattern was the only non-obvious one (two warnings from one construct).

## Next Phase Readiness
- Clean zero-warning baseline established for Phase 07 Plans 02 and 03
- Phase 07 Plan 02: structural simplification (dead code removal, complex function splitting)
- Phase 07 Plan 03: CBQN behavioral parity verification

---
*Phase: 07-code-simplification-cbqn-parity*
*Completed: 2026-03-02*
