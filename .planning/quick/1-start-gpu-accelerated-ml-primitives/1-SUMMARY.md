---
phase: quick-gpu-ml
plan: "01"
subsystem: rbqn-gpu
tags: [gpu, wgsl, compute-shaders, matmul, softmax, ml-primitives]
dependency_graph:
  requires: []
  provides: [unary-gpu-kernels, matmul-gpu-kernel, softmax-gpu-kernel]
  affects: [rbqn-gpu]
tech_stack:
  added: []
  patterns: [tiled-shared-memory-matmul, single-workgroup-softmax, multi-pass-softmax-fallback]
key_files:
  created:
    - crates/rbqn-gpu/src/shaders/unary_f32.wgsl
    - crates/rbqn-gpu/src/shaders/matmul_f32.wgsl
    - crates/rbqn-gpu/src/shaders/softmax_f32.wgsl
    - crates/rbqn-gpu/src/kernels/unary.rs
    - crates/rbqn-gpu/src/kernels/matmul.rs
    - crates/rbqn-gpu/src/kernels/softmax.rs
    - crates/rbqn-gpu/tests/gpu_kernels.rs
  modified:
    - crates/rbqn-gpu/src/kernels/mod.rs
decisions:
  - "Matmul tile size = 16x16 (256 threads per workgroup matching WORKGROUP_SIZE)"
  - "Softmax: single-workgroup path for <=256 elements; multi-pass composition for larger"
  - "Matmul bounds check: only guard final store, not barriers — all threads must reach every workgroupBarrier"
metrics:
  duration: "~3 minutes"
  completed: "2026-02-21"
  tasks_completed: 2
  files_created: 7
  files_modified: 1
---

# Quick Task 1: GPU-Accelerated ML Primitives — Summary

**One-liner:** Tiled f32 matmul, four unary elementwise ops, and single-workgroup softmax with multi-pass fallback, verified by 8 passing Metal GPU integration tests.

## What Was Built

### WGSL Compute Shaders

**`unary_f32.wgsl`** — Four entry points at `@workgroup_size(256)`:
- `exp_f32`, `sqrt_f32`, `neg_f32`, `abs_f32`
- Pattern: single input binding + output binding, bounds-checked via `arrayLength(&output)`

**`matmul_f32.wgsl`** — Tiled 16x16 matrix multiply:
- Uniform params buffer: `{M, N, K, _pad}` (16 bytes)
- Two `var<workgroup>` shared arrays (256 f32 each) for A and B tiles
- All threads participate in loads and barriers regardless of bounds — only the final write is guarded
- Dispatched as `((N+15)/16, (M+15)/16, 1)` workgroups

**`softmax_f32.wgsl`** — Single-pass softmax for vectors up to 256 elements:
- Two shared arrays: one for max-reduction, one for exp values
- Five-step algorithm: load, reduce-max, compute exp, reduce-sum, normalise
- Dispatched as exactly 1 workgroup (all 256 lanes handle the input)

### Rust Dispatch Modules

**`kernels/unary.rs`** — `pub fn unary_op(device, queue, cache, op, input, out)`:
- op ∈ {"exp", "sqrt", "neg", "abs"}
- Uses `PipelineKey::new("unary_f32", op, F32)` → entry point `{op}_f32`
- 2-binding bind group: input (read) + output (read_write)

**`kernels/matmul.rs`** — `pub fn matmul(device, queue, cache, a, b, out, m, n, k)`:
- Creates 16-byte uniform buffer with `[m, n, k, 0u32]`
- 4-binding bind group: A, B, C, params
- Uses `PipelineKey::raw("matmul_f32", "matmul_f32")`

**`kernels/softmax.rs`** — `pub fn softmax(device, queue, cache, input, out)`:
- Routes to single-workgroup path if `input.len() <= 256`
- Multi-pass fallback: `reduce(max)` → download scalar → `arith_scalar(sub)` → `unary(exp)` → `reduce(add)` → download scalar → `arith_scalar(div)`

### Integration Tests (`tests/gpu_kernels.rs`)

8 tests, all passing on Metal GPU:

| Test | Input | Verified |
|------|-------|---------|
| `test_unary_exp` | [0, 1, 2, -1] | exp values within 1e-5 |
| `test_unary_sqrt` | [0, 1, 4, 9] | [0, 1, 2, 3] within 1e-5 |
| `test_unary_neg` | [1, -2, 0, 3.5] | [-1, 2, 0, -3.5] exact |
| `test_unary_abs` | [-1, 2, -3.5, 0] | [1, 2, 3.5, 0] exact |
| `test_matmul_2x2` | 2x2 A, 2x2 B | [19, 22, 43, 50] within 1e-4 |
| `test_matmul_2x3_3x2` | 2x3 A, 3x2 B | [58, 64, 139, 154] within 1e-4 |
| `test_softmax_simple` | [1, 2, 3, 4] | monotonic, positive, sum~1.0 |
| `test_softmax_uniform` | [1, 1, 1, 1] | each ~0.25 |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed matmul shader workgroup barrier deadlock**
- **Found during:** Task 2 (test_matmul_2x3_3x2 failed, got 25 expected 58)
- **Issue:** The shader had an early `return` for out-of-bounds threads (row >= M || col >= N). In WGSL, all threads in a workgroup must reach every `workgroupBarrier()`. Out-of-bounds threads returning early prevented in-bounds threads from passing the barrier, causing incorrect partial accumulation.
- **Fix:** Removed early return. All threads participate in tile loading (writing 0.0 for OOB positions) and barriers. Only the final `mat_c[...] = acc` write is guarded by `if (in_bounds)`.
- **Files modified:** `crates/rbqn-gpu/src/shaders/matmul_f32.wgsl`
- **Commit:** d0b1aa0

## Self-Check: PASSED

- `crates/rbqn-gpu/src/shaders/unary_f32.wgsl` — FOUND
- `crates/rbqn-gpu/src/shaders/matmul_f32.wgsl` — FOUND
- `crates/rbqn-gpu/src/shaders/softmax_f32.wgsl` — FOUND
- `crates/rbqn-gpu/src/kernels/unary.rs` — FOUND
- `crates/rbqn-gpu/src/kernels/matmul.rs` — FOUND
- `crates/rbqn-gpu/src/kernels/softmax.rs` — FOUND
- `crates/rbqn-gpu/tests/gpu_kernels.rs` — FOUND
- Task 1 commit f72e68c — FOUND
- Task 2 commit d0b1aa0 — FOUND
