---
phase: "01"
plan: "02n"
subsystem: inverse-system
tags: [inverse, compiler, under, vm]
key-files:
  modified:
    - crates/rbqn-vm/src/derive.rs
decisions:
  - "Use ≍ (solo, prim 24) as native inverse of ⊑ (pick, prim 37)"
  - "Implement before_im/after_im patterns natively in md2d_inverse_reg"
  - "Add native fast-path to inv_swap for known swap inverses"
metrics:
  completed: "2026-02-22"
---

# Phase 01 Plan 02n: Native Inverse Support Summary

Three inverse-related failures during compiler compilation of `1+1` resolved by adding native inverse handling to derive.rs.

## One-liner

Native ⊑⁼, (val⊸G)⁼, and (F⟜val)⁼ inverse resolution matching CBQN before_im/after_im

## What Changed

### 1. Added ⊑⁼ to native_inverse_reg (prim 37 -> 24)

Pick inverse (`⊑⁼`) now resolves to solo (`≍`). The BQN runtime resolver had no table entry for `⊑`, returning an error-throwing Fork function. Adding the native mapping bypasses the resolver entirely.

### 2. Added md2d_inverse_reg() for compound modifier inverses

New function handles Md2D (2-modifier derived) inverse resolution, matching CBQN's `before_im` and `after_im` from `md2.c`:

- **⊸ (before, prim 55)**: `(val⊸G)⁻¹ = val⊸(inv_reg(G))` — when left operand is a value and right operand is a function, compose the value with the inverse of G using ⊸.
- **⟜ (after, prim 56)**: `(F⟜val)⁻¹ = (inv_swap(F))⟜val` — when right operand is a value and left operand is a function, compose the swap-inverse of F with the value using ⟜.

### 3. Added native fast-path to inv_swap()

`inv_swap` now checks `native_inverse_swap` before falling through to the BQN resolver, matching the pattern already used in `inv_reg`.

## Impact

- Eliminated all 25 "Inverse not found" panic messages during compiler initialization
- Reduced total panics from 25+ to 3 (2 caught "Inverse failed" + 1 unrelated ∾ rank error)
- The remaining ∾ rank error is a separate issue in the join primitive with multi-dimensional arrays

## Commits

| Commit | Description |
|--------|-------------|
| f633783 | fix(01-02): add native inverse support for pick, before, and after |

## Deviations from Plan

None - plan executed as written.

## Self-Check

Verified:
- `⊑⁼` resolves to `≍` (confirmed via trace: `c1 prim=≍` appears in output)
- `(arr⊸/)⁼` resolves natively via md2d_inverse_reg (zero "Inverse not found" messages)
- Md2D inverse failures handled gracefully (caught by catch_unwind in under_c1)
- All previously passing tests still pass (4/8, same as before)
