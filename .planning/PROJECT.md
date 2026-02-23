# RBQN

## What This Is

A GPU-accelerated Rust implementation of BQN that is a 1:1 drop-in replacement for CBQN. Standalone binary, no external dependencies. Built to run real BQN workloads — numeric, scripting, ML pipelines — with automatic GPU dispatch for large arrays via wgpu compute shaders.

## Core Value

Correct BQN execution with identical behavior to CBQN — if it runs in CBQN, it runs in RBQN.

## Current Milestone: v2.0 — CBQN Drop-in with GPU Acceleration

**Goal:** Standalone `rbqn` binary that passes the full official BQN test suite, with GPU-accelerated primitives for large arrays.

**Target features:**
- Bypass broken runtime0 overrides — populate runtime from native Rust primitives (Option B)
- Working compiler pipeline (compile and execute arbitrary BQN)
- All 44 primitives + all 20 modifiers
- Essential system functions for real-world use
- Pass all 13 official BQN test files
- Self-hosted compiler (no CBQN build dependency)
- GPU integration into primitive dispatch (threshold-gated, 50K+ elements)
- Shareable with Marshall as a standalone `cargo install`

## Requirements

### Validated

- VM: 31 opcodes + 18 optimized variants implemented
- Primitives: 43/44 functions (monadic + dyadic)
- Modifiers: 16/20 working
- GPU infrastructure: wgpu context, buffer pool, pipeline cache, matmul/softmax kernels
- Bootstrap: runtime0 executes, provide array filled (23 basic + 17 extended)
- System functions: Type, Decompose, Fill, GroupLen, GroupOrd

### Active

- [ ] Runtime bypass (Option B): build runtime from native Rust primitives
- [ ] Working compiler: compile and execute arbitrary BQN programs
- [ ] Missing modifiers: Undo (⁼), Under (⌾), Rank (⎉), Depth (⚇)
- [ ] Essential system functions (~44 remaining)
- [ ] Pass all 13 official BQN test files
- [ ] Self-hosted compiler: embed bytecode, remove CBQN dependency
- [ ] GPU dispatch: wire rbqn-gpu into primitive execution for large arrays
- [ ] Standalone binary: `cargo install rbqn` works

### Out of Scope

- FFI (•FFI) — complex, defer to later
- •HashMap — not in core BQN spec
- •term namespace — terminal UI, not needed for drop-in
- Multi-threaded execution — global mutex stores prevent this; future work
- Custom GPU kernels API — internal optimization only

## Context

- Existing codebase: ~8.3K lines Rust across 5 workspace crates (rbqn, rbqn-core, rbqn-prim, rbqn-vm, rbqn-gpu)
- NaN-boxing: B type = tagged u64, load-bearing architecture
- 19 iterations of debugging VM runtime0 overrides failed — Option B (bypass) is the path forward
- GPU uses f32 (wgpu SHADER_F64 not universal) with CPU fallback for f64 precision
- Official BQN test suite: mlochbaum/BQN/test/cases/ (13 files)
- Build-time CBQN dependency exists until self-hosting phase

## Constraints

- **Precision**: BQN uses f64 natively; GPU path uses f32 — must validate accuracy or fall back
- **NaN-boxing**: All values are tagged u64 (B type) — cannot change without rewriting everything
- **Global stores**: ARR_STORE, DERIVED_STORE, NS_STORE behind Mutex — single-threaded only
- **CBQN dependency**: Required at build time until self-hosting milestone achieved

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Option B: bypass runtime0 overrides | 19 debug iterations failed; native Rust primitives already work correctly | — Pending |
| NaN-boxing (B = tagged u64) | Load-bearing architecture, too costly to change | ✓ Good |
| GPU via wgpu compute shaders | Cross-platform, no CUDA dependency | ✓ Good |
| f32 GPU with CPU f64 fallback | SHADER_F64 not universal | ✓ Good |
| Threshold-gated GPU dispatch (50K+) | GPU overhead not worth it for small arrays | ✓ Good |
| Scrap M1 roadmap, replan from scratch | Old roadmap built on dead strategy (debug VM) | — Pending |

---
*Last updated: 2026-02-23 after milestone v2.0 planning*
