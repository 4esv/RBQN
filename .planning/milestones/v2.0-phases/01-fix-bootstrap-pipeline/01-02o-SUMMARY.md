---
phase: "01"
plan: "02o"
subsystem: "primitives"
tags: [pervasive, monadic, nested-arrays, boxed]
key-files:
  modified:
    - crates/rbqn-prim/src/arith_monad.rs
decisions:
  - "Added pervasive_monad_b recursive helper that works on B values directly (using get_arr/tag_arr) to recurse into nested Boxed arrays"
  - "add_c1 also gets Boxed recursion since + monad must assert numeric at all depths"
metrics:
  completed: "2026-02-22"
  tasks: 1
  files: 1
---

# Phase 01 Plan 02o: Pervasive Monadic Recursion for Nested Arrays

Fix pervasive monadic functions to recurse into Boxed (nested) arrays instead of erroring with "contained non-number".

## One-liner

Pervasive monadic arithmetic now recurses into nested Boxed arrays via pervasive_monad_b helper.

## Changes Made

### Task 1: Add recursive Boxed handling to pervasive_monad

**Commit:** `ffb0844`

Added `pervasive_monad_b` -- a recursive helper that operates on raw `B` values:
- If `x` is f64 scalar: apply `scalar_fn` directly
- If `x` is a numeric array: apply element-wise, return squeezed numeric array
- If `x` is a Boxed array: recurse into each element, return Boxed result preserving shape

Updated `pervasive_monad` to call `pervasive_monad_b` when the array is Boxed (non-numeric el_type). This fixes all 10 monadic pervasive functions: `+`, `-`, `x`, `div`, `pow`, `sqrt`, `floor`, `ceil`, `|`, `not`.

Updated `add_c1` separately since it has a custom implementation (identity with numeric assertion) -- it now also recurses into Boxed arrays.

## Verification

- Build succeeds with no new warnings
- Runtime0 integration tests pass (4/4)
- Bootstrap progresses past the compiler compilation stage (previously crashed with "Type error: -x: x contained non-number")

## Deviations from Plan

None - plan executed exactly as written.
