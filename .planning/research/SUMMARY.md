# Project Research Summary

**Project:** RBQN — GPU-accelerated BQN interpreter in Rust
**Domain:** Array language interpreter — runtime bypass, GPU integration, self-hosting compiler
**Researched:** 2026-02-23
**Confidence:** HIGH

## Executive Summary

RBQN is a Rust implementation of the BQN array programming language targeting CBQN drop-in compatibility plus GPU acceleration as a differentiator. The codebase is structurally complete — all five layers (rbqn-core, rbqn-prim, rbqn-vm, rbqn-gpu, rbqn binary) exist with correct architecture — but the interpreter is not functional. The core blocker is that the runtime bypass (Option B) must be verified working before anything else. Once native Rust primitives are correctly wired as the runtime array, the compiler pipeline unblocks, which in turn unblocks the official test suite, which gates self-hosting and GPU integration.

The recommended build order is strictly serial in the first three phases: verify runtime bypass → establish test infrastructure → achieve language completeness. GPU integration is explicitly Phase 4 and self-hosting is Phase 5. Attempting GPU integration before the test suite passes wastes effort on acceleration of broken results. Attempting self-hosting before 13/13 test files pass risks embedding wrong bytecode that is harder to debug than the original failure. Every research file confirms this ordering independently.

The key risks are: (1) the inverse system (`⁼`) silently breaking due to `•PrimInd` returning -1 when native fruntime entries are not used as the runtime array — this must be asserted immediately after bypass; (2) GPU f32 precision loss silently producing wrong results on f64 BQN arrays — requires a precision guard before wiring any GPU arithmetic; (3) `catch_unwind` around `setInv`/`setPrims` callbacks swallowing panics and leaving the inverse system unconfigured with no error output. All three are preventable with targeted assertions at phase boundaries.

---

## Key Findings

### Recommended Stack

No new runtime dependencies are needed. The workspace already contains wgpu 24.0.5, bytemuck, pollster 0.4, and rustyline 15.0.0 — all correct for their purposes. `rbqn-prim` already declares `rbqn-gpu` as a dependency but calls nothing from it. The GPU integration work is purely wiring, not adding infrastructure.

Two dev-only additions are recommended: `criterion 0.8` for statistically rigorous CPU vs GPU benchmarks (required to validate the 100K element GPU threshold), and `insta 1` for snapshot testing against official BQN test output. The existing `test_cbqn_compat.sh` shell harness is a useful prototype but does not gate CI. `std::sync::OnceLock` (stable since Rust 1.70) replaces any need for `once_cell` or `lazy_static` for the GPU context singleton.

**Core technologies:**
- `wgpu 24.0.5`: GPU compute via Metal/Vulkan/DX12 — already in workspace, correct choice
- `bytemuck`: Safe f32/i32 byte casting for GPU buffer transfers — already in workspace
- `pollster 0.4`: `block_on` to bridge async wgpu init into synchronous VM — already in `rbqn-gpu`
- `std::sync::OnceLock`: GPU context singleton, lazy init — stdlib, no new dep
- `criterion 0.8` (dev): Statistical benchmarks for GPU threshold validation
- `insta 1` (dev): Snapshot testing against CBQN reference output

### Expected Features

The MVP is defined as: RBQN passes all 13 official BQN test files and installs via `cargo install`. Everything in P1 below is required for that definition. GPU features are P2 — the differentiating value proposition against CBQN which has no GPU support at all.

**Must have (table stakes — P1):**
- Runtime bypass (Option B): without it, native primitives are shadowed; 3/21 compat tests currently pass
- Working compiler pipeline: follows directly from runtime bypass; required for all test files
- Undo (`⁼`) native inverse table: required for `undo.bqn`; all 14 spec-required inverses must be registered
- Under (`⌾`) structural mode: required for `under.bqn`; native implementations for ⌽, ⍉, k⊸⊏, k⊸↑, k⊸↓ minimum
- Fill element propagation: required for `fill.bqn`; fill through ↑, «, », >, ⥊↑
- DYNM opcode fix: `a↩expr` currently returns SENTINEL instead of mutating variable
- REPL state persistence: `a←5` on line 1 must be visible on line 2
- `•Out`, `•Exit`, `•args`, `•BQN`: required for any real BQN script and by the test runner
- Self-hosting: removes `CBQN_PATH` build dependency; enables `cargo install rbqn`
- Monadic shifts `«`/`»` c1: trivial but currently None in dispatch

**Should have (competitive — P2):**
- GPU array transfer layer: `BqnArr ↔ GpuBuffer` bridge; unlocks all GPU primitives
- GPU dispatch for `+`, `×`, `-`, `÷` on arrays >100K elements: CBQN has no GPU support at all
- GPU fold/scan (`+´`, `+\`): bottlenecks in large array workloads
- GPU matmul: killer demo for BQN as ML preprocessing language
- Benchmarks: CPU vs GPU timing with criterion; validates 100K threshold

**Defer (v2+):**
- Garbage collection: monotonic stores leak but not blocking for correctness
- Result-based error propagation: 139 panic sites; long-term technical debt
- `•Import`, `•file` namespace: multi-file scripting
- `•FFI`: defer indefinitely; out of scope per PROJECT.md
- Multi-threaded VM: requires rearchitecting global Mutex stores

### Architecture Approach

RBQN uses a five-crate layered architecture with strict dependency direction: `rbqn-core` (NaN-boxed values, arrays, global stores) ← `rbqn-gpu` (wgpu kernels) ← `rbqn-prim` (primitive dispatch, GPU integration point) ← `rbqn-vm` (compiler, VM, modifiers) ← `rbqn` (bootstrap, REPL, CLI). This boundary is correct and must not be violated — GPU state must not leak into `rbqn-core`, and primitive implementations must stay in `rbqn-prim`. The architecture correctly places GPU dispatch in `rbqn-prim` where array data is available and the CPU fallback is co-located.

**Major components:**
1. `rbqn-core/value.rs` + `array.rs`: NaN-boxed B(u64) type system and BqnArr with typed ArrData — stable, do not change
2. `rbqn-prim/dispatch.rs` + primitive modules: 64-entry dispatch table; GPU integration point via new `gpu_ctx.rs` + `gpu_transfer.rs`
3. `rbqn-vm/compiler.rs` + `vm.rs` + `derive.rs`: BQN bytecode compiler, opcode interpreter, derived function dispatch — stable
4. `rbqn-vm/modifiers.rs`: `¨ ´ `` ˘ ⌜` and stubs for `⁼ ⌾ ⎉ ⚇` — needs Undo and Under completion
5. `rbqn/bootstrap.rs`: 4-stage bootstrap (fruntime → runtime_0 → runtime1 → compiler → formatter) — runtime bypass already in place
6. `rbqn-gpu/`: GpuContext, GpuBuffer, PipelineCache, WGSL kernels — complete infrastructure, zero callers

### Critical Pitfalls

1. **PrimInd breaks silently after runtime bypass** — The runtime array passed to the compiler must be `fruntime` (native NativeFn entries), not `rtObjRaw` (BQN-wrapped). If `rtObjRaw` is used, `•PrimInd "+"` returns ¯1 instead of 0 and the entire inverse system (`⁼`) silently fails. Assert `•PrimInd "+" = 0` immediately after bootstrap as a regression guard. (This is already correct in current bootstrap.rs — the risk is future refactor regression.)

2. **`setInv`/`setPrims` callbacks swallowed by `catch_unwind`** — bootstrap.rs wraps these in `catch_unwind` and discards panics. A panic means `⌾` (Under) and `⁼` (Undo) have no inverse tables and silently compute wrong results. Remove `catch_unwind` around these calls; let them fail loudly so the root cause is visible.

3. **GPU f32 precision loss on f64 BQN arrays** — All GPU kernels use f32; BQN semantics are f64. For arrays with values above 2^24 or precision-sensitive arithmetic, the GPU path silently returns wrong answers. Add a precision guard to `should_use_gpu` before wiring any GPU arithmetic. Initial GPU dispatch should be limited to sort/grade (ordinal operations are precision-insensitive).

4. **GPU async/sync deadlock via missing staging buffers** — Current `GpuBuffer::storage()` has `STORAGE | COPY_SRC | COPY_DST` but not `MAP_READ`. Downloading data requires a staging buffer that is not currently pooled. Every GPU readback creates a new staging buffer — add to `BufferPool` before any kernel dispatch wiring, and always use `device.poll(wgpu::Maintain::Wait)` for synchronous readback.

5. **Self-hosting before test suite passes is wasted effort** — The CBQN compiler source (`c.bqn`) is ~3000 lines that exercises every language feature. Attempting to compile it before the interpreter is correct will immediately crash with misleading errors. Self-hosting is gated on 13/13 test files passing — no exceptions.

---

## Implications for Roadmap

Based on research, 5-phase structure with strict serial ordering for phases 1-3:

### Phase 1: Runtime Bypass Verification
**Rationale:** The bypass (Option B) is already implemented in bootstrap.rs. Phase 1 is verification, not implementation — confirm all 21 compat test expressions pass and add regression guards. Everything else is blocked until this phase is green.
**Delivers:** Correctly executing native Rust primitives; working compiler pipeline; DYNM/ALIM opcode fixes; REPL state persistence
**Addresses:** Runtime bypass, working compiler pipeline, DYNM fix, REPL persistence, `•Out`/`•Exit`/`•args`/`•BQN` (the trivial ones)
**Avoids:** PrimInd regression (add assertion); `setInv` swallowed by catch_unwind (remove catch_unwind); Wrong env argument format to compiler (env is `⟨runtime, sysval_resolver, var_names, var_depths⟩`)

### Phase 2: Test Infrastructure
**Rationale:** Before investing in language completeness features, establish a quantitative baseline. Run the official test suite immediately after compiler pipeline works. Run in order of increasing difficulty: `simple.bqn` → `literal.bqn` → `syntax.bqn` → `prim.bqn` → `bytecode.bqn` → `fill.bqn` → `identity.bqn`.
**Delivers:** Known pass/fail count across all 13 official test files; CI gate that prevents regression
**Uses:** `insta 1` for snapshot testing; existing `test_cbqn_compat.sh` as reference
**Avoids:** False confidence from manual `1+1` testing; deferring fill/identity tests (fill failures cascade into prim.bqn and identity.bqn)

### Phase 3: Language Completeness
**Rationale:** Drive from 0/13 to 13/13 test files. The hard features (Undo, Under, Fill) must be completed here. They are complex but their scope is well-defined by the BQN spec.
**Delivers:** 13/13 official BQN test files passing; complete `⁼` inverse table (14 required primitives); structural `⌾` for ⌽, ⍉, k⊸⊏, k⊸↑, k⊸↓; fill propagation through ↑, «, », >, ⥊↑; Rank (`⎉`) and Depth (`⚇`) correct semantics
**Implements:** `rbqn-prim/inverse.rs` (complete), `rbqn-vm/modifiers.rs` (Undo + Under native), `rbqn-prim/sysfn.rs` (44+ system functions), `rbqn/src/repl.rs` (persistent scope)
**Avoids:** Character arithmetic returning numeric instead of char type; empty fold returning 0 instead of identity element; pervasive arithmetic not descending into nested boxes

### Phase 4: GPU Integration
**Rationale:** GPU is the differentiator against CBQN. Integration is safe only after the interpreter is correct — GPU dispatch on broken primitives produces wrong-but-fast results. All GPU infrastructure (GpuContext, kernels, dispatch thresholds) already exists; this phase is wiring.
**Delivers:** Transparent CPU/GPU dispatch for arrays >100K elements; GPU matmul demo; criterion benchmarks validating 100K threshold
**Uses:** `wgpu 24.0.5`, `bytemuck`, `pollster` (all existing); `criterion 0.8` (new dev dep); `OnceLock` for lazy GPU init
**Implements:** `rbqn-prim/src/gpu_ctx.rs` (OnceLock singleton), `rbqn-prim/src/gpu_transfer.rs` (f64↔f32 with precision guard), GPU dispatch in add_c2/mul_c2/sub_c2/div_c2/floor_c1/ceil_c1
**Avoids:** GPU f32 precision loss (add guard before wiring arithmetic; start with sort/grade only); GPU deadlock (pool staging buffers first); Pipeline cache miss overhead (pre-warm common pipelines at context init)

### Phase 5: Self-Hosting
**Rationale:** Enables `cargo install rbqn` without requiring CBQN installed. Strictly gated on 13/13 test files passing — attempting earlier wastes effort diagnosing compiler crashes in `c.bqn` that are actually interpreter bugs.
**Delivers:** Standalone binary deployable via `cargo install`; no CBQN_PATH required at build time; committed bytecode in `rbqn/src/embedded/`
**Implements:** `build.rs` fallback to committed bytecode when CBQN_PATH unset; `rbqn/src/embedded/` with pre-generated .bc files
**Avoids:** Bytecode mismatch between CBQN-compiled and RBQN-compiled output (validate all 13 test files with RBQN-compiled bytecode before switching; diff against CBQN gen/ files before committing)

### Phase Ordering Rationale

- **Phases 1-3 are serial by dependency:** runtime bypass unblocks compiler pipeline; compiler pipeline unblocks test suite; test suite drives language completeness to 13/13
- **Phase 4 is gated on Phase 3 completion:** GPU on broken primitives is faster-wrong, not faster-right; criterion benchmarks are meaningless if correctness is not established
- **Phase 5 is gated on Phase 4 stability:** self-hosting bytecode must be generated from a correct interpreter; if interpreter has known bugs, the embedded compiler bytecode may differ from CBQN's in ways that are hard to diagnose

### Research Flags

Phases needing deeper research during planning:
- **Phase 3 (Undo/Under):** `⌾` structural mode complexity is HIGH per FEATURES.md. The BQN spec defines required structural functions but implementing the composition cases (`S∘T`, `k⊸S`, `S⍟k`) requires careful reading of the inferred properties spec. Recommend `/gsd:research-phase` for Under implementation specifically.
- **Phase 4 (GPU staging buffers):** The exact pooling strategy for staging buffers in `BufferPool` is not specified in existing code. `buffer.rs` allocates fresh on every download. Needs design before implementation to avoid per-call allocation overhead.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Runtime bypass verification):** Bypass is already implemented; work is testing and asserting. Patterns are documented in CBQN load.c.
- **Phase 2 (Test infrastructure):** insta snapshot testing is well-documented; BQN test suite structure is fully documented in BQN repo README.
- **Phase 5 (Self-hosting):** Architecture is clear; `include_bytes!` + committed files is standard Rust pattern used by all other BQN implementations.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All dependencies verified from Cargo.lock; no speculative additions needed |
| Features | HIGH | CBQN source and BQN spec verified; test suite defines "done" quantitatively |
| Architecture | HIGH | Derived entirely from reading actual codebase; no speculation |
| Pitfalls | HIGH | Sourced from CBQN load.c analysis, 19-iteration debug history, direct code review |

**Overall confidence:** HIGH

### Gaps to Address

- **`try_structural_under` implementation status:** ARCHITECTURE.md notes the function "exists but implementation status unknown." Needs code review before Phase 3 planning to determine if it's a stub or partially implemented.
- **Rank (`⎉`) dyadic frame broadcasting:** FEATURES.md rates this MEDIUM complexity with "needs testing against spec." The tricky part (dyadic rank frame broadcasting) may require spec research during Phase 3.
- **GPU threshold validation:** The 100K element threshold is documented as "hardcoded" and "reasonable for Metal." Criterion benchmarks in Phase 4 may reveal it needs tuning per backend. Do not treat as fixed until benchmarks confirm.
- **`setInv` callback exact argument format:** PITFALLS.md documents that `setInv` is called dyadically as `c2(setInv, bi_setInvSwap, bi_setInvReg)` — verify this against bootstrap.rs before Phase 1 completes, as the current catch_unwind may be hiding an argument-format bug.

---

## Sources

### Primary (HIGH confidence)
- `crates/rbqn/src/bootstrap.rs` — bootstrap implementation; bypass already in place
- `crates/rbqn-prim/src/dispatch.rs` — 64-entry primitive dispatch table
- `crates/rbqn-gpu/src/{context,buffer,dispatch,kernels/arith}.rs` — GPU infrastructure (complete, zero callers)
- `crates/rbqn-vm/src/derive.rs` — DerivedKind, c1/c2, DERIVED_STORE
- `crates/rbqn-core/src/{value,array}.rs` — B type, BqnArr, ArrData
- CBQN `src/load.c` — bootstrap sequence, provide array, runtime array construction (lines 460-530)
- `.planning/quick/3-assess-progress-toward-cbqn-compatible-g/PROGRESS.md` — 19-iteration debug history
- BQN spec inferred properties: https://mlochbaum.github.io/BQN/spec/inferred.html
- BQN undo: https://mlochbaum.github.io/BQN/doc/undo.html
- BQN under: https://mlochbaum.github.io/BQN/doc/under.html

### Secondary (MEDIUM confidence)
- `.planning/research/gpu-strategy.md` — dispatch thresholds, f64 problem, sort sign-bit bug
- `crates/rbqn-gpu/src/kernels/sort.rs` — sign-bit comment "treat as unsigned sort"
- wgpu buffer usage docs: https://docs.rs/wgpu/latest/wgpu/struct.BufferUsages.html

### Tertiary (LOW confidence)
- `criterion 0.8.2` on docs.rs — version confirmed; benchmark design is inference
- GPU threshold crossover at 100K elements — reasonable estimate for Metal; needs empirical validation in Phase 4

---
*Research completed: 2026-02-23*
*Ready for roadmap: yes*
