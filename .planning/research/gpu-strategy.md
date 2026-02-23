# GPU Acceleration Strategy for RBQN

**Domain:** GPU compute for array programming language runtime
**Researched:** 2026-02-21
**Overall confidence:** MEDIUM

---

## Executive Summary

GPU acceleration for array languages is a well-studied but rarely production-deployed area. The theoretical fit is excellent -- BQN's whole-array semantics map directly to GPU parallelism. The practical challenge is that CPU SIMD implementations (like CBQN's) are already very fast for most array sizes users encounter, and GPU dispatch overhead creates a crossover point that only large arrays (50K+ elements) reliably exceed.

RBQN already has a solid `rbqn-gpu` crate foundation using wgpu with WGSL compute shaders. The existing implementation covers the right kernel categories (element-wise arithmetic, reductions, scans, radix sort, gather/select) and has basic buffer pooling and kernel fusion. The main gaps are: threshold tuning based on actual benchmarks, f64 support (BQN is f64-native but wgpu only supports f32 natively), proper integration with the VM dispatch path, and more sophisticated fusion.

The recommended strategy is threshold-gated lazy dispatch: keep arrays on CPU by default, dispatch to GPU only when array size exceeds empirically-determined thresholds per operation category, and use kernel fusion to amortize dispatch overhead when multiple operations chain together on large arrays.

---

## 1. wgpu Compute Shaders for Array Operations

### Current State of wgpu (Confidence: HIGH)

wgpu is the correct choice for this project. It provides:

- **Cross-platform GPU access** via Metal (macOS), Vulkan, DX12, and WebGPU backends
- **WGSL compute shaders** as the primary shader language (stable, well-documented)
- **Subgroup operations** (landed in wgpu via PR #5301, spec CR January 2025) enabling warp-level reductions that are 2-3x faster than workgroup barrier approaches
- **Timestamp queries** via `wgpu::Features::TIMESTAMP_QUERY` for profiling kernel execution time in nanoseconds

**What the existing rbqn-gpu crate does well:**
- GpuContext initialization with Metal preference on macOS
- Buffer pool with size-based reuse (acquire/release pattern)
- Pipeline cache avoiding redundant shader compilation
- Separate WGSL shaders per type (f32, i32) with workgroup size 256
- Multi-pass reduction with workgroup-level shared memory
- Blelloch exclusive scan (work-efficient, O(n) work)
- Radix sort with 4-bit digits, 8 passes
- Kernel fusion builder generating dynamic WGSL for chained element-wise ops

**What needs improvement:**

1. **f64 gap**: BQN uses f64 natively. wgpu's `ShaderF64` feature exists but requires `wgpu::Features::SHADER_F64` and is not universally supported (missing on many mobile GPUs, some integrated GPUs). Strategy: use f32 for GPU path when precision is acceptable, fall back to CPU for f64-requiring operations. This is the single biggest architectural tension.

2. **Buffer pool fragmentation**: Current pool does `swap_remove` on first buffer >= requested size. Should use best-fit or size-bucketed approach to reduce waste.

3. **No async pipeline**: All dispatches currently block on `queue.submit()`. Should batch command encoders and submit once per "GPU burst" to reduce submission overhead.

4. **Missing uniform buffer pooling**: `arith_scalar` creates a new 4-byte uniform buffer per call. Should pool these.

Sources:
- wgpu docs: https://docs.rs/wgpu/latest/wgpu/
- Subgroup tracking: https://github.com/gfx-rs/wgpu/issues/5555
- wgpu-profiler: https://github.com/Wumpf/wgpu-profiler

### WGSL vs rust-gpu (Confidence: MEDIUM)

rust-gpu compiles Rust to SPIR-V, letting you write GPU kernels in Rust. Tempting for type safety and code sharing. **Do not use it for this project.** Reasons:

- SPIR-V output requires Vulkan backend; does not work with Metal directly (macOS is the developer's primary platform)
- Experimental status, frequent breaking changes
- WGSL is simpler to generate dynamically (critical for kernel fusion)
- The existing WGSL shaders are straightforward and maintainable

Source: https://github.com/Rust-GPU/rust-gpu

---

## 2. Prior Art: GPU Array Language Implementations

### Futhark (Confidence: HIGH)

Futhark is the gold standard for GPU-targeting array language compilation. Key lessons:

- **Fusion is the single most important optimization.** Futhark's compiler performs vertical (producer-consumer) and horizontal fusion on a dependency graph. `map f (map g A)` becomes one kernel. `map` + `reduce` fuses into `redomap`.
- **Incremental flattening** handles nested parallelism by selecting parallelism granularity at runtime via threshold parameters.
- **Auto-tuning** (`futhark-autotune`) determines optimal thresholds empirically per GPU.
- **Small arrays (<=5 elements) should never go to GPU.** Futhark recommends tuples/records for tiny constant arrays.
- **Free operations**: slicing, take, drop, transpose, reverse are index-space transformations with zero cost (unless materialized).

Implication for RBQN: Futhark proves that fusion + threshold tuning is the winning strategy. But Futhark is a *compiler* that generates GPU code ahead of time. RBQN is an *interpreter* with a JIT-like dispatch path, so we need runtime fusion decisions, not compile-time.

Sources:
- https://futhark.readthedocs.io/en/latest/performance.html
- https://futhark-lang.org/publications/pldi17.pdf

### Co-dfns / ArrayFire (Confidence: MEDIUM)

Co-dfns is an APL compiler targeting ArrayFire as its GPU backend. Key insights:

- Uses ArrayFire's JIT engine which automatically fuses element-wise operations into single CUDA/OpenCL kernels
- Works best for "data sizes that exceed 1-10 MB" (roughly 125K-1.25M f64 elements)
- Co-dfns achieved "a maximum factor of 6 improvement with the GPU translation"
- Marshall Lochbaum's analysis: GPU acceleration for array languages is theoretically elegant but practically limited because "the BQN compiler runs at 15MB/s" -- most programs are not bottlenecked on array operations

ArrayFire itself has Rust bindings (`arrayfire` crate) but adds a heavy C++ dependency. **Not recommended** for RBQN -- wgpu is already integrated and more portable.

Sources:
- https://mlochbaum.github.io/BQN/implementation/codfns.html
- https://aplwiki.com/wiki/Co-dfns

### Burn Framework (Confidence: HIGH)

Burn (Rust tensor framework) pioneered "Tensor Operation Streams" for runtime kernel fusion:

- Operations are queued lazily into a stream
- The stream processor detects fusible sequences and JIT-compiles fused kernels
- Uses Rust's ownership system to determine tensor lifetimes (no GC needed)
- Maintains execution plans that are cached and even serializable to disk
- **Currently fuses only element-wise ops** (not reductions, matmuls, convolutions)
- Uses wgpu as a first-party backend

This is the closest prior art to what RBQN should do. The stream-based approach translates directly to BQN's expression evaluation.

Source: https://burn.dev/blog/fusion-tensor-operation-streams/

---

## 3. Threshold-Based Dispatch

### The Crossover Problem (Confidence: MEDIUM)

GPU dispatch is not free. Overhead sources:

| Source | Approximate Cost |
|--------|-----------------|
| Kernel launch overhead | 50-100 us |
| Buffer upload (CPU->GPU) | 1-10 us per KB |
| Buffer download (GPU->CPU) | 5-20 us per KB |
| wgpu command encoder + submit | 10-50 us |
| Pipeline lookup (cached) | < 1 us |
| Bind group creation | 5-20 us |

A CPU SIMD addition over 100K f64 elements takes roughly 12-25 us (at ~4 ns/element with AVX2). The GPU can do it in ~5 us of compute but spends 100+ us on dispatch overhead.

**Crossover estimates by operation category:**

| Operation | Estimated GPU Crossover | Rationale |
|-----------|------------------------|-----------|
| Element-wise arithmetic (single) | ~500K elements | CPU SIMD is very fast; single GPU dispatch can't amortize overhead |
| Fused element-wise chain (3+ ops) | ~50K elements | Fusion saves multiple CPU passes; GPU amortizes dispatch over more compute |
| Reduction (fold) | ~100K elements | GPU reduction needs multiple passes; CPU sequential is simple |
| Scan (prefix sum) | ~50K elements | GPU Blelloch scan is O(n) work, good parallelism |
| Sort | ~500K elements | GPU radix sort has high constant factor; CPU sort is heavily optimized |
| Table (outer product) | ~1K per dimension (~1M total) | Quadratic work makes GPU worthwhile earlier |
| Gather/Select | ~200K elements | Memory-bound; GPU memory bandwidth wins for large arrays |
| Group/GroupOrd | ~200K elements | Histogram + scatter pattern maps well to GPU |

**Critical caveat:** These are rough estimates. The existing `dispatch.rs` uses 100K as the base threshold and 500K for sort, which is in the right ballpark but should be tuned empirically with `wgpu-profiler` timestamp queries on actual hardware.

### Recommended Dispatch Strategy

```
fn should_dispatch_gpu(op: OpCategory, len: usize, data_on_gpu: bool) -> bool {
    if data_on_gpu {
        // Data already on GPU -- always cheaper to compute there
        // Only download when result is needed by CPU-only operation
        return len > 64; // trivially small arrays not worth even GPU-local dispatch
    }
    len >= threshold_for(op)
}
```

The key insight: **once data is on the GPU, keep it there.** The upload/download cost is the dominant overhead. If a chain of operations all exceed threshold individually, the second and subsequent operations should use GPU unconditionally since transfer is already paid.

### Profiling Infrastructure

Use `wgpu-profiler` or raw timestamp queries to measure actual kernel execution times. Build an auto-tuner that:
1. Runs each kernel type at multiple array sizes (1K, 10K, 100K, 1M, 10M)
2. Compares against CPU baseline
3. Stores optimal thresholds per (GPU model, operation)
4. Falls back to conservative defaults on unknown hardware

Source: https://github.com/Wumpf/wgpu-profiler

---

## 4. Kernel Fusion

### Current State in rbqn-gpu (Confidence: HIGH)

The existing `FusionBuilder` generates dynamic WGSL for chained element-wise operations. It supports:
- Binary ops: Add, Sub, Mul, Div
- Scalar broadcast: ScalarAdd, ScalarMul

This is a good start but limited.

### What to Add

**Tier 1 -- Element-wise fusion (extend existing builder):**
- Comparison ops: Eq, Ne, Lt, Le, Gt, Ge (output boolean/u32)
- Math functions: Abs, Neg, Floor, Ceil, Sqrt, Exp, Log (monadic)
- Power, Modulo (dyadic)
- Boolean ops: And, Or, Not
- Type coercion within fused kernel (e.g., i32 input -> f32 intermediate -> i32 output)

**Tier 2 -- Map-reduce fusion (new pattern):**
- Fuse element-wise computation with a trailing reduction
- Example: `+/ a - b` should be one kernel (compute `a-b` per element, then workgroup reduce)
- This matches Futhark's `redomap` pattern
- Requires extending the fusion builder to emit a reduce epilogue

**Tier 3 -- Scan fusion:**
- Fuse element-wise computation with a trailing scan
- Example: `+` a * b` (scan of element-wise product)
- Less common but still valuable

**Tier 4 -- Multi-output fusion (horizontal):**
- When the same input feeds multiple independent operations
- Example: `(+/ x), (-/ x)` should read `x` once
- Harder to detect at the VM level

### Fusion Detection Strategy

Burn's approach (lazy stream + ownership tracking) is the right model. In RBQN terms:

1. When a BQN primitive operates on an array, check if result will be consumed by another primitive before any non-GPU operation
2. If yes, record the operation in a fusion queue instead of executing
3. When the chain terminates (result is printed, stored to variable used elsewhere, or hits a non-fusible operation), compile and execute the fused kernel

The VM already evaluates a bytecode stream. The dispatch path in `rbqn-prim` should check for fusible sequences by peeking ahead in the bytecode or by lazily deferring execution.

---

## 5. Memory Management

### Buffer Pooling (Confidence: HIGH)

The existing `BufferPool` is functional but needs refinement:

**Size bucketing:**
```rust
// Instead of linear scan for first-fit, use power-of-2 buckets
const BUCKET_SIZES: &[usize] = &[
    256, 1024, 4096, 16384, 65536, 262144, 1048576, 4194304, 16777216
];
```

**Pool limits:**
- Cap total GPU memory usage (e.g., 25% of available VRAM or 512 MB, whichever is smaller)
- LRU eviction when pool is full
- Track pool memory usage for diagnostics

**Staging buffer reuse:**
- The current `download_raw` creates a new staging buffer per download
- Pool staging buffers by size bucket

### Transfer Minimization

**Lazy materialization:**
- Represent GPU-resident arrays as a special variant of the B (NaN-boxed value) type
- Tag: could use an unused NaN tag like `0xFFF5` for GPU array references
- The GPU buffer handle is stored in a global table (similar to DERIVED_STORE)
- CPU materialization only happens on: print, element access, reshape requiring CPU logic, or passing to a non-GPU primitive

**Write-combining uploads:**
- When multiple small arrays need uploading for a single kernel, combine into one `write_buffer` call with offsets
- Use `mapped_at_creation: true` for initial large uploads to avoid an extra copy

**Double buffering:**
- For iterative algorithms (e.g., sort with multiple passes), alternate between two buffers to avoid allocation churn
- The existing sort implementation already does this with `buf_a`/`buf_b` swap

### The f64 Problem

BQN values are f64. wgpu's `ShaderF64` is not universally available. Options:

1. **f32 approximation** (current approach): Cast to f32 for GPU, accept precision loss. Fine for integer-valued operations, sorting, comparisons. Unacceptable for scientific computing.

2. **Emulated f64 via two f32s** (double-single arithmetic): Each f64 is represented as `(hi: f32, lo: f32)` where `hi + lo = original`. Adds ~4x compute overhead but maintains ~48 bits of precision. Used by some GLSL-era libraries.

3. **Feature-gated native f64**: Check `adapter.features().contains(wgpu::Features::SHADER_F64)` at runtime. If available, use f64 shaders. Otherwise, fall back to f32 or CPU. This is the cleanest approach.

**Recommendation:** Option 3 with f32 fallback. For integer arrays (i8, i16, i32), GPU acceleration is straightforward. For f64 arrays, only dispatch to GPU when native f64 is available OR when the operation is precision-insensitive (comparisons, sorting by rank, boolean operations).

---

## 6. Which BQN Primitives Benefit Most from GPU

### Tier 1: Strong GPU Candidates (Confidence: HIGH)

These operations have high arithmetic intensity or embarrassing parallelism:

| Primitive | BQN | Why GPU Wins |
|-----------|-----|-------------|
| Arithmetic (dyadic) | `+ - x ÷ ⋆ \| ⌊ ⌈` | Embarrassingly parallel, pure map |
| Comparison | `< > ≤ ≥ = ≠` | Embarrassingly parallel, pure map |
| Table | `f⌜` | O(n*m) work, maps to 2D dispatch |
| Each (flat) | `f¨` when f is arithmetic | Same as map when f is known |
| Scan | `` +` ×` ⌈` ⌊` `` | Blelloch scan is well-studied on GPU |
| Sort/Grade | `∧ ∨ ⍋ ⍒` | GPU radix sort scales well for large arrays |
| Replicate | `/` | Parallel prefix sum + scatter |
| Where | `/` (monadic) | Parallel prefix sum |

### Tier 2: Moderate GPU Candidates (Confidence: MEDIUM)

| Primitive | BQN | Notes |
|-----------|-----|-------|
| Fold | `+´ ×´ ⌈´ ⌊´` | GPU reduction needs multiple passes; wins only for very large arrays |
| Group | `⊔` | Histogram + scatter; complex but parallelizable |
| Select | `⊏` | Gather is memory-bound; GPU wins on bandwidth |
| Search | `⊐ ∊` | Hash-based on CPU is hard to beat; radix-based on GPU is viable for sorted data |
| Bins | `⍋⍒` (dyadic) | Binary search parallelizes well |

### Tier 3: Poor GPU Candidates (Confidence: HIGH)

| Primitive | BQN | Why CPU Wins |
|-----------|-----|-------------|
| Take/Drop | `↑ ↓` | Index-space transform, zero-cost on CPU |
| Reshape | `⥊` | Metadata change, no compute |
| Transpose | `⍉` | Memory-bound, complex access pattern |
| Enclose/Merge | `< >` | Structural, not data-parallel |
| Depth/Shape | `≡ ≢` | O(1) metadata lookup |
| Under | `⌾` | Control flow, not data-parallel |
| Each (nested) | `f¨` on nested arrays | Irregular parallelism, varying inner sizes |
| Fold (non-assoc) | `f´` for non-associative f | Sequential dependency |

### Where GPU Really Shines: Compound Expressions

The biggest GPU wins come not from individual primitives but from *chains* of operations on large arrays:

```bqn
(+/ a×b)          # dot product: fused multiply + reduce
(a ⌈ b) - (a ⌊ b) # range: fused with 2 outputs from shared inputs
+` (n ⥊ 1)        # prefix sum of ones: scan
∧ a +⌜ b          # sorted outer-product sums: table + sort (2D)
```

Each of these, if fused, avoids intermediate array materialization and amortizes GPU dispatch overhead across multiple operations.

---

## 7. Architecture Recommendations

### Integration Point

The GPU dispatch should be transparent to the VM. Recommended integration:

```
VM bytecode execution
  -> rbqn-prim dispatch
     -> check array size against threshold
        -> if GPU: check if fusible with next op(s)
           -> if fusible: queue to fusion builder
           -> if not: dispatch single GPU kernel
        -> if CPU: normal SIMD path
```

### Lazy GPU Array Type

Add a GPU-resident array variant to the value system:

```rust
enum ArrayStorage {
    Cpu(Vec<u8>),           // current: data in CPU memory
    Gpu(GpuBufferHandle),   // new: data lives on GPU
    GpuPending(FusionPlan), // new: data doesn't exist yet, will be computed by fused kernel
}
```

`GpuPending` is the lazy evaluation variant. The fused kernel only executes when the result is consumed by something that needs concrete data.

### Module Organization

The existing crate structure is good. Suggested additions:

```
rbqn-gpu/
  src/
    context.rs      -- [exists] GPU device/queue management
    buffer.rs       -- [exists] Buffer types and pooling (extend: size buckets, f64 handling)
    pipeline.rs     -- [exists] Shader module and pipeline caching
    dispatch.rs     -- [exists] Threshold logic (extend: per-hardware tuning, GPU-resident awareness)
    fusion.rs       -- [exists] Element-wise fusion builder (extend: map-reduce, multi-output)
    lazy.rs         -- [new] Lazy GPU array type, deferred execution
    profiler.rs     -- [new] Timestamp query integration, auto-tuner
    kernels/
      arith.rs      -- [exists] Element-wise arithmetic
      reduce.rs     -- [exists] Parallel reduction
      scan.rs       -- [exists] Prefix scan (Blelloch)
      sort.rs       -- [exists] Radix sort
      select.rs     -- [exists] Gather/scatter
      table.rs      -- [new] Outer product (2D dispatch)
      replicate.rs  -- [new] Prefix sum + scatter for /
      search.rs     -- [new] Parallel membership/index-of
      compare.rs    -- [new] Comparison operations
    shaders/
      *.wgsl        -- [exists] Per-type shader files
```

---

## 8. Pitfalls

### Critical

**f64 precision loss.** BQN programs expect f64 semantics. Silently using f32 will produce wrong results for programs that depend on precision beyond ~7 decimal digits. Must either gate on native f64 support or clearly document the precision tradeoff.

**GPU dispatch overhead dominates.** For typical interactive BQN usage (arrays under 10K elements), GPU will be slower than CPU every time. The GPU path must be purely additive -- if threshold check says CPU, there should be zero GPU overhead (no buffer allocation, no pipeline lookup).

**Atomic sort correctness.** The existing `sort_i32` does NOT properly handle negative numbers (comment says "treat as unsigned sort, correct for non-negative values"). Must implement sign-bit flip for correct signed sorting.

### Moderate

**WGSL compilation latency.** First use of a shader module triggers compilation that can take 10-100ms. The pipeline cache helps for repeated ops but cold starts are painful. Consider pre-warming common pipelines at context creation.

**Buffer pool memory leaks.** The current pool never shrinks. Long-running programs could accumulate large unused GPU allocations. Need periodic or LRU cleanup.

**Fusion correctness with side effects.** If a BQN operation between two fusible ops has side effects (print, assign), the fusion must be broken. The VM integration needs careful tracking of operation boundaries.

**Subgroup portability.** Subgroup operations (2-3x faster reductions) require hardware support. The existing `has_subgroup_ops()` check is correct but no shaders currently use subgroups. Dual shader paths (with/without subgroups) add maintenance burden.

### Minor

**Metal atomics.** On macOS/Metal, atomic operations in compute shaders have different performance characteristics than Vulkan. The radix sort scatter phase uses atomics and may need Metal-specific tuning.

**Workgroup size.** The fixed 256 is reasonable for most operations but suboptimal for reductions on some hardware. Consider 64 (matches AMD wavefront) as an alternative.

---

## 9. Implementation Phases

### Phase 1: Benchmark Infrastructure
- Add timestamp query profiling to existing kernels
- Benchmark CPU vs GPU crossover for each kernel type at various array sizes
- Tune thresholds based on measurements
- Fix sort_i32 sign-bit handling

### Phase 2: VM Integration
- Add GPU-resident array type to value system
- Implement transparent dispatch in rbqn-prim
- Keep-on-GPU logic for chained operations
- CPU fallback for all operations

### Phase 3: Extended Fusion
- Add comparison and math ops to fusion builder
- Implement map-reduce fusion (element-wise + trailing fold)
- Detect fusible sequences in VM dispatch path

### Phase 4: New Kernels
- Table (outer product) with 2D dispatch
- Replicate (/) via prefix sum + scatter
- Parallel search primitives

### Phase 5: Optimization
- Size-bucketed buffer pool with LRU eviction
- Subgroup-accelerated reduction/scan shaders
- f64 support gated on SHADER_F64 feature
- Auto-tuner for per-hardware thresholds

---

## Sources

### High Confidence (official docs, established projects)
- wgpu API docs: https://docs.rs/wgpu/latest/wgpu/
- wgpu subgroup tracking: https://github.com/gfx-rs/wgpu/issues/5555
- Futhark performance guide: https://futhark.readthedocs.io/en/latest/performance.html
- Futhark PLDI'17 paper: https://futhark-lang.org/publications/pldi17.pdf
- Burn fusion blog: https://burn.dev/blog/fusion-tensor-operation-streams/
- BQN implementation notes: https://mlochbaum.github.io/BQN/implementation/perf.html
- BQN vs Co-dfns: https://mlochbaum.github.io/BQN/implementation/codfns.html
- Linebender GPU sorting: https://linebender.org/wiki/gpu/sorting/
- wgpu-profiler: https://github.com/Wumpf/wgpu-profiler

### Medium Confidence (community, benchmarks, blogs)
- ArrayFire Rust bindings: https://github.com/arrayfire/arrayfire-rust
- GPU array languages overview: https://codereport.github.io/GPUArrayLanguages/
- WebGPU Radix Sort: https://github.com/kishimisu/WebGPU-Radix-Sort
- wgpu_sort with subgroups: https://docs.rs/wgpu_sort
- rust-gpu project: https://github.com/Rust-GPU/rust-gpu

### Low Confidence (general guidance, extrapolated)
- GPU dispatch overhead estimates (50-100us kernel launch) -- from CUDA literature, wgpu may differ
- Crossover thresholds -- rough estimates, need empirical validation
- f64 emulation via double-single -- known technique but not benchmarked with wgpu
