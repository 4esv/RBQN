---
phase: 07-code-simplification-cbqn-parity
plan: 03
subsystem: testing
tags: [rust, refactoring, code-quality, simplification]

# Dependency graph
requires:
  - phase: 07-01
    provides: zero-warning workspace, clean baseline for structural simplification
  - phase: 07-02
    provides: CBQN parity fixes (formatter, errors, glyph)
provides:
  - "Simplified rbqn-vm dispatch (derive.rs, modifiers.rs) — -88 lines"
  - "Simplified rbqn-prim structural/arithmetic helpers (structural.rs, arith_dyad.rs) — -30 lines"
  - "Shared helper functions eliminating duplication: native_prim_idx, make_md1/md2_block_val, b_to_prim, enclose_scalar, strides_from_shape, pervasive_fill"
  - "Zero clippy warnings maintained; all 1316 BQN tests still green"
affects: [all future phases that read derive.rs, modifiers.rs, structural.rs]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Extract prim_idx from B via shared native_prim_idx() — avoids repeating id extraction + match across glyph/primind dispatch"
    - "make_md1_block_val/make_md2_block_val — reconstruct temporary modifier B values from block+scope (used in InvMd1Block/InvMd2Block)"
    - "b_to_prim(b, opt_arr) — convert B+optional arr to PrimResult::Scalar or PrimResult::Array"
    - "enclose_scalar(x) — create rank-0 BqnArr{shape:[],data:Boxed([x]),fill:Some(prototype_of(x))}"
    - "strides_from_shape(&shape) — row-major stride vector for N-D iteration"
    - "pervasive_fill(&results, fallback) — compute fill as prototype_of(results[0]) or fallback"

key-files:
  created: []
  modified:
    - crates/rbqn-vm/src/derive.rs
    - crates/rbqn-vm/src/modifiers.rs
    - crates/rbqn-prim/src/structural.rs
    - crates/rbqn-prim/src/arith_dyad.rs

key-decisions:
  - "Remove 4 thin dispatch_native_md1/md2_c1/c2 wrapper functions in derive.rs — call crate::modifiers:: directly at call sites"
  - "Use typed_arr_from_b_vec from rbqn-core to simplify results_to_arr_fill — override fill after the call to preserve None semantics"
  - "merge_cells_result delegates scalar/empty cases to results_to_arr — removes 20 lines of duplicate typed array construction"
  - "strides_from_shape added to structural.rs rather than rbqn-core — it's used only internally in join/rotate computations"

patterns-established:
  - "When extracting DerivedKind::NativeFn/Md1/Md2 prim_idx from a B value: use native_prim_idx() helper rather than inline is_fun/is_md1/is_md2 + store lookup"
  - "Rank-0 array construction: prefer enclose_scalar(x) over inline BqnArr struct literal"

requirements-completed: [SIMP-03]

# Metrics
duration: 35min
completed: 2026-03-02
---

# Phase 7 Plan 03: Code Simplification — rbqn-vm and rbqn-prim Summary

**Extracted 6 shared helpers across derive.rs, modifiers.rs, structural.rs, arith_dyad.rs, reducing duplication by 118 lines while keeping all 1316 BQN tests green**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-03-02T22:12:34Z
- **Completed:** 2026-03-02
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- `cargo clippy --workspace` still produces 0 warnings after all changes
- `cargo build --workspace` clean
- All 1316 BQN test cases still pass (13/13 files)
- test_cbqn_compat.sh still passes 21/21
- Line count reductions: derive.rs -36, modifiers.rs -52, structural.rs -24, arith_dyad.rs -6 = -118 total

## Task Commits

1. **Task 1: Simplify rbqn-vm crate** - `aff7fd8` (refactor)
2. **Task 2: Simplify rbqn-prim crate** - `f2f9e8c` (refactor)

## Files Created/Modified

- `crates/rbqn-vm/src/derive.rs` - Remove 4 thin wrapper functions; add native_prim_idx, make_md1/md2_block_val helpers
- `crates/rbqn-vm/src/modifiers.rs` - Simplify results_to_arr_fill via typed_arr_from_b_vec; simplify merge_cells_result to delegate scalar cases
- `crates/rbqn-prim/src/structural.rs` - Add b_to_prim, enclose_scalar, strides_from_shape helpers; apply to identity/ltack/rtack, enclose, take/drop multi-axis, join, rotate
- `crates/rbqn-prim/src/arith_dyad.rs` - Add pervasive_fill helper; apply to 3 pervasive_boxed_* functions

## Decisions Made

**Thin dispatch wrappers removed:**
The 4 functions `dispatch_native_md1_c1/c2` and `dispatch_native_md2_c1/c2` in derive.rs were one-line delegates to `crate::modifiers::*`. Removing them and calling the modifiers crate directly at the 4 call sites reduces noise without any semantic change.

**typed_arr_from_b_vec for results_to_arr_fill:**
`rbqn-core` already provides `typed_arr_from_b_vec` which handles f64/c32/boxed dispatch identically to the old code. The only subtlety is that `typed_arr_from_b_vec` sets default fills (`0.0`/space) for non-empty arrays — we override `out.fill = fill` afterward to preserve the caller's explicit `None` semantics.

**merge_cells_result delegation:**
The scalar and empty cases in `merge_cells_result` were identical to `results_to_arr` — 20 duplicate lines removed by calling `results_to_arr` for those branches.

## Deviations from Plan

None — plan executed exactly as written. Both tasks completed without incident.

## Issues Encountered

**Strides type error:** After extracting `strides_from_shape`, the call `strides_from_shape(outer_shape)` failed because `outer_shape` is a `Vec<usize>` not `&[usize]`. Fixed with `&outer_shape`. Also, the `let rank =` variable in `rotate_along_axis` was previously needed for the stride loop bounds but became unused after the extraction — removed it to keep zero warnings.

## Next Phase Readiness

- Phase 7 complete — all 3 plans done
- Clean zero-warning workspace maintained through all simplification work
- Code is structurally cleaner with shared helper conventions established

---
*Phase: 07-code-simplification-cbqn-parity*
*Completed: 2026-03-02*

## Self-Check: PASSED

- Files verified: derive.rs, modifiers.rs, structural.rs, arith_dyad.rs — all exist
- Commits verified: aff7fd8 (Task 1), f2f9e8c (Task 2) — both in git history
