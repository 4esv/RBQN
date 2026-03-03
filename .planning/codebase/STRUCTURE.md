# Codebase Structure

**Analysis Date:** 2026-02-21

## Directory Layout

```
RBQN/
├── Cargo.toml                    # Workspace root (5 members)
├── crates/
│   ├── rbqn/                     # Binary crate -- CLI, bootstrap, REPL
│   │   ├── Cargo.toml
│   │   ├── build.rs              # Parses CBQN gen files into embedded bytecode
│   │   └── src/
│   │       ├── main.rs           # Entry point, CLI dispatch, exec_string, format_result
│   │       ├── bootstrap.rs      # 4-stage bootstrap: runtime0 -> runtime1 -> compiler -> formatter
│   │       ├── cli.rs            # Argument parsing (Args, Mode enum)
│   │       ├── repl.rs           # Interactive REPL with rustyline
│   │       └── embedded/
│   │           └── mod.rs        # EmbeddedBytecode struct + include! of generated code
│   ├── rbqn-core/                # Leaf crate -- fundamental types, zero deps
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs            # Re-exports B, BqnArr, ArrData, ElType, BqnError
│   │       ├── value.rs          # B type (NaN-boxed u64), tag constants, constructors, accessors
│   │       ├── array.rs          # BqnArr struct, ArrData enum (Bit/I8/I16/I32/F64/C8/C16/C32/Boxed), squeeze_num
│   │       ├── arrstore.rs       # Global ARR_STORE: HashMap<u64, BqnArr>, tag_arr/get_arr
│   │       ├── eltype.rs         # ElType enum (Bit..B), width/is_num/is_chr methods
│   │       ├── error.rs          # BqnError enum (Type/Rank/Shape/Domain/Assert/Nyi), throw functions
│   │       ├── fill.rs           # Fill value handling (placeholder)
│   │       ├── format.rs         # Basic formatting utilities (placeholder)
│   │       ├── squeeze.rs        # Array type narrowing (placeholder)
│   │       └── compare.rs        # Value comparison utilities (placeholder)
│   ├── rbqn-vm/                  # VM crate -- bytecode interpreter, compiler, scoping
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs            # Module declarations
│   │       ├── vm.rs             # eval_bc: main VM interpreter loop, exec_block, exec_block_with_args
│   │       ├── compiler.rs       # compile_all/compile_block: transforms source BC to internal BC
│   │       ├── bytecode.rs       # Op enum (all opcodes), bc_len, stack_diff, stack_consumed
│   │       ├── block.rs          # Block, Body, Comp structs; eval_fun_block, m_md1_block, m_md2_block
│   │       ├── derive.rs         # Derived/DerivedKind, DERIVED_STORE, c1/c2 dispatch, prim_to_b, system fn dispatch
│   │       ├── scope.rs          # Scope, ScopeExt, v_get/v_set/v_seth/v_get_move, bad read/write checks
│   │       ├── modifiers.rs      # Native modifier implementations (each, fold, scan, table, cells, choose, repeat, catch, atop, over, before, after, val, const, swap)
│   │       ├── namespace.rs      # NS, NSDesc, NS_STORE, GID_MAP, str2gid/gid2str, field access
│   │       └── env.rs            # EnvStack for error position tracking (10k depth limit)
│   ├── rbqn-prim/                # Primitives crate -- 64 native BQN primitive implementations
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs            # Module declarations, re-exports
│   │       ├── dispatch.rs       # Primitive struct, PrimResult, MonadFn/DyadFn types, get_runtime() -> Vec<Primitive>
│   │       ├── arith_monad.rs    # Monadic arithmetic: +- ×÷⋆√⌊⌈|¬
│   │       ├── arith_dyad.rs     # Dyadic arithmetic: +- ×÷⋆√⌊⌈|¬∧∨
│   │       ├── compare.rs        # Comparison: <>=≠≤≥≡≢
│   │       ├── structural.rs     # Structural: ⊣⊢⥊∾≍⋈↑↓↕«»⌽⍉ (identity, reshape, join, couple, take, drop, range, shift, reverse, transpose)
│   │       ├── select.rs         # Selection: ⊏⊑ (first_cell, select, first, pick)
│   │       ├── slash.rs          # Indices/replicate: / (indices_c1, replicate_c2)
│   │       ├── search.rs         # Search: ⊐⊒∊⍷ (indexOf, count, memberOf, find, deduplicate, mark_firsts)
│   │       ├── sort.rs           # Sort/grade: ⍋⍒ (grade_up, grade_down, bins_up, bins_down)
│   │       ├── group.rs          # Group: ⊔ (group_indices, group, group_len, group_ord)
│   │       ├── fold.rs           # Fold support (placeholder, main impl in rbqn-vm/modifiers.rs)
│   │       ├── md1.rs            # 1-modifier support (placeholder)
│   │       ├── md2.rs            # 2-modifier support (placeholder)
│   │       ├── inverse.rs        # Inverse support (placeholder)
│   │       └── sysfn.rs          # System functions: !assert, •Type, •Fill
│   └── rbqn-gpu/                 # GPU acceleration crate (experimental)
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs            # Module declarations
│           ├── context.rs        # WebGPU device/queue initialization
│           ├── buffer.rs         # GPU buffer management
│           ├── pipeline.rs       # Compute pipeline creation
│           ├── dispatch.rs       # GPU operation dispatch
│           ├── fusion.rs         # Kernel fusion optimization
│           └── kernels/
│               ├── mod.rs        # Kernel module declarations
│               ├── arith.rs      # Arithmetic compute shaders
│               ├── reduce.rs     # Reduction kernels
│               ├── scan.rs       # Scan kernels
│               ├── select.rs     # Selection kernels
│               └── sort.rs       # Sort kernels
├── tests/                        # Empty -- no test files yet
└── .planning/
    └── codebase/                 # Architecture documentation (this file)
```

## Directory Purposes

**`crates/rbqn/`:**
- Purpose: Application binary -- wires everything together
- Contains: CLI parsing, bootstrap orchestration, REPL loop, code execution pipeline
- Key files: `build.rs` (compile-time bytecode embedding), `bootstrap.rs` (runtime initialization)

**`crates/rbqn-core/`:**
- Purpose: Zero-dependency leaf crate with all fundamental types
- Contains: `B` type, `BqnArr`, `ArrData`, `ElType`, `BqnError`, global array store
- Key files: `value.rs` (B type definition), `array.rs` (BqnArr + ArrData), `arrstore.rs` (global store)

**`crates/rbqn-vm/`:**
- Purpose: The virtual machine -- bytecode compilation, interpretation, scoping, dispatch
- Contains: VM loop, bytecode compiler, opcodes, blocks/bodies, derived objects, modifiers, namespaces
- Key files: `vm.rs` (interpreter), `compiler.rs` (bytecode transform), `derive.rs` (c1/c2 dispatch + derived store)

**`crates/rbqn-prim/`:**
- Purpose: Native implementations of BQN's 64 primitives
- Contains: Function implementations organized by category (arithmetic, comparison, structural, etc.)
- Key files: `dispatch.rs` (Primitive struct + get_runtime), `structural.rs` (largest file -- many structural ops)

**`crates/rbqn-gpu/`:**
- Purpose: Experimental GPU acceleration via WebGPU/wgpu
- Contains: GPU context, buffers, compute pipelines, WGSL kernels
- Key files: `context.rs`, `dispatch.rs`

## Key File Locations

**Entry Points:**
- `crates/rbqn/src/main.rs`: Binary entry point, CLI dispatch
- `crates/rbqn/src/bootstrap.rs`: `bootstrap()` -- called once to initialize the runtime
- `crates/rbqn-vm/src/vm.rs`: `eval_bc()` -- the VM interpreter loop

**Configuration:**
- `Cargo.toml`: Workspace root with all members
- `crates/rbqn/build.rs`: Build-time CBQN bytecode parsing

**Core Logic:**
- `crates/rbqn-vm/src/derive.rs`: `c1()`/`c2()` -- central dispatch for all BQN function calls
- `crates/rbqn-vm/src/compiler.rs`: `compile_all()` -- bytecode compilation
- `crates/rbqn-vm/src/modifiers.rs`: All native modifier implementations
- `crates/rbqn-prim/src/dispatch.rs`: `get_runtime()` -- the 64-primitive registry

**Types:**
- `crates/rbqn-core/src/value.rs`: `B` type definition
- `crates/rbqn-core/src/array.rs`: `BqnArr`, `ArrData`
- `crates/rbqn-vm/src/block.rs`: `Block`, `Body`, `Comp`
- `crates/rbqn-vm/src/scope.rs`: `Scope`, `ScopeExt`
- `crates/rbqn-vm/src/bytecode.rs`: `Op` enum (all opcodes)

**Testing:**
- `tests/`: Empty directory, no tests exist yet

## Naming Conventions

**Files:**
- `snake_case.rs` for all Rust source files
- Crate names use kebab-case: `rbqn-core`, `rbqn-vm`, `rbqn-prim`, `rbqn-gpu`
- Module files match their purpose: `arith_monad.rs`, `arith_dyad.rs`, `structural.rs`

**Directories:**
- `crates/<name>/src/` for source code
- `crates/<name>/src/kernels/` for GPU compute kernel modules

## Where to Add New Code

**New BQN primitive function (e.g. fixing an existing one):**
- Find the category in `crates/rbqn-prim/src/` (arith_monad, arith_dyad, compare, structural, select, slash, search, sort, group)
- Implement the `MonadFn` or `DyadFn` signature: `fn(B, Option<&BqnArr>) -> Result<PrimResult>` or `fn(B, Option<&BqnArr>, B, Option<&BqnArr>) -> Result<PrimResult>`
- Wire it into `get_runtime()` in `crates/rbqn-prim/src/dispatch.rs`

**New BQN modifier implementation:**
- Native modifiers live in `crates/rbqn-vm/src/modifiers.rs` (NOT in rbqn-prim, because they need `c1`/`c2`)
- Add handling in `native_md1_c1`/`native_md1_c2` or `native_md2_c1`/`native_md2_c2`
- Use the constants `MD1_*` / `MD2_*` for index matching

**New system function (e.g. `•BQN`, `•Import`):**
- Add dispatch case in `dispatch_sys_c1`/`dispatch_sys_c2` in `crates/rbqn-vm/src/derive.rs`
- Register the system value index in `sysv_lookup()` in `crates/rbqn-vm/src/vm.rs`
- Create the function via `m_sys_fn(idx)` in `crates/rbqn/src/bootstrap.rs`

**New VM opcode:**
- Add variant to `Op` enum in `crates/rbqn-vm/src/bytecode.rs`
- Set `bc_len()`, `stack_diff()`, `stack_consumed()` for the new op
- Handle in `eval_bc()` match in `crates/rbqn-vm/src/vm.rs`
- If generated by the compiler: add emission logic in `compile_block()` in `crates/rbqn-vm/src/compiler.rs`

**New core type or array operation:**
- Types go in `crates/rbqn-core/src/` -- keep this crate dependency-free
- If the operation needs `c1`/`c2`, it must be in `rbqn-vm`, not `rbqn-core` or `rbqn-prim`

**Tests:**
- Currently no test infrastructure exists. Tests would go in `tests/` or as `#[cfg(test)]` modules within crates.

## Special Directories

**`crates/rbqn/src/embedded/`:**
- Purpose: Contains `mod.rs` which defines `EmbeddedBytecode`, `ObjectEntry`, `BlockEntry` types and uses `include!()` to pull in build-time generated code from `$OUT_DIR/embedded_bytecode.rs`
- Generated: Yes, by `crates/rbqn/build.rs` at compile time
- Committed: The template (`mod.rs`) is committed; the generated file is not (it lives in `target/`)

**`.claude/worktrees/`:**
- Purpose: Git worktrees used for parallel development tracks
- Contains: `track-bootstrap`, `track-core-vm`, `track-gpu`, `track-primitives`, `frolicking-jingling-storm`
- Generated: Yes (git worktrees)
- Committed: The `.claude/` directory is in `.gitignore` or untracked

**`target/`:**
- Purpose: Cargo build artifacts
- Generated: Yes
- Committed: No

---

*Structure analysis: 2026-02-21*
