---
phase: "01"
plan: "02f"
subsystem: "rbqn-prim/structural"
tags: [reshape, computed-dimensions, compiler-init]
key-files:
  modified:
    - crates/rbqn-prim/src/structural.rs
decisions:
  - "Treat MD2 values in shape as exact division (∘), FUN values as floor (⌊)"
  - "Default to exact division for unrecognized non-numeric shape elements"
metrics:
  completed: "2026-02-21"
---

# Phase 01 Plan 02f: Fix reshape computed dimensions

Implement BQN-spec computed reshape dimensions (∘⌊⌈ in shape array) to unblock compiler init.

## Problem

During compiler initialization (`c1(compgen, glyphs_b)`), the BQN compiler source uses `⥊` (reshape) with a shape like `⟨11, ∘⟩` where `∘` (atop, prim index 53) is a 2-modifier value meaning "compute this dimension". The native `reshape_c2` only handled numeric shape arrays via `i32_iter()`, which threw "Type error: Expected numeric array" when encountering the MD2 value.

## Root Cause

BQN spec allows reshape shape arrays to contain special marker values:
- `∘` (atop, MD2) = exact division (total / product_of_other_dims, error if remainder)
- `⌊` (floor, FUN) = floor division
- `⌈` (ceil, FUN) = ceil division

The implementation only supported flat numeric shapes.

## Fix

Added `reshape_computed()` function that:
1. Scans shape array elements, collecting numeric dimensions and detecting non-numeric markers
2. Identifies the compute mode from the B value's type tag (MD2 = exact, FUN = floor/ceil)
3. Computes the missing dimension from total element count / product of known dimensions
4. Performs reshape with the fully-resolved shape

Also refactored `structural.rs` to use `typed_arr()` helper throughout, producing properly typed numeric/char arrays instead of always Boxed.

## Verification

- `CBQN_PATH=... cargo run -- -e '1+1'` no longer crashes on "Expected numeric array" at `⥊`
- All 29 existing tests pass
- Compiler init proceeds past reshape to a different error (destructuring length mismatch)

## Commits

| Hash | Description |
|------|-------------|
| 91f16aa | fix(01-02f): implement computed reshape dimensions |

## Deviations from Plan

None - plan executed exactly as written.
