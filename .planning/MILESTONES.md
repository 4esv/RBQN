# RBQN Milestones

## ✅ v2.0 — CBQN Drop-in with GPU Acceleration

**Shipped:** 2026-03-03
**Phases:** 1–7 (7 phases, 33 plans, 54 summaries)
**Timeline:** 2026-02-20 → 2026-03-03 (11 days)
**LOC:** ~127K lines Rust

**Delivered:** A standalone `rbqn` binary passing all 13 official BQN test files (1316 tests, 0 failures) with GPU acceleration for large arrays and zero external build dependencies.

**Key accomplishments:**
1. Option B runtime bypass — native Rust primitives replace broken runtime0 strategy; `•PrimInd "+"` returns correct index
2. Full compiler pipeline — compile and execute arbitrary BQN; REPL with variable persistence
3. All 13 official BQN test files green — 1316 tests, 0 failures, including prim, under, undo, header, namespace
4. GPU acceleration — wgpu compute shaders for arithmetic, sort/grade, fold/scan, matmul, softmax; precision guard + kernel fusion
5. Self-hosting — `cargo install rbqn` with no CBQN dependency; bytecode embedded from BQN source
6. CBQN output parity — •Fmt box-drawing, •Repr correct output, clean error messages, •Glyph byte-for-byte match
7. Zero-warning workspace — 0 clippy + 0 compiler warnings; -88 lines in VM dispatch via simplification

**Git range:** feat(gpu): initial workspace scaffold → docs(v2.0): milestone audit

**Archive:** `.planning/milestones/v2.0-ROADMAP.md`, `.planning/milestones/v2.0-REQUIREMENTS.md`

### Known Tech Debt
- SELF-02: self-compiled bytecode swap test not machine-recorded (human verification pending)
- •FFI, •bit, •term: stub implementations (correct error behavior but no real functionality)
- GPU path not exercised by official test suite (arrays too small for dispatch threshold)
- Test scripts have hardcoded /Users/axel/ paths (not portable)
- •math.Comb/LCM monadic: incorrect implementations (return 1.0 and 0.0 respectively)

---
