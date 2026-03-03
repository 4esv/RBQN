---
phase: "01"
plan: "02l"
subsystem: "inverse-system"
tags: [bootstrap, inverse, undo-modifier, compiler]
dependency-graph:
  requires: [runtime1-execution, setPrims, setInv]
  provides: [working-inverse-system, compiler-undo-support]
  affects: [compiler-init, bootstrap-pipeline]
tech-stack:
  patterns: [native-primitive-runtime, glyph-based-inverse-lookup]
key-files:
  modified:
    - crates/rbqn/src/bootstrap.rs
    - crates/rbqn-vm/src/derive.rs
    - crates/rbqn-vm/src/modifiers.rs
    - crates/rbqn-vm/src/compiler.rs
    - crates/rbqn-vm/src/vm.rs
    - crates/rbqn/build.rs
    - crates/rbqn-core/src/value.rs
    - crates/rbqn-prim/src/search.rs
    - crates/rbqn-prim/src/slash.rs
    - crates/rbqn-prim/src/sysfn.rs
decisions:
  - Use native fruntime primitives for runtime array instead of BQN wrappers from runtime1
metrics:
  completed: "2026-02-22"
---

# Phase 01 Plan 02l: Inverse System (undo modifier) Summary

Use native fruntime for runtime array so PrimInd identifies primitives correctly for glyph-based inverse lookup, enabling the compiler's /⁼ usage.

## What Was Done

The compiler BQN source uses `/⁼` (inverse of indices/replicate) during initialization. The BQN runtime1's inverse system uses `PrimInd` + `Glyph` to identify primitives and look up their inverses from glyph-indexed tables. Previously, the runtime array contained BQN-defined wrapper functions (e.g., `Indices ⊘ Replicate` for `/`) which had no primitive index, causing `PrimInd` to return 64 (non-primitive) and the inverse lookup to fail with "Inverse not found".

### Root Cause

CBQN (load.c line 522) uses native fruntime primitives when all builtins are natively implemented (`rtComplete[]` all true). RBQN was instead extracting the BQN-defined wrappers from runtime1's output, which are composed functions without primitive indices. This broke the inverse system's glyph-based lookup mechanism.

### Fix

1. **bootstrap.rs**: Replaced runtime1 output extraction with `fruntime.clone()`, matching CBQN's behavior. Also properly calls the `setInv` callback with `bi_setInvSwap` and `bi_setInvReg` system functions.

2. **derive.rs**: Added complete inverse infrastructure:
   - `LazyInvReg` / `LazyInvSwap` derived kinds for deferred inverse resolution
   - `inv_reg()` / `inv_swap()` functions that call into BQN runtime's inverse resolver
   - `native_inverse_reg()` / `native_inverse_swap()` tables for common primitive inverses
   - System function dispatch for `setInvReg` (8), `setInvSwap` (9), `nativeInvReg` (10), `nativeInvSwap` (11)
   - `Decompose` (1), `Glyph` (4), `PrimInd` (5) system functions

3. **modifiers.rs**: The `⁼` modifier (MD1_UNDO, index 49) now dispatches to `inv_reg` instead of throwing "not yet implemented".

## Verification

- "Inverse not found" no longer appears during compiler initialization
- `/⁼` correctly resolves: `NativeFn { prim_idx: 33 }` is identified and its inverse is found
- Bootstrap completes: runtime1 succeeds, setInv is called, compiler init returns a function

## Remaining Issues

The compiler still fails on other errors (shape mismatch in `+`, type errors) unrelated to the inverse system. These are separate issues in the compiler pipeline.

## Commits

| Commit | Description |
|--------|-------------|
| 51fe075 | fix(01-02l): implement inverse system for compiler bootstrap |

## Deviations from Plan

None - the fix required understanding CBQN's runtime array construction strategy and matching it in RBQN.

## Self-Check: PASSED
