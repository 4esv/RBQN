---
phase: 05-gpu-integration
plan: 01
subsystem: infra
tags: [gpu, wgpu, sort, radix-sort, sign-bit, precision-guard, singleton, cli]

requires:
  - phase: 04-full-test-suite-green
    provides: "Working BQN runtime with 13 passing test files"

provides:
  - "GpuRuntime singleton (OnceLock<Option<GpuRuntime>>) with GPU_DISABLED and GPU_DEBUG statics"
  - "--no-gpu CLI flag to disable GPU dispatch"
  - "RBQN_GPU_DEBUG env var for per-dispatch stderr logging"
  - "Precision guard: gpu_safe_integer / gpu_safe_arr checking F64 values within 2^24"
  - "Transfer functions: arr_to_gpu_i32 / gpu_i32_to_arr with squeeze_num compaction"
  - "should_dispatch delegating to rbqn_gpu::dispatch::should_use_gpu"
  - "Correct sort_i32 with sign-bit XOR for negative value support"

affects: [05-02, 05-03, 05-04, 05-05]

tech-stack:
  added: [pollster = "0.4" in rbqn crate]
  patterns:
    - "GPU dispatch gated by OnceLock singleton checked at call site"
    - "Precision guard must be checked before any F64 GPU arithmetic dispatch"
    - "Transfer: arr_to_gpu_i32 + gpu_i32_to_arr as canonical CPU<->GPU round-trip"

key-files:
  created:
    - "crates/rbqn/src/gpu_runtime.rs"
  modified:
    - "crates/rbqn/src/cli.rs"
    - "crates/rbqn/src/main.rs"
    - "crates/rbqn/Cargo.toml"
    - "crates/rbqn-gpu/src/kernels/sort.rs"

key-decisions:
  - "GpuRuntime stored as OnceLock<Option<GpuRuntime>> — None when --no-gpu or no adapter found"
  - "Precision guard uses 2^24 (16_777_216.0) as f32 safe integer limit to avoid GPU rounding errors"
  - "sort_i32 sign-bit fix is CPU-side XOR pass (download, transform, upload) — simpler than GPU shader pass, kernel dominates for large arrays"
  - "squeeze_num applied to gpu_i32_to_arr output for compact integer storage"

patterns-established:
  - "GPU init: call gpu_runtime::init(args.no_gpu) immediately after CLI parse, before bootstrap"
  - "GPU debug: check gpu_runtime::debug_enabled() then eprintln!(\"[gpu] ...\") pattern"

requirements-completed: [GPU-01, GPU-02, GPU-07]

duration: 1min
completed: 2026-02-28
---

# Phase 5 Plan 1: GPU Runtime Foundation Summary

**GpuRuntime singleton with precision guard, BqnArr transfer layer, --no-gpu kill switch, and corrected sort_i32 sign-bit XOR for negative value support**

## Performance

- **Duration:** 1 min
- **Started:** 2026-02-28T03:32:50Z
- **Completed:** 2026-02-28T03:33:55Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- GpuRuntime singleton initialized via pollster::block_on(GpuContext::new()), stored in OnceLock<Option<GpuRuntime>>
- --no-gpu flag added to CLI (sets GPU_DISABLED, stores None in singleton)
- RBQN_GPU_DEBUG=1 env var enables [gpu] dispatch logging to stderr
- Precision guard (gpu_safe_integer / gpu_safe_arr) validates F64 arrays before GPU dispatch
- Transfer functions (arr_to_gpu_i32 / gpu_i32_to_arr) bridge BqnArr and GpuBuffer
- sort_i32 fixed: XOR 0x8000_0000 before radix sort, XOR back after — correctly orders negative values

## Task Commits

Each task was committed atomically:

1. **Task 1: GPU runtime singleton, CLI flag, debug logging, precision guard, and transfer layer** - `0cdfced` (feat)
2. **Task 2: Fix sort_i32 sign-bit bug** - `85a2a2b` (fix)

**Plan metadata:** (docs commit — see final)

## Files Created/Modified

- `crates/rbqn/src/gpu_runtime.rs` - GpuRuntime singleton, precision guard, transfer functions, debug logging
- `crates/rbqn/src/cli.rs` - Added no_gpu field to Args, --no-gpu long flag, help text entry
- `crates/rbqn/src/main.rs` - Added mod gpu_runtime declaration, gpu_runtime::init(args.no_gpu) call
- `crates/rbqn/Cargo.toml` - Added pollster = "0.4" dependency
- `crates/rbqn-gpu/src/kernels/sort.rs` - Fixed sort_i32 with sign-bit XOR pass

## Decisions Made

- GpuRuntime stored as `OnceLock<Option<GpuRuntime>>` so failed GPU init (no adapter) is identical to --no-gpu: callers just get `None` from `get()`
- Precision guard uses 16_777_216.0 (2^24) as the safe integer threshold — this is the exact limit where f32 loses integer precision
- sort_i32 fix uses CPU-side XOR (download, XOR, upload) rather than a GPU shader pass — simpler and the radix sort kernel dominates cost for large arrays anyway
- squeeze_num applied after gpu_i32_to_arr to compact output to smallest integer type (I8/I16/I32)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `ElementKind` was imported but unused in gpu_runtime.rs (Rule 1 auto-fix: removed from import list before committing). Minor.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- GpuRuntime::get() available for all subsequent plans
- Precision guard ready to gate F64 GPU dispatch
- Transfer functions (arr_to_gpu_i32 / gpu_i32_to_arr) ready for plan 05-02 sort wiring
- sort_i32 correctly handles negative values — regression test in plan 05-05

---
*Phase: 05-gpu-integration*
*Completed: 2026-02-28*

## Self-Check: PASSED

- FOUND: crates/rbqn/src/gpu_runtime.rs (130 lines, >= 80 required)
- FOUND: crates/rbqn/src/cli.rs (contains "no_gpu" 4 times)
- FOUND: crates/rbqn-gpu/src/kernels/sort.rs (contains "0x8000_0000" 2 times)
- FOUND: commit 0cdfced (feat: GPU runtime singleton)
- FOUND: commit 85a2a2b (fix: sort_i32 sign-bit bug)
