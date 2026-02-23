# Architecture Research

**Domain:** GPU-accelerated BQN interpreter in Rust
**Researched:** 2026-02-23
**Confidence:** HIGH (derived entirely from reading actual codebase)

---

## Standard Architecture

### System Overview

```
┌──────────────────────────────────────────────────────────────────┐
│                        rbqn (binary)                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────┐     │
│  │ main.rs  │  │ repl.rs  │  │ cli.rs   │  │ bootstrap.rs │     │
│  └────┬─────┘  └────┬─────┘  └──────────┘  └──────┬───────┘     │
│       │             │                              │             │
│       └──────exec_string()──────────┬──────────────┘             │
│                                    │ Runtime{prims,fruntime,     │
│                                    │  runtime,compiler,formatter}│
├────────────────────────────────────┼─────────────────────────────┤
│                    rbqn-vm         │                              │
│  ┌────────────┐  ┌─────────────┐  ┌─────────────┐               │
│  │ compiler.rs│  │    vm.rs    │  │  derive.rs  │               │
│  │ compile_all│  │  eval_bc()  │  │  c1/c2 disp │               │
│  └─────┬──────┘  └──────┬──────┘  └──────┬──────┘               │
│        │                │                │                       │
│  ┌─────┴──────┐  ┌──────┴──────┐  ┌──────┴──────┐               │
│  │  block.rs  │  │   scope.rs  │  │ modifiers.rs│               │
│  │   Block/   │  │ Scope/Env   │  │ ¨ ´ ` ˘ etc │               │
│  │   Body     │  │             │  │             │               │
│  └────────────┘  └─────────────┘  └─────────────┘               │
├──────────────────────────────────────────────────────────────────┤
│                       rbqn-prim                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐         │
│  │arith_monad│  │arith_dyad│  │structural│  │ dispatch │         │
│  │/dyad     │  │          │  │ select   │  │ Primitive│         │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐         │
│  │ search   │  │  sort    │  │  group   │  │  sysfn   │         │
│  │  slash   │  │  fold    │  │ inverse  │  │  md1/md2 │         │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘         │
├──────────────────────────────────────────────────────────────────┤
│                       rbqn-core                                  │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌───────┐  │
│  │ value.rs│  │ array.rs│  │arrstore │  │ error.rs│  │format │  │
│  │  B(u64) │  │ BqnArr  │  │ARR_STORE│  │BqnError │  │squeeze│  │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘  └───────┘  │
├──────────────────────────────────────────────────────────────────┤
│                       rbqn-gpu (isolated)                        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐         │
│  │ context  │  │  buffer  │  │ pipeline │  │ dispatch │         │
│  │GpuContext│  │ GpuBuffer│  │PipeCache │  │threshold │         │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘         │
│  ┌──────────────────────────────────────────────────────┐        │
│  │ kernels/: arith, matmul, softmax, reduce, scan, sort │        │
│  │ shaders/: 15 WGSL files                              │        │
│  └──────────────────────────────────────────────────────┘        │
└──────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Status |
|-----------|----------------|--------|
| `rbqn-core/value.rs` | NaN-boxed `B = u64` type; tag constants; is_fun/is_arr/etc. | Stable, do not change |
| `rbqn-core/array.rs` | `BqnArr` with typed `ArrData` (Bit/I8/I16/I32/F64/C8/C16/C32/Boxed) | Stable |
| `rbqn-core/arrstore.rs` | Global `ARR_STORE: Mutex<HashMap<u64, Arc<BqnArr>>>` — all arrays live here | Stable |
| `rbqn-prim/dispatch.rs` | `Primitive` struct, `get_runtime()` — 64-entry dispatch table | Stable |
| `rbqn-prim/arith_*.rs` | Arithmetic, structural, search, sort, group, select primitives | Mostly stable |
| `rbqn-prim/sysfn.rs` | System functions: Type, Decompose, Fill, GroupLen, GroupOrd | Needs 44+ additions |
| `rbqn-prim/inverse.rs` | Inverse registry for `⁼` — currently a 3-line stub | Needs implementation |
| `rbqn-vm/derive.rs` | `Derived` store, `DerivedKind`, `c1`/`c2` dispatch entry points | Core, stable |
| `rbqn-vm/compiler.rs` | `compile_all()` — converts raw bytecode arrays into `Block` | Stable |
| `rbqn-vm/vm.rs` | `eval_bc()` — opcode interpreter loop, `exec_block*` | Stable |
| `rbqn-vm/modifiers.rs` | `¨ ´ ` ˘ ⌜ ∘ ○ ⊸ ⟜ ◶ ⍟ ⎊`; stubs for `⁼ ⌾ ⎉ ⚇` | 4 missing |
| `rbqn/bootstrap.rs` | 4-stage bootstrap: fruntime → runtime_0 → runtime1 → compiler → formatter | Works (bypass done) |
| `rbqn/build.rs` | Parses CBQN gen/ files into embedded bytecode at compile time | Needs self-hosting path |
| `rbqn/embedded/` | Static bytecode arrays: RUNTIME0, RUNTIME1, COMPILER, FORMATTER | Self-hosting target |
| `rbqn-gpu/context.rs` | `GpuContext` — wgpu device + queue init | Complete, unused |
| `rbqn-gpu/buffer.rs` | `GpuBuffer`, `BufferPool` — GPU buffer management | Complete, unused |
| `rbqn-gpu/pipeline.rs` | `PipelineCache` — compiled shader pipeline caching | Complete, unused |
| `rbqn-gpu/dispatch.rs` | `should_use_gpu(op, len)` — threshold routing logic | Complete, unused |
| `rbqn-gpu/kernels/` | arith, matmul, softmax, reduce, scan, sort, select, unary | Mixed: arith/matmul done, rest scaffolding |

---

## Existing Dependency Graph

```
rbqn-core          (no deps)
    ↑
rbqn-gpu           (rbqn-core + wgpu)
    ↑
rbqn-prim          (rbqn-core + rbqn-gpu)  ← gpu imported but NEVER CALLED
    ↑
rbqn-vm            (rbqn-core + rbqn-prim)
    ↑
rbqn               (rbqn-core + rbqn-vm + rbqn-prim + rbqn-gpu + rustyline)
```

**Key constraint:** `rbqn-prim` depends on `rbqn-gpu` (in Cargo.toml) but imports nothing from it. The GPU integration point is explicitly in `rbqn-prim` — that is where dispatch should happen.

---

## Three Integration Changes Required

### 1. Runtime Bypass (Option B) — Already Partially Done

**What exists:** `bootstrap.rs` already implements Option B: `runtime_0` is built directly from `fruntime` (native Rust primitives) without executing runtime0 bytecode. The bypass is live as of recent commits.

**What the bypass changes:** Instead of running CBQN's runtime0 BQN source to produce 24 "override" functions, it hard-codes the 24 slots from `fruntime`:

```rust
// bootstrap.rs: runtime_0 built from fruntime directly
let runtime_0: Vec<B> = vec![
    fruntime[6],  // ⌊ (floor)
    fruntime[7],  // ⌈ (ceil)
    // ... 24 entries total
];
```

**Then `runtime` is set to `fruntime` too:**

```rust
let runtime: Vec<B> = fruntime.clone(); // all 64 native primitives
```

**What this means:** The broken runtime0 BQN execution path is gone. The runtime array passed to the compiler contains `NativeFn { prim_idx }` derived objects — not BQN FunBlocks. This means `3×4 = 12` should work because `runtime[2]` (×) is the actual native multiply, not a broken BQN-derived wrapper.

**Remaining work for Option B:**
- Verify all 21 compat test expressions pass (currently 3/21)
- Pervasion edge cases (array + scalar, nested boxes) — handled by existing `arith_dyad.rs` pervasive_dyad already
- Fill propagation — currently stub (•fillBy sys_idx 7 returns x unchanged)

---

### 2. GPU Dispatch Integration

**Integration point:** `rbqn-prim/{arith_dyad.rs, arith_monad.rs, structural.rs}` — the hot primitive implementations.

**What needs to be added:**

**A. GpuContext singleton in rbqn-prim:**
The GPU context needs to be initialized once and accessible from primitives. Since `rbqn-prim` already depends on `rbqn-gpu`, this is the right place.

```rust
// New file: rbqn-prim/src/gpu_ctx.rs
use std::sync::OnceLock;
use rbqn_gpu::context::GpuContext;

static GPU_CTX: OnceLock<Option<GpuContext>> = OnceLock::new();

pub fn gpu_ctx() -> Option<&'static GpuContext> {
    GPU_CTX.get_or_init(|| {
        pollster::block_on(GpuContext::new())
    }).as_ref()
}
```

**B. Array transfer layer:**
Convert `ArrData::F64(Vec<f64>)` → `GpuBuffer<f32>` and back. Since SHADER_F64 is not universal, f64 must downcast to f32 for GPU, then upcast back.

```rust
// New file: rbqn-prim/src/gpu_transfer.rs
pub fn f64_slice_to_gpu(ctx: &GpuContext, data: &[f64]) -> GpuBuffer {
    let f32_data: Vec<f32> = data.iter().map(|&x| x as f32).collect();
    GpuBuffer::from_data(&ctx.device, ElementKind::F32, &f32_data)
}

pub fn gpu_to_f64_vec(ctx: &GpuContext, buf: &GpuBuffer, len: usize) -> Vec<f64> {
    let f32_data = buf.read_blocking(&ctx.device, &ctx.queue, len);
    f32_data.iter().map(|&x| x as f64).collect()
}
```

**C. Dispatch hook in hot primitives:**
The threshold logic already exists in `rbqn-gpu/dispatch.rs`. Wire it into arithmetic primitives:

```rust
// In arith_dyad.rs: add_c2 example
pub fn add_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if let (Some(wa), Some(xa)) = (wa, xa) {
        if wa.el_type() == ElType::F64 && xa.el_type() == ElType::F64 {
            let len = wa.ia();
            if rbqn_gpu::dispatch::should_use_gpu("arith", len) {
                if let Some(ctx) = crate::gpu_ctx::gpu_ctx() {
                    return gpu_add(ctx, wa, xa);  // gpu_transfer + arith_binary + readback
                }
            }
        }
    }
    // existing CPU path
    pervasive_dyad(w, wa, x, xa, |a, b| a + b, "+")
}
```

**D. Async bridge:** wgpu's device.poll() can block synchronously. The pattern used by `rbqn-gpu/kernels/arith.rs` already submits commands and relies on the caller to poll. Add `ctx.device.poll(wgpu::Maintain::Wait)` after submit for synchronous readback.

**Primitives to wire first (highest ROI):**
1. `+` (add_c2) — most common in BQN
2. `×` (mul_c2) — matmul path
3. `-` (sub_c2), `÷` (div_c2) — arithmetic
4. `/` with reduction (fold via ´)
5. `⌊` (floor_c1), `⌈` (ceil_c1) — unary GPU path exists

---

### 3. Self-Hosting (Remove CBQN Build Dependency)

**Current state:**
- `build.rs` reads CBQN's `build/*/gen/` directory at compile time
- If `CBQN_PATH` is not set, `embedded::RUNTIME1`, `embedded::COMPILER`, `embedded::FORMATTER` are empty slices
- `bootstrap.rs` early-returns with a sentinel compiler when bytecode is empty

**Target state:**
- `rbqn/src/embedded/` contains pre-generated Rust files with bytecode arrays embedded as `&[i32]`
- `build.rs` is still there but only regenerates when explicitly opted in (or becomes a no-op with pre-committed data)
- `cargo install rbqn` works without any external deps

**Self-hosting build order:**
```
1. Build RBQN with CBQN_PATH pointing to CBQN source
   → generates embedded bytecode files in OUT_DIR

2. Commit those generated files into rbqn/src/embedded/
   → they become the "boot" bytecode

3. Modify build.rs to skip regeneration if embedded/ files exist and CBQN_PATH is unset
   → cargo install rbqn now works standalone

4. Eventually: use RBQN itself to recompile c.bqn → regenerate embedded bytecode
   → true self-hosting achieved
```

**Architectural change:** The `embedded/mod.rs` currently re-exports from `OUT_DIR/embedded_bytecode.rs`. After self-hosting, this file becomes a committed static file rather than a build artifact. The module boundary stays identical — only the source of truth changes.

---

## Data Flow

### Current: User BQN Expression Execution

```
rbqn CLI args / REPL
    ↓
exec_string(rt, code)
    ↓
c2(rt.compiler, comp_args, src_chars) → ⟨bc, objs, blocks, bodies⟩
    ↓
compile_all(bc, objs, blocks, bodies) → Block (IR)
    ↓
eval_fun_block(block, root_scope) → B
    ↓ (on NativeFn)
c1/c2 in derive.rs → prim dispatch → rbqn-prim functions
    ↓
format_result(rt, &val) → String → stdout
```

### Post-Runtime-Bypass: Same Flow, No BQN Wrappers

The bypass eliminates the BQN-defined override wrappers from the execution path. All primitives in `rt.runtime[]` are `NativeFn { prim_idx }` derived objects, dispatching directly to Rust functions. The flow is unchanged — what changes is what `c1/c2` resolves to for each primitive.

### Post-GPU-Integration: Dispatch Fork in Primitives

```
c1/c2 in derive.rs → prim dispatch
    ↓
rbqn-prim function (e.g., add_c2)
    ↓
should_use_gpu("arith", len)?
    ├── YES (len ≥ 100K): gpu_ctx() → f64→f32 → GpuBuffer → kernel → f32→f64 → BqnArr
    └── NO: pervasive_dyad (CPU path)
    ↓
PrimResult → B
```

### Bootstrap Flow (4 Stages)

```
build.rs: parse CBQN gen/ → embedded bytecode arrays (compile time)
    ↓
bootstrap() at startup:
    Stage 0: get_runtime() → Primitive[64]
             (0..64).map(prim_to_b) → fruntime[64]  (NativeFn derived objects)

    Stage 1: build runtime_0[24] from fruntime (NO bytecode execution)
             build_provide(fruntime) → provide[40]

    Stage 2: exec_stage(RUNTIME1, objs using runtime_0, ...) → r1_result
             r1_result[0] = rtObjRaw (BQN-defined runtime, used for rt_under, rt_depth)
             r1_result[1] = setPrims callback → registers •Decompose, •PrimInd
             r1_result[2] = setInv callback → registers inverse tables
             runtime = fruntime (NOT rtObjRaw — native primitives win)

    Stage 3: exec_stage(COMPILER, objs using runtime_0 + runtime, ...)
             → compgen function → c1(compgen, glyphs) → compiler function

    Stage 4: exec_stage(FORMATTER, ...) → fmt_module
             → c1(fmt_module, ⟨Type, Decompose, Glyph, Repr⟩) → (fmt_fn, repr_fn)

    Returns: Runtime { prims, fruntime, runtime, compiler, formatter, glyphs }
```

---

## Component Boundaries: New vs Modified

### New Components Needed

| Component | Location | Purpose | Notes |
|-----------|----------|---------|-------|
| `gpu_ctx.rs` | `rbqn-prim/src/` | Singleton GPU context init | `OnceLock<Option<GpuContext>>`, init on first primitive call |
| `gpu_transfer.rs` | `rbqn-prim/src/` | `BqnArr ↔ GpuBuffer` conversion | f64→f32 downcast on upload, upcast on readback |
| `gpu_prim.rs` | `rbqn-prim/src/` | GPU-accelerated primitive implementations | Thin wrappers calling `rbqn-gpu/kernels/` |
| Embedded bytecode files | `rbqn/src/embedded/` | Pre-committed bytecode for standalone build | Replaces build artifact with committed source |
| `sysfn_io.rs` | `rbqn-prim/src/` | •Out, •Show, •BQN, •Import, •FChars/FLines/FBytes | Currently all missing |
| `sysfn_math.rs` | `rbqn-prim/src/` | •math.* namespace | Missing |

### Modified Components

| Component | Change | Risk |
|-----------|--------|------|
| `rbqn-prim/arith_dyad.rs` | Add GPU dispatch branch in add_c2, mul_c2, sub_c2, div_c2 | Low — additive change, CPU path unchanged |
| `rbqn-prim/arith_monad.rs` | Add GPU dispatch for floor_c1, ceil_c1, exp_c1 | Low — additive |
| `rbqn-prim/dispatch.rs` | Add GPU-aware primitives to get_runtime() if needed | Low |
| `rbqn-prim/sysfn.rs` | Add 44+ system function implementations | Medium — needs each tested individually |
| `rbqn-prim/inverse.rs` | Implement ⁼ inverse table fully | Medium |
| `rbqn-vm/modifiers.rs` | Implement ⎉ (Rank) and ⚇ (Depth) natively | High — complex semantics |
| `rbqn/build.rs` | Add self-hosting mode: skip regen if embedded files committed | Low |
| `rbqn/src/embedded/mod.rs` | Switch from `include!(concat!(env!...))` to committed file | Low |

### Unchanged Components (do not touch)

| Component | Reason |
|-----------|--------|
| `rbqn-core/value.rs` | NaN-boxing is load-bearing — any change breaks everything |
| `rbqn-core/array.rs` | `BqnArr` layout is referenced everywhere |
| `rbqn-core/arrstore.rs` | Global store pattern; change requires GC design |
| `rbqn-vm/compiler.rs` | `compile_all()` is correct and stable |
| `rbqn-vm/vm.rs` | Opcode interpreter is correct; DYNM/ALIM are minor bugs |
| `rbqn-vm/derive.rs` | `DerivedKind`, `c1`, `c2` are stable — bypass did not change these |
| `rbqn-gpu/context.rs` | GpuContext is correct — only needs to be called |
| `rbqn-gpu/kernels/arith.rs` | arith_binary() is implemented — only needs a caller |

---

## Build Order for Milestone v2.0

The dependencies between work items dictate this order:

```
Phase 1: Verify runtime bypass works
    rbqn/bootstrap.rs (bypass already in place)
    → run compat tests → fix remaining failures
    Unblocks: everything that requires a working interpreter

Phase 2: Test infrastructure
    Add test runner for official BQN test suite
    Re-enable rbqn-vm integration tests
    Establishes baseline for subsequent phases

Phase 3: Language completeness
    rbqn-prim/sysfn.rs: •Out, •Show, •BQN (most critical)
    rbqn-prim/sysfn.rs: •FChars, •FLines, •FBytes, •Import
    rbqn-vm/modifiers.rs: ⁼ (Undo) — needed by many BQN idioms
    rbqn-vm/modifiers.rs: ⌾ (Under) — complex, BQN runtime already provides it
    rbqn-vm/modifiers.rs: ⎉ (Rank), ⚇ (Depth) — complex
    Fix DYNM, ALIM opcodes in vm.rs
    Fix REPL state persistence
    Unblocks: passing official BQN test suite

Phase 4: GPU integration
    rbqn-prim/src/gpu_ctx.rs: singleton init
    rbqn-prim/src/gpu_transfer.rs: BqnArr ↔ GpuBuffer
    rbqn-prim/src/gpu_prim.rs: GPU arithmetic
    Wire add_c2, mul_c2, sub_c2, div_c2, floor_c1, ceil_c1
    Benchmarks: CPU vs GPU timing
    Unblocks: GPU demo, performance validation

Phase 5: Self-hosting
    Compile with CBQN_PATH, extract bytecode
    Commit embedded bytecode to rbqn/src/embedded/
    Modify build.rs: skip if embedded files present
    Verify: cargo build WITHOUT CBQN_PATH works
    Unblocks: cargo install rbqn, sharing with Marshall
```

---

## Architectural Patterns

### Pattern 1: NaN-boxed Value Threading

**What:** Every BQN value is a `B(u64)`. Type is encoded in top 16 bits. Heap objects (arrays, derived functions) use the bottom 48 bits as a store key.

**When to use:** Always — all primitive signatures take and return `B`.

**Trade-offs:** Zero-copy for scalars and characters. Arrays require a `Mutex<HashMap>` lookup. Prevents multi-threading. The global store is the primary scalability constraint.

**Example:**
```rust
// All primitives follow this signature pattern:
pub fn add_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult>
// wa/xa are pre-fetched array refs (None if w/x is scalar)
```

### Pattern 2: Derived Object Indirection

**What:** All callable values (functions, modifiers, derived forms) are stored in `DERIVED_STORE` and referenced by a NaN-boxed key. `c1(f, x)` looks up the `Derived` and dispatches on `DerivedKind`.

**When to use:** Every time a new callable value is created (fork, atop, md1/md2 application, block evaluation).

**Trade-offs:** Flexible dispatch. Each call pays a `Mutex::lock()`. Leaks memory (no GC). Not thread-safe.

**Example:**
```rust
// Creating a derived fork: (f g h) → one Derived in store
pub fn m_fork(f: B, g: B, h: B) -> B {
    let d = Derived { kind: DerivedKind::Fork, f, g, h, bl: None, sc: None };
    let id = store_derived(d);
    tagu64(id << 3, FUN_TAG)
}
```

### Pattern 3: Panic-based BQN Error Propagation

**What:** BQN errors use `panic!` (via `rbqn_core::error::throw()`). Callers wrap in `catch_unwind`. This is how CBQN's longjmp error model maps to Rust.

**When to use:** All user-visible errors, type errors, assertion failures.

**Trade-offs:** Works, but ~139 panic sites. Prevents `?` operator use in error paths. Makes GPU async unsafe (panics across async boundaries are UB). Future work: migrate to `Result<B>` throughout.

**Implication for GPU:** GPU kernels must not panic. GPU errors should return a CPU fallback, not propagate through panic.

### Pattern 4: Threshold-Gated GPU Dispatch

**What:** `rbqn-gpu/dispatch.rs` defines `should_use_gpu(op, len)` with configurable thresholds per op type.

**When to use:** Inside hot primitives in `rbqn-prim`. Check before allocating GPU resources.

**Current thresholds:**
```rust
const GPU_THRESHOLD: usize = 100_000;
// sort: 500K, scan/reduce: 50K, all others: 100K
```

**50K elements at f64 = 400KB upload + 400KB download = ~0.8MB transfer.** On Metal (macOS), memory bandwidth is ~200GB/s, so transfer cost is ~4µs. Kernel overhead dominates below 10K elements. The 100K threshold is reasonable for Metal; may need tuning for other backends.

---

## Anti-Patterns

### Anti-Pattern 1: Adding More Runtime0 Debug Iterations

**What people do:** Try to fix the BQN-written runtime0 execution by patching VM opcodes.

**Why it's wrong:** 19 iterations failed. The bypass (Option B) already works and is committed. The override architecture is fundamentally mismatched — CBQN's bytecodeSubmodule build never runs runtime0 as bytecode; it builds from native C functions directly. Running runtime0 BQN source in RBQN's VM will never produce correct results because the BQN source assumes richer runtime context than RBQN provides during stage 0.

**Do this instead:** Keep the bypass. If specific edge cases (fill propagation, pervasion depth) need the BQN runtime's behavior, extract the specific function from `rtObjRaw` (like we do for rt_under and rt_depth) rather than running all of runtime0.

### Anti-Pattern 2: Initializing GpuContext at Startup

**What people do:** Create the GPU device in `bootstrap()` and add it to the `Runtime` struct.

**Why it's wrong:** GPU init on macOS (Metal) takes 100-500ms. This penalizes all users of `rbqn`, including those who never use GPU. Also, `GpuContext::new()` is async, which requires a Tokio runtime at the call site.

**Do this instead:** Initialize lazily in `rbqn-prim` via `OnceLock`. First GPU-eligible primitive call triggers init. Non-GPU users pay zero cost. Use `pollster::block_on()` for the async init call (already a dep of rbqn-gpu).

### Anti-Pattern 3: Storing GpuBuffers in BqnArr

**What people do:** Add a `gpu_data: Option<GpuBuffer>` field to `BqnArr` for "GPU-resident" arrays.

**Why it's wrong:** `BqnArr` is in `rbqn-core` which has no GPU dependency. Adding GPU fields to core violates crate boundaries and forces all users of `rbqn-core` to pull in wgpu. It also complicates Clone semantics (GPU buffers aren't cheap to clone).

**Do this instead:** GPU dispatch is a primitive-level optimization. Primitives take `BqnArr`, upload to GPU, run kernel, download back, return a new `BqnArr`. No GPU state leaks out of the primitive boundary. If future work requires GPU-resident arrays (lazy evaluation pipeline), design a separate `GpuArr` type in `rbqn-gpu` and add it as a variant to `ArrData` via a feature flag.

### Anti-Pattern 4: Implementing Self-Hosting Before Interpreter Works

**What people do:** Try to run `c.bqn` through RBQN before the interpreter passes the BQN test suite.

**Why it's wrong:** The CBQN compiler (`c.bqn`) is ~3000 lines of real BQN that exercises virtually every language feature. If simple expressions like `3×4` still fail, compiling `c.bqn` will immediately crash. This wastes time on a premature milestone.

**Do this instead:** Self-hosting is Phase 5. It's blocked on Phases 1-3 (working interpreter, language completeness). The self-hosting architecture is already correct — just needs the interpreter to actually work first.

### Anti-Pattern 5: Replacing the Mutex Stores Before Adding GC

**What people do:** Replace `Mutex<HashMap>` with lock-free structures for performance.

**Why it's wrong:** The global stores leak memory (no GC). A leaking lock-free store is faster-leaking garbage. The single-threaded constraint is fundamental to the current design — the mutex is not the bottleneck, the lack of GC is. Premature optimization.

**Do this instead:** Add garbage collection (mark-and-sweep or reference counting) first. After GC exists, profile to determine if the store is actually a bottleneck.

---

## Integration Points

### Internal Boundaries

| Boundary | Current | Post-GPU |
|----------|---------|----------|
| `rbqn-prim` → `rbqn-gpu` | Import declared, nothing called | `gpu_ctx()` + `gpu_transfer` + kernel calls |
| `rbqn/bootstrap` → `rbqn-prim` | `get_runtime()` at startup | Unchanged |
| `rbqn/bootstrap` → `rbqn-vm` | `compile_all`, `eval_fun_block` | Unchanged |
| Primitive → PrimResult | Returns `Scalar(B)` or `Array(BqnArr)` | Unchanged — GPU path still returns PrimResult |
| `c1`/`c2` dispatch | Looks up Derived, calls prim fn | Unchanged |

### External Boundaries

| Boundary | Current | Self-Hosting |
|----------|---------|--------------|
| `build.rs` → CBQN gen/ | Required at build time | Optional — committed bytecode used instead |
| `rbqn` binary → CBQN runtime | None (bootstrap only) | None (same) |
| `cargo install rbqn` | Fails without CBQN_PATH | Works standalone |

---

## Scalability Considerations

| Concern | Current | After GPU | Future (GC) |
|---------|---------|-----------|-------------|
| Array size | Unlimited (heap) but no GC → OOM on large programs | GPU path for >100K elements | GC enables reclaiming arrays |
| Concurrency | Single-threaded (Mutex stores) | Single-threaded (wgpu is Send but stores aren't) | Requires Arc store redesign |
| Startup time | ~50ms (bootstrap stages) | ~50ms + GPU init lazily | Same |
| Primitive throughput | CPU-bound | GPU for large numeric arrays | — |

---

## Sources

- `crates/rbqn/src/bootstrap.rs` — actual bootstrap implementation (read 2026-02-23)
- `crates/rbqn-prim/src/dispatch.rs` — 64-entry primitive dispatch table
- `crates/rbqn-gpu/src/{context,buffer,dispatch,kernels/arith}.rs` — GPU infrastructure
- `crates/rbqn-vm/src/derive.rs` — DerivedKind, c1/c2, DERIVED_STORE
- `crates/rbqn-core/src/{value,array}.rs` — B type, BqnArr, ArrData
- `.planning/quick/3-assess-progress-toward-cbqn-compatible-g/PROGRESS.md` — current state assessment
- `.planning/PROJECT.md` — milestone goals and constraints

---
*Architecture research for: RBQN GPU-accelerated BQN interpreter — runtime bypass + GPU integration + self-hosting*
*Researched: 2026-02-23*
