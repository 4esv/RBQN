# Phase 1: Fix Bootstrap Pipeline - Research

**Researched:** 2026-02-21
**Domain:** Rust VM debugging, BQN modifier/primitive dispatch, bootstrap sequencing
**Confidence:** HIGH — all findings are from direct codebase inspection

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **Debugging strategy:** Full pipeline trace — systematically trace runtime0 execution to find all issues, not just the 2 known failures. No instrumentation in source — debug externally using test assertions and comparisons, keep the codebase clean. Incremental commits — commit each fix individually as it's verified, not batched.
- **Verification depth:** Runtime1: smoke test a subset of key functions (arithmetic, array ops, modifiers) then move to compiler — compiler tests will catch remaining issues. Compiler: comprehensive testing — test all syntax forms including functions, trains, namespaces, headers, not just `1+1`. Test format: both Rust integration tests (#[test]) for internal correctness AND BQN script tests for end-to-end behavior. Failure policy: core functionality must pass to complete Phase 1, edge cases can be tracked and deferred to Phase 4 (Primitive Correctness).

### Claude's Discretion

- Whether to use CBQN as reference oracle or spec-based verification at each stage
- Formatter priority and approach — roadmap lists it but it may be blocked on deeper issues
- Error reporting style when bootstrap stages fail

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| R-BOOT-2 | Runtime0 executes and produces ~24 core runtime functions | Two confirmed bugs found in derive.rs (fork c1) and modifiers.rs (each returns Boxed arrays). Fix these and runtime0 will produce correct output. |
| R-BOOT-3 | Runtime1 executes using runtime0 output, produces 64-value runtime | Blocked entirely by R-BOOT-2. Once runtime0 is correct, runtime1 likely works — bootstrap.rs already calls it. Smoke-test needed. |
| R-BOOT-4 | SetPrims callback enables •Decompose and •Glyph | bootstrap.rs lines 373-375 skip set_prims. Must call it with current •Decompose (sys_idx=1) and •Glyph (sys_idx=4). |
| R-BOOT-5 | SetInv callback sets up inverse tables for ⁼ and ⌾ | bootstrap.rs lines 373-375 skip set_inv. Must call it to register inverse functions. ⁼ and ⌾ themselves are out of scope for Phase 1 but the callback must be invoked. |
| R-BOOT-6 | Compiler loads and can compile BQN source strings | bootstrap.rs already calls exec_stage for compiler and invokes it with glyphs. Blocked on runtime1 correctness. |
| R-BOOT-7 | Formatter loads and provides •Fmt and •Repr | bootstrap.rs has formatter support but the placeholder •Repr is wrong (uses sys_idx=1 = Decompose). Formatter is lower priority and may have other issues. |
</phase_requirements>

---

## Summary

Phase 1 fixes the bootstrap pipeline so `CBQN_PATH=/path/to/CBQN cargo run -- -e '1+1'` outputs `2`. The pipeline is: runtime0 → runtime1 → compiler → formatter. Runtime0 currently fails on 2/9 tests. Direct code inspection has identified the exact bugs causing those failures plus additional pre-existing bugs that will likely cause further failures once those are fixed.

The primary blockers are two bugs in the VM's modifier/primitive dispatch layer: (1) a fork monadic dispatch bug documented in `CONCERNS.md` that causes any monadic fork to return wrong results, and (2) the `each` modifier producing `ArrData::Boxed` output arrays which cannot be processed by the pervasive arithmetic primitives that expect typed numeric arrays. Both fixes are small, localized changes.

The bootstrap also skips two required callbacks (`setPrims`, `setInv`) that runtime1 returns and that must be invoked. These are currently dead code at `bootstrap.rs:373-375`. Calling them is necessary even though ⁼ and ⌾ are not implemented in Phase 1.

**Primary recommendation:** Fix the two confirmed bugs first (fork c1 and each output type), then establish a test harness to drive systematic debugging of any remaining runtime0 failures before proceeding to runtime1/compiler.

---

## Standard Stack

### Core

| Component | Version | Purpose | Why Standard |
|-----------|---------|---------|--------------|
| Rust `#[test]` / `#[cfg(test)]` | Built-in | Unit tests for primitive correctness | No external framework needed; workspace already uses `cargo test` |
| `cargo test -p rbqn -- --nocapture` | Built-in | Run tests with stdout for debugging | Needed because VM errors use `panic!` not `Result` |
| `std::panic::catch_unwind` | Built-in | Capture panics from the VM in tests | VM uses panic-based error reporting; tests must catch panics |
| CBQN reference oracle | External (existing) | Verify expected BQN outputs | Already required at build time via `CBQN_PATH` |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `cargo test -- --test-threads=1` | Built-in | Prevent test interference | Global stores (`DERIVED_STORE`, `ARR_STORE`) are singletons; parallel tests share state |
| `env_logger` / `eprintln!` | std | Tracing modifier dispatch | External trace only (no source instrumentation) — print to stderr in test harness |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `#[test]` in existing crate files | Integration tests in `tests/` at workspace root | Integration tests require `CBQN_PATH`; unit tests for prim/core can run standalone. Use both. |
| CBQN as oracle | BQN spec as oracle | CBQN is already available at build time; spec is slower to interpret. Use CBQN. |

---

## Architecture Patterns

### Test Infrastructure Pattern

The codebase has zero tests today. The pattern documented in `TESTING.md` is:

```rust
// Co-located unit test (no CBQN needed):
// File: crates/rbqn-vm/src/derive.rs
#[cfg(test)]
mod tests {
    use super::*;
    use rbqn_core::B;

    #[test]
    fn fork_c1_correct() {
        // (+ - ×) 3 should be (3+3) - (3×3) = 6 - 9 = ... wait, that's atop.
        // Monadic fork (f g h) x = (f x) g (h x)
        // (⊢ + ⊢) 3 = (⊢ 3) + (⊢ 3) = 3 + 3 = 6
        // Test via bootstrap + c1
    }
}

// Integration test (requires CBQN_PATH):
// File: tests/bootstrap.rs
#[test]
#[ignore = "requires CBQN_PATH"]
fn bootstrap_runtime0_complete() {
    let rt = rbqn::bootstrap::bootstrap().unwrap();
    assert_eq!(rt.runtime.len(), 64);
}
```

### Runtime0 Verification Pattern

The existing manual verification approach (per MEMORY.md) is:
1. Bootstrap → get `runtime_0` Vec<B>
2. Call each entry with known inputs
3. Compare output to CBQN reference

To automate this, wrap in `#[test]` and add assertions. The 9 tests mentioned in MEMORY.md are: ⊢, ⌊, <, ∾, ≍, ⊏, ↑, `+´`, `+¨`.

### Modifier Dispatch Architecture

The dispatch chain for `+´` (fold with add):
```
MD1C opcode → m1_d(fold_modifier, add_fn) → stores Md1D{f: add_fn, g: fold_modifier}
FN1C opcode → c1(derived_Md1D, array)
  → in c1: modifier.is_md1() → get_derived(mid) → Md1D
  → md.kind == NativeMd1{prim_idx: 50} → dispatch_native_md1_c1(50, add_fn, self, array)
  → modifiers::native_md1_c1(50, operand=add_fn, x=array)
  → fold_c1(f=add_fn, x=array)
  → for each elem: c2(add_fn, elem, acc) → works correctly
```

The dispatch chain for `+¨` (each with add):
```
MD1C opcode → m1_d(each_modifier, add_fn) → Md1D
FN1C/FN2C → c1/c2(derived_Md1D, ...)
  → native_md1_c1(47, operand=add_fn, x=array)
  → each_c1(f=add_fn, x=array)
  → results.push(c1(add_fn, elem))  ← each element is a scalar B
  → output: BqnArr::new_vec_b(results) ← ArrData::Boxed!
  → When further arithmetic is applied: f64_iter() on Boxed → Err(Type)
```

---

## Confirmed Bugs

### Bug 1: Fork Monadic Dispatch (CRITICAL)

**Location:** `crates/rbqn-vm/src/derive.rs:203-206`

**Current broken code:**
```rust
DerivedKind::Fork => {
    let hx = c1(d.h, x);
    let gx = c1(d.g, hx);
    c2(d.f, B::m_f64(0.0), gx) // TODO: proper fork c1
}
```

**Correct code:**
```rust
DerivedKind::Fork => {
    let hx = c1(d.h, x);
    let fx = c1(d.f, x);
    c2(d.g, fx, hx)
}
```

BQN: `(f g h) x` = `(f x) g (h x)`. The current code passes 0.0 as the left argument, which is wrong for any fork. This affects every train pattern in runtime0.

**Confidence:** HIGH — confirmed by reading derive.rs:203-206 and CONCERNS.md documentation of this exact bug.

### Bug 2: Each Modifier Returns Boxed Arrays (CRITICAL)

**Location:** `crates/rbqn-vm/src/modifiers.rs:172-185` (`each_c1`) and lines 187-228 (`each_c2`)

**Root cause:**
```rust
fn each_c1(f: B, x: B) -> B {
    // ...
    let mut results = Vec::with_capacity(n);
    for i in 0..n {
        results.push(c1(f, get_elem(&arr, i)));  // elements are B values
    }
    let mut out = BqnArr::new_vec_b(results);  // ArrData::Boxed — WRONG for numeric results
    out.shape = arr.shape.clone();
    crate::vm::tag_arr(out)
}
```

When `+¨ ⟨1,2,3⟩` applies `+` monadically to each element, each result is `B::m_f64(...)` (scalars). These are stored in `ArrData::Boxed`. Then any subsequent numeric operation calls `f64_iter()` on the result, which returns `Err(Type("Expected numeric array"))` for `ArrData::Boxed`.

**Fix:** After collecting results, detect if all elements are numeric scalars and produce a typed array:
```rust
fn boxed_to_typed(results: Vec<B>, shape: Vec<usize>) -> B {
    // If all results are f64/i32 scalars, squeeze into typed array
    // Otherwise keep as Boxed
    if results.iter().all(|b| b.is_f64()) {
        let vals: Vec<f64> = results.iter().map(|b| b.o2f()).collect();
        let mut out = BqnArr::new_vec_f64(vals);
        out.shape = shape;
        return crate::vm::tag_arr(array::squeeze_num(out));
    }
    let mut out = BqnArr::new_vec_b(results);
    out.shape = shape;
    crate::vm::tag_arr(out)
}
```

Same fix applies to `each_c2`, `cells_c1`, `cells_c2`, and `scan_c1/c2`.

**Confidence:** HIGH — confirmed by tracing `each_c1` → `new_vec_b` → `f64_iter()` failure path.

### Bug 3: SetPrims and SetInv Callbacks Not Called

**Location:** `crates/rbqn/src/bootstrap.rs:372-375`

**Current dead code:**
```rust
let rt_obj_raw = r1_arr.get(0).map_err(|e| BqnError::Domain(e.to_string()))?;
// setPrims and setInv are for inverse system — skip for now
// let set_prims = r1_arr.get(1);
// let set_inv = r1_arr.get(2);
```

Runtime1 returns `⟨runtime_array, setPrims, setInv⟩`. The `setPrims` function registers •Decompose and •Glyph with the runtime. The `setInv` function registers inverse tables. Both must be called — `setPrims` with the glyph arrays that `build_provide` built, and `setInv` with placeholder data if ⁼ and ⌾ are not yet implemented.

**Confidence:** HIGH — confirmed by reading bootstrap.rs:372-375 and the comment there.

### Bug 4: Formatter •Repr Placeholder Is Wrong

**Location:** `crates/rbqn/src/bootstrap.rs:418`

```rust
let repr_fn = m_sys_fn(1); // placeholder for •Repr
```

`sys_idx=1` is `•Decompose`, not `•Repr`. `•Repr` is not yet implemented. The formatter will fail with a wrong function. This is a known deferred item (formatter is lower priority per CONTEXT.md).

**Confidence:** HIGH — confirmed by reading bootstrap.rs:415-422 and the sysv_lookup table in vm.rs:51-59.

### Bug 5: `+´` Returns Wrong Result (Known)

The known failure `+´ returns 3 instead of 6` is caused by Bug 1 (fork dispatch). In runtime0, `+´` likely goes through a train or derived function that uses monadic fork dispatch. Once Bug 1 is fixed, fold itself (`fold_c1`) correctly iterates right-to-left. The `fold_c1` implementation in `modifiers.rs:257-268` is correct.

However, confirm: `+´ ⟨1,2,3⟩` = 6. The fold accumulates right-to-left: acc=3, then acc=c2(+, 2, 3)=5, then acc=c2(+, 1, 5)=6. This is correct. The current wrong result of 3 suggests the fork is executing the wrong branch.

**Confidence:** MEDIUM — the chain from fork bug to `+´` returning 3 is plausible but not directly traced. Fix fork first, then verify.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CBQN oracle for expected values | Custom BQN interpreter | `CBQN_PATH cbqn -e 'expr'` subprocess | CBQN is already present; calling it from Rust test setup is 5 lines |
| Type detection in each_c1 | Complex type inference | `b.is_f64()` check + `squeeze_num()` | Both exist in the codebase already |
| Bootstrap integration test setup | Custom harness | `bootstrap::bootstrap()` in `#[test]` | Function already exists and returns `Result<Runtime>` |

---

## Common Pitfalls

### Pitfall 1: Fixing Each but Not All Modifier Outputs

**What goes wrong:** Fix `each_c1` and `each_c2` but miss `cells_c1`, `cells_c2`, `scan_c1`, `scan_c2` — all produce `ArrData::Boxed` outputs via `new_vec_b`. These will fail the same way when runtime0/1 exercises them.

**How to avoid:** Apply the same `boxed_to_typed` helper to all modifier output sites in `modifiers.rs`.

### Pitfall 2: Tests Racing on Global Stores

**What goes wrong:** `DERIVED_STORE` and `ARR_STORE` are global singletons. Multiple `#[test]` functions running in parallel create IDs that don't interfere, but if one test panics while holding the mutex, the mutex is poisoned and all subsequent tests fail.

**How to avoid:** Run tests with `-- --test-threads=1`. Use `catch_unwind` in test bodies where panic is expected.

### Pitfall 3: Fork Fix Misidentifies the Arguments

**What goes wrong:** BQN fork semantics: monadic `(f g h) x = (f x) g (h x)`. But `Derived` stores: `f=tine_left`, `g=center`, `h=tine_right`. The current broken code calls `c1(d.g, ...)` as the first step — that's the center function, not the right tine.

Verify the storage convention in `m_fork`:
```rust
pub fn m_fork(f: B, g: B, h: B) -> B {
    // Called as: m_fork(f, g, h) from TR3D opcode: pop order is f=left, g=center, h=right
    // TR3D: let f = pop!(); let g = pop!(); let h = pop!();
```

Check `vm.rs:TR3D` opcode: pops f, g, h in that order. So `d.f` = left tine, `d.g` = center, `d.h` = right tine. The correct monadic fork is `c2(d.g, c1(d.f, x), c1(d.h, x))`. This matches the CONCERNS.md fix.

**How to avoid:** Verify the pop order in TR3D before writing the fix.

### Pitfall 4: SetPrims Calling Convention Unknown

**What goes wrong:** We know setPrims and setInv exist in the runtime1 return array, but we don't know their exact calling convention without testing. Calling them with wrong argument types will panic.

**How to avoid:** Use CBQN as oracle to understand what setPrims expects. Alternatively, call with `catch_unwind` and inspect the panic message. The setPrims function likely expects `⟨glyphs_array⟩` matching the format already built in bootstrap.rs.

### Pitfall 5: Compiler Stage Fails for Non-Runtime0 Reasons

**What goes wrong:** After fixing runtime0, runtime1 runs and produces the compiler, but the compiler still fails. The failure might be in exec_stage's body selection (`bodies[0]` vs the correct monadic body), not in the primitives.

**How to avoid:** Add explicit error messages when each stage result doesn't match expected shape. The compiler should return an array; if it returns a non-array, log what it returned.

---

## Code Examples

### Running Bootstrap in a Test

```rust
// tests/bootstrap_integration.rs
// Run with: CBQN_PATH=/path/to/CBQN cargo test --test bootstrap_integration

#[test]
fn runtime0_returns_array() {
    let rt = rbqn::bootstrap::bootstrap().expect("bootstrap should succeed");
    // runtime is the 64-element array (runtime1 output[0])
    assert_eq!(rt.runtime.len(), 64, "runtime must have 64 entries");
}
```

### Testing a Specific BQN Expression

Since the compiler won't work until Phase 1 is complete, test intermediate stages via direct function calls:

```rust
#[test]
fn fold_add_1_2_3() {
    // Build +´ manually without compiler
    let add = rbqn_vm::derive::prim_to_b(0); // + is index 0
    let fold = rbqn_vm::derive::prim_to_b(50); // ´ is index 50
    let derived = rbqn_vm::derive::m1_d(fold, add);

    // Build ⟨1,2,3⟩
    let arr = rbqn_core::array::BqnArr::new_vec_i32(vec![1, 2, 3]);
    let x = rbqn_vm::vm::tag_arr(arr);

    let result = rbqn_vm::derive::c1(derived, x);
    assert_eq!(result.o2f(), 6.0);
}
```

### The Fork Fix (Exact Change)

**File:** `crates/rbqn-vm/src/derive.rs`

```rust
// BEFORE (lines 202-206):
DerivedKind::Fork => {
    let hx = c1(d.h, x);
    let gx = c1(d.g, hx);
    c2(d.f, B::m_f64(0.0), gx) // TODO: proper fork c1
}

// AFTER:
DerivedKind::Fork => {
    let hx = c1(d.h, x);
    let fx = c1(d.f, x);
    c2(d.g, fx, hx)
}
```

### The Each Fix (Reusable Helper)

**File:** `crates/rbqn-vm/src/modifiers.rs`

```rust
/// Convert a Vec<B> of element results into a typed or boxed array.
/// Produces a compact numeric array if all elements are f64 scalars.
fn results_to_arr(results: Vec<B>, shape: Vec<usize>) -> B {
    if !results.is_empty() && results.iter().all(|b| b.is_f64()) {
        let vals: Vec<f64> = results.iter().map(|b| b.o2f()).collect();
        let mut out = rbqn_core::BqnArr::new_vec_f64(vals);
        out.shape = shape;
        return crate::vm::tag_arr(rbqn_core::array::squeeze_num(out));
    }
    let mut out = rbqn_core::BqnArr::new_vec_b(results);
    out.shape = shape;
    crate::vm::tag_arr(out)
}
```

Apply this in: `each_c1`, `each_c2`, `cells_c1`, `cells_c2`, `scan_c1`, `scan_c2`.

### SetPrims/SetInv Invocation Pattern

**File:** `crates/rbqn/src/bootstrap.rs`

```rust
// After extracting runtime1 result:
let rt_obj_raw = r1_arr.get(0).map_err(|e| BqnError::Domain(e.to_string()))?;
let set_prims = r1_arr.get(1).ok(); // function: setPrims(glyphs)
let set_inv = r1_arr.get(2).ok();   // function: setInv(inv_table)

// Call setPrims with glyph arrays
if let Some(sp) = set_prims {
    let glyphs_b = {
        let fn_arr = tag_arr(BqnArr::new_vec_c32(glyphs[0].clone()));
        let md1_arr = tag_arr(BqnArr::new_vec_c32(glyphs[1].clone()));
        let md2_arr = tag_arr(BqnArr::new_vec_c32(glyphs[2].clone()));
        tag_arr(BqnArr::from_b_vec(vec![fn_arr, md1_arr, md2_arr]))
    };
    // NOTE: wrap in catch_unwind until calling convention is confirmed
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c1(sp, glyphs_b)));
}

// Call setInv with empty/placeholder inverse table
if let Some(si) = set_inv {
    let empty_inv = tag_arr(BqnArr::empty_harr());
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c1(si, empty_inv)));
}
```

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|-----------------|--------|
| Manual BQN verification (7/9 pass) | Add `#[test]` functions for each runtime0 entry | Tests catch regressions automatically |
| Skip setPrims/setInv | Call both callbacks after runtime1 | Unblocks •Decompose, •Glyph in compiler |
| Boxed array output from `¨` | Typed array output via `results_to_arr` | Enables pervasive arithmetic on ¨ results |

---

## Open Questions

1. **What does setPrims expect as argument?**
   - What we know: It exists at r1_arr[1] as a callable B
   - What's unclear: Exact argument shape. CBQN calls it as `setPrims(glyphs)` but the format may differ
   - Recommendation: Wrap first call in `catch_unwind`, log the panic message to understand the expected format

2. **Are there more runtime0 failures beyond the 2 known ones?**
   - What we know: 7/9 tests pass currently (⊢, ⌊, <, ∾, ≍, ⊏, ↑ pass; +´ and +¨ fail)
   - What's unclear: Whether fixing fork and each exposes further failures in other runtime0 functions
   - Recommendation: After fixing bugs 1 and 2, run the full runtime0 verification to detect any new failures before moving to runtime1

3. **Does the compiler's body selection use bodies[0] or the correct monadic body?**
   - What we know: `exec_stage` in bootstrap.rs:312 uses `block.bodies[0]` — which per MEMORY.md is the fail body
   - What's unclear: Whether this is the actual bug causing compiler failures, or whether the compiler block structure is different
   - Recommendation: Add a check: if `block.bodies[0]` has `bc_offset == 0` and the first bytecode is `FAIL`, use `block.bodies[1]` instead. This matches the block body layout from MEMORY.md.

4. **Is the formatter stage needed for Phase 1 success?**
   - What we know: Success criterion is `cargo run -- -e '1+1'` outputs `2`, and `main.rs` falls back to `format_b()` when the formatter is unavailable
   - What's unclear: Whether runtime1/compiler have dependencies on formatter being correct
   - Recommendation: Treat formatter as optional for Phase 1. The fallback formatter in main.rs is sufficient for `1+1` to display `2`.

---

## Sources

### Primary (HIGH confidence)

- Direct inspection of `crates/rbqn-vm/src/derive.rs` — fork c1 bug confirmed at line 203-206
- Direct inspection of `crates/rbqn-vm/src/modifiers.rs` — each_c1 output type bug confirmed at line 182
- Direct inspection of `crates/rbqn-core/src/array.rs` — `f64_iter()` returns `Err` for `ArrData::Boxed` confirmed at line 134
- Direct inspection of `crates/rbqn/src/bootstrap.rs` — setPrims/setInv skip confirmed at lines 373-375
- `.planning/codebase/CONCERNS.md` — documents fork bug, each output type issue, and other known concerns with file:line references

### Secondary (MEDIUM confidence)

- MEMORY.md documentation of 7/9 runtime0 test status
- CONCERNS.md documentation of exec_stage body selection issue

### Tertiary (LOW confidence)

- The inference that fork bug causes `+´` to return 3 — plausible but not directly traced through runtime0 bytecode

---

## Metadata

**Confidence breakdown:**
- Bug identification: HIGH — confirmed by reading source code
- Fix correctness: HIGH for fork fix (matches CONCERNS.md documentation), MEDIUM for each fix (approach is right, exact typing logic needs validation)
- SetPrims/setInv calling convention: LOW — need to test or read CBQN source
- Runtime1/compiler blocking: HIGH — cascade dependency is clear from bootstrap.rs structure

**Research date:** 2026-02-21
**Valid until:** Stable (no external dependencies that change)
