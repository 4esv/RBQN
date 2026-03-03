---
phase: 01-fix-bootstrap-pipeline
plan: 01
subsystem: vm
tags: [rust, bqn, vm, derive, modifiers, fork, each, fold]

requires: []
provides:
  - "Correct fork monadic dispatch: (f g h) x = (f x) g (h x)"
  - "Typed numeric array output from each/scan/cells/table modifiers"
  - "results_to_arr helper in modifiers.rs for typed output"
  - "13 integration tests in crates/rbqn-vm/tests/runtime0.rs"
affects:
  - "01-02 and beyond (runtime1/compiler unblock)"

tech-stack:
  added: []
  patterns:
    - "results_to_arr: detect homogeneous result types and produce typed arrays"
    - "Integration tests in crates/rbqn-vm/tests/ using prim_to_b + m1_d to build derived objects"

key-files:
  created:
    - "crates/rbqn-vm/tests/runtime0.rs"
  modified:
    - "crates/rbqn-vm/src/derive.rs"
    - "crates/rbqn-vm/src/modifiers.rs"

key-decisions:
  - "Integration tests placed in crates/rbqn-vm/tests/ not workspace root (binary crate has no lib.rs)"
  - "results_to_arr handles numeric and character homogeneous arrays; falls back to Boxed for mixed types"
  - "squeeze_num applied to numeric results so integers get I8/I16/I32 representation"

patterns-established:
  - "Fork c1 pattern: hx = c1(h,x); fx = c1(f,x); c2(g, fx, hx)"
  - "Modifier output: always use results_to_arr instead of BqnArr::new_vec_b + tag_arr"

requirements-completed: [R-BOOT-2]

duration: 5min
completed: 2026-02-21
---

# Phase 01 Plan 01: Fix Fork Dispatch and Typed Modifier Output Summary

**Two-bug fix: corrected (f g h) x fork semantics and added results_to_arr helper so each/scan/cells return typed numeric arrays instead of Boxed**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-21T20:10:33Z
- **Completed:** 2026-02-21T20:14:58Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Fork monadic dispatch now computes `(f x) g (h x)` — `+´` returns 6 for ⟨1,2,3⟩
- `results_to_arr` helper produces typed numeric or character arrays from modifier output
- Applied to `each_c1`, `each_c2`, `cells_c1`, `cells_c2`, `scan_c1`, `scan_c2`, `table_c2`
- 13 integration tests all pass, covering fork, fold, each, scan, and basic primitive regression

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix fork monadic dispatch and typed modifier output** - `7ef661d` (fix)
2. **Task 2: Create runtime0 integration tests** - `4b5386e` (test)

## Files Created/Modified
- `crates/rbqn-vm/src/derive.rs` - Fork c1 arm: replaced broken 3-step with correct (f x) g (h x)
- `crates/rbqn-vm/src/modifiers.rs` - Added results_to_arr helper, applied to all modifier output sites
- `crates/rbqn-vm/tests/runtime0.rs` - 13 integration tests for fork, fold+, each+, scan, primitives

## Decisions Made
- Integration tests placed in `crates/rbqn-vm/tests/` rather than workspace `tests/` because `rbqn` is a binary crate with no lib target — rbqn-vm already has a lib and has the needed API
- `results_to_arr` applies `squeeze_num` to numeric output so integers compress to I8/I16/I32 as appropriate
- Character result detection uses `b.is_c32()` for homogeneous char arrays

## Deviations from Plan

### Minor Deviation: Test location

**[Rule 3 - Blocking] Tests placed in crates/rbqn-vm/tests/ not workspace root**
- **Found during:** Task 2 (Creating runtime0 integration tests)
- **Issue:** Plan specified `tests/runtime0.rs` at workspace root, but workspace-level tests have no crate association — the `rbqn` crate is a binary with no lib.rs
- **Fix:** Created tests in `crates/rbqn-vm/tests/runtime0.rs` which is the natural location for rbqn-vm integration tests. Tests access all needed VM primitives directly.
- **Files modified:** `crates/rbqn-vm/tests/runtime0.rs`
- **Verification:** `cargo test --test runtime0 -p rbqn-vm -- --test-threads=1` passes 13/13

---

**Total deviations:** 1 minor (test location adjusted for Cargo constraints)
**Impact on plan:** Tests are equivalent in coverage and run with the same command pattern. No scope creep.

## Issues Encountered
None significant. The two bugs were straightforward code fixes once the root causes were identified.

## Next Phase Readiness
- Runtime0 fork dispatch and modifier typing are now correct
- `+´` returns 6 for ⟨1,2,3⟩; `+¨` returns typed numeric array
- Ready to proceed with runtime1/compiler loading verification (plan 01-02)
- The `results_to_arr` pattern should be evaluated for table_c1 (currently redirects to each_c1) if issues arise

## Self-Check: PASSED

All files verified present and commits verified in git history.

---
*Phase: 01-fix-bootstrap-pipeline*
*Completed: 2026-02-21*
