---
phase: 05-gpu-integration
plan: 03
subsystem: gpu
tags: [gpu, wgpu, fold, scan, reduce, prefix-sum, wgsl]

requires:
  - phase: 05-01
    provides: GpuRuntime singleton, arr_to_gpu_i32, download_i32, should_dispatch, precision guard
  - phase: 05-02
    provides: GPU_FOLD_HOOK and GPU_SCAN_HOOK OnceLock statics in modifiers.rs, register functions

provides:
  - gpu_fold(): dispatches +´ ×´ ⌊´ ⌈´ on arrays >=50K elements to GPU reduce kernel
  - gpu_scan(): dispatches +` on arrays >=50K elements to GPU inclusive prefix sum kernel
  - prim_idx_of(): extracts NativeFn prim_idx from B function value
  - Fixed scan shader propagate bug (workgroup-id vs element-index block mapping)
affects: [test-suite, benchmarks, any future fold/scan optimization]

tech-stack:
  added: []
  patterns:
    - GPU dispatch via OnceLock function pointer hooks (modifiers.rs calls back into rbqn crate)
    - prim_idx_of() pattern for mapping BQN primitives to GPU kernel ops at dispatch time
    - catch_unwind wrapping all GPU dispatch for silent CPU fallback on error

key-files:
  created: []
  modified:
    - crates/rbqn/src/gpu_runtime.rs
    - crates/rbqn-gpu/src/shaders/scan_i32.wgsl
    - crates/rbqn-gpu/src/shaders/scan_f32.wgsl

key-decisions:
  - "prim_idx 0=+, 2=×, 6=⌊, 7=⌈ per PRIM_GLYPHS order — plan had wrong indices (plan said 6=+, 10=×, 12=⌊, 13=⌈ which are provide-array positions)"
  - "scan propagate shader must use idx/512 for block_sums index, not wid.x — workgroup size (256) != scan block size (512) caused off-by-512 errors"
  - "gpu_scan only supports + (prim_idx 0) — GPU scan kernel implements prefix add only, other ops fall back to CPU"

requirements-completed: [GPU-03, GPU-05, GPU-06]

duration: 6min
completed: 2026-02-28
---

# Phase 5 Plan 3: GPU Fold/Scan Dispatch Summary

**GPU-accelerated +´ (reduce) and +` (inclusive scan) wired into fold_c1/scan_c1 via OnceLock hooks, with scan shader propagate bug fixed for correct multi-block prefix sums**

## Performance

- **Duration:** 6 min
- **Started:** 2026-02-28T03:37:30Z
- **Completed:** 2026-02-28T03:44:16Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- GPU fold dispatch: `+´ 100000⥊1` → `[gpu] reduce_add 100000 elements (i32)` + `100000`
- GPU scan dispatch: `+\` 100000⥊1` → `[gpu] scan_add 100000 elements (i32)` + correct prefix sums
- Fixed scan shader propagate bug: wid.x was used to index block_sums but wid.x maps to 256-element workgroups while block_sums has one entry per 512-element scan block
- Small arrays (<50K) use CPU fallback silently; `--no-gpu` suppresses all dispatch

## Task Commits

1. **Task 1+2: GPU fold+scan implementation + shader fix** - `d0601ac` (feat)

Note: GPU_FOLD_HOOK and GPU_SCAN_HOOK infrastructure (modifiers.rs + main.rs registrations) were pre-committed in 05-02 as "pre-existing uncommitted work". This plan adds the actual gpu_fold/gpu_scan implementations and fixes the scan shader.

## Files Created/Modified

- `crates/rbqn/src/gpu_runtime.rs` - Added prim_idx_of(), gpu_fold(), gpu_fold_inner(), gpu_scan(), gpu_scan_inner()
- `crates/rbqn-gpu/src/shaders/scan_i32.wgsl` - Fixed propagate_i32: use `idx/512u` instead of `wid.x` for block_sums index
- `crates/rbqn-gpu/src/shaders/scan_f32.wgsl` - Same propagate fix for f32 variant

## Decisions Made

- prim_idx mapping corrected: plan specified wrong indices. PRIM_GLYPHS ordering: 0=+, 2=×, 6=⌊, 7=⌈. Plan said 6=+, 10=×, 12=⌊, 13=⌈ (those are provide-array positions, not PRIM_GLYPHS indices).
- GPU scan only dispatches for + (prim_idx 0). The inclusive_scan kernel only implements prefix-add; other ops return None and fall back to CPU.
- prim_idx_of() function placed in gpu_runtime.rs rather than modifiers.rs because it needs rbqn_vm::derive::get_derived, which is available in the rbqn crate but would create a circular dep in rbqn-vm.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Wrong prim_idx mapping in plan**
- **Found during:** Task 1 (implementing gpu_fold_inner)
- **Issue:** Plan specified prim_idx 6=+, 10=×, 12=⌊, 13=⌈ but these are provide-array indices. PRIM_GLYPHS string indices are 0=+, 2=×, 6=⌊, 7=⌈ and NativeFn.prim_idx uses PRIM_GLYPHS ordering.
- **Fix:** Used correct PRIM_GLYPHS indices; verified against fold_identity() in modifiers.rs which uses same mapping
- **Files modified:** crates/rbqn/src/gpu_runtime.rs
- **Committed in:** d0601ac

**2. [Rule 1 - Bug] Scan shader propagate used wid.x instead of element-based block index**
- **Found during:** Task 2 (testing gpu_scan)
- **Issue:** propagate_i32/f32 used `prefix_sums[wid.x]` but propagate workgroup_size=256 while scan blocks=512 elements. Each scan block's prefix sum needs to be applied to 512 elements, but propagate dispatches a workgroup per 256 elements. wid.x=1 (elements 256-511) wrongly read prefix_sums[1] (block 1's prefix) instead of prefix_sums[0] (block 0's prefix). Result: elements 256-511 were off by 512.
- **Fix:** Changed to `let block_idx = idx / 512u; prefix_sums[block_idx]` in both scan_i32.wgsl and scan_f32.wgsl
- **Files modified:** crates/rbqn-gpu/src/shaders/scan_i32.wgsl, crates/rbqn-gpu/src/shaders/scan_f32.wgsl
- **Verification:** `¯1⊑+\` 100000⥊1` returns 100000, sequence shows 1,2,3,...,257,...
- **Committed in:** d0601ac

---

**Total deviations:** 2 auto-fixed (2 Rule 1 bugs)
**Impact on plan:** Both fixes required for correctness. No scope creep.

## Issues Encountered

- GPU_FOLD_HOOK, GPU_SCAN_HOOK statics and register_gpu_fold/register_gpu_scan in modifiers.rs + main.rs were committed by the 05-02 agent as "pre-existing uncommitted work". This plan's implementations built on that infrastructure correctly.

## Next Phase Readiness

- Primary Phase 5 success criterion satisfied: `+´ 1e5⥊1` dispatches to GPU and returns 100000
- GPU scan (+`) dispatches correctly with accurate prefix sums
- Ready for 05-04 and beyond

---
*Phase: 05-gpu-integration*
*Completed: 2026-02-28*
