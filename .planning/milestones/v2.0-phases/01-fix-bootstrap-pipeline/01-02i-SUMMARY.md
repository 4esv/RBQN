---
phase: "01"
plan: "02i"
subsystem: primitives
tags: [bugfix, couple, shift, runtime1]
key-files:
  modified:
    - crates/rbqn-prim/src/structural.rs
    - crates/rbqn-prim/src/dispatch.rs
decisions:
  - "Broadcast scalar to fill array shape for couple (matching BQN spec)"
  - "Added monadic forms for both shift-before and shift-after"
metrics:
  duration: "101s"
  completed: "2026-02-22"
---

# Phase 01 Plan 02i: Fix Couple Broadcast + Monadic Shift Summary

Fix two independent primitive bugs blocking compiler initialization: couple (scalar+array) shape mismatch and missing monadic shift-before/after.

## Completed Tasks

| Task | Description | Commit | Key Changes |
|------|------------|--------|-------------|
| 1 | Fix couple + monadic shifts | 271e7fb | couple_c2 broadcast, shifta_c1, shiftb_c1, dispatch registration |

## What Changed

### Bug 1: couple_c2 scalar+array broadcast
Previously, `scalar ≍ array` threw a shape mismatch error. In BQN, the scalar should be broadcast (replicated) to fill the array's shape, producing a `[2]++≢array` result. For example, `0 ≍ ⟨1,2,3,4,5,6,7⟩` produces a 2x7 array where the first row is all zeros.

### Bug 2: Monadic shift-before (`) and shift-after (`)
Both `»` and `«` were registered with `c1: None`, causing "primitive has no monadic form" errors. Added:
- `shiftb_c1`: shifts array right by one, fills left with type fill (0 for numbers)
- `shifta_c1`: shifts array left by one, fills right with type fill
- Both handle multi-rank arrays (shift along first axis)

### Runtime Progress
After these fixes, runtime1 initialization progresses further into the compiler bootstrap. The next failure is a shape mismatch in arithmetic (`[7] vs [2,7]`), which is a separate downstream issue.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing functionality] Added monadic shift-after (`«`)**
- **Found during:** Task 1
- **Issue:** `«` also had `c1: None`, same bug as `»`
- **Fix:** Added `shifta_c1` function and registered in dispatch table
- **Files modified:** structural.rs, dispatch.rs
- **Commit:** 271e7fb

## Verification

- `cargo build` compiles cleanly (no errors)
- All 13 runtime0 integration tests pass
- Runtime1 initialization progresses past the couple/shift errors

## Self-Check: PASSED
