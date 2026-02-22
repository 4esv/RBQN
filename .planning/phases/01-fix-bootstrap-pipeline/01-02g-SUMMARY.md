---
phase: "01"
plan: "02g"
subsystem: "vm/derive"
tags: [vm, modifiers, immediate-execution, destructuring]
dependency-graph:
  requires: [01-01]
  provides: [immediate-modifier-execution]
  affects: [compiler-init, formatter-init, runtime1-functions]
tech-stack:
  patterns: [CBQN-compatible-modifier-dispatch]
key-files:
  modified:
    - crates/rbqn-vm/src/derive.rs
    - crates/rbqn-vm/src/scope.rs
decisions:
  - "Match CBQN md1Bl_d/md2Bl_d: execute immediate modifiers at MD1C/MD2C time with [r,f] or [r,f,g] args"
metrics:
  duration: "~45min"
  completed: "2026-02-21"
---

# Phase 01 Plan 02g: Fix Immediate Modifier Execution Summary

Immediate 1-modifier and 2-modifier blocks now execute at MD1C/MD2C application time, matching CBQN's md1Bl_d/md2Bl_d behavior.

## Root Cause

The "destructuring length mismatch (6 targets vs N values)" error occurred because RBQN deferred execution of ALL modifier blocks to c1/c2 dispatch, even for immediate modifiers (imm=true). CBQN distinguishes:

- **Immediate modifiers** (imm=true): Execute block at MD1C/MD2C time with args `[m, f]` or `[m, f, g]` (positions: 0=self-ref, 1=operand, [2=right-operand])
- **Non-immediate modifiers** (imm=false): Create a Derived value, execute later at c1/c2 with args `[self, x, w, modifier, operand, ...]`

RBQN was always creating Derived:Md1D/Md2D regardless of immediacy. When runtime1 functions like `_multiAxis` (an immediate 1-modifier that destructures `f` into 6 parts) were invoked, the bytecode read `VARU 0 1` expecting the operand at position 1. But the deferred c1 dispatch placed the operand at position 4, causing the mismatch.

## Fix Applied

Modified `m1_d` and `m2_d` in `derive.rs` to check if the modifier is an immediate block. If so, execute the block immediately with CBQN-compatible argument layout:

- `m1_d(m, f)` for immediate blocks: `exec_block_with_args(&bl, body, psc, &[m, f])`
- `m2_d(m, f, g)` for immediate blocks: `exec_block_with_args(&bl, body, psc, &[m, f, g])`

## Verification

- The "destructuring length mismatch" error no longer fires during compiler or formatter initialization
- Bootstrap now proceeds past both compiler and formatter stages
- New error (shape mismatch in `+` operation) indicates progress to a different, later stage of execution

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed build error: missing reshape_computed function**
- **Found during:** Initial build
- **Issue:** `crates/rbqn-prim/src/structural.rs` referenced `reshape_computed` which didn't exist
- **Fix:** Replaced with `Err(BqnError::Nyi(...))` stub (user later provided full implementation)
- **Files modified:** `crates/rbqn-prim/src/structural.rs` (not committed, user modified externally)

## Commits

| Hash | Description |
|------|-------------|
| cc3f643 | fix(01-02g): execute immediate modifier blocks at MD1C/MD2C time |
