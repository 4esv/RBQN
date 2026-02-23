# RBQN Progress Assessment: Toward CBQN-Compatible GPU-Accelerated BQN

**Assessment Date:** 2026-02-23
**Assessed by:** Automated analysis + actual test execution

---

## 1. Executive Summary

RBQN has a solid foundation — NaN-boxing value representation, stack-based VM, 4-stage bootstrap, 44 native Rust primitives, and GPU compute infrastructure — but it is not yet functional as a BQN interpreter beyond trivial expressions. The core problem is that the compiled BQN pipeline is broken: runtime0 produces 24 override functions that shadow the working native Rust primitives, but those overrides execute incorrectly for almost everything except `+`. The result is that `3×4` returns `3` instead of `12`, `⌊3.7` returns `3.7` instead of `3`, and all modifier expressions fail with type errors. The CBQN compatibility test currently stands at **3/21 expressions passing (14%)** and **5/8 formatter integration tests passing** (with the 3 failing tests — floats, negatives, strings — traced to the same broken override pipeline). The GPU crate is 529 lines of Rust + 757 lines of WGSL but has zero integration with the interpreter. RBQN is not ready to share with Marshall.

---

## 2. What Works Today

### Build and Bootstrap

- Compiles cleanly with `CBQN_PATH=/path/to/CBQN cargo build`
- `build.rs` parses CBQN C codegen files into embedded bytecode (runtime0, runtime1, compiler, formatter)
- All 4 bootstrap stages load without crashing:
  - Runtime0 executes and produces 24 functions
  - Runtime1 loads
  - Compiler loads
  - Formatter loads (with fallback)
- `fallback_bootstrap_no_crash` test passes

### Native Primitives (when called directly)

All 44 primitive functions are implemented in Rust with both monadic (c1) and dyadic (c2) dispatch. When called directly from Rust (integration tests), they work correctly. However, these are shadowed at runtime by the broken runtime0 overrides.

18 integration tests in `rbqn-vm` exist but are marked `#[ignore]` — none are actively running.

### Formatter (partial)

5/8 formatter integration tests pass:
- `fmt_empty_array` PASS — `⟨⟩`
- `fmt_nothing` PASS — `·`
- `fmt_integer` PASS — `42`
- `fmt_zero` PASS — `0`
- `fallback_bootstrap_no_crash` PASS

3 fail due to broken override pipeline (see section 3).

### End-to-End Expressions That Work

From the CBQN compat test harness (3/21 pass):
- `1+1` => `2` (dyadic `+` works through runtime)
- `3‿4‿5` => `⟨ 3 4 5 ⟩` (strand literals work)
- `{𝕩+1}5` => `6` (monadic block with `+` works)

---

## 3. What's Broken (Critical)

### The Override Pipeline Problem

Runtime0 executes CBQN's BQN-written runtime source code and produces 24 "override" functions that replace the native Rust primitives. Almost all of these overrides produce wrong results because the VM cannot correctly execute the BQN patterns in the runtime source.

**Actual test results from `test_cbqn_compat.sh`:**

| Expression | CBQN | RBQN | Symptom |
|-----------|------|------|---------|
| `3×4` | 12 | 3 | Returns left arg |
| `2⋆10` | 1024 | 2 | Returns left arg |
| `÷4` | 0.25 | 4 | Returns arg unchanged |
| `\|¯5` | 5 | ¯5 | Returns arg unchanged |
| `⌊3.7` | 3 | 3.7 | Returns arg unchanged |
| `⌈3.2` | 4 | 3.2 | Returns arg unchanged |
| `↕5` | ⟨0 1 2 3 4⟩ | 5 | Returns arg unchanged |
| `≢3‿4‿5` | ⟨3⟩ | 3 | Returns scalar not shape |
| `⌽"abc"` | "cba" | Error | Type error: !𝕩 must be 0 or 1 |
| `3⥊1` | ⟨1 1 1⟩ | 3 | Returns left arg |
| `+´↕10` | 45 | Error | "Interpreting non-1-modifier as 1-modifier" |
| `×´1+↕5` | 120 | Error | Same modifier type error |
| `+\`↕5` | ⟨0 1 3 6 10⟩ | Error | Same modifier type error |
| `+˜3` | 6 | Error | Same modifier type error |
| `+¨1‿2‿3` | ⟨1 2 3⟩ | Error | Same modifier type error |
| `+´∘×˜ 3‿4` | 25 | Error | Same modifier type error |
| `"hello"∾" world"` | "hello world" | Error | Type error: !𝕩 must be 0 or 1 |
| `2{𝕨×𝕩}3` | 6 | 2 | Returns left arg (× broken) |

**Pattern analysis:**

1. **Monadic primitives:** Almost all return `𝕩` unchanged. The runtime0-derived function for `⌊`, `⌈`, `|`, `↕`, `÷` (monadic), `⌽`, `⥊` etc. executes and returns the argument instead of computing.

2. **Dyadic primitives (non-+):** Return `𝕨` (left argument). `×`, `⋆`, `÷`, `⥊` dyadic all return `w`. Only `+` returns the correct result.

3. **Modifiers (´ ` ˜ ¨ etc.):** All fail with "Interpreting non-1-modifier as 1-modifier". The runtime0 functions for `´`, `` ` ``, `˜`, `¨` are being stored as or resolved to wrong types. When the VM tries to apply them as 1-modifiers, they don't have the right `DerivedKind`.

4. **String/character errors:** `⌽"abc"` and `"hello"∾" world"` crash with `Type error: !𝕩: 𝕩 must be 0 or 1`, suggesting character array handling is broken in the assert primitive path.

### Formatter Failures (3/8)

The formatter uses the compiled BQN pipeline to format values. The 3 failing formatter tests:
- `fmt_negative` FAIL — `1-3` produces `1` (same `-` override bug: returns left arg)
- `fmt_float` FAIL — related formatting issue
- `fmt_string` FAIL — likely the character type error

### Root Cause Hypothesis

The native Rust primitives work correctly. Runtime0 loads and executes the CBQN-written BQN runtime source, which produces 24 override functions. These overrides are stored as `DerivedKind::FunBlock` / `DerivedKind::Md1Block` etc. based on the bytecode's type annotations.

The VM is probably executing runtime0's bytecode with subtle bugs in complex BQN patterns (SETH/PRED, body pairs, multiple header cases), causing the override functions to be constructed incorrectly. The function bodies may be running but returning the wrong value (e.g., missing a RETN in the correct branch, falling through to a branch that returns a parameter, or having wrong body-swap logic for header-based dispatch).

This problem has persisted through ~19 fix iterations (01-02a through 01-02s in the planning phases) without resolution, suggesting the bug requires a different debugging approach rather than incremental patching.

---

## 4. Missing Language Features

### Modifiers (4/20 missing)

| Modifier | Status |
|---------|--------|
| `⁼` Undo | NOT IMPLEMENTED — `inverse.rs` is a 3-line stub |
| `⌾` Under | NOT IMPLEMENTED |
| `⎉` Rank | NOT IMPLEMENTED |
| `⚇` Depth | NOT IMPLEMENTED |

These 4 block significant BQN idioms: `⊐⁼` (inverse index), `⍒⁼` (grade inverse), any rank-based operation.

### System Functions (~44 missing)

Only 6 system values are implemented:
- `•Type` (0) — DONE
- `•Decompose` (1) — DONE
- `•Glyph` (4) — STUB only
- `•Fill` (7) — DONE
- `•GroupLen` (22) — DONE
- `•GroupOrd` (23) — DONE

Missing (partial list):
`•BQN`, `•ReBQN`, `•Show`, `•Out`, `•Fmt`, `•Repr`, `•Import`, `•FChars`, `•FLines`, `•FBytes`, `•file`, `•SH`, `•Exit`, `•args`, `•path`, `•name`, `•wdpath`, `•state`, `•UnixTime`, `•MonoTime`, `•math.*`, `•MakeRand`, `•rand`, `•ParseFloat`, `•Hash`, `•Cmp`, `•FromUTF8`, `•ToUTF8`, `•FFI`, `•HashMap`, `•bit`, `•term`

### VM/Interpreter Bugs

- **DYNM opcode:** Returns `SENTINEL` instead of a mutable variable reference. Dynamic variable mutation silently fails.
- **ALIM opcode:** Silently discards operand. Namespace field limiting broken.
- **REPL state:** Each line evaluated in isolation — `a←5` in one line is invisible to the next.
- **Monadic shifts:** `«` and `»` have no c1 implementation (listed as `None` in dispatch table).
- **Fill tracking:** `•fillBy` (sys_idx 7) returns `x` unchanged — fill elements not propagated.
- **Heap limit:** `-M` flag parsed but discarded — no memory protection.

### Multi-Rank Primitive Gaps

Several primitives only handle vectors and throw errors for rank>1:
- `∾` monadic join for rank>1 arrays
- Structural operations always produce boxed output regardless of input type

---

## 5. GPU Acceleration Status

The `rbqn-gpu` crate has **zero integration with the interpreter**. It is listed as a Cargo dependency of `rbqn-prim` but never imported or called from any BQN primitive.

### Infrastructure (Done)
- `GpuContext` — wgpu device/queue initialization
- `BufferPool` — GPU buffer management
- `PipelineCache` — compiled pipeline caching
- Dispatch thresholds — size-based CPU/GPU routing logic

### Kernels with Real Implementations
- Unary elementwise: exp, sqrt, neg, abs
- Matmul: tiled 16x16
- Softmax: single-workgroup + multi-pass for large arrays

### Kernels as Scaffolding Only
- Arithmetic binary ops (add, sub, mul, div) — structure present, not wired
- Reduce — structure present
- Scan — structure present
- Sort — structure present
- Select/gather — structure present

### What's Missing for GPU Integration
1. Array transfer: `BqnArr` -> `GpuBuffer` conversion (typed data: F64, I32, etc.)
2. Result transfer: `GpuBuffer` -> `BqnArr` readback
3. Dispatch hook: primitives need to check array size and route to GPU
4. Async handling: wgpu is async, the VM is sync — bridging strategy needed
5. Benchmarks: no way to measure GPU vs CPU performance
6. Tests: no GPU kernel tests exist

**Metric:** rbqn-gpu is 529 lines of Rust + 757 lines of WGSL across 15 shader files. The infrastructure design is sound but entirely disconnected from the interpreter.

---

## 6. Gap Analysis: Path to CBQN Drop-In

### Tier 0: Blocking Everything — Fix the Runtime Override Pipeline

Until this is fixed, nothing else matters. The broken overrides shadow every working native primitive.

**Options:**

**Option A: Debug VM execution of runtime0 source.** The VM must be executing runtime0's BQN source incorrectly. The override functions are constructed wrong — either they return the wrong value, have wrong body layout after SETH/PRED processing, or the bytecode body-swap logic is off. This requires systematic tracing (the RBQN_PRIM_TRACE infrastructure from quick task 2 is relevant here).

**Option B: Bypass runtime0 overrides entirely.** RBQN has 44 working native Rust primitives. Instead of executing runtime0 to get override functions, populate the runtime array directly from the native Rust implementations. CBQN's runtime0 overrides exist to handle edge cases (pervasive extension, fill propagation) but the basic primitive behavior is already in Rust. This would immediately unblock most of the 18 failing compat tests.

**Option B is likely faster** given 19 failed fix attempts on Option A. The cost is that edge cases (pervasion, fills) would need separate implementation.

### Tier 1: Language Completeness (after pipeline fix)

- Implement 4 missing modifiers: `⁼` `⌾` `⎉` `⚇`
- Implement essential system functions: `•BQN`, `•Show`, `•Out`, `•Import`, `•FChars`/`•FLines`/`•FBytes`, `•math.*`
- Fix DYNM, ALIM opcodes
- Fix monadic shifts
- Fix fill tracking
- REPL state persistence
- Run official BQN test suite (13 test files in mlochbaum/BQN/test/cases/)

### Tier 2: Self-Hosting (remove CBQN dependency)

- Compile BQN compiler source (c.bqn) using RBQN's compiler
- Embed resulting bytecode — remove `CBQN_PATH` build requirement
- Currently: `cargo build` without `CBQN_PATH` produces a binary where the compiler is `SENTINEL` (completely non-functional)

### Tier 3: Wire GPU to Interpreter

1. Design dispatch: when does an array op go GPU vs CPU? (threshold: >1000 elements?)
2. Implement array transfer layer: `ArrData::F64` -> `GpuBuffer<f32>` -> `ArrData::F64`
3. Wire hot primitives: `+`, `×`, `/`, `÷`, reshape, fold, scan, grade
4. Handle async: blocking `device.poll()` or async runtime
5. Benchmarking infrastructure

The GPU code is sound but needs an integration layer. Estimated: 1-2 weeks of focused work after interpreter is functional.

### Tier 4: Production Quality (for sharing)

- Garbage collection: replace monotonic HashMap stores with mark-and-sweep or refcount GC
- Error handling: convert panic-based errors to `Result` propagation (large refactor, ~139 panic sites)
- Performance: replace HashMap lookup per value access with direct Arc pointers in NaN-box payload
- REPL with variable persistence
- CI/CD pipeline
- Documentation

---

## 7. Codebase Metrics

### Line Counts (current, 2026-02-23)

| Crate | Rust Lines |
|-------|-----------|
| rbqn-core | 824 |
| rbqn-vm | 4,373 |
| rbqn-prim | 3,278 |
| rbqn (binary + build.rs) | 1,293 + 695 = 1,988 |
| rbqn-gpu | 529 |
| **Total Rust** | **10,992** |
| GPU shaders (WGSL) | 757 (15 files) |
| **Grand total** | **~11,750** |

### Test Coverage

| Test Suite | Status |
|-----------|--------|
| `rbqn-vm` integration tests | 18 tests, ALL IGNORED |
| `rbqn` formatter tests | 5/8 pass, 3 fail (fmt_float, fmt_negative, fmt_string) |
| CBQN compat harness | 3/21 pass (14%) |
| Official BQN test suite | Not runnable (compiler broken) |

### Bootstrap Iterations

Phase 1, Plan 02 has been iterated ~19 times (01-02a through 01-02s in summaries) without resolving the runtime override pipeline. This is the project's single largest outstanding issue.

---

## 8. Prioritized Next Steps

### 1. [CRITICAL] Resolve Runtime Override Pipeline — Choose Option A or B

**Option A (debug):** Use RBQN_PRIM_TRACE (from quick task 2) to trace every primitive call during runtime0 execution. Map which runtime0-produced functions are wrong and why. Look specifically at SETH/PRED body dispatch — the body-swap logic in compile_block is the most likely culprit for overrides that return the wrong value.

**Option B (bypass):** Populate the `runtime` array directly from native Rust primitives instead of running runtime0. This requires understanding which provide slots map to which primitives and writing the mapping. Most of the 18 failing compat tests would immediately start passing.

**Recommendation:** Try Option B first. If runtime0 overrides exist to handle cases that need the full BQN runtime (pervasion, fills, etc.), those cases can be handled separately. The 19 failed Option A iterations suggest it needs a fundamentally different approach.

### 2. Run Official BQN Test Suite

Once the compiler pipeline works, immediately run all 13 official test files:
```
simple.bqn, literal.bqn, token.bqn, syntax.bqn, bytecode.bqn,
prim.bqn, header.bqn, unhead.bqn, namespace.bqn, fill.bqn,
identity.bqn, under.bqn, undo.bqn
```
This establishes a quantitative baseline and surfaces remaining bugs systematically.

### 3. Implement Missing Modifiers

`⁼` and `⌾` are needed for common BQN idioms. `⎉` and `⚇` for multi-rank work. Priority: `⁼` first (most commonly used).

### 4. Implement Core System Functions

Minimum viable set for real programs: `•BQN`, `•Show`/`•Out`, `•Import`, `•FChars`/`•FLines`/`•FBytes`.

### 5. Wire GPU Crate

GPU infrastructure exists and is well-designed. Needs: array transfer layer, dispatch integration, and at minimum benchmarks. Estimated 1-2 weeks after interpreter is functional.

### 6. Add Garbage Collection

The bootstrap alone creates thousands of objects that are never freed. Any non-trivial REPL session exhausts memory. This is mandatory for production use.

---

## 9. Honest Assessment for Sharing with Marshall

**Not ready to share.** RBQN currently works for exactly 3 BQN expressions: `1+1`, strand literals, and `{𝕩+1}5`. Everything else is broken. Showing this to Marshall as a CBQN drop-in would be misleading.

### What RBQN has going for it

- Clean Rust implementation with sound architecture
- All 44 primitives implemented natively (they work when called directly)
- GPU compute infrastructure that's well-designed and ready to wire up
- NaN-boxing value representation that matches CBQN's approach
- 4-stage bootstrap that mirrors CBQN's bootstrap philosophy
- Solid VM with all opcodes implemented

### The fundamental problem

The self-hosted runtime — which CBQN wrote in BQN and which RBQN must execute to initialize itself — is executing incorrectly in RBQN's VM. Almost every primitive is broken through the compiled path because the BQN-written override functions are constructed wrong. This isn't a "few edge cases" problem — it's the primary execution path for all BQN code.

### Estimated effort

| Goal | Estimate |
|------|----------|
| Fix runtime pipeline (Option B — bypass overrides) | 1-3 days |
| Fix runtime pipeline (Option A — debug VM) | Unknown (19 iterations haven't solved it) |
| Pass CBQN compat test 21/21 | 1-2 weeks after pipeline fix |
| Pass official BQN test suite | 2-4 weeks after pipeline fix |
| Wire GPU for demo | 1-2 weeks |
| "Demo-ready for Marshall" | 1-2 months total |
| CBQN drop-in (self-hosting, GC, full system functions) | 3-6 months |

### The path forward

The fastest path to a shareable demo is Option B (bypass runtime0 overrides, use native Rust primitives directly) combined with wiring the GPU for a compelling ML/array demo. RBQN's GPU-accelerated BQN angle is genuinely novel — CBQN has no GPU support — and that differentiation is worth emphasizing once the interpreter works.
