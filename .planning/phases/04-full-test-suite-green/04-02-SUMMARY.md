---
phase: 04-full-test-suite-green
plan: 02
subsystem: primitives
tags: [prim-tests, table, slash, group, reshape, each, cells]
dependency_graph:
  requires: [04-01]
  provides: [table-scalar-args, group-multidim, reshape-modes, each-scalar-rank0]
  affects: [prim.bqn tests]
tech_stack:
  added: []
  patterns: [static-prim-registration, match-on-atomness, outer-product-group]
key_files:
  created: []
  modified:
    - crates/rbqn-vm/src/modifiers.rs
    - crates/rbqn-prim/src/slash.rs
    - crates/rbqn-prim/src/group.rs
    - crates/rbqn-prim/src/structural.rs
    - crates/rbqn/src/bootstrap.rs
decisions:
  - "table_c2 uses match on (w.is_atom(), x.is_atom()) to avoid rank-0 array creation that would break bootstrap"
  - "reshape modes: ∘=exact, ⌊=floor+cycle, ⌽=ceil+cycle, ↑=ceil+pad; identified via static prim B values"
  - "group_indices_multidim uses Cartesian outer-product per-dimension matching, not parallel indexing"
  - "each/cells on scalars now return rank-0 arrays (BQN semantics), not plain scalars"
  - "slash empty-w + atom-x returns <x (rank-0 enclosed), matches BQN spec"
metrics:
  duration: ~50min
  completed: 2026-02-24
  tasks_completed: 2
  files_modified: 5
---

# Phase 4 Plan 2: Fix prim.bqn Failure Clusters Summary

Reduced prim.bqn failures from 106 to 95 (11 fixed). Target was 40+ but deeper issues in pick, join, and group multi-dim prevented full target.

## Tasks Completed

### Task 1: Table scalar args + slash atom handling
- **table_c2**: handled scalar (atom) args using `match (w.is_atom(), x.is_atom())`. Scalar w with array x gives x.shape result. Both scalars give rank-0 result.
  - Previous approach used `scalar_as_unit()` creating rank-0 boxed arrays, which caused runtime1 bootstrap to fail when `⊑` received rank-0 w. Fixed by using direct match pattern.
- **slash replicate_c2**:
  - Atom `x` treated as rank-0: empty `w` returns `<x` (rank-0 enclosed), count-1 `w` returns rank-1 of n copies
  - Rank-0 enclosed `w` (like `<arr`) unwrapped to get inner array as counts

### Task 2: Group multi-dimensional + reshape modes + each/cells scalar behavior
- **group_indices_multidim**: New function for `⊔⟨l₁,...,lₙ⟩`. For each cell at `(g₁,...,gₙ)`:
  - Collect matching positions for each dimension: `matching_k = {i | lₖ[i]=gₖ}`
  - Cell shape = `(|matching₁|,...,|matchingₙ|)` (outer-product structure)
  - Cell element at `(r₁,...,rₙ)` = `⟨matching₁[r₁],...,matchingₙ[rₙ]⟩`
  - Scalar elements in outer list treated as length-1 arrays for Cartesian product
- **structural.rs reshape modes**: Added 4-mode system for computed dimensions:
  - Mode 0: ∘ (exact division, error if not divisible)
  - Mode 1: ⌊ (floor, cyclic)
  - Mode 2: ⌽ (ceiling, cyclic)
  - Mode 3: ↑ (ceiling, pad with fill)
  - Prim B values registered via `set_floor_prim`/`set_take_prim` at bootstrap
- **each_c1/c2 scalar**: `f¨ scalar` now returns rank-0 array (was returning plain scalar)
- **cells_c1 scalar**: `f˘ scalar` now returns rank-0 array
- **slash empty+atom**: `⟨⟩/x` where x is atom returns `<x`

## Deviations from Plan

**1. [Rule 1 - Bug] Bootstrap panic from table scalar_as_unit approach**
- **Found during**: Task 1 implementation
- **Issue**: Creating rank-0 boxed arrays via `scalar_as_unit` in table_c2 caused runtime1 bootstrap panic. The `⊑` (pick) receives rank-0 w from table result and panics.
- **Fix**: Rewrote table_c2 using match on (w.is_atom(), x.is_atom()) to avoid rank-0 intermediates
- **Files modified**: crates/rbqn-vm/src/modifiers.rs

**2. [Rule 3 - Blocking] reshape_computed mode detection required cross-crate prim registration**
- **Found during**: Task 2, reshape mode fix
- **Issue**: `reshape_computed` in `rbqn-prim` couldn't access `prim_to_b()` from `rbqn-vm` (crate boundary)
- **Fix**: Added static `FLOOR_PRIM_B`/`TAKE_PRIM_B` in structural.rs, set from bootstrap.rs after fruntime is built
- **Files modified**: crates/rbqn-prim/src/structural.rs, crates/rbqn/src/bootstrap.rs

**3. Fewer prim.bqn fixes than expected (11 vs 40+ planned)**
- Many "table failures" were actually failures in other operations within the same expression
- Pick (`⊑`) with boxed index lists is unimplemented (needed for many tests)
- Join (`∾`) with atom args errors (needed for ∾⌜ tests)
- Group multi-dim with ¯1 exclusion logic complex (partially fixed)

## Self-Check

### Files exist:
- [x] crates/rbqn-vm/src/modifiers.rs
- [x] crates/rbqn-prim/src/slash.rs
- [x] crates/rbqn-prim/src/group.rs
- [x] crates/rbqn-prim/src/structural.rs
- [x] crates/rbqn/src/bootstrap.rs

### Commits exist:
- 002fe9a: fix(04-02): table scalar args and slash atom/enclosed handling
- 06a9f3f: fix(04-02): group multidim, reshape modes, each/cells on scalars

### Test results:
- prim.bqn: 95 failures (down from 106, net -11)
- simple/literal/syntax/bytecode/token: 294/294 passed (no regressions)
- namespace: 50/50 passed (no regressions)

## Self-Check: PASSED
