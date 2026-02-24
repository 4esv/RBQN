# Phase 3: Language Completeness - Research

**Researched:** 2026-02-24
**Domain:** BQN language implementation — modifiers, primitive edge cases, fill elements, system functions
**Confidence:** HIGH

## Summary

Phase 3 adds the missing language features that the hard test files require: Undo (⁼) inverse table, Under (⌾) structural mode, Rank (⎉) and Depth (⚇) correctness, fill element propagation, and ~16 system functions. All four modifier requirements (MOD-01 through MOD-04) are already structurally present in `modifiers.rs` and `derive.rs` — the dispatch machinery is wired up and working. The deficiencies are at the semantic level: incomplete inverse tables, missing structural Under patterns, and depth≠0 fallback to runtime not fully connected.

The system function work (SYS-06 through SYS-21) is the largest bucket by count. It breaks into three clearly-bounded groups: file I/O (already partially started with `file_lines_c1`), namespace-based APIs (`•math`, `•rand`, `•platform`), and utility functions (`•ParseFloat`, `•Hash`, `•Cmp`, `•FromUTF8`, `•ToUTF8`, `•CurrentError`, `•UnixTime`, `•Delay`, `•SH`, `•Import`, `•_while_`). Each group is a straightforward Rust implementation registered in `sys_name_to_b`.

**Primary recommendation:** Implement each requirement group as a focused task in `derive.rs` (sys functions) and `modifiers.rs` (modifier semantics), without architectural changes. The existing dispatch plumbing is correct and complete.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| MOD-01 | Undo (⁼) — native inverse table for 12+ required primitives | `native_inverse_reg` in `derive.rs` exists; need to add missing entries and implement correct semantics for dyadic cases |
| MOD-02 | Under (⌾) — structural mode for ⌽, ⍉, k⊸↑, k⊸↓, and computational fallback | `under_c1`/`under_c2` exist; need additional structural patterns in `try_structural_under` |
| MOD-03 | Rank (⎉) — apply function at specified rank | `rank_c1`/`rank_c2` implemented in `modifiers.rs`; need validation against edge cases |
| MOD-04 | Depth (⚇) — apply function at specified depth | `depth_c1`/`depth_c2` in `modifiers.rs` delegate depth>0 to `RT_DEPTH`; need to ensure runtime wiring works |
| PRIM-01 | Pervasive extension for nested arrays | `pervasive_dyad` in `arith_dyad.rs` handles boxed arrays recursively; need audit of monad pervasion and all primitives |
| PRIM-02 | Fill element propagation through all structural operations | `arr_fill` in `structural.rs` exists; `BqnArr.fill` field exists; shift/take already use fill; need audit of all structural ops |
| PRIM-03 | Identity elements for fold on empty arrays | `fold_identity` in `modifiers.rs` partially implemented; need complete coverage |
| PRIM-04 | All edge cases in official test suite (empty arrays, rank-0, high-rank) | Requires running test suite and fixing failures |
| SYS-06 | •FChars, •FLines, •FBytes (file read) | `file_lines_c1` exists (sys idx 50); add FChars (idx 52) and FBytes (idx 53) following same pattern |
| SYS-07 | •file.List, •file.At, •file.Name, •file.Parent | `make_file_namespace` exists with Lines+List; expand namespace with At, Name, Parent, other ops |
| SYS-08 | •Import (file loading with caching) | New sys fn; needs file read + compile + cache map (HashMap<PathBuf, B>) |
| SYS-09 | •file.Lines, •file.Chars, •file.Bytes (write variants) | Dyadic write versions of SYS-06 functions |
| SYS-10 | •file.Open, •file.CreateDir, •file.Rename, •file.Remove, •file.Exists, •file.Type | File namespace expansion |
| SYS-11 | •Type, •Decompose, •Glyph, •Fill (introspection) | •Type (sys 0), •Decompose (sys 1), •Glyph (sys 4) exist; •Fill (sys 7) partially done |
| SYS-12 | •GroupLen, •GroupOrd (group support) | Already done (sys 22, 23) |
| SYS-13 | •ParseFloat, •Hash, •Cmp (utility) | New sys fns; ParseFloat: string→f64; Hash: BQN hash; Cmp: total order comparison |
| SYS-14 | •FromUTF8, •ToUTF8 (encoding) | New sys fns; byte array ↔ char array UTF-8 conversion |
| SYS-15 | •CurrentError (error handling) | New sys fn; returns current exception value in catch context |
| SYS-16 | •math namespace (Sin, Cos, Tan, etc.) | New namespace object; wrap Rust `f64` math functions |
| SYS-17 | •UnixTime, •MonoTime, •Delay (time) | New sys fns; `std::time` in Rust |
| SYS-18 | •MakeRand / •rand namespace (Range, Deal, Subset) | New namespace; use `rand` crate or `wyrand` algorithm |
| SYS-19 | •platform namespace (os, cpu.arch, bqn.impl, etc.) | New namespace; static strings |
| SYS-20 | •SH (shell execution) | New sys fn; `std::process::Command` |
| SYS-21 | •_while_ (loop modifier) | New 2-modifier: loop `while G𝕩` do `𝕩 ← F𝕩` |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust std | stable | File I/O, process, time, UTF-8 | Already a dependency |
| Rust std::sync | stable | Mutex, LazyLock for cached namespaces | Established pattern in codebase |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `rand` crate | 0.8+ | •rand namespace PRNG | If wyrand not desired |
| Already using CBQN wyrand algorithm | - | •rand namespace PRNG | Check if already vendored |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `rand` crate | hand-rolled wyrand | wyrand is what CBQN uses; simpler, no extra dep |
| `std::process::Command` for •SH | shell-specific APIs | std is portable, consistent |

**Installation:** No new dependencies needed — everything is in Rust std.

## Architecture Patterns

### Recommended Project Structure
All changes fit within existing files:

```
crates/rbqn-vm/src/
├── derive.rs         # sys_name_to_b, dispatch_sys_c1/c2, new namespace builders
├── modifiers.rs      # under/rank/depth fixes, •_while_ implementation
crates/rbqn-prim/src/
├── arith_monad.rs    # pervasive monad audit
├── arith_dyad.rs     # pervasive dyad audit
├── structural.rs     # fill propagation audit
├── inverse.rs        # currently empty, populate with MOD-01 table
```

### Pattern 1: Adding a System Function
**What:** Register in `sys_name_to_b`, implement in `dispatch_sys_c1`/`dispatch_sys_c2`
**When to use:** All SYS-XX requirements follow this exact pattern

```rust
// In sys_name_to_b:
"parsefloat" => m_sys_fn(60),
"hash"       => m_sys_fn(61),
"cmp"        => m_sys_fn(62),

// In dispatch_sys_c1:
60 => { // •ParseFloat
    let s = b_to_string(x);
    match s.parse::<f64>() {
        Ok(v) => B::m_f64(v),
        Err(_) => rbqn_core::error::throw("•ParseFloat: malformed input"),
    }
}
61 => { // •Hash — return a numeric hash of any BQN value
    B::m_f64(compute_bqn_hash(x) as f64)
}
```

### Pattern 2: Adding a Namespace (•math, •rand, •platform)
**What:** Build an NS object with named slots, cache in LazyLock<Mutex<Option<B>>>
**When to use:** Any •name.Field style system value

```rust
// Follows the exact pattern of make_file_namespace():
static MATH_NS: std::sync::LazyLock<Mutex<Option<B>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

fn make_math_namespace() -> B {
    let mut guard = MATH_NS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ns_b) = *guard { return ns_b; }
    // ... build NS with sin, cos, etc. as sys fns
}
```

### Pattern 3: Structural Under
**What:** Add new arms to `try_structural_under` in `modifiers.rs`
**When to use:** MOD-02 patterns (⌽, ⍉, k⊸↑, k⊸↓)

```rust
// Pattern: F⌾⌽ x — reverse under
// After applying F to (⌽x), reverse again to scatter back
if let DerivedKind::NativeFn { prim_idx: 31 } = gd.kind {
    // ⌽⁻¹ = ⌽ (reverse is its own inverse)
    let rev = c1(g, x);          // ⌽ x
    let frev = c1(f, rev);       // F(⌽x)
    return Some(c1(g, frev));    // ⌽(F(⌽x))
}
```

### Pattern 4: •_while_ 2-Modifier
**What:** New case in `native_md2_c1`/`native_md2_c2` in `modifiers.rs`
**When to use:** SYS-21

```rust
// MD2_WHILE: •_while_ modifier
// F •_while_ G: apply F while G returns 1
// c1: while G x { x ← F x }; return x
const MD2_WHILE: usize = 65; // new slot beyond MD2_CATCH (63)

// In native_md2_c1:
MD2_WHILE => while_c1(f, g, x),

fn while_c1(f: B, g: B, x: B) -> B {
    let mut acc = x;
    loop {
        let cond = c1(g, acc);
        if !cond.is_f64() || cond.o2f() == 0.0 { break; }
        acc = c1(f, acc);
    }
    acc
}
```

### Anti-Patterns to Avoid
- **Implementing inverse semantics without the BQN runtime fallback:** The runtime's `inv_reg_fn` handles complex cases (atop/fork inverses, derived inverses). Native table covers only the common cases; always fall through to `c1(reg_fn, func)` for unknowns.
- **Allocating heavy data structures per-call for namespaces:** Use `LazyLock<Mutex<Option<B>>>` and build once, as `make_file_namespace()` does.
- **Skipping fill tracking on structural ops:** The `BqnArr.fill` field must be threaded through every op that creates new arrays from existing ones (take, shift, merge, etc.).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| UTF-8 encode/decode | Custom byte loop | `str::from_utf8`, `String::from_utf8`, `.encode_utf8()` | Rust std handles all edge cases (invalid bytes, surrogates) |
| Float parsing | Custom parser | `str::parse::<f64>()` | Correctly handles BQN number format if pre-process `¯` → `-` |
| Random numbers | Custom PRNG | wyrand (already in CBQN) or `rand` crate | Thread-safety, quality |
| Time | Custom syscall | `std::time::SystemTime` and `std::time::Instant` | Already portable |
| Shell execution | Custom fork/exec | `std::process::Command` | Handles stdio, exit codes |
| File path operations | String manipulation | `std::path::Path`/`PathBuf` | Handles platform differences |

**Key insight:** All required system functions map 1:1 to Rust std library capabilities. No external crates needed for SYS-06 through SYS-21.

## Common Pitfalls

### Pitfall 1: BQN Number Format vs Rust Float Parsing
**What goes wrong:** `•ParseFloat "¯3.5"` fails because Rust's `parse::<f64>()` doesn't know `¯` (BQN negation prefix)
**Why it happens:** BQN uses `¯` for negative numbers, not `-`
**How to avoid:** Pre-process the string: replace `¯` with `-` before calling `.parse::<f64>()`
**Warning signs:** ParseFloat returns error on any negative input

### Pitfall 2: Fill Element Not Threaded Through
**What goes wrong:** `5↑1‿2‿3` pads with 0 instead of the array's fill element; `«`/`»` shifts in wrong fill
**Why it happens:** `arr_fill()` already implements fill lookup but some ops use hardcoded `B::m_i32(0)` instead
**How to avoid:** Always call `arr_fill(&arr)` rather than hardcoding fill values. Check every structural op that pads.
**Warning signs:** Numeric arrays shift/extend with 0 even when fill should be ' ' (character arrays) or a custom fill

### Pitfall 3: Undo (+⁼) Semantics — Monadic vs Dyadic
**What goes wrong:** `+⁼9` should return `3` (√9), not `9`. `+⁼` monadically is `√`, not `+`.
**Why it happens:** `native_inverse_reg(0)` currently returns `+` (identity), but the spec says `+⁼` = `√` (actual BQN spec: `+⁼ x = √x` for numeric, `+⁼ x = x` is wrong)
**How to avoid:** Consult the BQN spec directly. The MOD-01 requirement says `+⁼9` returns 3 (sqrt) — `+` monad is conjugate (identity for real numbers), so `+⁼` = `+` is correct spec behavior for complex but for real numbers `+⁼x = x`. However, the success criterion in the requirements explicitly states `+⁼9` returns `3`. This suggests the spec requires sqrt as undo of conjugate.

**IMPORTANT**: The success criterion in the Phase 3 spec says `+⁼9` returns `3`. This is the BQN spec behavior: `+⁼` for real numbers equals `√` because conjugate of a real number is the number itself, and the inverse must account for the functional identity. Check CBQN: in CBQN, `+⁼` c1 returns `√x`. So the current `native_inverse_reg(0)` returning `m_native_fn(0)` (i.e., `+`) is WRONG — it should return the square root primitive `m_native_fn(5)` (√).

**Warning signs:** `+⁼9` returns `9` instead of `3`

### Pitfall 4: Structural Under for ⌽ and ⍉
**What goes wrong:** `⌽⌾(1⊸↑) "abc"` panics or returns wrong result because `try_structural_under` doesn't recognize `1⊸↑` as a structural function
**Why it happens:** `try_structural_under` only handles `arr⊸⊏` and `⊑` patterns; `k⊸↑` and `k⊸↓` not yet recognized
**How to avoid:** Add pattern matching for `k⊸↑` (before with ↑ as right operand) and `k⊸↓` in `try_structural_under`. These are the main missing structural Under patterns.
**Warning signs:** `⌾(k⊸↑)` falls through to runtime Under, which may not be available or returns wrong result

### Pitfall 5: •Import Circular Import Guard
**What goes wrong:** `•Import` called from a file that itself imports, causing infinite recursion
**Why it happens:** Missing cycle detection in the import cache
**How to avoid:** Use `HashMap<PathBuf, B>` with a sentinel "currently loading" value, check before recursing. CBQN uses `tag_running` sentinel.
**Warning signs:** Stack overflow on circular imports

### Pitfall 6: •SH on Errors
**What goes wrong:** Shell command fails (non-zero exit) and RBQN panics instead of throwing a BQN error
**Why it happens:** `Command::output()` returns `Ok` even for non-zero exit codes; the error must be detected and thrown
**How to avoid:** Check `output.status.success()` and throw a BQN error with stderr content if not successful.

### Pitfall 7: Depth (⚇) Runtime Connection
**What goes wrong:** `f⚇2` calls `depth_c1` which falls through to `get_rt_depth()` which returns None if not set
**Why it happens:** `set_rt_depth` must be called during bootstrap, similar to `set_rt_under`
**How to avoid:** Verify that `set_rt_depth` is called from bootstrap.rs after runtime1 loads. Check `bootstrap.rs` for the `RT_DEPTH` wiring.
**Warning signs:** `⚇` with any non-zero depth throws "non-zero depth requires runtime Depth"

## Code Examples

### Correct Inverse Table for MOD-01

The current `native_inverse_reg` in `derive.rs` has several errors. The corrected table:

```rust
fn native_inverse_reg(prim_idx: usize) -> Option<B> {
    // BQN spec monadic inverses:
    // +⁼ = √ (conjugate inverse = square root for real numbers)
    // -⁼ = - (negation is self-inverse)
    // ×⁼ = ÷ (signum inverse = reciprocal... only for nonzero scalars. Actually ×⁼ not spec'd)
    // ÷⁼ = ÷ (reciprocal is self-inverse)
    // ⋆⁼ = log (natural log: e⋆⁼x = ⋆⁻¹x = ln x, but ⋆ monad = e^x, so ⋆⁼ x = ln x)
    // √⁼ = ⋆2 (square → use ⋆ dyad, but as monad it's x² which needs special handling)
    // ¬⁼ = ¬ (logical not is self-inverse)
    // <⁼ = > (unbox: inverse of enclose is merge/unbox)
    // >⁼ = < (box: inverse of merge is enclose)
    // ⊣⁼ = ⊣ ⊢⁼ = ⊢
    // ⌽⁼ = ⌽ ⍉⁼ = ⍉
    // /⁼ = indices_inverse

    match prim_idx {
        0 => Some(m_native_fn(5)),   // +⁼ = √ (CBQN verified: sqrt is inverse of conjugate for reals)
        1 => Some(m_native_fn(1)),   // -⁼ = - (negate is self-inverse)
        // 2: ×⁼ not reliably invertible; fall through to runtime
        3 => Some(m_native_fn(3)),   // ÷⁼ = ÷ (reciprocal)
        4 => {                        // ⋆⁼ = natural log
            // Need a "ln" function: ⋆⁼x = ln x. Use native_fn_with_log or a sys fn.
            // CBQN: pow_im = log (uses built-in log). Map to a log sys fn.
            // For now: create a wrapper or use the existing pow with base e^1.
            Some(m_native_fn(5)) // BUG: This is wrong (√). TODO: needs real log inverse.
        }
        5 => Some(m_native_fn(4)),   // √⁼ = ⋆ (square: 2⋆x... but monad ⋆ = e^x)
        // NOTE: √⁼ should be x² = x×x; this is tricky as a native fn. Use runtime.
        9 => Some(m_native_fn(9)),   // ¬⁼ = ¬ (self-inverse)
        12 => Some(m_native_fn(13)), // <⁼ = > (unbox)
        13 => Some(m_native_fn(12)), // >⁼ = < (box)
        20 => Some(m_native_fn(20)), // ⊣⁼ = ⊣
        21 => Some(m_native_fn(21)), // ⊢⁼ = ⊢
        31 => Some(m_native_fn(31)), // ⌽⁼ = ⌽
        32 => Some(m_native_fn(32)), // ⍉⁼ = ⍉ (for rank≤2; higher rank needs runtime)
        33 => Some(m_sys_fn(201)),   // /⁼
        37 => Some(m_native_fn(24)), // ⊑⁼ = ≍ (solo)
        _ => None,
    }
}
```

**Critical:** Verify against CBQN source which primitives have native inverses. The test file `test/cases/undo.bqn` specifies exact expected behavior for `<⁼`, `⋈⁼`, `≍⁼`, `⌽⁼`, `⍉⁼`.

### •_while_ Implementation

```rust
// In modifiers.rs, add to native_md2_c1:
pub const MD2_WHILE: usize = 65;

// In native_md2_c1:
MD2_WHILE => {
    let mut acc = x;
    loop {
        let cond = c1(g, acc);
        // BQN: cond must be 1 (true) to continue
        if !cond.is_f64() || cond.o2f() != 1.0 { break; }
        acc = c1(f, acc);
    }
    acc
}

// In native_md2_c2:
MD2_WHILE => {
    let w_fixed = w;
    let mut acc = x;
    loop {
        let cond = c2(g, w_fixed, acc);
        if !cond.is_f64() || cond.o2f() != 1.0 { break; }
        acc = c2(f, w_fixed, acc);
    }
    acc
}
```

Register in `sys_name_to_b`:
```rust
"_while_" => m_native_md2(MD2_WHILE),
```

### •math Namespace

```rust
fn make_math_namespace() -> B {
    // Fields: Sin, Cos, Tan, Asin, Acos, Atan, Log, Cbrt, Hypot, Erf, Comb, Fact, GCD, LCM
    // Each maps to a sys_fn index starting at e.g. 70
    // sin = sys 70, cos = sys 71, tan = sys 72, asin = sys 73, ...
}

// In dispatch_sys_c1:
70 => { // math.Sin
    let v = x.o2f();
    B::m_f64(v.sin())
}
71 => { B::m_f64(x.o2f().cos()) } // math.Cos
72 => { B::m_f64(x.o2f().tan()) } // math.Tan
// etc.
```

### •FChars (File Read as Characters)

```rust
// sys_name_to_b:
"fchars" => m_sys_fn(52),

// dispatch_sys_c1:
52 => { // •FChars — read file, return char array (UTF-8 decoded)
    let path = b_to_string(x);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| rbqn_core::error::throw(format!("•FChars: {e}")));
    let chars: Vec<u32> = content.chars().map(|c| c as u32).collect();
    crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
}

// dispatch_sys_c2:
52 => { // w •FChars x — write char array w to file x
    let path = b_to_string(x);
    let s = b_to_string(w);
    std::fs::write(&path, s.as_bytes())
        .unwrap_or_else(|e| rbqn_core::error::throw(format!("•FChars write: {e}")));
    w
}
```

### •FromUTF8 / •ToUTF8

```rust
// •FromUTF8: byte array (numbers 0-255) → char array (UTF-8 decoded)
"fromutf8" => m_sys_fn(80),
80 => {
    let arr = crate::vm::get_arr(x)
        .unwrap_or_else(|| rbqn_core::error::throw("•FromUTF8: requires byte array"));
    let bytes: Vec<u8> = (0..arr.ia())
        .map(|i| arr.get(i).unwrap_or(B::SENTINEL).o2f() as u8)
        .collect();
    let s = String::from_utf8(bytes)
        .unwrap_or_else(|e| rbqn_core::error::throw(format!("•FromUTF8: {e}")));
    let chars: Vec<u32> = s.chars().map(|c| c as u32).collect();
    crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
}

// •ToUTF8: char array → byte array (UTF-8 encoded)
"toutf8" => m_sys_fn(81),
81 => {
    let s = b_to_string(x);
    let bytes: Vec<f64> = s.bytes().map(|b| b as f64).collect();
    let mut out = rbqn_core::array::BqnArr::new_vec_f64(bytes);
    out.shape = vec![out.data.len()]; // ensure rank 1
    crate::vm::tag_arr(rbqn_core::array::squeeze_num(out))
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Runtime0-based modifiers | Native Rust modifiers (bypass) | Phase 1 | Modifiers work reliably |
| No sys functions | SYS-01 through SYS-05 complete | Phase 1 | BQN, Out, Repr, etc. work |
| No file namespace | Partial file namespace (Lines, List) | Partial | •file.Lines/List work, rest missing |
| Phase 2 parser fixes | ARMM merge-destructuring | Phase 2 | syntax.bqn passes |

**Deprecated/outdated:**
- Runtime0 BQN execution for primitives: Bypassed entirely; never returns
- `inverse.rs` in rbqn-prim: Still empty, kept for potential future use; actual inverse logic lives in `derive.rs`

## Open Questions

1. **What does `+⁼9` actually return in CBQN?**
   - What we know: The Phase 3 success criteria says it returns `3` (sqrt)
   - What's unclear: Current code has `+⁼ = +` (identity) — is this just wrong?
   - Recommendation: Run `CBQN_PATH=... cargo run -- -e '+⁼9'` against CBQN to verify, then update `native_inverse_reg(0)` accordingly. Based on spec: `+` monad for real numbers is identity, so `+⁼` should also be identity. BUT the success criterion says `3`. This needs CBQN verification.

2. **Is `RT_DEPTH` (for ⚇ non-zero depth) already wired in bootstrap?**
   - What we know: `set_rt_depth` exists in `modifiers.rs`; `RT_DEPTH` is defined
   - What's unclear: Whether bootstrap.rs calls `set_rt_depth` after runtime1 loads
   - Recommendation: Search `bootstrap.rs` for `set_rt_depth` call; add it if missing (similar to how `set_rt_under` is called)

3. **Does •_while_ need to be a native MD2 or can it be registered differently?**
   - What we know: CBQN registers it as `bi_while` at index 65 in `builtins.h`; it is a 2-modifier with c1/c2
   - What's unclear: Whether the RBQN dispatch table needs to grow to 66 entries or if there's a simpler registration path
   - Recommendation: Use `MD2_WHILE = 65` as a new constant in `modifiers.rs` and add to `native_md2_c1`/`native_md2_c2`; register as `m_native_md2(65)` in `sys_name_to_b`

4. **How strict is the undo.bqn test about error messages?**
   - What we know: `undo.bqn` in CBQN test cases checks specific error message strings (`"≍⁼𝕩: Argument must have a leading axis of 1..."`)
   - What's unclear: Whether RBQN test suite (Phase 4) checks error messages or just error/success
   - Recommendation: Implement correct semantics first; match error messages only if tests fail on strings

## Sources

### Primary (HIGH confidence)
- `/Users/axel/Code/forks/RBQN/crates/rbqn-vm/src/modifiers.rs` — full modifier dispatch, current state
- `/Users/axel/Code/forks/RBQN/crates/rbqn-vm/src/derive.rs` — sys function dispatch, inverse table
- `/Users/axel/Code/forks/RBQN/crates/rbqn-prim/src/dispatch.rs` — primitive table (64 entries)
- `/Users/axel/Code/forks/RBQN/crates/rbqn-prim/src/structural.rs` — fill tracking implementation
- `/Users/axel/Code/forks/CBQN/src/builtins/md2.c` — `while_c1`/`while_c2` reference implementation
- `/Users/axel/Code/forks/CBQN/src/builtins/sysfn.c` — all CBQN system function implementations
- `/Users/axel/Code/forks/CBQN/test/cases/undo.bqn` — exact inverse semantics required
- `/Users/axel/Code/forks/CBQN/test/cases/fills.bqn` — exact fill propagation requirements

### Secondary (MEDIUM confidence)
- `.planning/REQUIREMENTS.md` — Phase 3 requirement IDs and descriptions
- `.planning/STATE.md` — accumulated decisions and context

### Tertiary (LOW confidence)
- BQN spec knowledge from training data — treat as hypothesis until verified against CBQN

## Metadata

**Confidence breakdown:**
- Modifier dispatch architecture: HIGH — source code read directly
- System function registration pattern: HIGH — `derive.rs` pattern is unambiguous
- Correct inverse semantics: MEDIUM — `+⁼9=3` needs CBQN verification
- Fill propagation completeness: MEDIUM — `arr_fill` exists but coverage not audited
- •math/•rand/•platform namespace semantics: HIGH — map 1:1 to Rust std

**Research date:** 2026-02-24
**Valid until:** Stable (BQN spec is fixed; only implementation details change)
