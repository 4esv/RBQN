---
phase: "01"
plan: "02s"
subsystem: "vm, primitives, decompose"
tags: [decompose, select, windows, choose, compiler]
dependency-graph:
  requires: [01-01]
  provides: [decompose-ordering, first-axis-select, windows-empty, choose-array-index]
  affects: [compiler-pipeline, runtime-correctness]
tech-stack:
  patterns: [CBQN-compatible-decompose, dual-path-choose, first-axis-indexing]
key-files:
  modified:
    - crates/rbqn-vm/src/derive.rs
    - crates/rbqn-vm/src/modifiers.rs
    - crates/rbqn-prim/src/select.rs
    - crates/rbqn-prim/src/structural.rs
decisions:
  - "Decompose Md1D/Md2D element order matches CBQN exactly"
  - "FunBlock decompose returns type 1 matching CBQN block_decompose"
  - "Choose modifier uses CBQN dual-path: scalar index (fast) vs array index (via pick)"
  - "Select uses first_dim for bounds, not total element count"
  - "Windows returns empty array (not error) when window > length"
metrics:
  duration: "128m"
  completed: "2026-02-22"
---

# Phase 01 Plan 02s: Fix Decompose, Select, Windows, Choose

Fix four bugs blocking compiler execution: Decompose element ordering, Select first-axis indexing, Windows empty-array handling, and Choose array-index support.

## One-liner

Decompose Md1D/Md2D element order fixed to match CBQN, Select now indexes along first axis, Windows returns empty for oversized windows, Choose handles non-scalar conditions.

## Changes Made

### 1. Decompose Element Ordering (ROOT CAUSE of `chooser` crash)

**File:** `crates/rbqn-vm/src/derive.rs` (dispatch_sys_decompose_c1)

**Bug:** Md1D returned `[4, modifier, operand]` but CBQN returns `[4, operand, modifier]`. Similarly Md2D returned `[5, modifier, f_operand, g_operand]` but CBQN returns `[5, f_operand, modifier, g_operand]`. The compiler's type-dispatch logic (via `=` comparison) failed because the expected element at position 1 was wrong, producing per-element `[0,0,0,0]` instead of a scalar match.

**Fix:** Swapped to CBQN order. Also changed FunBlock from type 0 to type 1 matching CBQN's `block_decompose`.

### 2. Choose (`chooser`) Array Index Support

**File:** `crates/rbqn-vm/src/modifiers.rs`

**Bug:** `choose_c1`/`choose_c2` called `to_usz()` on the condition result, which panicked on non-number values. CBQN's `cond_c1` has a dual-path: scalar number index (fast path) and non-number (uses `pick` for array indexing).

**Fix:** Added `pick_from` helper that calls dyadic `pick` (primitive 37) and `b_to_index` helper for scalar extraction. `choose_c1`/`choose_c2` now check `is_f64()` first for the fast path, falling through to pick for non-scalar conditions.

### 3. Select (`select`) First-Axis Indexing

**File:** `crates/rbqn-prim/src/select.rs`

**Bug:** `select_c2` used `arr.ia()` (total element count) for index bounds instead of `arr.shape[0]` (first dimension). For a `[1,0]` matrix (1 row, 0 columns, 0 total elements), `resolve_index(0, 0)` failed even though row 0 exists. Also, scalar `w` didn't return a cell for multi-dimensional arrays.

**Fix:** Compute `first_dim` from `arr.shape[0]`. Scalar path returns selected cell for rank > 1. Non-boxed numeric path also uses `first_dim` and properly handles cell extraction. Removed `.max(1)` from `cell_size` computation to correctly handle zero-column matrices.

### 4. Windows (`windows`) Empty Result

**File:** `crates/rbqn-prim/src/structural.rs`

**Bug:** `windows_c2` threw a domain error when window size > array length. CBQN returns an empty array with shape `[0, n, ...cell_shape]`.

**Fix:** When `n > first_dim`, return empty array with correct shape instead of erroring. Also fixed vector windows to return flat numeric arrays with shape `[num_windows, n]` instead of boxed sub-arrays, matching BQN semantics.

## Current State

- Runtime0 and Runtime1 execute correctly
- Compiler initializes (`compgen(glyphs)` succeeds)
- Compiler compilation of user input gets 500+ VM operations before hitting `0 pick empty_array`
- The remaining `0 pick empty` error is a DIFFERENT issue from what this plan fixed - the compiler encounters an empty internal table during processing

## Deferred Issues

The `0⊑⟨⟩` error at the end of compilation appears to be caused by the compiler's internal table construction producing an empty array where CBQN would have data. This likely stems from another subtle difference between our primitive implementations and CBQN's. Candidates include:
- Fill element handling in `take`/`drop` operations
- Character comparison with `>` (our impl returns a value where CBQN would error)
- Potential issues in `Group` or `Replicate` producing different-sized results

## Commits

| Hash | Description |
|------|-------------|
| d57089f | fix(01-02): fix Decompose ordering, Select first-axis, Windows empty, Choose array-index |

## Self-Check: PASSED
