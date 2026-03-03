---
phase: "01"
plan: "02p"
subsystem: "rbqn-prim arithmetic"
tags: [pervasive, dyad, boxed, under, depth]
key-files:
  modified:
    - crates/rbqn-prim/src/arith_dyad.rs
decisions:
  - "Recurse element-wise through Boxed arrays in pervasive_dyad rather than requiring all-numeric f64_iter"
  - "Route 'other' classified arrays in add_c2/sub_c2 through pervasive_dyad instead of erroring"
metrics:
  completed: "2026-02-22"
---

# Phase 01 Plan 02p: Dyadic Pervasive Recursion for Boxed Arrays

Depth>1 pervasive arithmetic via element-wise Boxed array recursion in pervasive_dyad.

## Problem

During compiler compilation, Under (F⌾G) with structural inverses (e.g., `mask⊸/`) produces arrays containing mixed types -- some elements are numbers, others are sub-arrays or function values. When downstream arithmetic (`+`, `-`, etc.) receives these Boxed arrays, `f64_iter()` fails because not all elements are f64 scalars. The `char_num_class` function classifies such arrays as `'o'` (other), and `add_c2` / `sub_c2` immediately error with "Unexpected argument types".

BQN arithmetic is pervasive: it should recurse into nested array structure element-by-element until reaching numeric scalars.

## Solution

1. Added three recursive helpers to `arith_dyad.rs`:
   - `pervasive_boxed_scalar_arr`: scalar op Boxed array, recurse per element
   - `pervasive_boxed_arr_scalar`: Boxed array op scalar, recurse per element
   - `pervasive_boxed_arr_arr`: Boxed array op Boxed array, recurse element pairs

2. Modified `pervasive_dyad` to fall back to these helpers when `f64_iter()` fails (instead of propagating the error). The fast numeric path is tried first; Boxed recursion is the fallback.

3. Updated `add_c2` and `sub_c2` to route `'o'` (Boxed/other) classified arrays through `pervasive_dyad` instead of returning a type error. Other dyadic ops (`mul_c2`, `div_c2`, etc.) already call `pervasive_dyad` unconditionally and benefit automatically.

## Verification

- `CBQN_PATH=... cargo run -- -e '1+1'` no longer crashes with "Unexpected argument types"
- Bootstrap pipeline now advances past the compiler's Under-based operations
- Next failure is in Group (⊔) -- a separate pre-existing issue unrelated to pervasion

## Commits

| Hash | Description |
|------|-------------|
| 306b74a | fix(01-02p): add depth>1 pervasive recursion for dyadic arithmetic |

## Deviations from Plan

None -- plan executed as written.
