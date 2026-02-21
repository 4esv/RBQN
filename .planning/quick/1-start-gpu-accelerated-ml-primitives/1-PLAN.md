---
phase: quick-gpu-ml
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/rbqn-gpu/src/shaders/unary_f32.wgsl
  - crates/rbqn-gpu/src/shaders/matmul_f32.wgsl
  - crates/rbqn-gpu/src/shaders/softmax_f32.wgsl
  - crates/rbqn-gpu/src/kernels/unary.rs
  - crates/rbqn-gpu/src/kernels/matmul.rs
  - crates/rbqn-gpu/src/kernels/softmax.rs
  - crates/rbqn-gpu/src/kernels/mod.rs
  - crates/rbqn-gpu/src/lib.rs
  - crates/rbqn-gpu/tests/gpu_kernels.rs
autonomous: true
must_haves:
  truths:
    - "Elementwise exp, sqrt, neg, abs execute on GPU and return correct f32 results"
    - "Matrix multiplication of two 2D f32 arrays returns correct product"
    - "Softmax of an f32 vector returns correct probability distribution summing to 1.0"
    - "All kernels are accessible through the public crate API"
  artifacts:
    - path: "crates/rbqn-gpu/src/shaders/unary_f32.wgsl"
      provides: "WGSL compute shaders for exp, sqrt, neg, abs"
    - path: "crates/rbqn-gpu/src/shaders/matmul_f32.wgsl"
      provides: "Tiled matrix multiply shader with shared memory"
    - path: "crates/rbqn-gpu/src/shaders/softmax_f32.wgsl"
      provides: "Softmax shader (reduce-max, subtract, exp, reduce-sum, divide)"
    - path: "crates/rbqn-gpu/src/kernels/unary.rs"
      provides: "Rust dispatch for unary elementwise ops"
    - path: "crates/rbqn-gpu/src/kernels/matmul.rs"
      provides: "Rust dispatch for matmul with M/N/K uniform params"
    - path: "crates/rbqn-gpu/src/kernels/softmax.rs"
      provides: "Rust multi-pass softmax dispatch"
    - path: "crates/rbqn-gpu/tests/gpu_kernels.rs"
      provides: "Integration tests proving GPU correctness"
  key_links:
    - from: "crates/rbqn-gpu/src/kernels/unary.rs"
      to: "crates/rbqn-gpu/src/shaders/unary_f32.wgsl"
      via: "include_str! + PipelineCache"
      pattern: "include_str!.*unary_f32"
    - from: "crates/rbqn-gpu/src/kernels/matmul.rs"
      to: "crates/rbqn-gpu/src/shaders/matmul_f32.wgsl"
      via: "include_str! + PipelineCache"
      pattern: "include_str!.*matmul_f32"
    - from: "crates/rbqn-gpu/src/kernels/softmax.rs"
      to: "crates/rbqn-gpu/src/shaders/softmax_f32.wgsl"
      via: "include_str! + PipelineCache"
      pattern: "include_str!.*softmax_f32"
---

<objective>
Implement the three core ML GPU kernel categories — unary elementwise ops, matrix multiplication, and softmax — as working compute shaders with Rust dispatch code and integration tests.

Purpose: These are the foundational GPU primitives needed for ML workloads (neural net forward pass = matmul + activation + softmax). The existing crate has full infrastructure (GpuContext, BufferPool, PipelineCache, upload/download helpers) but only binary arith and reduction kernels. This fills the ML-specific gaps.

Output: Three new kernel modules with WGSL shaders, Rust dispatch, and passing GPU integration tests.
</objective>

<execution_context>
@/Users/axel/.claude/get-shit-done/workflows/execute-plan.md
@/Users/axel/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@crates/rbqn-gpu/src/context.rs (GpuContext — device/queue init, use for tests)
@crates/rbqn-gpu/src/buffer.rs (GpuBuffer, ElementKind, upload/download helpers, BufferPool)
@crates/rbqn-gpu/src/pipeline.rs (PipelineCache, PipelineKey — follow this pattern exactly)
@crates/rbqn-gpu/src/dispatch.rs (WORKGROUP_SIZE=256, workgroup_count helper)
@crates/rbqn-gpu/src/kernels/arith.rs (reference pattern: shader_for, arith_binary, bind groups)
@crates/rbqn-gpu/src/kernels/reduce.rs (reference pattern: multi-pass reduction with intermediates)
@crates/rbqn-gpu/src/shaders/arith_f32.wgsl (reference: elementwise shader structure)
@crates/rbqn-gpu/src/shaders/reduce_f32.wgsl (reference: workgroup shared memory reduction)
@crates/rbqn-gpu/Cargo.toml (deps: wgpu, bytemuck, pollster)
</context>

<tasks>

<task type="auto">
  <name>Task 1: Unary elementwise kernels (exp, sqrt, neg, abs) + matmul shader and dispatch</name>
  <files>
    crates/rbqn-gpu/src/shaders/unary_f32.wgsl
    crates/rbqn-gpu/src/kernels/unary.rs
    crates/rbqn-gpu/src/shaders/matmul_f32.wgsl
    crates/rbqn-gpu/src/kernels/matmul.rs
    crates/rbqn-gpu/src/kernels/mod.rs
    crates/rbqn-gpu/src/lib.rs
  </files>
  <action>
    **Unary elementwise WGSL shader** (`unary_f32.wgsl`):
    Follow the exact same pattern as `arith_f32.wgsl` but with single input buffer:
    - binding 0: `var<storage, read> input: array<f32>`
    - binding 1: `var<storage, read_write> output: array<f32>`
    - @workgroup_size(256), bounds check with `arrayLength(&output)`
    - Entry points: `exp_f32` (use WGSL `exp()`), `sqrt_f32` (use `sqrt()`), `neg_f32` (negate), `abs_f32` (use `abs()`)

    **Unary dispatch** (`unary.rs`):
    Follow `arith.rs` pattern but simpler — only 2 bind group entries (input + output):
    - `const SHADER_F32: &str = include_str!("../shaders/unary_f32.wgsl");`
    - `pub fn unary_op(device, queue, cache, op: &str, input: &GpuBuffer, out: &GpuBuffer)` — op is one of "exp", "sqrt", "neg", "abs"
    - Use `PipelineKey::new("unary_f32", op, elem)` for caching
    - Bind group layout: 2 entries (binding 0 = input read, binding 1 = output read_write)
    - Dispatch `workgroup_count(out.len(), WORKGROUP_SIZE)` workgroups

    **Matmul WGSL shader** (`matmul_f32.wgsl`):
    Tiled matrix multiply using shared memory for performance:
    - Tile size = 16x16 (sweet spot for GPU occupancy)
    - Bindings: 0=A(storage,read), 1=B(storage,read), 2=C(storage,read_write), 3=params(uniform) containing {M: u32, N: u32, K: u32, _pad: u32}
    - `@workgroup_size(16, 16)` — 2D workgroups
    - Entry point: `matmul_f32`
    - Algorithm: Each thread computes one element of C. Loop over tiles along K dimension. Load tile of A and tile of B into shared memory (`var<workgroup>`), workgroupBarrier(), accumulate dot product, workgroupBarrier(). Write final sum to C[row * N + col].
    - Bounds check: `if (row >= M || col >= N) { return; }` and guard shared memory loads

    **Matmul dispatch** (`matmul.rs`):
    - `const SHADER_F32: &str = include_str!("../shaders/matmul_f32.wgsl");`
    - `pub fn matmul(device, queue, cache, a: &GpuBuffer, b: &GpuBuffer, out: &GpuBuffer, m: u32, n: u32, k: u32)`
    - Create uniform buffer with [m, n, k, 0u32] (4 x u32 = 16 bytes)
    - Bind group: 4 entries (A, B, C, params)
    - Dispatch workgroups: `((n + 15) / 16, (m + 15) / 16, 1)` — x covers columns, y covers rows
    - Use `PipelineKey::raw("matmul_f32", "matmul_f32")`

    **Update modules:**
    - Add `pub mod unary;` and `pub mod matmul;` to `kernels/mod.rs`
    - No changes needed to `lib.rs` (kernels module already exported)
  </action>
  <verify>
    `cargo check -p rbqn-gpu` compiles without errors. Shader files exist with correct entry points. All new modules are reachable from `rbqn_gpu::kernels::{unary, matmul}`.
  </verify>
  <done>
    Four unary ops (exp, sqrt, neg, abs) and tiled matmul have WGSL shaders and Rust dispatch code that compiles cleanly.
  </done>
</task>

<task type="auto">
  <name>Task 2: Softmax kernel and integration tests for all new kernels</name>
  <files>
    crates/rbqn-gpu/src/shaders/softmax_f32.wgsl
    crates/rbqn-gpu/src/kernels/softmax.rs
    crates/rbqn-gpu/src/kernels/mod.rs
    crates/rbqn-gpu/tests/gpu_kernels.rs
  </files>
  <action>
    **Softmax WGSL shader** (`softmax_f32.wgsl`):
    Softmax = exp(x - max(x)) / sum(exp(x - max(x))). Implement as single-pass for vectors that fit in one workgroup (up to 256 elements), which covers most ML use cases:
    - Bindings: 0=input(storage,read), 1=output(storage,read_write)
    - `var<workgroup> shared: array<f32, 256>;`
    - `@workgroup_size(256)` entry point `softmax_f32`
    - Step 1: Load input[idx] to shared[lid], pad with -FLT_MAX (-3.40282347e+38) if out of bounds
    - Step 2: Parallel reduce to find max (tree reduction in shared memory, barrier between steps)
    - Step 3: Each thread computes `exp(shared_orig - max_val)`, store in shared
    - Step 4: Parallel reduce to find sum of exps
    - Step 5: Each thread writes `exp_val / sum` to output
    - Need to keep original values — use a second shared array `var<workgroup> shared_exp: array<f32, 256>;` or reload from input

    **Softmax dispatch** (`softmax.rs`):
    - `const SHADER_F32: &str = include_str!("../shaders/softmax_f32.wgsl");`
    - `pub fn softmax(device, queue, cache, input: &GpuBuffer, out: &GpuBuffer)` — single workgroup dispatch for now
    - For vectors > 256 elements, fall back to multi-pass: use existing `reduce` kernel for max, then `arith_scalar` for subtract, then `unary_op("exp")`, then `reduce` for sum, then `arith_scalar` for divide. This composes existing kernels.
    - Single-workgroup path: `PipelineKey::raw("softmax_f32", "softmax_f32")`, dispatch 1 workgroup
    - Add `pub mod softmax;` to `kernels/mod.rs`

    **Integration tests** (`tests/gpu_kernels.rs`):
    Use `pollster::block_on` to run async GPU code. Each test:
    1. Create GpuContext via `GpuContext::new().await.expect("no GPU")`
    2. Create PipelineCache
    3. Upload test data, run kernel, download results, compare

    Test cases:
    - `test_unary_exp`: input [0.0, 1.0, 2.0, -1.0] -> expect [1.0, e, e^2, 1/e], tolerance 1e-5
    - `test_unary_sqrt`: input [0.0, 1.0, 4.0, 9.0] -> expect [0.0, 1.0, 2.0, 3.0], tolerance 1e-5
    - `test_unary_neg`: input [1.0, -2.0, 0.0, 3.5] -> expect [-1.0, 2.0, 0.0, -3.5], exact
    - `test_unary_abs`: input [-1.0, 2.0, -3.5, 0.0] -> expect [1.0, 2.0, 3.5, 0.0], exact
    - `test_matmul_2x2`: A=[1,2,3,4] B=[5,6,7,8] M=2,N=2,K=2 -> C=[19,22,43,50]
    - `test_matmul_2x3_3x2`: A=[1,2,3,4,5,6] B=[7,8,9,10,11,12] M=2,N=2,K=3 -> C=[58,64,139,154]
    - `test_softmax_simple`: input [1.0, 2.0, 3.0, 4.0] -> check sum ~= 1.0 and each output > 0 and output[3] > output[2] > output[1] > output[0], tolerance 1e-5
    - `test_softmax_uniform`: input [1.0, 1.0, 1.0, 1.0] -> each output ~= 0.25

    Each test should be `#[test]` using `pollster::block_on(async { ... })`. Add `#[cfg(test)]` guard. Add `pollster` and `bytemuck` to dev-dependencies in Cargo.toml if not already present (they are regular deps so tests can use them).
  </action>
  <verify>
    `cargo test -p rbqn-gpu -- --nocapture` — all 8 tests pass (on a machine with GPU; tests that can't acquire GPU context should skip gracefully with a message).
  </verify>
  <done>
    Softmax kernel works for vectors up to 256 elements (single workgroup) and falls back to multi-pass composition for larger vectors. All 8 integration tests pass, proving GPU compute correctness for unary ops, matmul, and softmax.
  </done>
</task>

</tasks>

<verification>
1. `cargo check -p rbqn-gpu` — full crate compiles
2. `cargo test -p rbqn-gpu` — all integration tests pass
3. New kernel modules accessible: `rbqn_gpu::kernels::{unary, matmul, softmax}`
4. Shader files parse as valid WGSL (validated by wgpu at pipeline creation time)
</verification>

<success_criteria>
- Four unary elementwise ops (exp, sqrt, neg, abs) execute correctly on GPU
- Tiled f32 matrix multiplication produces correct results for small matrices
- Softmax produces valid probability distributions (sum to 1.0, all positive)
- Eight integration tests pass, verifying all kernel outputs against CPU reference values
- All code follows existing crate patterns (PipelineCache, GpuBuffer, workgroup dispatch)
</success_criteria>

<output>
After completion, create `.planning/quick/1-start-gpu-accelerated-ml-primitives/1-SUMMARY.md`
</output>
