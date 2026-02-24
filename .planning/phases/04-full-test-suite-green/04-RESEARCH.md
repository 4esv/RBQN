# Phase 4: Full Test Suite Green - Research

**Researched:** 2026-02-24
**Domain:** BQN interpreter correctness — primitives, namespace semantics, block headers, fill elements, undo/under
**Confidence:** HIGH

## Summary

Phase 4 drives all 13 official BQN test files from their current mixed pass rates to 100% (0 failures). The test suite currently scores ~215 total failures across 8 files. The failures cluster into five distinct root causes: (1) namespace export broken at the compiler level, (2) block header inverse bodies not wired into the undo dispatch, (3) primitive edge cases in table, group, slash, reshape, and comparison primitives, (4) fill element propagation missing for complex cases, and (5) undo/under gaps for rotate-inverse, transpose-inverse, scan-inverse, and replicate-inverse.

The good news: five files are already at 100% (simple, literal, syntax, bytecode, token) and will not regress. The bad news: namespace and header failures share a common root cause (missing `ns_desc` population) that alone blocks 35+ tests. Fixing the namespace descriptor compiler bug is the highest-leverage single fix in the phase.

**Primary recommendation:** Fix namespace export first (compiler.rs body parsing), then wire block inverse bodies, then clear prim/fill/undo/under clusters in priority order by count.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TEST-05 | Pass prim.bqn | 106 failures identified; clusters in table/group/slash/reshape/scan/comparison — all fixable |
| TEST-06 | Pass token.bqn | Already passing (29/29) — no work needed |
| TEST-07 | Pass header.bqn | 11 failures + stack overflow; needs namespace export fix + predicate-in-headerless fix |
| TEST-08 | Pass unhead.bqn | 17 failures; needs block header inverse body dispatch (inv_x_body/inv_w_body) |
| TEST-09 | Pass namespace.bqn | 24 failures; needs ns_desc population in compile_block (body_arr[2]/[3]) |
| TEST-10 | Pass fill.bqn | 28 failures; needs fill propagation through reshape/transpose/shift/group |
| TEST-11 | Pass identity.bqn | 2 failures; needs ∾ identity element for fold on empty leading axis |
| TEST-12 | Pass under.bqn | 9 failures; needs Under with ≍, ⊐⌾<, ⊏⎉N patterns |
| TEST-13 | Pass undo.bqn | 18 failures; needs ⌽⁼, ⍉⁼, +`⁼, /⁼, ×˜⁼ inverses |
| SYS-22 | •FFI | NOT tested in official suite — implement but low priority for test gate |
| SYS-23 | •bit namespace | NOT tested in official suite — implement but low priority for test gate |
| SYS-24 | •term namespace | NOT tested in official suite — implement but low priority for test gate |
| SYS-25 | •ns namespace | NOT tested in official suite (only in fuzz.bqn) — implement but low priority |
| SYS-26 | •HashMap | NOT tested in official suite — implement but low priority for test gate |
</phase_requirements>

## Current Test Status (Phase 3 exit)

Verified 2026-02-24 by running `this.bqn` from `/Users/axel/Code/forks/BQN/test/`:

| File | Passing | Total | Failures | Priority |
|------|---------|-------|----------|----------|
| simple | 20 | 20 | 0 | DONE |
| literal | 52 | 52 | 0 | DONE |
| syntax | 156 | 156 | 0 | DONE |
| bytecode | 37 | 37 | 0 | DONE |
| token | 29 | 29 | 0 | DONE |
| prim | 458 | 564 | 106 | P1 |
| namespace | 26 | 50 | 24 | P1 |
| fill | 34 | 62 | 28 | P2 |
| undo | 50 | 68 | 18 | P2 |
| unhead | 27 | 44 | 17 | P2 |
| under | 55 | 64 | 9 | P3 |
| header | ~145 | 156 | ~11 + overflow | P1 |
| identity | 12 | 14 | 2 | P3 |

**Total remaining failures:** ~215

**Run command:**
```bash
cd /Users/axel/Code/forks/BQN/test && CBQN_PATH=/Users/axel/Code/forks/CBQN /path/to/rbqn this.bqn 2>&1 | tail -5
```

## Architecture Patterns

### How Namespace Exports Work (⇐)

CBQN's body data format (from `CBQN/src/load.c` lines 37-46):
```
body = [
  bytecodeOffset,   // index [0]
  variableCount,    // index [1]
  [...variableIDs], // index [2]: nameList index for each variable slot
  [...exportMask]   // index [3]: nonzero = exported, value = unique global ID
]
```

CBQN's `m_nsDesc` (`CBQN/src/ns.c:5`):
1. Reads `varIDs = bodyRepr[2]` and `exported = bodyRepr[3]`
2. For each variable slot `i`: if `exported[i]` is truthy, call `str2gid(nameList[varIDs[i]])` to get the global name ID
3. Store in `NSDesc.expGIDs[i]`
4. Assign `body->nsDesc = r` so the VM uses it at RETD

**RBQN Bug:** `compiler.rs:compile_block` reads `body_arr.get(0)` (bc_offset) and `body_arr.get(1)` (vam) but **never reads `body_arr.get(2)` or `body_arr.get(3)`**. The `Body.ns_desc` field is **never populated** for user code. The `Body.var_data` field is allocated but never written.

**Fix location:** `crates/rbqn-vm/src/compiler.rs`, inside the body-processing loop starting at line 190, after reading `vam`. Add:
```rust
// Build ns_desc if body has namespace data (body_arr has 4 elements)
let ns_desc = if bo_ia >= 4 {
    let var_ids_b = body_arr.get(2).unwrap_or(B::SENTINEL);
    let export_mask_b = body_arr.get(3).unwrap_or(B::SENTINEL);
    build_ns_desc(var_ids_b, export_mask_b, vam, comp.name_list, imm, ty)
} else {
    None
};
```

Then assign `ns_desc` to the body. Helper `build_ns_desc` uses the nameList from `comp.name_list` to resolve global IDs.

**Impact:** Fixes ALL 24 namespace.bqn failures + ~3 header.bqn failures.

### How Block Header Inverses Work (⁼ headers)

BQN allows blocks to declare their own inverse:
```bqn
{𝕊⁼: 𝕩-1}     # inverse body: x-1
```

The CBQN compiler puts this in `bodyObj[2]` (inv_x monadic body). Our `compiler.rs` already extracts this into `Block.inv_x_body` (line 551). **But the undo dispatch never uses it.**

**Current path for `f⁼x` where f is FunBlock:**
1. `inv_reg(f)` → not native → calls BQN runtime's `INV_REG_FN(f)`
2. BQN runtime says "Can't invert blocks (add an undo header?)" — it doesn't know about native inv bodies

**Fix:** In `derive.rs:inv_reg`, before falling through to the BQN runtime, check if the function is a `FunBlock` with `inv_x_body`:
```rust
if let DerivedKind::FunBlock = d.kind {
    let bl = d.bl.as_ref().unwrap();
    if let Some(ref inv_body) = bl.inv_x_body {
        // Return a wrapper that calls exec_block with inv_x_body
        return m_inv_block(d.bl.clone(), d.sc.clone(), InvKind::X);
    }
}
```

Also need new `DerivedKind::InvBlock` and corresponding c1/c2 dispatch.

**Impact:** Fixes all 17 unhead.bqn failures.

### How ns_desc Gets from Body to Scope

In `vm.rs:exec_block` at `Op::RETD`:
```rust
// Current (works):
if let Some(ref ns_desc) = body.ns_desc {
    return store_ns(NS { desc: ns_desc.clone(), sc: ... });
}
```
This code already exists and is correct — it only needs `body.ns_desc` to be non-None, which the compiler fix provides.

**Additionally**, the FLDO/FLDM/FLDG opcodes for dot-access (`.a`) already work for namespace objects returned from function calls. The issue is purely that the namespace is never created because `ns_desc` is never set.

### namespace.rs NSDesc and get_by_gid

The `get_by_gid` method in `namespace.rs:65` searches `exp_gids` for the matching GID. This is correct. The namespace lookup code is fine — just needs ns_desc to be populated.

**Important detail from CBQN `ns.c:10`:**
```c
i32 off = (ty==0?0:ty==1?2:3) + (imm?0:3);
```
This offset accounts for implicit args (𝕤/𝕩/𝕨 for functions, 𝕄/𝕩/𝕨 for modifiers). The variable slots for exported names start AFTER these implicit args. The `off` calculation must be replicated when building NSDesc.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Name-to-GID mapping | Custom hash map | Existing `namespace::str2gid` / `namespace::gid2str` | Already handles global ID assignment correctly |
| Body format parsing | New parser | Read `bo_ia` from existing `get_arr(body_repr)` | Body array already decoded in compile_block |
| Inverse body dispatch | New VM path | Extend `DerivedKind` enum with `InvBlock` variant | Fits existing c1/c2 dispatch pattern |
| Fill element tracking | New fill system | Extend existing `arr_fill` field in BqnArr | `BqnArr` already has fill element support |

## Common Pitfalls

### Pitfall 1: namespace.rs GID offset for implicit args
**What goes wrong:** `exp_gids[i]` is indexed by variable slot number, but CBQN variable slots 0..off are reserved for implicit args (𝕤, 𝕩, 𝕨, 𝔽, 𝔾, 𝕣). Exported variable slots start at `off`.
**Why it happens:** CBQN's `m_nsDesc` applies `off = (ty==0?0:ty==1?2:3) + (imm?0:3)` to account for these implicit slots.
**How to avoid:** Replicate the offset calculation: for immediate blocks (`imm=true`): `off=0`; for non-immediate fn blocks: `off=3`; for non-immediate md1: `off=5`; for non-immediate md2: `off=6`.
**Warning signs:** `{a⇐5}.a` returns `Namespace does not have field 'a'` even after ns_desc fix — means GID stored at wrong slot index.

### Pitfall 2: table (⌜) requires array left arg
**What goes wrong:** `3+⌜↕6` fails with "Expected array argument".
**Root cause:** Our table implementation requires both args to be arrays; BQN spec says scalars should be treated as rank-0 arrays.
**How to avoid:** In `md2.rs` table dispatch, enclose scalar left arg: `if !w.is_arr() { w = enclose(w); }`. Same for scalar right arg.
**Warning signs:** Any `scalar OP⌜ array` failing.

### Pitfall 3: slash (/) on atoms
**What goes wrong:** `⟨⟩⊸/≡<` on a char ('a') fails "𝕩 must be an array".
**Root cause:** Monadic `/` (replicate by boolean) requires array right arg; BQN spec says atom right arg means "replicate this scalar once per 1 in w".
**How to avoid:** In slash dispatch, enclose scalar 𝕩 when 𝕨 is a boolean/integer array. Also: empty 𝕨 with any 𝕩 should return empty result of 𝕩's type.
**Warning signs:** `/` on char or number (not array) failing.

### Pitfall 4: group (⊔) with list of index arrays
**What goes wrong:** `⊔⟨1‿0‿0‿2⟩` fails "𝕩 must be an integer array".
**Root cause:** Multi-dimensional group (𝕩 is a list of integer arrays) is not implemented; only single integer array is supported.
**How to avoid:** Check if 𝕩 is a boxed array where each element is an integer array; if so, use the multi-dimensional group algorithm.
**Warning signs:** `⊔` with nested array argument failing.

### Pitfall 5: Stack overflow in header.bqn modifier headers
**What goes wrong:** `2{⟨a,b⟩_r:a+b;a _r: a‿a _r}` causes infinite recursion.
**Root cause:** The block `{⟨a,b⟩_r:a+b;a _r: a‿a _r}` has a destructuring modifier header AND recursive self-reference. The `a _r: a‿a _r` case recursively calls _r with `a‿a` as left arg. Our dispatch may not handle multi-element list left args in modifier headers, causing infinite retry.
**How to avoid:** Ensure SETH/PRED dispatch for modifier headers correctly fails-over without re-entering the same body. Check that `a _r` with list left arg doesn't loop.
**Warning signs:** Stack overflow on any multi-header block test.

### Pitfall 6: Structural equality for derived functions
**What goes wrong:** `{a‿b←⟨+´,+´⟩⋄a=b}` returns 0 instead of 1.
**Root cause:** CBQN considers two derived functions structurally equal if they have the same structure (same modifier, same operands). Our DERIVED_STORE assigns unique IDs, so two separately-created `+´` have different IDs and are not `=`.
**How to avoid:** Implement structural equality for derived functions in the `=` dispatch. When both are `Md1D` with same modifier and operand, return 1. This may be complex; assess if it's worth the effort vs. test count.
**Warning signs:** Any `F=F` or `F≡F` where F is a derived function returning 0.

### Pitfall 7: reshape (⥊) with ↑ auto-dimension
**What goes wrong:** `↑‿3⥊"hello"` returns wrong result (no error, wrong shape).
**Root cause:** BQN spec: if one dimension in the shape is `↑` (the function), compute that dimension from element count and other dimensions. Not implemented.
**How to avoid:** In reshape dispatch, check if any shape element is the `↑` function object; compute the missing dimension as `⌈(≠𝕩)÷×´(other dims)`.
**Warning signs:** `↑‿N⥊x` returning wrong result.

## Code Examples

### Parsing body namespace data in compiler.rs

```rust
// Source: CBQN src/ns.c:5-30, adapted for Rust
// Inside compile_block body loop, after reading vam from body_arr:

// Build namespace descriptor if body has namespace info (body_arr has 4+ elements)
let ns_desc: Option<Arc<NSDesc>> = if bo_ia >= 4 {
    let var_ids_b = body_arr.get(2).unwrap_or(B::SENTINEL);
    let export_mask_b = body_arr.get(3).unwrap_or(B::SENTINEL);

    if var_ids_b.is_arr() && export_mask_b.is_arr() {
        let var_ids_arr = get_arr(var_ids_b).unwrap();
        let export_arr = get_arr(export_mask_b).unwrap();
        let ia = var_ids_arr.ia();

        // Offset for implicit args: off = (ty==0?0:ty==1?2:3) + (imm?0:3)
        let arg_off = if imm { 0 } else { 3 }
            + match ty { 0 => 0, 1 => 2, _ => 3 };

        let actual_vam = (ia as i32 + arg_off) as usize;
        let mut exp_gids: Vec<i32> = vec![-1; actual_vam.max(2)];

        for i in 0..ia {
            let cid = var_ids_arr.get(i).map(|v| v.o2f() as usize).unwrap_or(0);
            let cexp = export_arr.get(i).map(|v| v.o2f() != 0.0).unwrap_or(false);

            if cexp && comp.name_list.is_arr() {
                if let Some(nl_arr) = get_arr(comp.name_list) {
                    if let Some(name_b) = nl_arr.get(cid) {
                        if name_b.is_arr() {
                            if let Some(s) = b_to_string_opt(name_b) {
                                let gid = namespace::str2gid(&s);
                                let slot_idx = i + arg_off as usize;
                                if slot_idx < exp_gids.len() {
                                    exp_gids[slot_idx] = gid;
                                }
                            }
                        }
                    }
                }
            }
        }

        Some(Arc::new(NSDesc { var_am: actual_vam as i32, exp_gids }))
    } else {
        None
    }
} else {
    None
};
```

Then when creating the `Body`:
```rust
let mut body = Arc::new(Body::new(final_vam as u16, bc_start, h_max as u32, mpsc as u16));
if let Some(desc) = ns_desc {
    Arc::get_mut(&mut body).unwrap().ns_desc = Some(desc);
}
```

### Block inverse body dispatch (unhead pattern)

```rust
// In derive.rs, new DerivedKind variant:
#[derive(Debug, Clone, PartialEq)]
enum InvKind { X, W, M }

DerivedKind::InvBlock { kind: InvKind },

// Constructor:
pub fn m_inv_block_x(bl: Arc<Block>, psc: Arc<Scope>) -> B {
    store_derived(Derived {
        kind: DerivedKind::InvBlock { kind: InvKind::X },
        f: B::SENTINEL, g: B::SENTINEL, h: B::SENTINEL,
        bl: Some(bl), sc: Some(psc),
    })
}

// In inv_reg, before BQN runtime fallthrough:
if let DerivedKind::FunBlock = d.kind {
    let bl = d.bl.as_ref().unwrap();
    if bl.inv_x_body.is_some() {
        return m_inv_block_x(d.bl.clone().unwrap(), d.sc.clone().unwrap());
    }
}

// c1 dispatch for InvBlock:
DerivedKind::InvBlock { kind: InvKind::X } => {
    let bl = d.bl.as_ref().unwrap().clone();
    let psc = d.sc.as_ref().unwrap().clone();
    let inv_body = bl.inv_x_body.as_ref().unwrap().clone();
    crate::vm::exec_block_with_args(&bl, inv_body, psc, &[f, x])
}
```

### Table (⌜) scalar left arg fix

```rust
// In modifiers.rs, native_md2_c1/c2 dispatch for prim_idx 48 (⌜):
fn table_c2(f: B, w: B, x: B) -> B {
    // BQN spec: scalar args are treated as rank-0 arrays
    let w_arr = if w.is_arr() { w } else { tag_arr(BqnArr::from_b_vec(vec![w])) };
    let x_arr = if x.is_arr() { x } else { tag_arr(BqnArr::from_b_vec(vec![x])) };
    // ... existing table logic using w_arr, x_arr
}
```

### Reshape (⥊) with ↑ auto-dimension

```rust
// In structural.rs, reshape_c2 dispatch:
fn reshape_c2(w: B, x: B) -> B {
    // Check if any shape element is the ↑ function (prim_idx 26)
    if let Some(shape_arr) = get_arr(w) {
        let take_fn = prim_to_b(26); // ↑
        let auto_dim_pos = (0..shape_arr.ia()).find(|&i| {
            shape_arr.get(i).map(|v| v == take_fn).unwrap_or(false)
        });

        if let Some(pos) = auto_dim_pos {
            let other_dims_product: usize = (0..shape_arr.ia())
                .filter(|&i| i != pos)
                .map(|i| shape_arr.get(i).map(|v| v.o2f() as usize).unwrap_or(0))
                .product();
            let elem_count = x_len(x);
            let auto_dim = if other_dims_product == 0 { 0 }
                           else { (elem_count + other_dims_product - 1) / other_dims_product };
            // Rebuild shape with auto_dim at pos
            // ... then proceed with regular reshape
        }
    }
}
```

### Undo gaps: rotate inverse, scan inverse, replicate inverse

```rust
// native_inverse_reg additions:
31 => Some(m_native_fn(31)), // ⌽⁼ = ⌽ (rotate is self-inverse for negative rotate)
// NOTE: Actually ⌽⁼ needs special handling: n⌽⁼x = (-n)⌽x
// This requires a wrapper, not just returning ⌽

// Scan inverse (+`⁼): scan inverse of prefix sums
// +`⁼[a,b,c,...] = [a, b-a, c-b, ...]
// This is -˜` applied to the result (successive differences)
// → add to native_inverse for scan (prim_idx 52 = `)

// Replicate inverse (/⁼): computes counts from replicated array
// /⁼x = counts array such that /counts ≡ ⍷x
// Complex; BQN runtime may handle if provided proper inverse registration
```

### Join identity for ∾ fold

```rust
// In fold/modifiers.rs, identity for ∾ (prim_idx 23):
// ∾˝ on empty leading axis: identity is ↕(0 ∾ trailing_shape)
// For x with shape s₁‿s₂‿...‿sₙ where s₀=0:
//   trailing_shape = s₁‿...‿sₙ
//   identity = ↕(0 ∾ trailing_shape) = empty array with shape 0‿s₁‿...‿sₙ

// In identity_for_fn:
23 => { // ∾
    // Identity of ∾ for fold on axis with shape s is ↕(0∾s) = empty array
    // The trailing shape comes from x (the argument to fold)
    if let Some(arr) = get_arr(x) {
        let trailing_shape = arr.shape()[1..].to_vec(); // drop leading axis
        let empty_shape = std::iter::once(0).chain(trailing_shape).collect::<Vec<_>>();
        return Some(tag_arr(BqnArr::empty_with_shape(empty_shape)));
    }
    None
}
```

## Standard Stack

### Core (no changes from Phase 3)
| Component | Location | Purpose |
|-----------|----------|---------|
| Rust 1.85+ | workspace | Primary implementation language |
| BqnArr | rbqn-core | Array value type with fill element support |
| B (NaN-boxing) | rbqn-core | Value type: f64, c32, or tagged pointer |
| compile_block | rbqn-vm/compiler.rs | Compiles CBQN bytecode to RBQN IR blocks |
| vm::exec_block | rbqn-vm/vm.rs | Executes compiled blocks with scope |
| derive::c1/c2 | rbqn-vm/derive.rs | Dispatches function/modifier application |
| modifiers.rs | rbqn-vm | ¨ ⌜ ´ ˝ ` ⁼ ⌾ ⎉ ⚇ ⍟ dispatch |
| structural.rs | rbqn-prim | ⥊ ↑ ↓ ↕ ≍ ⋈ ∾ ⌽ ⍉ implementation |
| group.rs | rbqn-prim | ⊔ group-by implementation |
| namespace.rs | rbqn-vm | NS/NSDesc storage and GID lookup |

### Key System Interfaces
| Interface | How Used |
|-----------|---------|
| `compile_block` → `body.ns_desc` | NSDesc built from body_arr[2]/[3] for namespace bodies |
| `derive::inv_reg` → `FunBlock.inv_x_body` | Block inverse body dispatch for ⁼ on user-defined blocks |
| `vm::exec_block` → `RETD` → `store_ns` | Returns namespace value when body has ns_desc |
| `namespace::str2gid` | Converts string field name to global integer ID |
| `namespace::get_by_gid` | Looks up field value by GID in NS scope |

## Architecture Patterns

### Recommended Plan Structure

Phase 4 has ~215 failures to clear. Recommended 5-plan structure:

**04-01: Namespace export fix** (highest ROI)
- Fix `compile_block` to read `body_arr[2]`/`[3]` and build `NSDesc`
- Fix GID offset calculation (implicit arg offset)
- **Impact:** Closes all 24 namespace.bqn + ~3 header.bqn failures = 27 tests

**04-02: Block header inverses** (unhead + undo)
- Wire `inv_x_body`/`inv_w_body`/`inv_m_body` into `derive::inv_reg` dispatch
- New `DerivedKind::InvBlock` variant for c1/c2 dispatch
- **Impact:** Closes all 17 unhead.bqn + some undo.bqn failures = ~20 tests

**04-03: Primitive gaps** (prim)
- Table (⌜) scalar args (17 failures)
- Group (⊔) multi-dimensional (14 failures)
- Slash (/) on atoms + empty left (11 failures)
- Reshape (⥊) with ↑ auto-dimension (5+ failures)
- Derived function structural equality for = (5 failures)
- **Impact:** Closes ~50 prim.bqn failures

**04-04: Fill propagation + undo gaps**
- Fill element for complex arithmetic/structural operations (28 fill)
- ⌽⁼ (rotate inverse), ⍉⁼ (rank>2), +`⁼, /⁼ (18 undo)
- ∾˝ identity on empty (2 identity)
- **Impact:** Closes fill + undo + identity = ~48 tests

**04-05: Under + header gaps + remaining prim**
- Under with ≍, ⊐⌾<, ⊏⎉N patterns (9 under)
- Header: predicate-in-headerless fix, modifier headers (8 header)
- Remaining prim failures from scan, pick, sort (50+ prim)
- Fix stack overflow in modifier header recursion
- **Impact:** Closes remaining failures across all files

### Recommended Project Structure (unchanged from Phase 3)
```
crates/
├── rbqn-vm/src/
│   ├── compiler.rs    # PRIMARY: namespace body parsing fix
│   ├── derive.rs      # PRIMARY: block inverse body dispatch
│   ├── modifiers.rs   # table scalar args, scan, undo gaps
│   ├── namespace.rs   # NSDesc structure (already correct)
│   └── vm.rs          # RETD namespace creation (already correct)
└── rbqn-prim/src/
    ├── structural.rs   # reshape ↑ auto-dim, transpose
    ├── group.rs        # multi-dim group
    ├── slash.rs        # replicate on atoms
    ├── fold.rs         # join identity, scan inverse
    ├── md1.rs          # table scalar args
    └── search.rs       # indexOf high-rank cases
```

### Anti-Patterns to Avoid
- **Skipping NSDesc offset:** Don't index exp_gids from 0; apply the implicit-arg offset matching CBQN's `(ty==0?0:ty==1?2:3) + (imm?0:3)`.
- **Populating ns_desc for all bodies:** Only bodies with `bo_ia >= 4` need namespace descriptors. The presence of body_arr[2]/[3] signals a namespace body.
- **New inv_reg table for FunBlock:** Don't duplicate the BQN runtime's inverse table. For FunBlock, check `inv_x_body` directly; only fall through for cases without header inverses.
- **Recursive fix for structural equality:** Derived function equality (`a=b` where both are `+´`) requires structural comparison, not pointer equality. Only implement if the test count justifies the complexity.

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| ns_desc never built | Build from body_arr[2]/[3] in compiler.rs | Enables all namespace tests |
| Block ⁼ always calls BQN runtime | Check inv_x_body before BQN runtime | Enables unhead tests |
| table requires array args | Enclose scalars | Fixes 17 prim tests |
| group requires 1D index array | Handle list of index arrays | Fixes 14 prim tests |
| slash requires array right arg | Handle scalar right arg | Fixes 11 prim tests |
| ∾ has no fold identity | Empty array with trailing shape | Fixes 2 identity tests |

## Open Questions

1. **Derived function structural equality**
   - What we know: `{a‿b←⟨+´,+´⟩⋄a=b}` returns 0 instead of 1. Affects ~5 prim tests.
   - What's unclear: Is CBQN doing pointer deduplication (interning) or structural comparison? CBQN may intern derived functions so two identical `+´` derivations share the same object.
   - Recommendation: Skip for 04-03 (complex), defer if remaining prim count allows hitting 100% without it.

2. **Stack overflow in modifier headers**
   - What we know: `2{⟨a,b⟩_r:a+b;a _r: a‿a _r}` overflows. The block has two modifier headers (`⟨a,b⟩_r:` and `a _r:`).
   - What's unclear: Whether this is infinite recursion (BQN semantics are correct but our dispatch loops) or a legitimate stack-depth issue.
   - Recommendation: Increase thread stack size as a stopgap (Rust default 8MB; `RUST_MIN_STACK=16777216`), then investigate SETH dispatch for multi-header modifier blocks.

3. **Fill element propagation for arithmetic on mixed boxed arrays**
   - What we know: `0‿' '‿' ' ≡ ⊑»⥊ 0‿1‿'c'+○<0‿'b'‿2` fails with "evaluation failed" — not just wrong fill.
   - What's unclear: Whether this is a fill tracking bug or an arithmetic bug on mixed-type boxed arrays.
   - Recommendation: Test `0‿1‿'c'+○<0‿'b'‿2` in isolation first to determine root cause before attributing to fill.

## Sources

### Primary (HIGH confidence)
- **Direct test execution** — ran `this.bqn` against current binary 2026-02-24, got exact failure counts and messages
- **CBQN src/ns.c:5-30** — authoritative implementation of `m_nsDesc` including offset calculation
- **CBQN src/load.c:37-46** — authoritative body data format documentation
- **CBQN src/vm.c:392** — where `m_nsDesc` is called during block compilation
- **`crates/rbqn-vm/src/compiler.rs`** — confirmed `body_arr.get(2)/get(3)` are never read
- **`crates/rbqn-vm/src/derive.rs`** — confirmed `inv_reg` never checks `inv_x_body`
- **`crates/rbqn-vm/src/block.rs`** — confirmed `inv_x_body` is stored but never used

### Secondary (MEDIUM confidence)
- **Failure message analysis** — "Trying to read a field from non-namespace" and "Can't invert blocks" error messages confirm root causes
- **Behavioral comparison** — specific expressions tested against both RBQN and CBQN to confirm expected vs. actual

## Metadata

**Confidence breakdown:**
- Namespace fix: HIGH — root cause fully confirmed by code inspection and CBQN source
- Block inverse fix: HIGH — root cause confirmed; implementation pattern clear
- Table/slash/group fixes: HIGH — bugs reproduced with specific test cases
- Fill propagation: MEDIUM — many cases are "evaluation failed" requiring deeper investigation
- Undo gaps: MEDIUM — some clear (⌽⁼, ⍉⁼), some complex (/⁼, +`⁼)
- Structural equality: LOW — CBQN behavior not fully understood (interning vs. structural)

**Research date:** 2026-02-24
**Valid until:** 2026-03-24 (stable domain; BQN spec doesn't change)
