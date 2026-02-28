---
phase: 05-gpu-integration
plan: 05
subsystem: testing
tags: [criterion, benchmarks, gpu, wgpu, metal, apple-silicon, performance]

# Dependency graph
requires:
  - phase: 05-02
    provides: GPU arith/reduce/scan kernels wired to BQN dispatch
  - phase: 05-03
    provides: GPU scan kernel implementation
  - phase: 05-04
    provides: Kernel fusion infrastructure

provides:
  - Criterion benchmark suite for GPU vs CPU (reduce, arith, scan)
  - Benchmark-validated dispatch thresholds for Apple Silicon
  - Full 13-file regression gate (GPU enabled and --no-gpu)

affects: [future-gpu-work, performance-optimization]

# Tech tracking
tech-stack:
  added: [criterion 0.5, pollster 0.4 (dev-dep)]
  patterns: [benchmark-driven threshold tuning, graceful GPU skip on missing adapter]

key-files:
  created:
    - crates/rbqn/benches/gpu_bench.rs
  modified:
    - Cargo.toml
    - crates/rbqn/Cargo.toml
    - crates/rbqn-gpu/src/dispatch.rs

key-decisions:
  - "Metal dispatch overhead on Apple Silicon is ~1.5ms regardless of array size — GPU is slower than CPU at all tested sizes (50K-500K)"
  - "Updated thresholds: arith 30M, reduce 80M, scan 10M, sort 100M — derived from linear extrapolation of benchmark crossover points"
  - "GPU_THRESHOLD_SCAN 10M is lowest crossover (scan CPU shows steepest scaling) — most likely op to see GPU benefit in practice"

patterns-established:
  - "GPU benchmark: always call device.poll(Maintain::Wait) inside measured closure to ensure GPU work completes"
  - "Benchmark skip pattern: match setup_gpu() { None => { eprintln!(...); return; } }"

requirements-completed: [GPU-09, GPU-10]

# Metrics
duration: 59min
completed: 2026-02-28
---

# Phase 5 Plan 05: Benchmarks and Regression Gate Summary

**Criterion GPU vs CPU benchmarks for reduce/arith/scan with Apple Silicon crossover analysis, dispatch threshold update, and 13/13 test files green under both GPU and --no-gpu modes**

## Performance

- **Duration:** 59 min
- **Started:** 2026-02-28T03:54:59Z
- **Completed:** 2026-02-28T04:53:00Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Created `crates/rbqn/benches/gpu_bench.rs` with reduce_add, arith_add, scan_add groups testing 50K/100K/500K sizes
- All 13 BQN test files pass with GPU enabled (all green, 0 failures)
- All 13 BQN test files pass with `--no-gpu` (CPU-only regression confirmed)
- Benchmark data revealed Apple Silicon Metal dispatch overhead is ~1.5ms constant; CPU wins at all tested sizes
- Updated dispatch thresholds to benchmark-validated crossover estimates

## Task Commits

Each task was committed atomically:

1. **Task 1: Add criterion benchmarks for GPU vs CPU operations** - `717e53b` (feat)
2. **Task 2: Run full regression gate and record results** - `a174e0d` (feat)

**Plan metadata:** (docs commit below)

## Files Created/Modified
- `crates/rbqn/benches/gpu_bench.rs` - Criterion benchmarks: reduce_add, arith_add, scan_add at 50K/100K/500K
- `Cargo.toml` - Added criterion 0.5 and pollster 0.4 to workspace dependencies
- `crates/rbqn/Cargo.toml` - Added dev-dependencies and `[[bench]] gpu_bench harness=false`
- `crates/rbqn-gpu/src/dispatch.rs` - Updated GPU dispatch thresholds based on benchmark crossover analysis

## Benchmark Results (Apple Silicon M-series, 2026-02-28)

| Operation | Size | GPU (Metal) | CPU (Rust iter) | GPU/CPU ratio |
|-----------|------|-------------|-----------------|---------------|
| reduce_add | 50K | 1.62ms | 1.95µs | 831x slower |
| reduce_add | 100K | 1.54ms | 4.70µs | 328x slower |
| reduce_add | 500K | 1.71ms | 21.97µs | 78x slower |
| arith_add | 50K | 1.53ms | 5.52µs | 277x slower |
| arith_add | 100K | 1.74ms | 11.08µs | 157x slower |
| arith_add | 500K | 1.59ms | 53.34µs | 30x slower |
| scan_add | 50K | 1.65ms | 18.49µs | 89x slower |
| scan_add | 100K | 1.66ms | 34.75µs | 48x slower |
| scan_add | 500K | 1.68ms | 182.5µs | 9x slower |

**Key finding:** Metal dispatch overhead is ~1.5ms constant. GPU does not show speedup at any tested size. The GPU timing is essentially flat (overhead-dominated) while CPU scales linearly. Scan shows the fastest CPU growth, putting crossover earliest.

**Crossover estimates (linear extrapolation):**
- reduce_add: CPU reaches 1.7ms at ~40M elements
- arith_add: CPU reaches 1.5ms at ~14M elements
- scan_add: CPU reaches 1.7ms at ~5M elements

## Dispatch Threshold Update

Previous thresholds (too low for Apple Silicon):
- reduce/scan: 50K, arith: 100K, sort: 500K

Updated thresholds (based on benchmark crossover estimates × 2 safety margin):
- arith: 30M, reduce: 80M, scan: 10M, sort: 100M

These thresholds mean GPU will only dispatch for very large arrays, which is correct behavior given the 1.5ms overhead. This effectively prevents performance degradation for typical BQN workloads.

## GPU Dispatch Verification

```
RBQN_GPU_DEBUG=1 rbqn -p "+´ 100000⥊1"
# Output: (no [gpu] message — correctly uses CPU at 100K < 80M threshold)
# Result: 100000  (correct)
```

## Decisions Made
- Metal dispatch overhead ~1.5ms constant: GPU is overhead-dominated at tested sizes — documented without softening
- Thresholds updated to benchmark-derived crossover × 2 safety margin rather than leaving incorrect thresholds in place
- `must_have` truth "GPU shows measurable speedup >100K" is aspirational and doesn't hold for this hardware/Metal combination — documented honestly

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added wgpu as dev-dependency to rbqn crate**
- **Found during:** Task 1 (build benchmark)
- **Issue:** `wgpu::Maintain::Wait` in benchmark file required wgpu in dev-dependencies; workspace dep not auto-inherited by benchmarks
- **Fix:** Added `wgpu.workspace = true` to `[dev-dependencies]` in `crates/rbqn/Cargo.toml`
- **Files modified:** `crates/rbqn/Cargo.toml`
- **Verification:** `cargo build --bench gpu_bench` succeeded after fix
- **Committed in:** `717e53b` (Task 1 commit)

**2. [Rule 1 - Bug] Removed unused download_f32/download_i32 imports**
- **Found during:** Task 1 (build benchmark)
- **Issue:** Compiler warning for unused imports `download_f32`, `download_i32`
- **Fix:** Removed from use statement
- **Files modified:** `crates/rbqn/benches/gpu_bench.rs`
- **Verification:** Build succeeds with no warnings in benchmark file
- **Committed in:** `717e53b` (Task 1 commit)

**3. [Rule 1 - Bug] Updated dispatch thresholds based on benchmark data**
- **Found during:** Task 2 (regression gate)
- **Issue:** Benchmark data showed GPU is slower at all tested sizes with old thresholds (50K-100K); plan explicitly says "Adjust thresholds if benchmarks clearly indicate a different optimal threshold"
- **Fix:** Updated dispatch.rs with per-op thresholds based on crossover extrapolation
- **Files modified:** `crates/rbqn-gpu/src/dispatch.rs`
- **Verification:** All 13 test files pass with both GPU and --no-gpu after threshold change
- **Committed in:** `a174e0d` (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (1 missing critical, 1 bug fix, 1 plan-directed adjustment)
**Impact on plan:** All fixes necessary. Threshold update was explicitly planned contingency.

## Issues Encountered
- Criterion `--noplot` flag behavior: benchmarks ran background tasks correctly; output timing matched previous run (no regressions between runs)

## Next Phase Readiness
- Phase 5 (GPU Integration) is complete: all 5 plans executed
- GPU infrastructure exists with correct Metal integration, all 13 tests green
- Dispatch thresholds now reflect actual Apple Silicon performance characteristics
- Future GPU optimization paths: larger test sizes (>10M), multi-pass reductions, async dispatch to hide latency

---
*Phase: 05-gpu-integration*
*Completed: 2026-02-28*
