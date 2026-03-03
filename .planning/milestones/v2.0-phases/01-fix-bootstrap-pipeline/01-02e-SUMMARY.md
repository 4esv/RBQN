---
phase: "01"
plan: "02e"
subsystem: "bootstrap"
tags: [vm, bootstrap, setPrims, PrimInd, destructuring]
dependency-graph:
  requires: [01-02d]
  provides: [setPrims-fix, primind-sysfn, i32-boxed-coercion]
  affects: [compiler-init, formatter-init]
tech-stack:
  patterns: [CBQN-load.c-parity, sys-function-dispatch]
key-files:
  modified:
    - crates/rbqn/src/bootstrap.rs
    - crates/rbqn-vm/src/derive.rs
    - crates/rbqn-vm/src/vm.rs
    - crates/rbqn-vm/src/scope.rs
    - crates/rbqn-core/src/array.rs
decisions:
  - "Use sys index 5 for PrimInd to avoid collisions with existing indices"
  - "Skip setInv entirely rather than calling it wrong (CBQN calls dyadically)"
metrics:
  completed: "2025-02-21"
---

# Phase 01 Plan 02e: Fix Destructuring Length Mismatch Summary

Fix setPrims callback args to match CBQN, implement PrimInd sys function, harden Boxed array coercion in i32_iter.

## Changes Made

### 1. Fixed setPrims callback arguments (bootstrap.rs)

**Root cause**: `setPrims` was being called with `c1(setPrims, glyphs_array)` where `glyphs_array` had 3 elements (fn/md1/md2 glyph char arrays). The runtime1 setPrims callback expected `c1(setPrims, <<Decompose, PrimInd>>)` -- a 2-element array with system functions, matching CBQN's `c1G(setPrims, m_lvB_2(incG(bi_decp), incG(bi_primInd)))` in load.c line 494.

This caused a destructuring mismatch: 2 targets vs 3 values.

**Fix**: Pass `<<m_sys_fn(1), m_sys_fn(5)>>` (Decompose and PrimInd) instead of glyphs.

### 2. Implemented PrimInd system function (derive.rs, vm.rs)

Added `dispatch_sys_primind_c1` as sys index 5. Returns primitive index (0..63) for native functions/modifiers, or 64 for non-primitives. Matches CBQN's `primInd_c1` in sysfn.c.

### 3. Fixed setInv callback (bootstrap.rs)

`setInv` was being called monadically with an empty array. CBQN calls it dyadically: `c2(setInv, bi_setInvSwap, bi_setInvReg)`. Since we don't have inverse operations, we skip the call entirely rather than calling it with the wrong arity.

### 4. Added Boxed f64 handling in i32_iter (array.rs)

The VM sometimes produces Boxed arrays containing f64 scalar values (e.g., from LSTO/LSTM opcodes). The `i32_iter` method used by reshape and other operations would fail on these with "Expected numeric array". Added a handler for `ArrData::Boxed(v)` where all elements are f64, converting them to i32.

### 5. Improved destructuring mismatch error message (scope.rs)

Changed from generic "destructuring length mismatch" to include counts: "destructuring length mismatch (N targets vs M values)".

## Remaining Issues

- **Compiler init still fails**: After fixing the destructuring mismatch, the compiler initialization now fails with a different error: "Expected numeric array" from reshape. The shape array contains a 2-modifier value as one element, indicating deeper runtime issues. This is a separate problem from the destructuring mismatch.
- **Formatter init fails**: The formatter experiences a `6 targets vs 3 values` destructuring mismatch internally, likely caused by broken runtime function outputs. This is caught by catch_unwind and doesn't block execution.

## Deviations from Plan

None -- plan executed as written.

## Commits

| Hash | Description |
|------|-------------|
| 5b01b5c | fix(01-02e): fix destructuring mismatch in setPrims and harden type coercion |

## Self-Check: PASSED

- SUMMARY file exists: YES
- Commit 5b01b5c exists: YES
- All modified files present in commit: YES
- Tests pass: YES (13/13 runtime0 tests)
