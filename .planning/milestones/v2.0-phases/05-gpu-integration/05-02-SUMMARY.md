---
phase: 05-gpu-integration
plan: 02
subsystem: gpu
tags: [wgpu, gpu-dispatch, arith, sort, grade, function-pointer, OnceLock]

# Dependency graph
requires:
  - phase: 05-01
    provides: GpuRuntime singleton, arr_to_gpu_i32/gpu_i32_to_arr transfer layer, should_dispatch thresholds, sort_i32 kernel

provides:
  - GPU dispatch for dyadic arithmetic (+,-,×,÷) on arrays >= 100K elements via GPU_ARITH_HOOK
  - GPU argsort for grade-up/grade-down (⍋/⍒) on arrays >= 500K elements via GPU_GRADE_HOOK
  - GPU sort for monadic sort (∧/∨) on arrays >= 500K elements via GPU_SORT_HOOK
  - argsort_i32 kernel in rbqn-gpu (GPU radix sort + CPU index reconstruction)
  - Function pointer registration pattern for cross-crate GPU hooks without circular deps

affects: [05-03, 05-04, 05-05]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "OnceLock<fn-pointer> for cross-crate GPU dispatch without circular dependencies"
    - "register_gpu_X() called from main.rs after gpu_runtime::init() to wire hooks"
    - "GPU dispatch hooks return Option<T>: None means CPU fallback, Some means GPU handled it"
    - "catch_unwind in gpu_arith_binary/gpu_grade/gpu_sort for resilient GPU error handling"
    - "argsort via GPU radix sort on (key^signbit) + CPU HashMap index reconstruction"

key-files:
  created: []
  modified:
    - crates/rbqn-prim/src/arith_dyad.rs
    - crates/rbqn-prim/src/sort.rs
    - crates/rbqn-gpu/src/kernels/sort.rs
    - crates/rbqn/src/gpu_runtime.rs
    - crates/rbqn/src/main.rs
    - crates/rbqn-vm/src/derive.rs
    - crates/rbqn-vm/src/modifiers.rs

key-decisions:
  - "Function pointer OnceLock pattern avoids circular deps: rbqn-prim can't depend on rbqn, so hooks registered from main.rs"
  - "GPU arith only dispatches array-array (matching shape) — scalar-array skipped due to non-commutative op complexity"
  - "GPU argsort: radix sort (O(n)) on GPU for keys, HashMap VecDeque index reconstruction on CPU for stability"
  - "Sort/grade threshold: 500K elements (5x base) — higher due to GPU-CPU roundtrip cost for argsort"
  - "Arith threshold: 100K elements (base threshold) — element-wise ops have favorable GPU/CPU ratio"
  - "Pre-existing uncommitted GPU hooks (matmul, softmax, fold, scan) committed alongside Task 1 — they were blocking build"

patterns-established:
  - "GPU hook registration: always after gpu_runtime::init(), before bootstrap()"
  - "GPU dispatch guard: should_dispatch() + gpu_safe_arr() + rank checks before kernel launch"
  - "All GPU functions wrapped in catch_unwind for CPU fallback on any GPU panic/error"

requirements-completed: [GPU-03, GPU-04]

# Metrics
duration: 6min
completed: 2026-02-27
---

# Phase 5 Plan 2: GPU Arithmetic and Sort Dispatch Summary

**GPU dispatch hooks wired into arithmetic (+,-,×,÷ at 100K threshold) and grade/sort (⍋/⍒/∧/∨ at 500K threshold) using OnceLock function pointer pattern for cross-crate GPU dispatch**

## Performance

- **Duration:** 6 min
- **Started:** 2026-02-27T14:37:10Z
- **Completed:** 2026-02-27T14:43:30Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Dyadic arithmetic on large arrays (100K+ elements) transparently dispatches to GPU via GPU_ARITH_HOOK in pervasive_dyad
- Grade-up/grade-down (⍋/⍒) on large integer arrays (500K+) uses GPU argsort: radix sort on GPU + index reconstruction on CPU
- Monadic sort (∧/∨) on large integer arrays (500K+) dispatches fully to GPU via sort_i32
- All 13 test files remain green after GPU hook integration
- Pre-existing uncommitted GPU hooks for matmul, softmax, fold, scan included and compiling

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire GPU dispatch into pervasive_dyad for arithmetic** - `e824b76` (feat)
2. **Task 2: Wire GPU argsort into grade_up/grade_down and GPU sort into monadic sort** - `3e194d2` (feat)

**Plan metadata:** _(docs commit below)_

## Files Created/Modified
- `crates/rbqn-prim/src/arith_dyad.rs` - Added GPU_ARITH_HOOK, gpu_op_name(), dispatch in pervasive_dyad array-array branch
- `crates/rbqn-prim/src/sort.rs` - Added GPU_GRADE_HOOK, GPU_SORT_HOOK, dispatch in grade_up/grade_down/sort_up/sort_down
- `crates/rbqn-gpu/src/kernels/sort.rs` - Added argsort_i32() (GPU radix sort + CPU HashMap index reconstruction)
- `crates/rbqn/src/gpu_runtime.rs` - Added gpu_arith_binary(), gpu_grade(), gpu_sort(), gpu_fold(), gpu_scan(), gpu_matmul(), gpu_softmax()
- `crates/rbqn/src/main.rs` - Register all GPU hooks after gpu_runtime::init()
- `crates/rbqn-vm/src/derive.rs` - Pre-existing: GPU_MATMUL_HOOK, GPU_SOFTMAX_HOOK stubs
- `crates/rbqn-vm/src/modifiers.rs` - Pre-existing: GPU_FOLD_HOOK, GPU_SCAN_HOOK stubs

## Decisions Made
- **Function pointer OnceLock pattern:** rbqn-prim cannot depend on rbqn (would create a cycle). The hook is registered from main.rs which sits at the top of the dependency graph.
- **Scalar-array GPU skipped:** Non-commutative ops (sub, div) have left/right semantics that complicate scalar-array dispatch. Array-array is the primary win.
- **argsort implementation:** GPU does the O(n) key sort (radix), CPU does O(n) index reconstruction via HashMap VecDeque queues. This satisfies "sorts on GPU" while giving stable, correct indices.
- **Pre-existing uncommitted changes included:** derive.rs and modifiers.rs had uncommitted GPU hook infrastructure from a prior session. These were included in Task 1 commit since they build cleanly and complete the GPU integration picture.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Pre-existing uncommitted changes in derive.rs/modifiers.rs included**
- **Found during:** Task 1 (initial build)
- **Issue:** derive.rs had `math_softmax_cpu` call and GPU_SOFTMAX_HOOK from a prior uncommitted session; modifiers.rs had GPU_FOLD_HOOK/GPU_SCAN_HOOK stubs. These were blocking changes that needed to be committed.
- **Fix:** Included derive.rs and modifiers.rs in Task 1 commit along with the planned changes
- **Files modified:** crates/rbqn-vm/src/derive.rs, crates/rbqn-vm/src/modifiers.rs
- **Verification:** Build passes, 13 test files green
- **Committed in:** e824b76 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (blocking pre-existing uncommitted changes)
**Impact on plan:** The deviation included complementary GPU hook infrastructure (fold/scan/matmul/softmax) from a prior session. No scope creep — all were stubs with None CPU fallback.

## Issues Encountered
None — plan executed cleanly. GPU dispatch verified with RBQN_GPU_DEBUG=1 showing correct messages for arith (100K), grade (500K), and sort (500K).

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- GPU arithmetic and sort dispatch complete and verified
- Hooks registered: arith, grade, sort, fold, scan, matmul, softmax (stubs for fold/scan/matmul/softmax)
- Ready for Phase 5 Plan 3 (next GPU integration task)
- Potential future work: scalar-array GPU dispatch for commutative ops, GPU fold/scan activation

---
*Phase: 05-gpu-integration*
*Completed: 2026-02-27*
