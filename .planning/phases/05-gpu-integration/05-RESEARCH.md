# Phase 5: GPU Integration - Research

**Researched:** 2026-02-27
**Domain:** wgpu GPU dispatch wired into BQN primitive hot paths
**Confidence:** HIGH (codebase is fully inspected; all GPU infrastructure verified in place)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- Both ML (matmul, softmax) and general array ops (arithmetic, reduce, scan, sort) are first-class — no priority ordering
- Include matmul and softmax kernels in Phase 5 since the code already exists in rbqn-gpu
- Kernel fusion (GPU-08) is a must-have — chained elementwise ops like `2×a+b` should fuse into a single GPU dispatch to avoid round-trips
- No GPU available: silent CPU fallback, no message at all
- GPU error mid-computation (driver crash, OOM): retry on CPU and log a warning
- Kill switch: `--no-gpu` CLI flag disables all GPU dispatch
- GPU code always compiled in (no cargo feature gate) — runtime detection handles availability

### Claude's Discretion

- Sort/grade GPU scope: Claude decides which array types get GPU sort based on what radix sort handles cleanly
- Dispatch threshold tuning: 50K is the starting point; benchmarks may adjust
- Debug flag format for "visible via debug flag" success criterion
- Buffer pooling and transfer strategy details

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| GPU-01 | Array transfer layer (BqnArr ↔ GpuBuffer, f64→f32 conversion) | `buffer.rs` has `upload_f32/i32/u32` + `download_*`; need a bridge layer in `rbqn-prim` or `rbqn` that converts `ArrData` variants to GPU buffers. The precision guard lives here. |
| GPU-02 | Precision guard — only dispatch integer-valued or ordinal ops to GPU | f32 has 24-bit mantissa; integers representable exactly only up to 2^24. Guard: check `ElType::is_int()` OR check all values `v.abs() < 2^24 && v.fract() == 0.0` for F64 arrays. |
| GPU-03 | Dispatch hooks in hot primitives (arithmetic, sort/grade, reduce, scan) | Hook sites identified: `pervasive_dyad` in `arith_dyad.rs`, `fold_c1` in `modifiers.rs`, `scan_c1` in `modifiers.rs`, `grade_up_c1`/`grade_down_c1` in `sort.rs`. |
| GPU-04 | Element-wise arithmetic kernels (add, sub, mul, div for f32/i32) | Shaders exist: `arith_f32.wgsl`, `arith_i32.wgsl`, `arith_scalar_f32.wgsl`, `arith_scalar_i32.wgsl`. Rust dispatch: `kernels/arith.rs::arith_binary` and `arith_scalar`. |
| GPU-05 | Reduction kernels (sum, min, max, and, or) | Shader exists: `reduce_f32.wgsl` and `reduce_i32.wgsl` — covers `reduce_add`, `reduce_mul`, `reduce_min`, `reduce_max`. Missing: boolean and/or variants (BQN `∧´`/`∨´`). Rust dispatch: `kernels/reduce.rs::reduce`. |
| GPU-06 | Scan kernels (prefix sum) | Shaders: `scan_f32.wgsl`, `scan_i32.wgsl`. Rust: `kernels/scan.rs::inclusive_scan` and `exclusive_scan`. Both exist and are implemented. |
| GPU-07 | Sort/grade kernels (fix sign-bit bug in radix sort) | `kernels/sort.rs::sort_i32` has a documented bug: it copies i32 bits as u32 without flipping the sign bit, making negative values sort after positives. Fix: XOR 0x80000000 before sort, XOR back after. Sort kernel itself (`sort.wgsl`) is correct for u32. |
| GPU-08 | Kernel fusion for chained element-wise operations | `fusion.rs` has `FusionBuilder` with `generate_wgsl` (dynamic WGSL codegen) and `execute`. The fusion builder accumulates `FusedOp` variants (Add/Sub/Mul/Div/ScalarAdd/ScalarMul). Needs a fusion accumulator in the dispatch layer. |
| GPU-09 | Threshold validation — benchmark 50K/100K on Apple Silicon with criterion | Criterion not yet in Cargo.toml. Need to add as dev-dependency to `rbqn` or a new `benches/` crate. Current threshold in `dispatch.rs`: 100K default, 50K for reduce/scan, 500K for sort. |
| GPU-10 | Measurable speedup demonstrated on arrays >50K elements | Criterion bench must demonstrate speedup. Apple Silicon Metal backend used (already detected in `context.rs`: `wgpu::Backends::METAL` on macOS). |

</phase_requirements>

## Summary

The `rbqn-gpu` crate is structurally complete: device init, buffer pool, pipeline cache, WGSL shaders, and all required kernel implementations exist. The gap is entirely on the *wiring* side — the BQN primitive functions in `rbqn-prim` and `rbqn-vm` do not yet call into `rbqn-gpu`. Phase 5 is primarily a dispatch integration phase, not a kernel authoring phase.

The two concrete correctness issues that must be fixed before dispatch is enabled: (1) the sign-bit bug in `sort_i32` (negative values sort incorrectly) and (2) the precision guard (f32 cannot represent integers above 2^24 exactly, so F64 arrays with large values must fall back to CPU). The global `GpuContext` needs to be initialized once at startup (after CLI `--no-gpu` check) and accessed from primitive dispatch via a global static.

Kernel fusion (GPU-08) is the most structurally novel piece: the `FusionBuilder` exists but has no integration with the BQN evaluation loop. For phase 5 the minimum viable fusion approach is an explicit check in the `pervasive_dyad` fast path — when both operands are large numeric arrays and the function is one of `+`, `-`, `×`, `÷`, accumulate ops into `FusionBuilder` instead of dispatching immediately. This requires restructuring fold/scan to detect fusable patterns before executing element by element.

**Primary recommendation:** Wire GPU dispatch via a `GpuRuntime` singleton (initialized in `main.rs` after `--no-gpu` check), inject it into `pervasive_dyad`, `fold_c1`, `scan_c1`, and the sort/grade functions, then add criterion benchmarks to validate thresholds.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| wgpu | 24 (workspace) | Metal/Vulkan GPU backend | Already in workspace, used by rbqn-gpu |
| bytemuck | 1 (workspace) | Safe byte-casting for buffer uploads | Already used throughout rbqn-gpu |
| pollster | 0.4 | Block async wgpu futures on current thread | Already used in sort.rs and buffer.rs |
| criterion | 0.5 | Benchmarking with statistical analysis | Rust standard; needed for GPU-09/GPU-10 |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::sync::OnceLock | stdlib | Global singleton for GpuRuntime | One-time GPU init at startup |
| std::sync::atomic::AtomicBool | stdlib | `--no-gpu` kill switch flag | CLI flag sets this before init |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| pollster::block_on | tokio runtime | tokio is unnecessary; wgpu futures are single-call await, pollster is zero-dependency |
| OnceLock for GpuRuntime | LazyLock | OnceLock allows explicit init with return value; LazyLock requires fallible init workaround |

**Installation (add to workspace Cargo.toml and rbqn/Cargo.toml):**
```toml
# In [workspace.dependencies]:
criterion = { version = "0.5", features = ["html_reports"] }

# In crates/rbqn/Cargo.toml [dev-dependencies]:
criterion.workspace = true
```

## Architecture Patterns

### Recommended Project Structure

```
crates/rbqn/src/
├── gpu_runtime.rs      # GpuRuntime singleton: init, kill switch, context + cache
crates/rbqn-prim/src/
├── arith_dyad.rs       # Hook pervasive_dyad for GPU dispatch
├── fold.rs             # Hook fold_numeric for GPU reduce
├── sort.rs             # Hook grade_up/grade_down for GPU sort (after sign-bit fix)
crates/rbqn-vm/src/
├── modifiers.rs        # Hook fold_c1, scan_c1 for GPU reduce/scan
crates/rbqn-gpu/src/
├── kernels/sort.rs     # Fix sign-bit bug in sort_i32
crates/rbqn/benches/
└── gpu_bench.rs        # Criterion benchmarks for GPU-09/GPU-10
```

### Pattern 1: Global GpuRuntime Singleton

**What:** A `GpuRuntime` struct wrapping `GpuContext` + `PipelineCache` + `BufferPool`, stored in a global `OnceLock`. Initialized once in `main.rs` after the `--no-gpu` flag is checked.

**When to use:** All GPU dispatch paths check `gpu_runtime::get()` which returns `None` when unavailable or `--no-gpu` is set.

**Example:**
```rust
// crates/rbqn/src/gpu_runtime.rs
use std::sync::{OnceLock, Mutex};
use rbqn_gpu::{context::GpuContext, pipeline::PipelineCache, buffer::BufferPool};

pub struct GpuRuntime {
    pub ctx: GpuContext,
    pub cache: Mutex<PipelineCache>,
}

static GPU_RUNTIME: OnceLock<Option<GpuRuntime>> = OnceLock::new();
static GPU_DEBUG: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn init(no_gpu: bool) {
    GPU_RUNTIME.get_or_init(|| {
        if no_gpu { return None; }
        pollster::block_on(async {
            let ctx = GpuContext::new().await?;
            let cache = Mutex::new(PipelineCache::new(ctx.device.clone()));
            Some(GpuRuntime { ctx, cache })
        })
    });
}

pub fn get() -> Option<&'static GpuRuntime> {
    GPU_RUNTIME.get()?.as_ref()
}

pub fn debug_enabled() -> bool {
    GPU_DEBUG.load(std::sync::atomic::Ordering::Relaxed)
}
```

### Pattern 2: Dispatch Hook in pervasive_dyad

**What:** Before the element-by-element loop, check if both arrays are large numeric flat arrays and the operation maps to a GPU kernel. If so, upload, dispatch, download, return.

**When to use:** `pervasive_dyad` in `arith_dyad.rs` — only when both operands are arrays (not atoms or boxed), `el_type.is_num()`, and `ia() >= GPU_THRESHOLD`.

**Example (GPU-03/GPU-04 hook site):**
```rust
// In pervasive_dyad, before element-wise loop:
if let Some(gpu) = crate::gpu_runtime::get() {
    if wa_arr.ia() >= GPU_THRESHOLD && wa_arr.el_type().is_int() {
        if let Some(result) = try_gpu_arith(gpu, op_name, wa_arr, xa_arr) {
            return Ok(result);
        }
    }
}
// ... fall through to CPU path
```

NOTE: `rbqn-prim` currently does not depend on `rbqn-gpu`. The GPU hook needs to either:
- (a) Call through a function pointer registered at startup from `rbqn` (avoids circular dep), OR
- (b) Add `rbqn-gpu` as a dep of `rbqn-prim` (simpler, straightforward given workspace layout)

Option (b) is cleaner. Add `rbqn-gpu.workspace = true` to `rbqn-prim/Cargo.toml`.

### Pattern 3: Debug Flag

**What:** `RBQN_GPU_DEBUG=1` env var at startup sets a global `AtomicBool`. When enabled, print to stderr before each GPU dispatch:
```
[gpu] reduce_add 100000 elements (f32)
[gpu] arith mul 200000 elements (f32)
```
This satisfies the "visible via debug flag" success criterion for `+´ 1e5⥊1`.

### Pattern 4: Precision Guard (GPU-02)

**What:** Before any GPU dispatch, check if the array's values are safe for f32.

```rust
pub fn is_gpu_safe_f64(arr: &BqnArr) -> bool {
    // Integer-typed arrays (I8/I16/I32/Bit) are always safe — they convert to i32 exactly.
    if arr.el_type().is_int() { return true; }
    // F64: check all values fit in i32 range for integer ops.
    // For pure arithmetic, check all values satisfy abs < 2^24.
    if let ArrData::F64(vals) = &arr.data {
        return vals.iter().all(|&v| v.fract() == 0.0 && v.abs() < 16_777_216.0);
    }
    false
}
```

**Important:** The requirement says "precision guard blocks GPU dispatch for arrays with values above 2^24". This means F64 arrays with large integers or fractional values must CPU-fallback silently. For F64 arrays that pass the guard, convert to i32 before upload.

### Pattern 5: Sort/Grade GPU Dispatch with Sign-Bit Fix

**What:** `sort_i32` in `kernels/sort.rs` must flip sign bit before sorting and unflip after. The XOR mask for sign-bit conversion: `0x80000000u32`.

```rust
// Fix in sort_i32: before radix_sort_u32, XOR all values with 0x80000000
// This maps i32 range [-2^31..2^31-1] to u32 range [0..2^32-1] preserving order.
// After sort, XOR result with 0x80000000 to restore original i32 bit pattern.
```

**GPU-07 scope decision (Claude's discretion):** Only numeric arrays (I8/I16/I32/F64 that pass precision guard) get GPU sort. Character arrays (C8/C16/C32) are excluded — their ordinal sort is correct on CPU and radix sort would need a u32 encoding of Unicode code points, which adds complexity. Boxed arrays are always CPU.

### Pattern 6: Kernel Fusion (GPU-08)

**What:** Detect consecutive elementwise ops on the same array and fuse them. Minimum viable approach for Phase 5: at the `fold_c1` and direct multi-op sites, use `FusionBuilder::can_fuse(op)` to accumulate ops and dispatch once.

**Practical scope for Phase 5:** The fusion builder in `fusion.rs` is designed for explicit composition. The BQN evaluator calls `c2(f, w, x)` one call at a time — it has no forward lookahead. True lazy fusion would require an IR layer. For Phase 5, implement fusion at a fixed chained-call site:

- When `pervasive_dyad` is called for `+`, `-`, `×`, `÷` with a large array result, check if the result is immediately consumed by another arithmetic call in the same modifier application. This is complex in the current architecture.

**Simpler approach that still satisfies GPU-08:** Add explicit `fused_arith(ops: &[(op, arr)])` entry point in `rbqn-gpu` and call it from a new `apply_fused_arith` helper in `rbqn-prim`. The BQN runtime expression `2×a+b` compiles to: compute `a+b`, then `2×result`. After GPU dispatch of `a+b`, if the result stays as a GPU buffer and `2×` follows immediately on a GPU buffer, fuse in one pass. This requires tracking "pending GPU buffer" — a non-trivial addition.

**Recommended for Phase 5:** Implement `FusionBuilder` wiring at the `apply_fused` entry point in `rbqn-gpu` (already implemented in `fusion.rs`) and expose a `fused_arith_binary(ops)` function. Document that true expression-level fusion is a future optimization; Phase 5 delivers single-kernel fusion for explicit multi-op sequences.

### Anti-Patterns to Avoid

- **Allocating new staging buffers per download:** `download_raw` in `buffer.rs` creates a new staging buffer each call. For high-frequency dispatch, pool staging buffers alongside compute buffers in `BufferPool`.
- **Calling `device.poll(Maintain::Wait)` inside kernel dispatch:** This blocks the thread and prevents any overlap. Only call poll in the download path.
- **Panicking on GPU error:** Wrap every GPU call in a `std::panic::catch_unwind` or use `Result`-returning wrappers, fall back to CPU on any error, log warning.
- **f64→f32 truncation without guard:** Never cast f64 to f32 without first verifying values are in range. `as f32` in Rust saturates; this silently produces wrong results for large integers.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| GPU backend abstraction | Custom Metal/Vulkan API calls | wgpu (already in workspace) | wgpu handles Metal on macOS, Vulkan on Linux — no platform code needed |
| Shader compilation | SPIR-V/MSL hand-written shaders | WGSL (already in all shaders) | wgpu compiles WGSL → Metal MSL / Vulkan SPIR-V automatically |
| Benchmark harness | Manual timing with `std::time::Instant` | criterion | criterion gives statistical significance, wall-clock noise rejection |
| Prefix sum implementation | Hand-rolled scan loop | `kernels/scan.rs::inclusive_scan` | Already implemented, tested |
| Buffer pooling | Ad-hoc `Vec<Buffer>` | `buffer::BufferPool` | Already implemented in `buffer.rs` |
| Radix sort from scratch | Custom GPU sort | `kernels/sort.rs::radix_sort_u32` | Already implemented; just needs sign-bit fix |

## Common Pitfalls

### Pitfall 1: The Sign-Bit Bug in sort_i32

**What goes wrong:** `sort_i32` copies i32 bytes into a u32 buffer without flipping the sign bit, then radix-sorts as unsigned. Negative i32 values have the high bit set, so they appear larger than all positive values and sort AFTER them in ascending order.

**Why it happens:** Two's complement i32 and unsigned u32 have different orderings. `i32::MIN` (-2^31) as u32 is 2^31, which is larger than u32 representation of i32::MAX (2^31 - 1).

**How to avoid:** XOR every element with `0x80000000u32` before sorting (flips sign bit, maps i32 order → u32 order). XOR the result back after sort.

**Warning signs:** Sorting `1e5⥊↕100` (integers 0..99 repeated) — this is non-negative so the bug won't trigger. Test with an array containing negative values: `⍋ ¯5‿3‿¯1‿0‿2` must return `0‿2‿3‿4‿1`.

### Pitfall 2: GpuContext::new() is Async but main is Sync

**What goes wrong:** `GpuContext::new()` returns `Option<Self>` via `.await`. The `main()` function is synchronous. Using `tokio::block_on` or `async_std` is overkill.

**How to avoid:** Use `pollster::block_on(GpuContext::new())` — already used in `sort.rs`. The `pollster` crate is already a dependency of `rbqn-gpu`.

### Pitfall 3: Staging Buffer Not Pooled = Repeated Allocation on Hot Path

**What goes wrong:** `download_raw` allocates a new `MAP_READ` staging buffer on every call. For fold dispatched to GPU: upload (50K elements) → kernel → allocate staging → copy to staging → map → read → unmap → free staging. The allocation + free cost can exceed GPU speedup for moderate-size arrays.

**How to avoid:** Extend `BufferPool` to also pool staging buffers by size. Or for reduce (single scalar result), keep a permanently-allocated scalar staging buffer.

### Pitfall 4: rbqn-prim Has No Access to GpuRuntime

**What goes wrong:** `rbqn-prim` does not currently depend on `rbqn-gpu` or `rbqn`. Calling GPU dispatch from `pervasive_dyad` requires either adding the dep or using a global function pointer.

**How to avoid:** Add `rbqn-gpu.workspace = true` to `crates/rbqn-prim/Cargo.toml` `[dependencies]`. There is no circular dependency: `rbqn-gpu` → `rbqn-core` only; `rbqn-prim` → `rbqn-core` + `rbqn-gpu`.

### Pitfall 5: Criterion Benchmarks Require Async GPU Init

**What goes wrong:** Criterion benchmark closures are synchronous; GPU context init is async.

**How to avoid:** Use `pollster::block_on` inside the benchmark setup closure, outside the measured region:
```rust
fn bench_reduce(c: &mut Criterion) {
    let (ctx, mut cache) = pollster::block_on(setup()).expect("no GPU");
    c.bench_function("gpu_reduce_100k", |b| b.iter(|| { ... }));
}
```

### Pitfall 6: Dispatch Threshold in dispatch.rs Uses 100K Default

**What goes wrong:** `dispatch.rs` sets `GPU_THRESHOLD = 100_000` but the requirement says the 50K test (`+´ 1e5⥊1`) must dispatch to GPU. `1e5` is exactly 100_000 — and the threshold check is `len >= threshold`, so 100K passes. But the requirement test uses `1e5` = 100_000 elements, which is exactly the threshold. Reduce/scan use `GPU_THRESHOLD / 2 = 50_000`. So `+´ 1e5⥊1` (100K elements, a reduce op) currently uses the sort threshold of 500K — this is wrong.

**How to avoid:** The threshold in `dispatch.rs` for reduce/scan is 50K (`GPU_THRESHOLD / 2`). The success criterion test `+´ 1e5⥊1` has 100K elements which exceeds 50K — it would dispatch. Verify by logging the dispatch decision. Final threshold tuning comes from benchmarks (GPU-09).

## Code Examples

### Transfer Layer: BqnArr → GpuBuffer

```rust
// Source: buffer.rs upload_f32 pattern, adapted for ArrData
pub fn arr_to_gpu_i32(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    arr: &BqnArr,
) -> Option<GpuBuffer> {
    match &arr.data {
        ArrData::I32(v) => Some(upload_i32(device, queue, v)),
        ArrData::I8(v) => {
            let converted: Vec<i32> = v.iter().map(|&x| x as i32).collect();
            Some(upload_i32(device, queue, &converted))
        }
        ArrData::F64(v) => {
            // Only call this after precision guard passes
            let converted: Vec<i32> = v.iter().map(|&x| x as i32).collect();
            Some(upload_i32(device, queue, &converted))
        }
        _ => None,
    }
}

pub fn gpu_i32_to_arr(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buf: &GpuBuffer,
    shape: Vec<usize>,
) -> BqnArr {
    let data = pollster::block_on(download_i32(device, queue, buf));
    BqnArr { shape, data: ArrData::I32(data), fill: Some(B::m_i32(0)) }
}
```

### Precision Guard

```rust
// Returns true if arr can be safely dispatched to GPU as i32
pub fn gpu_safe_integer(arr: &BqnArr) -> bool {
    match arr.el_type() {
        ElType::Bit | ElType::I8 | ElType::I16 | ElType::I32 => true,
        ElType::F64 => {
            if let ArrData::F64(vals) = &arr.data {
                vals.iter().all(|&v| {
                    v.is_finite() && v.fract() == 0.0 && v.abs() < 16_777_216.0 // 2^24
                })
            } else { false }
        }
        _ => false,
    }
}
```

### Debug Logging

```rust
// In gpu_runtime.rs:
pub fn log_dispatch(op: &str, len: usize, kind: &str) {
    if GPU_DEBUG.load(Ordering::Relaxed) {
        eprintln!("[gpu] {op} {len} elements ({kind})");
    }
}

// Check RBQN_GPU_DEBUG at init time:
if std::env::var_os("RBQN_GPU_DEBUG").is_some() {
    GPU_DEBUG.store(true, Ordering::Relaxed);
}
```

### Sign-Bit Fix for sort_i32

```rust
// In kernels/sort.rs, sort_i32 — replace the naive copy with XOR pass:
// Upload as u32 with sign bit flipped
let n = input.len();
let raw_i32 = pollster::block_on(download_i32(device, queue, input));
let as_sortable: Vec<u32> = raw_i32.iter()
    .map(|&v| (v as u32) ^ 0x8000_0000u32)
    .collect();
let u32_buf = upload_u32(device, queue, &as_sortable);
let sorted_u32 = radix_sort_u32(device, queue, cache, &u32_buf);
// Restore: XOR back and reinterpret as i32
// NOTE: caller must download and XOR result, or add a GPU shader pass for this
```

### Criterion Benchmark Skeleton

```rust
// crates/rbqn/benches/gpu_bench.rs
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_reduce_100k(c: &mut Criterion) {
    let (ctx, mut cache) = pollster::block_on(
        rbqn_gpu::context::GpuContext::new()
    ).expect("GPU required for benchmark");
    let data: Vec<f32> = vec![1.0f32; 100_000];
    let buf = rbqn_gpu::buffer::upload_f32(&ctx.device, &ctx.queue, &data);

    c.bench_function("gpu_reduce_add_100k", |b| {
        b.iter(|| {
            let _result = rbqn_gpu::kernels::reduce::reduce(
                &ctx.device, &ctx.queue, &mut cache, "add", &buf
            );
            ctx.device.poll(wgpu::Maintain::Wait);
        })
    });
}

criterion_group!(benches, bench_reduce_100k);
criterion_main!(benches);
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Custom GLSL compute shaders | WGSL (WebGPU Shading Language) | wgpu 0.15+ | Single shader source for Metal+Vulkan+DX12 — no platform-specific paths |
| Tokio async for GPU futures | pollster::block_on | Established pattern | Zero-weight executor for single-shot async GPU calls |
| Feature-gated GPU support | Runtime detection + `--no-gpu` kill switch | Phase 5 decision | Always compiled in; runtime handles availability |

**Deprecated/outdated:**
- The `GPU_THRESHOLD` constants in `dispatch.rs` are in the wrong crate — they are unused by any caller currently. Phase 5 must consume them or move them to the transfer layer.

## Open Questions

1. **How does `rbqn-prim` access `GpuRuntime`?**
   - What we know: `rbqn-prim` has no dep on `rbqn-gpu` currently.
   - What's unclear: Should we add `rbqn-gpu` dep to `rbqn-prim`, or use a global function pointer registered from `rbqn`?
   - Recommendation: Add `rbqn-gpu` as direct dep to `rbqn-prim/Cargo.toml`. No circular dep exists (`rbqn-gpu` only depends on `rbqn-core`). Planner should include "add dep" as an explicit task step.

2. **Does `--no-gpu` need to be checked before bootstrap?**
   - What we know: Bootstrap runs before any primitive dispatch; GPU init in `main.rs` can precede bootstrap.
   - What's unclear: If `GpuContext::new()` takes >50ms on first call, it adds startup latency for all users, not just GPU users.
   - Recommendation: Initialize GPU lazily on first dispatch (use `OnceLock` with explicit init callable). Log a warning if `RBQN_GPU_DEBUG` is set and GPU init took >100ms.

3. **What constitutes "measurable speedup" for GPU-10?**
   - What we know: Criterion requires >5% speedup for significance; Apple Silicon M-series shares CPU/GPU memory (unified memory, low transfer cost).
   - What's unclear: At what array size does Metal's dispatch overhead pay off vs. unified memory advantage?
   - Recommendation: Start at 50K, expect definitive speedup at 200K+ for reduce. If 50K threshold doesn't show speedup in criterion, raise threshold to 100K. Document the benchmark result in VERIFICATION.md regardless of direction.

4. **Staging buffer pool for download?**
   - What we know: `download_raw` allocates a staging buffer per call; `BufferPool` only pools compute buffers.
   - What's unclear: Is re-allocation overhead significant compared to GPU kernel time?
   - Recommendation: Add a single reusable staging buffer per `GpuRuntime` for scalar results (reduce output is always 1 element). For array results (arith, scan), pool staging buffers alongside compute buffers.

## Sources

### Primary (HIGH confidence)

- Direct code inspection of `/Users/axel/Code/forks/RBQN/crates/rbqn-gpu/` — all kernel implementations, shaders, pipeline cache, buffer pool, context init
- Direct code inspection of `/Users/axel/Code/forks/RBQN/crates/rbqn-prim/` — all primitive dispatch sites, arith_dyad pervasive pattern, fold/scan in fold.rs, sort in sort.rs
- Direct code inspection of `/Users/axel/Code/forks/RBQN/crates/rbqn-vm/src/modifiers.rs` — fold_c1, scan_c1 dispatch
- Direct code inspection of dispatch.rs (rbqn-gpu) — threshold constants

### Secondary (MEDIUM confidence)

- wgpu documentation and established patterns for `pollster::block_on` + `Maintain::Wait` polling (cross-verified with usage in existing codebase)
- Criterion 0.5 benchmark patterns (standard Rust ecosystem; not verified against docs but well-established)

### Tertiary (LOW confidence)

- Apple Silicon Metal + wgpu unified memory speedup estimates — not benchmarked; claim based on known architecture. Actual threshold must be validated by GPU-09 criterion benchmarks.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all dependencies already in workspace and in use
- Architecture: HIGH — all kernel implementations verified by direct code inspection; dispatch sites precisely identified
- Pitfalls: HIGH for sign-bit bug (documented in code comment) and precision guard (mathematical fact); MEDIUM for performance threshold claims (requires empirical validation)

**Research date:** 2026-02-27
**Valid until:** 2026-03-27 (wgpu 24 is current; no expected breaking changes in 30 days)
