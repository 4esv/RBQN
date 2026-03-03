# RBQN

## What This Is

A GPU-accelerated Rust implementation of BQN that is a 1:1 drop-in replacement for CBQN. Standalone binary via `cargo install rbqn` — no external dependencies. Runs real BQN workloads — numeric, scripting, ML pipelines — with automatic GPU dispatch for large arrays via wgpu compute shaders on Apple Silicon and other GPU backends.

## Core Value

Correct BQN execution with identical behavior to CBQN — if it runs in CBQN, it runs in RBQN.

## Requirements

### Validated

- ✓ Runtime bypass (Option B): native Rust primitives serve as runtime — v2.0
- ✓ Working compiler pipeline: compile and execute arbitrary BQN programs — v2.0
- ✓ All 44 primitives + all 20 modifiers — v2.0
- ✓ Essential system functions (~50: file I/O, math, rand, platform, SH, Import, introspection) — v2.0
- ✓ All 13 official BQN test files passing (1316 tests, 0 failures) — v2.0
- ✓ Self-hosted compiler: embedded bytecode, `cargo install rbqn` with no CBQN dependency — v2.0
- ✓ GPU dispatch: wired into arithmetic, sort/grade, fold/scan, matmul, softmax — v2.0
- ✓ Precision guard: f64 arrays fall back to CPU; integer-valued dispatch to GPU — v2.0
- ✓ Zero-warning workspace (0 clippy + 0 compiler warnings) — v2.0
- ✓ CBQN output parity: •Fmt box-drawing, •Repr, clean errors, •Glyph — v2.0

### Active

*(Planning for v3.0)*

- [ ] SIMD-accelerated primitive dispatch for CPU path (PERF-01)
- [ ] Type-specialized array dispatch (avoid boxing for homogeneous arrays) (PERF-02)
- [ ] Full •FFI implementation (currently stubs NotImplemented)
- [ ] Full •bit/•term namespace implementation (currently partial stubs)
- [ ] Self-compiled bytecode human verification (SELF-02 swap test)
- [ ] Portable test scripts (remove hardcoded /Users/axel/ paths)
- [ ] •math.Comb monadic / •math.LCM monadic correct implementations

### Out of Scope

| Feature | Reason |
|---------|--------|
| Multi-threaded execution | Global mutex stores (ARR_STORE, DERIVED_STORE, NS_STORE) prevent this; architectural change needed |
| Custom GPU kernel API | Internal optimization only — no user-facing GPU interface |
| JIT compilation | Premature optimization — correctness first |
| GUI/IDE integration | Not part of CLI drop-in replacement |
| Package manager | Not in CBQN, not needed for drop-in |

## Context

**Shipped v2.0:** 2026-03-03

- ~127K lines Rust across 5 workspace crates: rbqn (CLI+REPL), rbqn-core (types/NaN-boxing), rbqn-vm (bytecode VM), rbqn-prim (44 primitives + 20 modifiers), rbqn-gpu (wgpu compute)
- NaN-boxing: B type = tagged u64 — load-bearing, cannot change without rewriting everything
- GPU uses f32 (wgpu SHADER_F64 not universal) with CPU fallback for f64 precision
- GPU dispatch thresholds: Metal ~1.5ms overhead; crossover at 30M–100M elements (official test suite arrays are too small to exercise GPU path)
- Self-hosting: four .bin files committed (runtime0.bin, runtime1x.bin, compiler.bin, formatter.bin), compiled from BQN source by Marshall Lochbaum

**Attribution:**
- BQN language and compiler source: Marshall Lochbaum (ISC License)
- CBQN reference implementation: dzaima and contributors (LGPL/GPL/MPL)

## Constraints

- **Precision**: BQN uses f64 natively; GPU path uses f32 — precision guard required
- **NaN-boxing**: All values are tagged u64 (B type) — cannot change without full rewrite
- **Global stores**: ARR_STORE, DERIVED_STORE, NS_STORE behind Mutex — single-threaded only for now
- **CBQN dependency**: Required at build time only for regenerating .bin files (rbqn-gen), not for normal build/install

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Option B: bypass runtime0 overrides | 19 debug iterations failed; native Rust primitives already work correctly | ✓ Correct — unblocked everything |
| NaN-boxing (B = tagged u64) | Load-bearing architecture, too costly to change mid-milestone | ✓ Good |
| GPU via wgpu compute shaders | Cross-platform, no CUDA dependency | ✓ Good |
| f32 GPU with CPU f64 fallback | SHADER_F64 not universal on all GPUs | ✓ Good |
| Threshold-gated GPU dispatch (30M–100M elements) | GPU overhead not worth it for small arrays | ✓ Good (but thresholds mean GPU path not exercised by test suite) |
| Embed bytecode in binary (.bin committed to repo) | Eliminates CBQN build dependency for users | ✓ Good |
| 5-crate workspace (core/vm/prim/gpu/bin) | Separation of concerns; gpu crate optional | ✓ Good |
| runtime1x over runtime1 in provide array | Extended provide (40 entries) gives more primitives at bootstrap | ✓ Correct |
| Code simplification as milestone phase | -88 lines in VM dispatch, shared helpers, zero warnings | ✓ Worth it |

---
*Last updated: 2026-03-03 after v2.0 milestone*
