# Phase 5: GPU Integration - Context

**Gathered:** 2026-02-27
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire existing rbqn-gpu kernels into BQN primitive dispatch so arrays above 50K elements transparently run on GPU. Covers arithmetic, sort/grade, fold, scan, matmul, softmax, and kernel fusion. Precision guards prevent incorrect f32 results. Benchmarks prove speedup on Apple Silicon. All 13 test files must still pass.

</domain>

<decisions>
## Implementation Decisions

### Kernel priorities
- Both ML (matmul, softmax) and general array ops (arithmetic, reduce, scan, sort) are first-class — no priority ordering
- Include matmul and softmax kernels in Phase 5 since the code already exists in rbqn-gpu
- Kernel fusion (GPU-08) is a must-have — chained elementwise ops like `2×a+b` should fuse into a single GPU dispatch to avoid round-trips

### Fallback behavior
- No GPU available: silent CPU fallback, no message at all — GPU is an invisible optimization
- GPU error mid-computation (driver crash, OOM): retry on CPU and log a warning so user knows GPU had issues
- Kill switch: `--no-gpu` CLI flag disables all GPU dispatch (no env var)
- GPU code always compiled in (no cargo feature gate) — runtime detection handles availability

### Claude's Discretion
- Sort/grade GPU scope: Claude decides which array types get GPU sort based on what radix sort handles cleanly (numeric-only vs including char ordinals)
- Dispatch threshold tuning: 50K is the starting point per requirements, benchmarks may adjust
- Debug flag format for "visible via debug flag" success criterion
- Buffer pooling and transfer strategy details

</decisions>

<specifics>
## Specific Ideas

- User wants GPU to be an invisible optimization — correctness always matches CPU, GPU is purely a speed layer
- Fusion is critical for ML workloads where chained elementwise ops dominate
- wgpu device, buffer pool, pipeline cache, and kernel stubs already exist in crates/rbqn-gpu/

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 05-gpu-integration*
*Context gathered: 2026-02-27*
