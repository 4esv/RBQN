---
phase: "01"
plan: "02m"
subsystem: "compiler-init"
tags: [compiler, under, group, mutex, array, inverse]
key-files:
  modified:
    - crates/rbqn-core/src/array.rs
    - crates/rbqn-prim/src/group.rs
    - crates/rbqn-vm/src/derive.rs
    - crates/rbqn-vm/src/modifiers.rs
    - crates/rbqn-vm/src/namespace.rs
    - crates/rbqn-vm/src/scope.rs
    - crates/rbqn-vm/src/vm.rs
decisions:
  - "Implemented structural Under with pattern detection rather than full inverse system"
  - "Fixed mutex poisoning globally rather than wrapping individual catch_unwind sites"
metrics:
  completed: "2026-02-22"
---

# Phase 01 Plan 02m: Fix compiler-stage errors for `1+1` compilation

Fix 7 distinct errors blocking `CBQN_PATH=... cargo run -- -e '1+1'` after compgen init succeeds.

## Changes Made

### 1. i32_iter empty Boxed arrays (array.rs)
Changed guard from `!v.is_empty() && v.iter().all(|b| b.is_f64())` to `v.is_empty() || v.iter().all(...)`. Empty Boxed arrays are valid numeric arrays (consistent with `is_num_arr()` and `f64_iter()` which already handled this case).

### 2. Group dyad (group.rs)
- `group_c2`: Added support for `≠w = 1+≠x` (BQN spec — last element of w specifies minimum result length). Previously only accepted `≠w = ≠x`.
- `group_c2`: Returns properly `tag_arr`-ed sub-arrays instead of `B::SENTINEL`.
- `group_indices_c1`: Returns `tag_arr(BqnArr::new_vec_i32(g))` instead of `B::SENTINEL`.

### 3. Under modifier (modifiers.rs)
Implemented `⌾` (Under) — both monadic `F⌾G x` and dyadic `w F⌾G x`:
- **Computational under**: Tries `G⁻¹(F(Gx))` via `inv_reg(g)`. Wrapped in `catch_unwind` to handle inverse-not-found gracefully.
- **Structural under (mask⊸/)**: Detects `Md2D` with Before(⊸) modifier and Replicate(/) right operand. Puts `F(Gx)` values back at mask=1 positions.
- **Structural under (idx⊸⊏)**: Detects Before(⊸) with Select(⊏). Puts values back at specified indices.
- **Generic structural under**: Creates identity index array `↕≠x`, applies G to find selected positions, overwrites those positions with `F(Gx)` values.
- **Empty selection optimization**: Returns x unchanged when G selects 0 elements (avoids shape mismatch in F).

### 4. Fold identity elements (modifiers.rs)
Added `fold_identity(f)` returning BQN-standard identities for empty-array fold:
- `+`/`-` -> 0, `×`/`÷`/`⋆` -> 1, `⌊` -> inf, `⌈` -> -inf, `∧` -> 1, `∨` -> 0, `∾` -> empty array

### 5. System function 100 (derive.rs)
Added c1 handler for sys_fn 100 (•BQN placeholder) returning SENTINEL. Previously only c2 was handled.

### 6. Mutex poisoning (derive.rs, scope.rs, namespace.rs, vm.rs)
Changed all `Mutex::lock().unwrap()` to `.lock().unwrap_or_else(|e| e.into_inner())` across the VM crate. This prevents poisoned mutex panics after `catch_unwind` recovers from errors in `⎊` (Catch) and `⌾` (Under).

### 7. Debug cleanup (derive.rs, modifiers.rs)
Removed extensive debug `eprintln!` statements from `inv_reg`, `set_inv_reg_fn`, `set_inv_swap_fn`, `dispatch_sys_c1` (setInvReg/setInvSwap), and undo modifier.

## Current Status

**Before**: `cargo run -- -e '1+1'` panicked with "Type error: Expected numeric array" immediately on compiler invocation.

**After**: Compiler processes through most of the compilation pipeline (tokenizing, parsing, code generation). Fails at a `+` dyad on a Boxed array containing sub-arrays — the structural Under fallback produces nested arrays where the BQN inverse system would produce flat numeric arrays.

**Root cause of remaining failure**: The BQN-level inverse function (set via `setInvReg`) cannot find inverses for compound functions like `mask⊸/`. CBQN's runtime builds inverse tables that handle this, but our runtime1's inverse tables don't cover these patterns. When computational Under fails, the structural Under fallback produces structurally correct but type-incompatible results.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] Mutex poisoning recovery**
- Found during: Under implementation (catch_unwind poisoned DERIVED_STORE mutex)
- Issue: All subsequent derived object lookups would panic after any caught error
- Fix: Global replacement of `.lock().unwrap()` with `.unwrap_or_else(|e| e.into_inner())`
- Files modified: derive.rs, scope.rs, namespace.rs, vm.rs

**2. [Rule 1 - Bug] Group SENTINEL values**
- Found during: compiler execution after group_c2 fix
- Issue: group_indices_c1 still returned SENTINEL instead of real arrays
- Fix: Changed to return `tag_arr(BqnArr::new_vec_i32(g))`
- Files modified: group.rs

**3. [Rule 2 - Missing functionality] Fold identity elements**
- Found during: compiler execution produced empty arrays that needed folding
- Issue: `´` on empty array panicked instead of returning identity
- Fix: Added identity lookup for common BQN functions
- Files modified: modifiers.rs

## Deferred Issues

1. **Structural Under type mismatch**: The structural Under fallback produces nested arrays where flat numeric arrays are expected. Fix requires either making the BQN-level inverse system handle compound functions, or implementing recursive pervasive arithmetic.

2. **•BQN system function**: sys_fn 100 returns SENTINEL instead of evaluating BQN code. Full implementation requires recursive compiler invocation.

## Commits

| Hash | Description |
|------|-------------|
| f572739 | fix(01-02): fix compiler init errors — Under, Group, empty arrays, mutex poisoning |

## Self-Check: PASSED
