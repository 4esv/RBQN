---
phase: 04-full-test-suite-green
plan: 09
subsystem: primitives
tags: [group, grade, sort, cells, select, fill, pick]

requires:
  - phase: 04-06, 04-07, 04-08
    provides: "Gap closure fixes reducing failures to ~12 remaining"
provides:
  - "All 13 BQN test files passing at 0 failures"
  - "Grade/sort on boxed arrays with proper BQN array ordering"
  - "Multi-axis select with enclosed (rank-0) indices"
  - "Cells-on-atom returning rank-0 arrays"
  - "Take-from-empty using proper fill elements"
affects: [04-10, phase-5]

tech-stack:
  added: []
  patterns: ["BQN array comparison ordering: rank first, shape, then contents"]

key-files:
  created: []
  modified:
    - crates/rbqn-core/src/compare.rs
    - crates/rbqn-prim/src/compare.rs
    - crates/rbqn-prim/src/group.rs
    - crates/rbqn-prim/src/select.rs
    - crates/rbqn-prim/src/structural.rs
    - crates/rbqn-vm/src/modifiers.rs

key-decisions:
  - "Implemented full BQN array ordering for grade: rank-first, then shape, then contents"
  - "Multi-axis select allows rank-0 (enclosed) indices for axis reduction"

patterns-established:
  - "Array comparison: compare by rank first, then by shape lexicographically, then by contents"

requirements-completed: [TEST-05, TEST-06, TEST-07, TEST-08, TEST-09, TEST-10, TEST-11, TEST-12, TEST-13]

duration: 35min
completed: 2026-02-27
---

# Plan 04-09: Fix Final Test Failures Summary

**Grade/sort on boxed arrays, cells-on-atom rank-0, multi-axis select with enclosed indices, take-fill propagation -- all 13 BQN test files now pass**

## Performance

- **Duration:** 35 min
- **Started:** 2026-02-27T23:25:00Z
- **Completed:** 2026-02-28T00:00:00Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- All 13 official BQN test files pass with 0 failures (564 prim, 62 fill, etc.)
- Fixed grade/sort comparison for boxed arrays with the full BQN array ordering spec
- Fixed cells modifier to return rank-0 arrays when applied to atoms
- Fixed multi-axis select to allow enclosed (rank-0) index elements
- Fixed take-from-empty to propagate proper fill elements
- Fixed multi-dim group tuple construction for varying-rank index arrays

## Task Commits

1. **Task 1+2: Fix all remaining prim/fill failures** - `174164f` (fix)

**Plan metadata:** included in this summary

## Files Created/Modified
- `crates/rbqn-core/src/compare.rs` - Full BQN array ordering for grade/sort
- `crates/rbqn-prim/src/compare.rs` - Boxed array comparison helpers
- `crates/rbqn-prim/src/group.rs` - Multi-dim group with proper tuple construction
- `crates/rbqn-prim/src/select.rs` - Rank-0 (enclosed) index support in multi-axis select
- `crates/rbqn-prim/src/structural.rs` - Take-from-empty fill propagation
- `crates/rbqn-vm/src/modifiers.rs` - Cells-on-atom returns rank-0 array

## Decisions Made
- Implemented full BQN array ordering for grade: compare by rank first, then shape lexicographically, then contents element-by-element
- Multi-axis select accepts rank-0 (enclosed) indices which reduce the corresponding axis

## Deviations from Plan
None - plan executed as written (Tasks 1 and 2 combined into a single commit since fixes were interdependent).

## Issues Encountered
- HashMap initialization code in derive.rs used `Some` pattern on `Result` returns from `arr.get()` -- fixed to use `Ok` pattern. This was a pre-existing compile error that prevented rebuild.

## Next Phase Readiness
- All test files pass, ready for system stubs (plan 04-10) and phase completion

---
*Phase: 04-full-test-suite-green*
*Completed: 2026-02-27*
