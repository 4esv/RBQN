---
phase: 05-gpu-integration
plan: 04
subsystem: gpu
tags: [wgpu, wgsl, fusion, kernel-fusion, FusionBuilder, OnceLock, function-pointer]

requires:
  - phase: 05-02
    provides: GPU_ARITH_HOOK pattern and OnceLock function-pointer approach for cross-crate GPU dispatch
  - phase: 05-03
    provides: GPU fold/scan dispatch infrastructure

provides:
  - FusionBuilder wired into dispatch layer via GPU_FUSED_HOOK in rbqn-prim
  - gpu_fused_arith function in gpu_runtime.rs for explicit multi-op GPU dispatch
  - try_fused_arith public API for VM-level fusion calls
  - WGSL codegen verified via 3 unit tests (fusion_wgsl_add_scalarmul, fusion_wgsl_scalar_only, fused_op_name_mapping)

affects: [06-benchmark-harness, future-vm-fusion]

tech-stack:
  added: []
  patterns:
    - "GPU_FUSED_HOOK: OnceLock<fn(ops, a, b)> — same function-pointer pattern as GPU_ARITH_HOOK for cross-crate dispatch"
    - "op_str_to_fused: string-to-FusedOp mapper for caller-friendly API"
    - "gpu_fused_arith wraps inner fn in catch_unwind for panic safety"

key-files:
  created: []
  modified:
    - crates/rbqn-prim/src/arith_dyad.rs
    - crates/rbqn/src/gpu_runtime.rs
    - crates/rbqn/src/main.rs

key-decisions:
  - "Explicit fused API (try_fused_arith) rather than auto-detection: BQN evaluator calls c2 one at a time with no lookahead — true expression-level fusion requires VM-level analysis, documented as future work"
  - "op_str_to_fused uses string keys (add/sub/mul/div/scalar_add/scalar_mul) matching existing gpu_op_name convention"
  - "WGSL codegen tested without GPU device — FusionBuilder.generate_wgsl is pure string generation, no wgpu required"

patterns-established:
  - "WGSL codegen unit tests: test generate_wgsl output directly without GPU device to verify shader correctness"

requirements-completed: [GPU-08]

duration: 3min
completed: 2026-02-27
---

# Phase 5 Plan 4: Kernel Fusion Wiring Summary

**FusionBuilder wired into arithmetic dispatch layer via GPU_FUSED_HOOK; explicit try_fused_arith API available for future VM-level fusion with WGSL codegen verified by 3 unit tests**

## Performance

- **Duration:** 3 min
- **Started:** 2026-02-27T09:29:33Z
- **Completed:** 2026-02-27T09:32:17Z
- **Tasks:** 1
- **Files modified:** 3

## Accomplishments

- Added `GPU_FUSED_HOOK` (OnceLock function pointer) and `register_gpu_fused` to `arith_dyad.rs` alongside existing `GPU_ARITH_HOOK`
- Added `try_fused_arith` public API: explicit entry point for multi-op GPU dispatch that future VM analysis can call
- Implemented `gpu_fused_arith` + `gpu_fused_arith_inner` in `gpu_runtime.rs` using the existing `FusionBuilder` from `rbqn-gpu`
- Added `op_str_to_fused` helper mapping string op names to `FusedOp` variants
- Registered the fused hook in `main.rs` at startup alongside existing GPU hooks
- Added `#[cfg(test)]` module with 3 WGSL codegen tests — all pass without needing a GPU device

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement explicit fused arithmetic entry point** - `c54f04c` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified

- `crates/rbqn-prim/src/arith_dyad.rs` - Added GPU_FUSED_HOOK, register_gpu_fused, try_fused_arith
- `crates/rbqn/src/gpu_runtime.rs` - Added gpu_fused_arith, op_str_to_fused, verify_fusion_wgsl, unit tests
- `crates/rbqn/src/main.rs` - Registered gpu_fused_arith hook at startup

## Decisions Made

- Chose explicit API over auto-fusion detection: the BQN evaluator calls `c2` one call at a time with no forward lookahead, making true lazy expression-level fusion impractical in Phase 5. The explicit `try_fused_arith` API is the correct deliverable — VM-level fusion detection is documented as future work.
- WGSL codegen tests use `generate_wgsl` directly (pure string generation) rather than executing on a GPU device, enabling tests to run anywhere without GPU hardware.

## Deviations from Plan

None - plan executed exactly as written. The plan explicitly described the simpler recommended approach (explicit fused API rather than pending-buffer tracking), which was implemented.

## Issues Encountered

- `Option<Option<GpuBuffer>>.transpose()` doesn't exist in Rust (only `Option<Result<T,E>>.transpose()` does) — fixed by using explicit `match` on the `Option<&BqnArr>` argument.

## Next Phase Readiness

- Kernel fusion infrastructure complete: GPU-08 satisfied
- `try_fused_arith` API callable from any code with access to `rbqn-prim`
- Future VM-level fusion pass can detect patterns like `2×a+b` and call `try_fused_arith` directly, eliminating multiple upload/download round-trips
- All 13 test files still green

---
*Phase: 05-gpu-integration*
*Completed: 2026-02-27*
