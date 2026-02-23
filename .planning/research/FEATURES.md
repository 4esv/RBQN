# Feature Research

**Domain:** CBQN-compatible GPU-accelerated BQN interpreter (Rust)
**Researched:** 2026-02-23
**Confidence:** HIGH (CBQN source and BQN spec verified; GPU patterns from wgpu ecosystem)

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features that a "CBQN drop-in replacement" must have. Missing any of these means RBQN is not
a drop-in — programs that run in CBQN will fail in RBQN.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Runtime bypass (Option B)** | Without it, native primitives are shadowed by broken runtime0 overrides; 3/21 compat tests pass | MEDIUM | Map native Rust primitives into the `runtime` array directly, skip runtime0 execution; provide array mapping is already documented in bootstrap.rs |
| **Working compiler pipeline** | CBQN can compile and run arbitrary BQN source; RBQN currently cannot | HIGH | Compiler loads but can't execute because runtime1 depends on broken overrides; fixing runtime bypass unblocks this |
| **Undo (⁼) — full inverse table** | Used constantly: `⊐⁼`, `/⁼`, `⌽⁼`, `⋆⁼` (log), `⍉⁼`; test file `undo.bqn` tests it explicitly | HIGH | Dispatch skeleton exists in modifiers.rs; `inv_reg` calls BQN runtime resolver but runtime is broken; need native inverse table for all required primitives per spec |
| **Under (⌾) — structural mode** | Used for: `F⌾(k⊸↓)`, `F⌾(⊏˘)`, `F⌾⌽`, etc.; test file `under.bqn` tests it | HIGH | Computational Under fallback exists; structural Under stub (`try_structural_under`) is called but implementation unknown; structural Under requires tracking selection indices through ⊏, ↑, ↓, ⌽, ⍉ |
| **Rank (⎉) — correct semantics** | Required for multi-rank operations; ⎉¯1 must equal ˘; three-element operand list must work | MEDIUM | rank_c1/rank_c2 implemented; needs testing against spec; dyadic rank frame broadcasting is the tricky part |
| **Depth (⚇) — non-zero depths** | ¨ (each) is ⚇¯1; non-zero depths require recursive descent into nested arrays | MEDIUM | depth_c1/depth_c2 delegate to BQN runtime Depth for non-zero values; broken until runtime loads; need native depth traversal |
| **Fill elements** | Used by ↑ (Take overshoot), « » (Nudge), > Merge, ⥊↑ reshape; test file `fill.bqn` | HIGH | fill field exists on BqnArr; •_fillBy_ stub just forwards to F; fill propagation through array operations is missing; number fill=0, char fill=space, array fill=recursive |
| **REPL state persistence** | `a←5` in line 1 must be visible to line 2; current REPL evaluates each line in isolation | LOW | Scope must persist across eval calls; straightforward fix — pass persistent scope to exec_string |
| **DYNM opcode fix** | Dynamic variable mutation (`a↩expr`) returns SENTINEL instead of mutating; silent data loss | MEDIUM | The opcode is parsed and reached; it returns wrong value; must mutate the scope variable and return the new value |
| **ALIM opcode fix** | Namespace field limiting (`ns.{x,y}`) discards operand silently | MEDIUM | Structure exists; operand is being ignored |
| **Monadic shifts** | `«` and `»` as monads (nudge operations) are listed as None in dispatch | LOW | Just need c1 implementations: `« x` = `0↓x` with 0-fill prepended, `» x` = `0↑¯1↓x` |
| **•Out** | Any real BQN script uses •Out to print; CBQN drop-in requires this | LOW | Not implemented; trivial: print string arg + newline to stdout |
| **•Show** | Used in REPLs and scripts to display values | LOW | Needs formatter to produce display string; formatter already loads (partially) |
| **•BQN** | Eval string in isolated scope; used by test suite | MEDIUM | Core eval infrastructure exists; needs isolated scope creation and string→B array conversion |
| **•Exit** | Scripts call •Exit 0 when done | LOW | Call `std::process::exit(code)` |
| **•args** | Scripts read command-line args via •args | LOW | Expose CLI argv as BQN character array list |
| **Pass official test suite (13 files)** | This is the quantitative definition of "CBQN drop-in" | HIGH | Requires: runtime bypass + compiler pipeline + fill elements + Undo + Under + all primitives working; `simple.bqn`, `prim.bqn`, `header.bqn` are the first gates |
| **Self-hosting (no CBQN build dep)** | "cargo install rbqn" requires no external CBQN installation | HIGH | Compile c.bqn with RBQN's own compiler; embed resulting bytecode; remove CBQN_PATH requirement from build.rs |
| **Standalone binary** | "cargo install rbqn" must produce a working binary | LOW | Infrastructure mostly there; depends on runtime bypass + self-hosting |

### Differentiators (Competitive Advantage)

Features that make RBQN worth using over CBQN rather than just equal to it.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **GPU dispatch for numeric arrays** | Automatic acceleration of matmul, fold, scan, elementwise ops on large arrays; CBQN has no GPU support at all | HIGH | Infrastructure exists (529 lines Rust + 757 WGSL); zero integration with interpreter; needs: array transfer layer (BqnArr→GpuBuffer), readback (GpuBuffer→BqnArr), dispatch hooks in primitives; threshold already defined at 100K elements (50K per PROJECT.md) |
| **GPU matmul primitive** | BQN as ML preprocessing/pipeline language; matmul is the killer demo | MEDIUM | Tiled 16×16 kernel exists in WGSL; transfer layer missing; needs F64→F32 downcast with validation |
| **GPU softmax** | Useful for ML workloads; single-workgroup + multi-pass for large arrays implemented | MEDIUM | Kernel exists; same transfer layer needed as matmul |
| **GPU fold/scan on large arrays** | `+´` and `+\`` of 1M+ elements; these are bottlenecks in real array workloads | HIGH | Reduce/scan kernel scaffolding exists; not fully implemented; challenging: WGSL parallel reduce requires workgroup synchronization |
| **Threshold-gated dispatch** | Transparent CPU/GPU switching with no user code changes; small arrays stay on CPU | LOW | `dispatch.rs` already has `should_use_gpu(op, len)` with thresholds; just needs wiring into primitives |
| **f32/f64 precision transparency** | GPU path runs f32; CPU fallback for f64-critical ops; user never sees the difference | MEDIUM | Architecture decision already made (PROJECT.md); validation logic needed to detect precision loss and fall back |
| **Rust-native safety** | Memory safety without GC pauses; no segfaults from type confusion | LOW | Existing architecture; worth emphasizing in documentation |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **•FFI (foreign function interface)** | Call C/Rust from BQN; power users want it | Complex ABI negotiation, platform-specific, safety boundary; out of scope per PROJECT.md | Defer; let users call rbqn as a library from Rust instead |
| **Multi-threaded VM** | Faster execution via parallelism | Global DERIVED_STORE, ARR_STORE, NS_STORE behind Mutex prevent this without major refactor; would require replacing HashMap+Mutex with lock-free structures | GPU parallelism handles the large-array use case better; CPU parallelism is future work |
| **•HashMap** | Mutable key-value store; common scripting need | Not in core BQN spec; adds significant implementation complexity for a non-standard feature | BQN namespaces can simulate this for many use cases |
| **•term namespace** | Terminal UI capabilities | Niche use case, platform-specific, not needed for drop-in | Defer; screen/terminal code rarely in numeric workloads |
| **Debugging/stepping** | REPL with inspect-on-error | Significant VM instrumentation; not needed for drop-in compliance | RBQN_PRIM_TRACE already exists for primitive tracing |
| **Custom GPU kernel API** | Let users write BQN → WGSL | GPU dispatch is internal optimization; exposing it creates a versioned API surface | GPU is purely internal; the BQN programs run identically on CPU or GPU |
| **GC (garbage collection) now** | Bootstrap creates thousands of leaked objects | Real but not blocking for correctness testing; adds significant implementation scope | Fix after interpreter works; GC is Tier 4 per PROGRESS.md |

---

## Feature Dependencies

```
[Runtime Bypass (Option B)]
    └──unblocks──> [Working Compiler Pipeline]
                       └──unblocks──> [Pass Official Test Suite]
                       └──unblocks──> [Self-Hosting]
                                          └──enables──> [Standalone Binary / cargo install]

[Runtime Bypass] ──also unblocks──> [•BQN eval]
[Runtime Bypass] ──also unblocks──> [Under structural mode] (runtime1 needed)
[Runtime Bypass] ──also unblocks──> [Depth non-zero] (runtime Depth needed)
[Runtime Bypass] ──also unblocks──> [Undo full inverse table] (INV_REG_FN from runtime)

[Working Compiler Pipeline]
    └──required by──> [Pass official test suite]
    └──required by──> [Self-Hosting]

[Fill Elements]
    └──required by──> [fill.bqn test file]
    └──required by──> [↑ with overshoot, « » nudge, > merge empty]

[Undo (⁼) full]
    └──required by──> [undo.bqn test file]
    └──required by──> [Under computational mode] (Under uses ⁼ as G⁼∘F○G)

[Under (⌾) structural]
    └──required by──> [under.bqn test file]
    └──required by──> [identity.bqn] (structural identity operations)

[GPU dispatch infrastructure (transfer layer)]
    └──enables──> [GPU matmul]
    └──enables──> [GPU elementwise ops]
    └──enables──> [GPU fold/scan]
    └──enables──> [GPU softmax]

[REPL state persistence]
    └──required by──> [Interactive REPL usefulness]

[•Out, •Show]
    └──required by──> [Most real BQN scripts]
    └──required by──> [namespace.bqn test file] (uses •Show)
```

### Dependency Notes

- **Runtime bypass requires understanding provide array mapping:** The 40-entry provide array is already mapped in bootstrap.rs. Option B means skipping exec_stage for runtime0 and directly populating `runtime` from the 24 functions CBQN's load.c expects, using native Rust primitives at the correct positions.

- **Undo requires INV_REG_FN from runtime1:** The `inv_reg` function in derive.rs already falls through to `INV_REG_FN` which is set by the `setInv` callback during bootstrap. Once runtime1 loads correctly (after runtime bypass), Undo for non-primitive functions will work via the BQN runtime's inverse tables. Native primitive inverses (the `native_inverse_reg` fast path) need a complete table of required inverses from the spec.

- **Under structural mode requires runtime1:** `try_structural_under` is called first but its implementation status is unclear. The fallback `get_rt_under()` depends on runtime1. Structural Under for the common cases (⌽, ⍉, k⊸⊏, k⊸↑, k⊸↓) can be implemented natively before runtime1 loads, covering most test cases.

- **Depth non-zero requires runtime Depth:** depth_c1/depth_c2 already delegate to `get_rt_depth()` for non-zero cases. Once runtime loads, this works. The only native case is depth=0 (already implemented as `depthf_c1/c2`).

- **GPU dispatch requires working interpreter first:** GPU ops on broken primitives produce wrong results regardless of where they run. GPU integration is strictly Tier 3 (PROGRESS.md) — only after interpreter is functional.

---

## MVP Definition

### Launch With (v1 — "passes test suite")

Minimum viable product: RBQN passes all 13 official BQN test files and installs via `cargo install`.

- [ ] **Runtime bypass** — unblocks everything else; 1-3 days per PROGRESS.md estimate
- [ ] **Working compiler pipeline** — compile and execute arbitrary BQN; follows from runtime bypass
- [ ] **Undo (⁼) native inverse table** — required for `undo.bqn`; fill in `native_inverse_reg` for all spec-required primitives (+, -, ×, ÷, ⋆, √, ∧, ¬, ⊢, ⊣, <, ⌽, ⍉, /)
- [ ] **Under structural mode** — required for `under.bqn`; implement native structural Under for ⌽, ⍉, k⊸⊏, k⊸↑, k⊸↓ at minimum
- [ ] **Fill element propagation** — required for `fill.bqn`; implement fill tracking through ↑, «, », >, ⥊↑
- [ ] **DYNM opcode fix** — required for any mutable variable program
- [ ] **REPL state persistence** — required for multi-line scripts
- [ ] **•Out, •Exit, •args** — required for any script that produces output
- [ ] **•BQN** — required for eval-based test patterns
- [ ] **Self-hosting** — remove CBQN_PATH build dependency; enables `cargo install`
- [ ] **Monadic shifts** — fix «/» c1 implementations (trivial)

### Add After Validation (v1.x — "GPU demo")

Once interpreter is verified correct, wire GPU for the differentiating use case.

- [ ] **GPU array transfer layer** — BqnArr↔GpuBuffer bridge; trigger: interpreter passes test suite
- [ ] **GPU dispatch in +, ×, ÷ primitives** — elementwise for arrays >100K elements; trigger: transfer layer working
- [ ] **GPU fold/scan** — `+´` and `+\`` on large arrays; trigger: elementwise GPU working
- [ ] **GPU matmul demo** — showcase BQN as ML array language; trigger: basic GPU dispatch working
- [ ] **Benchmarks** — CPU vs GPU comparison; trigger: GPU dispatch integrated
- [ ] **ALIM opcode fix** — namespace field limiting; trigger: namespace.bqn test failures identified

### Future Consideration (v2+ — "production")

- [ ] **Garbage collection** — replace monotonic HashMap stores; trigger: memory exhaustion in production use
- [ ] **Error propagation (Result not panic)** — ~139 panic sites; trigger: user-facing error reporting becomes priority
- [ ] **•Import** — script file loading; trigger: multi-file BQN programs become a use case
- [ ] **•file namespace** — file I/O; trigger: scripting use cases
- [ ] **•math namespace** — trig, special functions; trigger: scientific computing use cases
- [ ] **•FFI** — defer indefinitely
- [ ] **Multi-threaded VM** — requires rearchitecting global stores

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Runtime bypass (Option B) | HIGH | MEDIUM | P1 |
| Working compiler pipeline | HIGH | MEDIUM (follows from bypass) | P1 |
| Undo native inverse table | HIGH | MEDIUM | P1 |
| Under structural mode | HIGH | HIGH | P1 |
| Fill element propagation | HIGH | HIGH | P1 |
| Pass official test suite | HIGH | HIGH (depends on above) | P1 |
| Self-hosting / standalone binary | HIGH | HIGH | P1 |
| •Out / •Exit / •args | MEDIUM | LOW | P1 |
| •BQN eval | MEDIUM | MEDIUM | P1 |
| DYNM opcode fix | MEDIUM | MEDIUM | P1 |
| REPL state persistence | MEDIUM | LOW | P1 |
| Monadic shifts fix | LOW | LOW | P1 |
| GPU transfer layer | HIGH | HIGH | P2 |
| GPU elementwise dispatch | HIGH | MEDIUM | P2 |
| GPU fold/scan | HIGH | HIGH | P2 |
| GPU matmul demo | HIGH | MEDIUM | P2 |
| ALIM opcode fix | LOW | MEDIUM | P2 |
| •Import / •file | LOW | MEDIUM | P3 |
| •math namespace | LOW | MEDIUM | P3 |
| Garbage collection | MEDIUM | HIGH | P3 |
| Error propagation (Result) | MEDIUM | HIGH | P3 |

**Priority key:**
- P1: Must have for CBQN drop-in milestone
- P2: GPU differentiation — add after test suite passes
- P3: Production quality — defer until product-market fit

---

## Competitor Feature Analysis

| Feature | CBQN | dzaima/BQN | RBQN Plan |
|---------|------|------------|-----------|
| Full primitive set | Yes (44+20) | Yes | 43/44 + 16/20 native; missing 4 modifiers in implementation, but dispatch stubs exist |
| Undo/inverse system | Full, BQN-defined | Full | Partial (fast path for native primitives; runtime resolver needed) |
| Under (structural + computational) | Full | Full | Computational fallback only; structural stubs exist |
| Fill element tracking | Full | Full | Not implemented (field exists, propagation missing) |
| Official test suite | Passes all | Passes all | 0/13 runnable currently |
| Self-hosting | Yes | Yes | No (CBQN_PATH required at build time) |
| GPU acceleration | None | None | Infrastructure built, not wired |
| Cargo install | N/A | N/A | Goal; blocked on self-hosting |
| System functions | ~50 | ~40 | 6 implemented; ~44 remaining |
| REPL | Yes, persistent | Yes | Broken (no state persistence) |
| Namespace support | Full | Full | Partial (ALIM broken) |

---

## BQN Spec: Required Inverses for ⁼

Per the BQN inferred properties spec, these inverses are **required** (not optional):

| Function | Monadic inverse | Dyadic inverse |
|----------|----------------|----------------|
| `+` | `+` (self-inverse) | `x-w` |
| `-` | `-` (self-inverse) | `w-x` |
| `×` | — | `x÷w` |
| `÷` | `÷` | `w÷x` |
| `⋆` | `⋆⁼` = log | `w⋆⁼x` = log base w |
| `√` | `×˜` (square) | `w√⁼x` = w-th power |
| `∧` | — | `x÷w` (boolean) |
| `¬` | `¬` (self-inverse) | — |
| `⊢` | `⊢` (identity) | `𝕨` (left arg as result) |
| `⊣` | `⊣` | `𝕩` (right arg as result) |
| `<` | `>` if boxed scalar | — |
| `⌽` | `⌽` (self-inverse) | `(-w)⌽` |
| `⍉` | `⍉` (self-inverse for square) | — |
| `/` | `/⁼` = indices inverse | — |

**Optional but useful:**
- `∾⁼` = take first/last split
- `⊏⁼`, `⊑⁼`, `↑⁼`, `↓⁼`

## BQN Spec: Structural Under Functions

Per spec, these are the **required** structural functions for `⌾`:

**Monadic:** ⊣ ⊢ < > ∾ ⥊ ≍ ↑ ↓ ⌽ ⍉ ⊏ ⊑

**Dyadic:** ⊢ ⥊ ↑ ↓ ↕ ⌽ ⍉ / ⊏ ⊑ ⊔

**Combinations:** compositions (S∘T), with constants (k⊸S), powers (S⍟k)

The most common patterns in real BQN code: `F⌾⌽` (apply F to reversed), `F⌾(k⊸↑)` (apply to first k), `F⌾(k⊸↓)` (apply to last k), `F⌾⍉` (apply to transposed), `F⌾(arr⊸⊏)` (apply to selected elements).

---

## Sources

- BQN Undo specification: https://mlochbaum.github.io/BQN/doc/undo.html
- BQN Under specification: https://mlochbaum.github.io/BQN/doc/under.html
- BQN inferred properties spec: https://mlochbaum.github.io/BQN/spec/inferred.html
- BQN system functions spec: https://mlochbaum.github.io/BQN/spec/system.html
- BQN Rank modifier: https://mlochbaum.github.io/BQN/doc/rank.html
- BQN Depth modifier: https://mlochbaum.github.io/BQN/doc/depth.html
- BQN VM implementation notes: https://mlochbaum.github.io/BQN/implementation/vm.html
- CBQN source README: https://github.com/dzaima/CBQN/blob/master/src/README.md
- RBQN PROGRESS.md: .planning/quick/3-assess-progress-toward-cbqn-compatible-g/PROGRESS.md
- RBQN codebase: crates/rbqn-vm/src/modifiers.rs, crates/rbqn-vm/src/derive.rs, crates/rbqn/src/bootstrap.rs

---

*Feature research for: CBQN-compatible GPU-accelerated BQN interpreter*
*Researched: 2026-02-23*
