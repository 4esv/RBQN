# Coding Conventions

**Analysis Date:** 2026-02-21

## Naming Patterns

**Files:**
- Use `snake_case.rs` for all source files
- Module files use descriptive names matching their domain: `arith_dyad.rs`, `arith_monad.rs`, `structural.rs`
- Lib crate entry points are `lib.rs`; the binary crate has `main.rs`

**Functions:**
- Use `snake_case` for all functions
- BQN primitive functions follow `{glyph_name}_c1` (monadic) and `{glyph_name}_c2` (dyadic) pattern
  - Examples: `add_c1`, `add_c2`, `floor_c1`, `floor_c2`
- Constructor-like functions use `m_` prefix (from CBQN convention for "make"):
  - `m_f64()`, `m_i32()`, `m_c32()`, `m_fork()`, `m_atop()`, `m_native_fn()`, `m_sys_fn()`
- Query/predicate functions use `is_` prefix: `is_f64()`, `is_arr()`, `is_fun()`
- Type-query functions use `q_` prefix: `q_i32()`, `q_bit()`, `q_n()`
- Conversion functions use `o2` prefix (optimistic/unchecked): `o2f()`, `o2i()`, `o2c()`
- Checked conversion functions use `to_` prefix: `to_f64()`, `to_i32()`, `to_usz()`
- Global store operations use `store_`/`get_` prefix: `store_derived()`, `get_derived()`, `store_ns()`, `get_ns()`

**Variables:**
- Use `snake_case` for all variables
- BQN-specific abbreviations are kept short, matching CBQN conventions:
  - `sc` = scope, `psc` = parent scope, `bl` = block, `bc` = bytecode
  - `ia` = item amount (array length), `vam` = variable amount
  - `wa`/`xa` = w-array/x-array (the Optional<&BqnArr> form of w/x)
- Index variables: `i`, `j` for iteration; `d` for depth, `p` for position
- Accumulator: `acc` for fold/reduce operations

**Types:**
- Use `PascalCase` for all types and enums
- Enum variants use `PascalCase`: `DerivedKind::FunBlock`, `ArrData::Boxed`
- Exception: bytecode `Op` enum uses ALL_CAPS matching CBQN: `Op::FN1C`, `Op::SETN`, `Op::VARO`
- The core value type is `B` (single letter, matching CBQN's convention)

**Constants:**
- Use `SCREAMING_SNAKE_CASE`: `ARR_TAG`, `FUN_TAG`, `MD1_TAG`, `RT_LEN`, `RANK_MAX`
- Tag constants include `_TAG` suffix: `C32_TAG`, `VAR_TAG`, `EXT_TAG`
- Module-level constants for modifier indices: `MD1_EACH`, `MD2_ATOP`

## Code Style

**Formatting:**
- No `.rustfmt.toml` or `rustfmt.toml` present; uses default `rustfmt` settings
- No `.editorconfig` file
- Rust edition 2024 (cutting-edge)

**Linting:**
- No `clippy.toml` configuration
- Crate-level allows in `crates/rbqn-vm/src/lib.rs`: `#![allow(non_upper_case_globals, unused_assignments)]`
- Per-item allows: `#[allow(non_camel_case_types)]` on `Op` enum, `#[allow(non_snake_case)]` on functions named after BQN conventions (e.g., `self_indexOf_c1`), `#[allow(dead_code)]` on embedded data types

## Import Organization

**Order:**
1. `std` imports (`std::sync::Arc`, `std::collections::HashMap`)
2. External crate imports (none besides `rustyline` in the binary crate)
3. Workspace crate imports (`rbqn_core::*`, `rbqn_prim::*`)
4. Local crate imports (`crate::block::Block`, `crate::derive::c1`)

**Path Style:**
- Use fully qualified paths from workspace crates: `rbqn_core::B`, `rbqn_core::error::throw`
- Use `crate::` for intra-crate references: `crate::vm::tag_arr`, `crate::derive::c1`
- Glob import used in `rbqn-prim`: `use rbqn_core::*;` for convenience (since nearly every type is used)
- Use selective imports elsewhere: `use rbqn_core::{B, FUN_TAG, MD1_TAG, MD2_TAG, tagu64}`

**Path Aliases:**
- No `use as` aliases
- No path remapping in Cargo.toml

## Error Handling

**Two distinct patterns coexist:**

1. **Result-based (in `rbqn-prim`):**
   - All primitive functions return `Result<PrimResult>`
   - Uses `rbqn_core::Result<T>` which is `std::result::Result<T, BqnError>`
   - Errors propagated with `?` operator
   - Pattern: `let f = x.to_f64()?;`

2. **Panic-based (in `rbqn-vm`):**
   - VM and derive code uses `rbqn_core::error::throw()` which calls `panic!`
   - Error functions in `crates/rbqn-core/src/error.rs`: `throw()`, `throw_type()`, `throw_rank()`, `throw_shape()`, `throw_nyi()`
   - Each wraps a `BqnError` variant in `panic!`
   - Pattern: `rbqn_core::error::throw("VM: stack underflow")`

3. **Bridging (VM calling prims):**
   - In `crates/rbqn-vm/src/derive.rs`, prim results are unwrapped:
   ```rust
   let result = match c1_fn(x, x_arr.as_ref()) {
       Ok(r) => r,
       Err(e) => rbqn_core::error::throw(e.to_string()),
   };
   ```

**Error recovery:**
- `std::panic::catch_unwind` used for BQN's `Catch` modifier (`⎊`) in `crates/rbqn-vm/src/modifiers.rs`
- Also used in formatter invocation in `crates/rbqn/src/main.rs` for graceful fallback
- `.unwrap_or_else(|| throw(...))` pattern used frequently for Option/Result values that should always succeed

**Use `Result<PrimResult>` when writing new primitives in `rbqn-prim`. Use `throw()` functions when writing VM logic in `rbqn-vm`.**

## Logging

**Framework:** None. No logging framework in use.

**Patterns:**
- `eprintln!` for user-facing errors in `crates/rbqn/src/main.rs` and `crates/rbqn/src/repl.rs`
- No debug/trace logging anywhere
- No conditional compilation for debug output

## Comments

**When to Comment:**
- BQN glyph comments before each primitive function: `// + dyad: add`, `// ≍ monad: solo`
- Module-level comments explaining design: modifier dispatch rationale in `crates/rbqn-vm/src/modifiers.rs`
- Inline comments for CBQN port references: `// port of CBQN's isF64`
- TODO/NOTE/HACK tags used per project convention (see Known Issues section)

**Doc Comments (`///`):**
- Used sparingly (~46 occurrences across 8 files)
- Present on key public functions: `exec_block`, `compile_all`, `prim_to_b`, system dispatch functions
- Most functions have NO doc comments
- No module-level `//!` documentation in any file

**Prescriptive: Add `///` doc comments to all public functions. Use `// BQN glyph: description` style for primitive implementations.**

## Function Design

**Size:**
- Large match blocks are acceptable for bytecode dispatch (`eval_bc` in `crates/rbqn-vm/src/vm.rs`, 658 lines)
- Primitive implementations range 5-50 lines each
- Helper functions extracted when pattern repeats: `pervasive_dyad()`, `extract_cell()`

**Parameters:**
- Primitive monadic: `fn name_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult>`
- Primitive dyadic: `fn name_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult>`
- VM call functions: `fn c1(f: B, x: B) -> B` and `fn c2(f: B, w: B, x: B) -> B`
- The `Option<&BqnArr>` parameter pattern avoids repeated array store lookups

**Return Values:**
- Primitives return `Result<PrimResult>` where `PrimResult` is `Scalar(B)` or `Array(BqnArr)`
- VM-level functions return `B` directly (errors are panics)
- Bootstrap returns `Result<Runtime, BqnError>`

## Module Design

**Exports:**
- Each `lib.rs` explicitly declares `pub mod` and `pub use` for key types
- `rbqn-core` re-exports: `B`, tag constants, `BqnArr`, `ArrData`, `tag_arr`, `get_arr`, `ElType`, `BqnError`, `Result`
- `rbqn-prim` re-exports: `get_runtime`, `MonadFn`, `DyadFn`, `Primitive`, `PrimResult`

**Barrel Files:**
- `crates/rbqn-core/src/lib.rs` acts as barrel, re-exporting from submodules
- `crates/rbqn-prim/src/lib.rs` declares modules and re-exports dispatch types
- `crates/rbqn-vm/src/lib.rs` declares modules (includes a `body` re-export alias)

## Global Mutable State

**Pattern:** Static `LazyLock<Mutex<HashMap<u64, T>>>` stores for heap-allocated objects.

- `ARR_STORE` in `crates/rbqn-core/src/arrstore.rs` - maps id -> `BqnArr`
- `DERIVED_STORE` in `crates/rbqn-vm/src/derive.rs` - maps id -> `Arc<Derived>`
- `NS_STORE` in `crates/rbqn-vm/src/namespace.rs` - maps id -> `Arc<NS>`
- `GID_MAP`/`GID_NAMES` in `crates/rbqn-vm/src/namespace.rs` - global identifier registry

Each store uses an `AtomicU64` counter for ID generation. Objects are tagged with NaN-boxing: `tagu64(id << 3, TAG)`.

**Always use this pattern for new global stores. Lock briefly, clone out of the lock.**

## Unsafe Code

**Two occurrences, both in `rbqn-vm`:**

1. `crates/rbqn-vm/src/bytecode.rs:83` - `std::mem::transmute` for `Op` enum from `u32`. Guarded by bounds check (`v < Op::BC_SIZE as u32`). Necessary because Rust has no `#[repr(u32)]` enum-from-int without transmute or match.

2. `crates/rbqn-vm/src/compiler.rs:321` - `unsafe { &*Arc::as_ptr(p) }` to traverse scope parent chain without cloning. Avoids Arc clone overhead in a tight loop. This is sound because the Arc stays alive for the duration.

**Do not add new `unsafe` code without documenting the safety invariant.**

---

*Convention analysis: 2026-02-21*
