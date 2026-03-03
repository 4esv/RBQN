---
phase: "01"
plan: "02h"
subsystem: "rbqn-prim, rbqn-core"
tags: [pervasive, broadcasting, shape-agreement, arithmetic, comparison]
key-files:
  modified:
    - crates/rbqn-prim/src/arith_dyad.rs
    - crates/rbqn-prim/src/compare.rs
    - crates/rbqn-core/src/array.rs
    - crates/rbqn-prim/src/structural.rs
decisions:
  - "BQN uses leading-axis prefix agreement, not NumPy trailing-axis broadcasting"
  - "Empty Boxed arrays treated as numeric for pervasive dispatch"
  - "couple_c2 atom-array cases now error per BQN spec instead of producing malformed arrays"
metrics:
  completed: "2026-02-21"
---

# Phase 01 Plan 02h: Prefix Agreement Broadcasting Summary

Implemented BQN leading-axis prefix agreement for all pervasive arithmetic and comparison operations, plus fixed empty array handling and couple shape validation.

## Changes Made

### 1. Prefix Agreement in Numeric Arithmetic (`arith_dyad.rs`)

Added `is_shape_prefix` helper. Updated `pervasive_dyad` to handle array-array cases where one shape is a prefix of the other. For example, `arr[2] + arr[2,7]` broadcasts the [2] array across rows of the [2,7] array using `i % w_ia` indexing.

This covers: `+`, `-`, `*`, `/`, `^`, `sqrt`, `floor`, `ceil`, `|`, `not`, `and`, `or`, `log`.

### 2. Prefix Agreement in Char Arithmetic (`arith_dyad.rs`)

Updated `pervasive_char_add`, `pervasive_char_sub_num`, and `pervasive_char_sub_char` array-array cases with the same prefix agreement logic.

### 3. Prefix Agreement in Comparisons (`compare.rs`)

Added `is_shape_prefix` helper and updated `cmp_pervasive` with identical prefix broadcasting. Covers: `=`, `<`, `>`, `<=`, `>=`, `!=`.

### 4. Empty Array Handling (`array.rs`)

- `is_num_arr()`: Changed `!v.is_empty() &&` to `v.is_empty() ||` so empty Boxed arrays are classified as numeric (valid for pervasive dispatch).
- `f64_iter()`: Same change, so empty Boxed arrays return `Ok(vec![])` instead of erroring.

This fixes: `scalar + empty_array` now returns an empty array instead of a type error.

### 5. Couple Shape Validation (`structural.rs`)

Fixed `couple_c2` to reject atom-array and array-atom cases with a proper shape error per BQN spec. Previously produced malformed arrays with shape `[2, n]` but only `1+n` elements.

## Investigation Notes

The bootstrap `[7]+[2,7]` error during runtime1 init is NOT a prefix agreement issue (`[7]` is not a prefix of `[2,7]`). Root cause: a scalar 0 reaches `couple_c2` where an array was expected, producing `0 ≍ arr[7]` which is now properly rejected. The upstream cause (scalar 0 instead of array) is a separate VM issue to investigate.

## Commits

| Hash | Description |
|------|-------------|
| b7884b0 | fix(01-02h): implement prefix agreement broadcasting and fix couple shape checks |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed couple_c2 producing malformed arrays**
- **Found during:** Investigation of the [7]+[2,7] shape error
- **Issue:** `couple_c2` atom-array cases created arrays with shape [2,n] but only 1+n elements
- **Fix:** Reject atom-array cases with proper BqnError::Shape per BQN spec
- **Files modified:** crates/rbqn-prim/src/structural.rs
- **Commit:** b7884b0

## Self-Check: PASSED
