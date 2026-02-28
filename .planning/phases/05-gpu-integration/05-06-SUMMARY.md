---
phase: 05-gpu-integration
plan: 06
subsystem: gpu
tags: [bqn, wgpu, matmul, softmax, gpu-dispatch, ml-primitives]

# Dependency graph
requires:
  - phase: 05-gpu-integration/05-01
    provides: GpuRuntime singleton, precision guard, transfer layer
  - phase: 05-gpu-integration/05-02
    provides: GPU hook pattern (OnceLock function pointers), arith/grade/sort dispatch
provides:
  - "•math.MatMul BQN system function: GPU-dispatched rank-2 matrix multiply"
  - "•math.Softmax BQN system function: GPU-dispatched rank-1 softmax"
  - "CPU fallback implementations for both (math_matmul_cpu, math_softmax_cpu)"
  - "arr_to_f64 helper for numeric BqnArr → Vec<f64> conversion"
  - "GPU dispatch hooks (GPU_MATMUL_HOOK, GPU_SOFTMAX_HOOK) in derive.rs"
affects: [future-ml-plans, benchmark-plans]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "OnceLock function pointer hooks for GPU dispatch (same as arith/sort)"
    - "f32 GPU compute with f64 BQN interface (precision loss acceptable for ML)"
    - "Threshold-gated dispatch: matmul >= 50_000 elements, softmax >= 256 elements"

key-files:
  created: []
  modified:
    - "crates/rbqn-vm/src/derive.rs — GPU_MATMUL_HOOK, GPU_SOFTMAX_HOOK, math_matmul_cpu, math_softmax_cpu, updated math namespace"
    - "crates/rbqn/src/gpu_runtime.rs — gpu_matmul, gpu_softmax, arr_to_f64 functions"
    - "crates/rbqn/src/main.rs — register_gpu_matmul and register_gpu_softmax calls"

key-decisions:
  - "MatMul threshold: M*K + K*N >= 50_000 (total input elements, not output) — avoids GPU overhead for small matrices"
  - "Softmax threshold: n >= 256 — single workgroup path handles <=256 natively"
  - "f32 for GPU compute: matmul/softmax are ML primitives, f32 precision is standard"
  - "Monadic matmul throws error (needs left argument); dyadic softmax throws error (not standard)"

patterns-established:
  - "ML primitive pattern: GPU dispatch with CPU fallback for •math.* functions"
  - "arr_to_f64 helper: reusable pattern for converting any numeric BqnArr to f64 slice"

requirements-completed: [GPU-03, GPU-04]

# Metrics
duration: 7min
completed: 2026-02-28
---

# Phase 5 Plan 06: GPU MatMul and Softmax Summary

**•math.MatMul and •math.Softmax wired to GPU kernels via OnceLock hooks — f32 GPU dispatch with f64 CPU fallback for both operations**

## Performance

- **Duration:** 7 min
- **Started:** 2026-02-28T03:37:40Z
- **Completed:** 2026-02-28T03:44:52Z
- **Tasks:** 1
- **Files modified:** 3

## Accomplishments
- Added •math.MatMul (sys 1114) and •math.Softmax (sys 1115) to the •math namespace
- GPU dispatch via OnceLock hooks: matmul dispatches for M*K+K*N >= 50,000 elements, softmax for n >= 256
- CPU fallback implementations in derive.rs: numerically stable softmax (max-subtraction), naive triple-loop matmul
- Registered hooks in main.rs after GPU init; RBQN_GPU_DEBUG=1 logs dispatch messages
- All 13 test files remain green

## Task Commits

Work was committed as part of the preceding phase 05-02/05-03 execution (parallel agent):

1. **Task 1: •math.MatMul and •math.Softmax GPU dispatch** - `e824b76` (feat - included in 05-02 commit)

## Files Created/Modified
- `crates/rbqn-vm/src/derive.rs` — GPU_MATMUL_HOOK, GPU_SOFTMAX_HOOK OnceLock statics, register_gpu_matmul/softmax pub fns, 1114/1115 dispatch cases, math_matmul_cpu and math_softmax_cpu implementations, matmul/softmax entries in math namespace
- `crates/rbqn/src/gpu_runtime.rs` — gpu_matmul (f32 GPU kernel dispatch, 50K threshold), gpu_softmax (f32 GPU kernel dispatch, 256 threshold), arr_to_f64 helper
- `crates/rbqn/src/main.rs` — register_gpu_matmul and register_gpu_softmax hook registrations

## Decisions Made
- Used same OnceLock hook pattern as arith/sort/grade to avoid circular crate dependency
- GPU matmul converts f64→f32 (acceptable for ML use case) and returns f64 result
- Threshold-gated: small matrices fall through to CPU fallback automatically
- Dyadic softmax and monadic matmul both throw descriptive errors

## Deviations from Plan

None — plan executed exactly as written. The derive.rs and main.rs changes were found pre-committed (parallel agent session), and the gpu_runtime.rs functions were verified and committed as part of this execution.

## Issues Encountered
- Changes to derive.rs and main.rs were already committed by a parallel agent in the e824b76 commit (labeled 05-02 but contained 05-06 content). Verified all functionality worked correctly and committed remaining gpu_runtime.rs changes.

## Next Phase Readiness
- Both •math.MatMul and •math.Softmax are first-class BQN system functions
- GPU dispatch is transparent: same BQN code works on CPU and GPU
- Ready for any follow-on phases requiring ML primitive benchmarking or additional math ops

---
*Phase: 05-gpu-integration*
*Completed: 2026-02-28*
