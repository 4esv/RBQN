# Architecture

**Analysis Date:** 2026-02-21

## Pattern Overview

**Overall:** Multi-layered bytecode interpreter with self-hosted bootstrap

**Key Characteristics:**
- NaN-boxed value representation (`B` type) -- all BQN values fit in a single `u64`
- Stack-based virtual machine executing CBQN-compatible bytecode
- Four-stage bootstrap: build.rs parses CBQN C codegen -> runtime0 -> runtime1 -> compiler -> formatter
- Global stores (HashMap behind Mutex) for heap-allocated objects (arrays, derived functions, namespaces)
- Primitives implemented natively in Rust; modifiers dispatch through the VM's `c1`/`c2` call convention

## Crate Dependency Graph

```
rbqn (binary)
  ├── rbqn-core    (zero deps -- foundational types)
  ├── rbqn-vm      (depends on rbqn-core, rbqn-prim)
  ├── rbqn-prim    (depends on rbqn-core, rbqn-gpu)
  └── rbqn-gpu     (depends on rbqn-core, wgpu, bytemuck)
```

`rbqn-vm` depends on `rbqn-prim` for primitive dispatch; `rbqn-prim` does NOT depend on `rbqn-vm`. This means modifier implementations that need to call `c1`/`c2` live in `rbqn-vm` (`crates/rbqn-vm/src/modifiers.rs`), not in `rbqn-prim`.

## Layers

**rbqn-core -- Value Layer:**
- Purpose: Define the fundamental value type `B`, array type `BqnArr`, element types, error types, and the global array store
- Location: `crates/rbqn-core/src/`
- Contains: `B` (NaN-boxed u64), `BqnArr` (shape + typed data), `ArrData` enum, `ElType`, `BqnError`, `ARR_STORE`
- Depends on: nothing (leaf crate)
- Used by: every other crate

**rbqn-prim -- Primitive Layer:**
- Purpose: Implement BQN's 64 primitive functions as native Rust functions
- Location: `crates/rbqn-prim/src/`
- Contains: `Primitive` struct with `c1`/`c2` function pointers, arithmetic, comparison, structural, search, sort, group, slash operations
- Depends on: `rbqn-core`, `rbqn-gpu`
- Used by: `rbqn-vm` (for dispatch), `rbqn` (for bootstrap)

**rbqn-vm -- VM Layer:**
- Purpose: Bytecode compiler, VM interpreter loop, block/scope/derived object management, modifier dispatch
- Location: `crates/rbqn-vm/src/`
- Contains: `eval_bc` (main VM loop), `compile_all` (bytecode compiler), `Derived`/`DerivedKind`, `Block`/`Body`/`Scope`, `Op` enum, modifier implementations
- Depends on: `rbqn-core`, `rbqn-prim`
- Used by: `rbqn` (for execution)

**rbqn-gpu -- GPU Acceleration Layer:**
- Purpose: WebGPU-based compute kernels for array operations (experimental)
- Location: `crates/rbqn-gpu/src/`
- Contains: GPU context management, buffer handling, compute pipelines, kernel fusion, WGSL shaders for arithmetic/reduce/scan/select/sort
- Depends on: `rbqn-core`, `wgpu`, `bytemuck`
- Used by: `rbqn-prim`

**rbqn -- Application Layer:**
- Purpose: Binary entry point, CLI, REPL, bootstrap orchestration, embedded bytecode
- Location: `crates/rbqn/src/`
- Contains: `main.rs`, `bootstrap.rs`, `cli.rs`, `repl.rs`, `embedded/mod.rs`, `build.rs`
- Depends on: all other crates
- Used by: end user

## Data Flow

**Bootstrap Flow:**

1. `build.rs` at compile time reads CBQN's precompiled bytecode from `$CBQN_PATH/build/*/gen/{runtime0,runtime1,compiles,formatter}` (C source files containing integer arrays)
2. `build.rs` parses C source, extracts `iarrs_data[]`, `iarrs_lens[]`, object assignments (`a0p[N] = ...`), block info (`a1p[N] = ...`), body info (`a2p[N] = ...`)
3. Generates `embedded_bytecode.rs` with static arrays: `RUNTIME0`, `RUNTIME1`, `COMPILER`, `FORMATTER` as `EmbeddedBytecode` structs
4. At runtime, `bootstrap()` in `crates/rbqn/src/bootstrap.rs`:
   - Creates 64 `Primitive` entries via `rbqn_prim::get_runtime()`
   - Wraps each as a NaN-boxed `Derived::NativeFn` via `prim_to_b(i)` -> `fruntime[0..63]`
   - Builds 40-entry `provide` array mapping CBQN provide indices to fruntime entries and system functions
   - Executes `RUNTIME0` bytecode with provide -> produces ~24 override functions (`runtime_0`)
   - Executes `RUNTIME1` bytecode (references `runtime_0` via `RuntimePrev(n)`) -> produces 64-entry `runtime` array
   - Executes `COMPILER` bytecode (references both `runtime_0` and `runtime`) -> produces `compgen` function
   - Calls `compgen(glyphs)` to get the actual compiler function
   - Optionally executes `FORMATTER` bytecode -> produces `(fmt, repr)` pair

**User Code Execution Flow:**

1. `exec_string()` in `crates/rbqn/src/main.rs` calls the self-hosted compiler: `c2(rt.compiler, comp_args, src_b)`
2. Compiler returns `[bc, objs, blocks, bodies, indices?, tokenInfo?]`
3. `compile_all()` in `crates/rbqn-vm/src/compiler.rs` transforms source bytecode into optimized internal bytecode (inlining PUSH -> ADDI/ADDU, DFND -> DFND0/1/2, SETH -> SETH1/2, etc.)
4. Creates a root `Block` with nested child blocks, each containing `Body` entries
5. `eval_fun_block()` either executes immediately (if `imm`) or wraps in a `Derived::FunBlock`
6. `eval_bc()` in `crates/rbqn-vm/src/vm.rs` runs the stack-based VM loop

**Primitive Call Flow:**

1. VM encounters `FN1C`/`FN2C` opcode -> calls `c1(f, x)` or `c2(f, w, x)`
2. `c1`/`c2` in `crates/rbqn-vm/src/derive.rs` extracts the `Derived` object from the global store
3. Dispatches based on `DerivedKind`:
   - `NativeFn` -> looks up `Primitive.c1`/`.c2` from `rbqn_prim::get_runtime()`, calls it
   - `FunBlock` -> `exec_block_with_args()` with `[self, x, w]`
   - `Md1D`/`Md2D` -> unwraps modifier, dispatches to block or native modifier
   - `Fork`/`Atop` -> recursive `c1`/`c2` calls per train semantics
   - `SysFn` -> `dispatch_sys_c1`/`dispatch_sys_c2` for system values (Type, Fill, Glyph, Decompose, GroupLen, GroupOrd)

**State Management:**
- All mutable state is in global `LazyLock<Mutex<HashMap>>` stores:
  - `ARR_STORE` in `crates/rbqn-core/src/arrstore.rs` -- maps `u64` IDs to `BqnArr`
  - `DERIVED_STORE` in `crates/rbqn-vm/src/derive.rs` -- maps `u64` IDs to `Arc<Derived>`
  - `NS_STORE` in `crates/rbqn-vm/src/namespace.rs` -- maps `u64` IDs to `Arc<NS>`
  - `GID_MAP`/`GID_NAMES` in `crates/rbqn-vm/src/namespace.rs` -- global interned field names
- Scopes (`Scope`) are `Arc`-wrapped and form a linked list via `psc: Option<Arc<Scope>>`
- Variable mutation uses `Arc::make_mut()` for copy-on-write semantics

## Key Abstractions

**B (NaN-boxed value):**
- Purpose: Universal BQN value representation in a single `u64`
- Location: `crates/rbqn-core/src/value.rs`
- Pattern: Upper 16 bits encode the type tag; lower 48 bits encode the payload
- Tags: `FUN_TAG=0xFFF4`, `ARR_TAG=0xFFF7`, `MD1_TAG=0xFFF2`, `MD2_TAG=0xFFF3`, `NSP_TAG=0xFFF5`, `C32_TAG=0x7FF1`, `TAG_TAG=0x7FF2`, `VAR_TAG=0x7FF3`, `EXT_TAG=0x7FF4`
- f64 values stored directly as IEEE 754 bits (NaN range is reserved for tags)
- Special sentinels: `SENTINEL=0x7FF2000000000000` (Nothing/`·`), `NO_VAR=0x7FF2000000000001`, `OPT_OUT=0x7FF2000000000002`, `NO_FILL=0x7FF2000000000003`
- Heap objects (arrays, derived, namespaces) store a `u64` ID in the payload, shifted left by 3 bits

**BqnArr (typed array):**
- Purpose: BQN's array value with shape and typed storage
- Location: `crates/rbqn-core/src/array.rs`
- Pattern: `{ shape: Vec<usize>, data: ArrData, fill: Option<B> }`
- `ArrData` enum: `Bit(Vec<u64>)`, `I8`, `I16`, `I32`, `F64`, `C8`, `C16`, `C32`, `Boxed(Vec<B>)`
- Arrays are stored in `ARR_STORE` and referenced via NaN-boxed `B` with `ARR_TAG`

**Derived (callable objects):**
- Purpose: Represents all callable BQN values -- primitives, blocks, trains, modifier applications
- Location: `crates/rbqn-vm/src/derive.rs`
- Pattern: `{ kind: DerivedKind, f: B, g: B, h: B, bl: Option<Arc<Block>>, sc: Option<Arc<Scope>> }`
- `DerivedKind` variants: `Fork`, `Atop`, `Md1D`, `Md2D`, `Md2PartialL`, `Md2PartialR`, `FunBlock`, `Md1Block`, `Md2Block`, `NativeFn{prim_idx}`, `NativeMd1{prim_idx}`, `NativeMd2{prim_idx}`, `SysFn{sys_idx}`
- Stored in `DERIVED_STORE`, referenced via NaN-boxed `B` with `FUN_TAG`, `MD1_TAG`, or `MD2_TAG`

**Block / Body (compiled code):**
- Purpose: Compiled bytecode unit. A Block contains the bytecode and metadata; Bodies are entry points within it.
- Location: `crates/rbqn-vm/src/block.rs`
- Block: `{ comp: Arc<Comp>, ty: u8, imm: bool, bc: Vec<i32>, blocks: Vec<Arc<Block>>, bodies: Vec<Arc<Body>>, dy_body: Option<Arc<Body>> }`
- Body: `{ bc_offset: usize, var_am: u16, max_stack: u32, max_psc: u16, ns_desc: Option<Arc<NSDesc>> }`
- Block types: `ty=0` (function), `ty=1` (1-modifier), `ty=2` (2-modifier)
- `imm=true` means the block executes immediately at definition time (like immediate modifiers)
- `bodies[0]` is swapped to be the first monadic body after compilation; `dy_body` holds the dyadic body if different

**Scope (variable storage):**
- Purpose: Holds variable bindings for a block execution
- Location: `crates/rbqn-vm/src/scope.rs`
- Pattern: `{ psc: Option<Arc<Scope>>, body: Arc<Body>, var_am: u16, ext: Option<ScopeExt>, vars: Vec<B> }`
- Variable layout for functions: `[self, x, w, ...locals]`
- Variable layout for deferred 1-mod: `[self, x, w, _m_, f, ...locals]`
- Variable layout for deferred 2-mod: `[self, x, w, _m_, f, g, ...locals]`
- Scope chain traversed via `psc` links; `pscs` array built at eval start for O(1) depth access

**Primitive (native function):**
- Purpose: A single BQN primitive with monadic and dyadic implementations
- Location: `crates/rbqn-prim/src/dispatch.rs`
- Pattern: `{ name: &'static str, glyph: &'static str, c1: Option<MonadFn>, c2: Option<DyadFn> }`
- `MonadFn = fn(B, Option<&BqnArr>) -> Result<PrimResult>`
- `DyadFn = fn(B, Option<&BqnArr>, B, Option<&BqnArr>) -> Result<PrimResult>`
- `PrimResult` is either `Scalar(B)` or `Array(BqnArr)`
- 64 primitives total: indices 0-43 are functions, 44-52 are 1-modifiers, 53-63 are 2-modifiers

## Entry Points

**Binary entry (`main`):**
- Location: `crates/rbqn/src/main.rs`
- Triggers: Command-line invocation
- Responsibilities: Parse CLI args, run bootstrap, dispatch to REPL/eval/file execution

**Bootstrap:**
- Location: `crates/rbqn/src/bootstrap.rs` -> `bootstrap()`
- Triggers: Called once at startup from `main()`
- Responsibilities: Build fruntime, provide array, execute runtime0/1/compiler/formatter stages, return `Runtime` struct

**VM interpreter loop:**
- Location: `crates/rbqn-vm/src/vm.rs` -> `eval_bc()`
- Triggers: Every block execution (from `exec_block`, `exec_block_with_args`, `eval_fun_block`)
- Responsibilities: Fetch-decode-execute loop over bytecode, manage stack, handle variable access, dispatch function calls

**Bytecode compiler:**
- Location: `crates/rbqn-vm/src/compiler.rs` -> `compile_all()`
- Triggers: Bootstrap stage execution and user code compilation
- Responsibilities: Transform source bytecode into optimized internal bytecode, build Block/Body tree, resolve variable references, inline constants

**Function/modifier dispatch:**
- Location: `crates/rbqn-vm/src/derive.rs` -> `c1()`, `c2()`
- Triggers: Every function call in BQN (FN1C/FN2C opcodes, recursive from modifiers/trains)
- Responsibilities: Look up `Derived` by ID, dispatch to appropriate handler (native, block, train, modifier, system fn)

## VM Execution Model

**Stack Machine:**
- The VM is a stack-based interpreter. Each `Body` specifies `max_stack` (pre-computed by the compiler).
- Opcodes push/pop from `Vec<B>`. Stack effects documented in `crates/rbqn-vm/src/bytecode.rs` -> `stack_diff()`.

**Opcode Categories:**
- Constants: `ADDI` (push immediate value), `ADDU` (push immediate non-value), `PUSH` (transformed to ADDI/ADDU by compiler)
- Calls: `FN1C`/`FN2C` (call), `FN1O`/`FN2O` (optional-arg call), plus inline variants (`FN1Ci`, `FN2Ci`, etc.)
- Modifiers: `MD1C`, `MD2C` (apply modifier), `MD2L`/`MD2R` (partial application)
- Trains: `TR2D` (2-train/atop), `TR3D`/`TR3O` (3-train/fork)
- Variables: `VARO`/`VARM`/`VARU` (read/mutable-ref/read-and-clear), `EXTO`/`EXTM`/`EXTU` (scope extension)
- Assignment: `SETN`/`SETU`/`SETM`/`SETC` plus immediate and void variants
- Headers/predicates: `SETH1`/`SETH2`, `PRED1`/`PRED2`
- Blocks: `DFND0`/`DFND1`/`DFND2` (define function/1-mod/2-mod block)
- Arrays: `LSTO`/`LSTM` (list), `ARMO`/`ARMM` (merge array)
- Namespace: `FLDO`/`FLDM`/`FLDG`/`ALIM`
- Control: `RETN` (return), `RETD` (return namespace/destructure), `POPS`, `CHKV`, `VFYM`, `FAIL`
- System: `SYSV` (system value), `DYNO`/`DYNM` (dynamic variable)

**Bytecode Compilation (compiler.rs):**
The internal compiler transforms CBQN-format bytecode into an optimized form:
- `PUSH idx` -> `ADDI val_lo val_hi` or `ADDU val_lo val_hi` (inline the constant)
- `DFND idx` -> `DFND0/DFND1/DFND2 block_idx 0` (typed block definition)
- `VARO/VARM/VARU` -> possibly promoted to `EXTO/EXTM/EXTU` if accessing REPL scope extension
- `SETH` -> `SETH1` (immediate) or `SETH2` (deferred, with mono+dy body pointers)
- `PRED` -> `PRED1` (immediate) or `PRED2` (deferred)
- Bodies with predicates get argument remapping preamble

## Error Handling

**Strategy:** Panic-based error propagation with `catch_unwind` at boundaries

**Patterns:**
- `rbqn_core::error::throw(msg)` panics with a `BqnError::Domain` -- used throughout the VM and primitives for unrecoverable errors
- Typed throw variants: `throw_type()`, `throw_rank()`, `throw_shape()`, `throw_nyi()`
- Primitive functions return `Result<PrimResult>` -- errors are converted to panics at the dispatch boundary in `derive.rs`
- BQN's `Catch` modifier (`⎊`) uses `std::panic::catch_unwind()` in `crates/rbqn-vm/src/modifiers.rs`
- The formatter call in `main.rs` also uses `catch_unwind` for resilience

## Cross-Cutting Concerns

**Logging:** No structured logging. Debug output via `eprintln!` and `cargo:warning=` in build.rs.

**Validation:** The bytecode compiler validates bytecode bounds and body offsets. The VM validates stack operations at runtime. Primitives validate argument types and shapes via `Result` returns.

**Memory Management:** All heap objects (arrays, derived, namespaces) use global `LazyLock<Mutex<HashMap>>` stores with monotonic `AtomicU64` IDs. There is no garbage collection -- objects are never freed. `Arc` provides shared ownership for scopes and blocks.

**Thread Safety:** All global stores are behind `Mutex`. The `B` type is `Copy`. `BqnArr` is `Clone`. However, the panic-based error handling via `catch_unwind` requires `UnwindSafe`, handled with `AssertUnwindSafe` wrappers.

---

*Architecture analysis: 2026-02-21*
