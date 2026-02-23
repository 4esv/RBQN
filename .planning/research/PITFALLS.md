# Pitfalls Research

**Domain:** BQN interpreter implementation — runtime bypass, GPU integration, self-hosting
**Researched:** 2026-02-23
**Confidence:** HIGH (sourced from CBQN load.c analysis, RBQN codebase review, 19-iteration debug history, GPU strategy research)

---

## Critical Pitfalls

### Pitfall 1: Runtime Bypass Silently Breaks PrimInd / Inverse Lookups

**What goes wrong:**
Bypassing runtime0/runtime1 with native Rust primitives fixes execution but breaks the inverse system. The BQN runtime's `⁼` modifier uses `•PrimInd` to look up a primitive by its glyph index, then indexes into a precomputed inverse table. If the runtime array contains BQN-derived wrappers (e.g., `Indices ⊘ Replicate` for `/`) instead of native fruntime entries, `•PrimInd` returns -1 (or throws) because only native entries have a valid `prim_idx`. The inverse system silently fails: `⊐⁼`, `⍒⁼`, `⊏⁼`, `/⁼` all return wrong results or panic.

**Why it happens:**
CBQN's load.c has an explicit branch at line ~522:
```c
B r = nnbi ? Get(rtObjRaw, i) : inc(fruntime[i]);
```
When all builtins are natively implemented (`nnbi = false`), it skips the BQN wrapper and uses the native primitive. This matters because native fruntime entries carry a `prim_idx` field used by `•PrimInd`. If you use the BQN-wrapped version, `•PrimInd` cannot identify it.

RBQN's current bootstrap.rs already handles this correctly (line 472: `let runtime: Vec<B> = fruntime.clone()`). The pitfall is regressing: any future refactor that swaps fruntime back to rtObjRaw will silently break `⁼`.

**How to avoid:**
- The runtime array that gets passed to the compiler must be `fruntime` (native), not `rtObjRaw` (BQN-wrapped)
- Add an assertion: after bootstrap, `•PrimInd "+"` must return 0 (not -1)
- Write a test: `+⁼ 3` must return 3 before claiming the inverse system works

**Warning signs:**
- `⊐⁼`, `⍒⁼`, `/⁼` fail with "no inverse" or return wrong values
- `•PrimInd "+"` returns ¯1 instead of 0
- `⁼` works on user-defined functions but fails on all primitives

**Phase to address:**
Runtime bypass phase (first phase). Add `•PrimInd` test to the compat harness immediately after bypass is verified working.

---

### Pitfall 2: SetPrims / SetInv Callbacks Must Be Called in the Right Order

**What goes wrong:**
Runtime1 returns three values: `⟨rtObjRaw, setPrims, setInv⟩`. If `setPrims` is called before the runtime is fully wired (or not called at all), `•Decompose` cannot identify primitive types and returns wrong type codes. If `setInv` is not called, `⌾` (Under) and `⁼` (Undo) on the BQN-defined runtime functions have no inverse tables, causing silent wrong behavior.

**Why it happens:**
The callbacks register lookup tables inside the BQN runtime objects. The BQN runtime's Under and Undo implementations check these tables at invocation time. Missing the `setInv` call means structural under (like `mask⊸/ ⌾ x`) reverts to a naive `G⁼(F(G(x)))` that doesn't work for non-invertible structural operations.

The RBQN codebase already calls `setPrims` and `setInv` in bootstrap.rs (lines 476-492) but wraps them in `catch_unwind` and silently discards panics. A panic in either callback causes the inverse system to be silently misconfigured.

**How to avoid:**
- Do not silently swallow panics in `setPrims`/`setInv` — at minimum log a WARNING that inverse lookups will not work
- After calling `setPrims`, verify with an assertion: `•Type •Decompose "+"` should return recognizable output
- Extract `rt_under` from `rtObjRaw[57]` (CBQN: `n_under = 57`). RBQN already does this (bootstrap.rs line 501) — do not remove it

**Warning signs:**
- `(2 ⊸/ ⌾ x)` computes wrong result or panics
- `•Decompose` returns type 3 (function) for everything instead of primitive codes
- runtime1 loads and produces output but `⁼` never finds an inverse

**Phase to address:**
Runtime bypass phase. Add `•Decompose` and a simple `Under` test to compat harness.

---

### Pitfall 3: GPU f32 Precision Loss on BQN f64 Operations

**What goes wrong:**
BQN programs treat numbers as IEEE 754 f64. The GPU path casts to f32 for compute. For integer-valued arithmetic on small numbers, f32 is exact. But for any of the following, f32 silently gives wrong results:
- Numbers larger than 16,777,216 (2^24) — f32 cannot represent all integers above this
- Any subtraction of nearly-equal large numbers (catastrophic cancellation)
- Repeated multiply/fold — errors accumulate
- Comparison after arithmetic — `a = b` may give 1 on CPU and 0 on GPU for the same a,b

The bug is silent: the GPU dispatch returns a result, the NaN-boxed value is returned as valid f64, but the numeric bits are slightly wrong. BQN test cases that check exact equality (`≡`) will fail intermittently.

**Why it happens:**
The GPU buffer type is `ElementKind::F32`. When a f64 BQN array is uploaded to GPU, every element is narrowed to f32. The result is downloaded back and widened to f64. The round-trip introduces up to ~7e-8 relative error per operation.

The current `dispatch.rs` has no precision guard: `should_use_gpu("add", len)` dispatches if `len >= 100_000` regardless of whether the data is integer-valued or requires f64 precision.

**How to avoid:**
- Gate f64 GPU dispatch on `adapter.features().contains(wgpu::Features::SHADER_F64)`. On Metal (macOS), SHADER_F64 is NOT available — so all Metal paths must use f32 or fall back to CPU
- Add a precision guard to the dispatch function: only dispatch to GPU for f64 arrays when either (a) native f64 shader is available, OR (b) all values are integer-valued AND all are within ±2^24
- For now (until proper guard exists): restrict GPU dispatch to operations that are precision-insensitive: sort/grade (ordinal only), boolean ops, integer-array operations

**Warning signs:**
- `prim.bqn` tests involving large-number arithmetic fail when GPU is enabled
- `identity.bqn` fold tests give slightly wrong answers on arrays > 100K
- Same BQN expression gives different results with and without GPU (non-determinism)

**Phase to address:**
GPU integration phase. The precision guard must be implemented BEFORE wiring any GPU arithmetic to f64 BQN arrays.

---

### Pitfall 4: Self-Hosting Requires Bit-Exact Bytecode, Not "Functionally Equivalent"

**What goes wrong:**
Self-hosting means embedding the output of `RBQN compile c.bqn` (the BQN compiler source) instead of CBQN's precompiled bytecode. If RBQN's compiler produces slightly different bytecode than CBQN (different constant folding, different block numbering, different body ordering), the embedded bytecode works for most programs but fails on edge cases that depend on exact body layout.

The most dangerous failure mode: the runtime1 and compiler bytecode files (from CBQN's `gen/` directory) were compiled by CBQN. Replacing them with RBQN-compiled output changes the bytecode format in subtle ways. The bootstrap then executes different bytecode than what was tested.

**Why it happens:**
BQN's compiler is not formally specified to produce a unique bytecode output. Two correct compilers can produce functionally equivalent but byte-different bytecode. `header.bqn` and `bytecode.bqn` tests exercise exact opcode sequences and will fail if bytecode is semantically equivalent but differently ordered.

**How to avoid:**
- Do NOT attempt self-hosting until all 13 official test files pass with CBQN-generated bytecode
- When self-hosting: validate by running the official test suite against the RBQN-compiled bytecode, not just against CBQN-compiled bytecode
- Keep CBQN-generated gen/ files as the reference; compare RBQN-generated bytecode against them using a diff tool before switching
- Test the self-hosting step in isolation: first verify `RBQN compile r1.bqn` produces functionally equivalent runtime1, then switch

**Warning signs:**
- Tests that pass with CBQN bytecode but fail with RBQN-compiled bytecode
- `bytecode.bqn` test failures that are intermittent (opcode boundary issues)
- Compiler compiles user code but not its own source

**Phase to address:**
Self-hosting phase (must come after test suite passes). Never attempt before 13/13 test files green.

---

### Pitfall 5: The Test Suite Tests Edge Cases You Won't Hit in Manual Testing

**What goes wrong:**
Manual testing of `1+1`, `↕5`, and simple blocks gives false confidence. The official BQN test suite (`prim.bqn`, `fill.bqn`, `identity.bqn`) exercises cases that almost never appear in casual use:

- **Fill elements on Take**: `5↑1‿2‿3` → `⟨1 2 3 0 0⟩` (fill with 0). `5↑"abc"` → `"abc  "` (fill with space). A wrong fill implementation silently produces `⟨1 2 3 1 1⟩` (using first element).
- **Fold identity on empty**: `+´⟨⟩` must return 0. `×´⟨⟩` must return 1. `⌊´⟨⟩` must return ∞. Getting these wrong causes `identity.bqn` to fail entirely.
- **Pervasive arithmetic**: `1‿2 + ⟨3‿4, 5‿6⟩` must recursively descend into nested arrays. A non-pervasive `+` returns a type error or wrong shape.
- **Rank-0 Enclose/Pick**: `<3` must return a rank-0 box containing 3. `⊑<3` must return 3. Many implementations mishandle this.
- **Character arithmetic**: `"a" + 1` → `"b"`. Arithmetic involving characters requires special handling that most CPU numeric paths skip.

**Why it happens:**
These edge cases are described in the BQN spec but not obvious from the main primitive documentation. They are guaranteed to fail in any incomplete implementation and will all be caught by the test suite.

**How to avoid:**
- Run the official test suite early — immediately after the compiler pipeline works, before investing in GPU or system functions
- Start with `simple.bqn` and `literal.bqn` (easiest), then `syntax.bqn`, `prim.bqn`, `bytecode.bqn`
- `fill.bqn` and `identity.bqn` are the hardest and can be deferred to a later phase

**Warning signs:**
- `prim.bqn` fails on any fill-related test (Take, Group with extras)
- `identity.bqn` fails on empty fold
- Character expressions `"a"+1` return numeric 98 instead of `"b"`
- Nested array arithmetic gives shape errors

**Phase to address:**
Test suite phase (immediately after compiler pipeline). Do not defer test suite to after GPU work.

---

### Pitfall 6: Panic-Based Error Handling Will Corrupt State Across Catch Boundaries

**What goes wrong:**
RBQN uses panic for runtime errors (139 panic sites per progress report). BQN has a `⎊` (Catch) modifier that catches errors and runs an alternative. If the catch implementation uses Rust's `catch_unwind`, any mutable global state (ARR_STORE, DERIVED_STORE, NS_STORE) that was partially mutated before the panic is left in an inconsistent state. The next expression after a caught error may use a corrupt store and produce wrong results or a segfault.

**Why it happens:**
Rust panics do not roll back mutable state. `catch_unwind` stops unwinding but every global HashTable, every counter, every Arc reference that was incremented and not decremented is still there. Stores that were halfway through an insert when the panic occurred may have an entry in an inconsistent state.

The bootstrap already uses `catch_unwind` for runtime1, compiler, and formatter. If runtime1 panics halfway through constructing its result array, some objects are in ARR_STORE and some are not. The formatter may then find those objects via NaN-box tags and produce garbled output.

**How to avoid:**
- Treat panic as non-recoverable for now. Remove or gate the `⎊` catch modifier until `Result`-based error propagation is in place
- At minimum: the compat harness must use a fresh bootstrap for each test expression — not share bootstrap state between tests
- Never advertise `⎊` as working until error handling is converted to `Result`
- Add a known-broken marker to `⎊` in the modifier dispatch table

**Warning signs:**
- `⎊` appears to work in isolation but subsequent expressions produce wrong results
- Memory usage grows unboundedly after repeated catches
- ARR_STORE size increases monotonically across test runs without GC

**Phase to address:**
The error handling conversion (Result-based) is long-term technical debt. In the short term: document that `⎊` is not safe to use and don't test it in the compat harness.

---

### Pitfall 7: GPU Async/Sync Mismatch Will Deadlock or Return Stale Data

**What goes wrong:**
wgpu is async. The VM is synchronous. Bridging them naively causes deadlocks:
- Calling `device.poll(wgpu::Maintain::Wait)` blocks the thread until GPU completes, but this also blocks wgpu's internal work that uses the same thread
- Calling `queue.submit()` + `buffer.slice(..).map_async()` + `device.poll(Wait)` is the correct synchronous pattern but only works if the buffer was created with `MAP_READ` usage in addition to `COPY_DST` — the current `GpuBuffer::storage()` only has `STORAGE | COPY_SRC | COPY_DST`, so downloading requires an intermediate staging buffer that is not pooled

**Why it happens:**
The current `GpuBuffer` type (`buffer.rs`) does not include `MAP_READ` usage. Downloading data requires creating a new `staging_buffer` on every readback. This is correct but expensive and easy to forget in new code.

The documentation says `device.poll(Wait)` is safe on native (non-WebGPU) but new contributors often call `poll(Poll)` (non-blocking) and miss that the data isn't ready yet.

**How to avoid:**
- Always use the pattern: submit command encoder → create staging buffer → copy GPU→staging → submit copy → poll(Wait) → map staging → read → unmap
- Pool staging buffers in `BufferPool` — they are currently allocated fresh on every download
- Add a wrapper function `download_f64(gpu_buf) -> Vec<f64>` that handles the full staging pattern; never inline it

**Warning signs:**
- GPU results are all zeros or uninitialized on first call, correct on second call (stale data from previous compute)
- Deadlock in `device.poll(Wait)` on WebGPU target (not native Rust, but easy to hit if testing in browser)
- GPU results are correct in isolation but wrong when two GPU ops chain without a readback between them

**Phase to address:**
GPU integration phase. Fix before wiring any kernel to the VM — correct the buffer pool to include staging buffers.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| `catch_unwind` for all bootstrap stages | Bootstrap never hard-crashes | State corruption after any panic; `⎊` is broken | Never for production; acceptable to reach demo |
| f32 GPU for f64 BQN arrays | GPU dispatch works today | Silent wrong answers for large-number or precision-sensitive programs | Only for sort/grade (ordinal only) — never for arithmetic |
| panic! for all runtime errors | Simple to implement | 139 sites to convert; test suite `identity.bqn` catches missing error propagation | Acceptable until test suite phase |
| fruntime as runtime (skip rtObjRaw) | Fixes PrimInd; avoids BQN wrapper confusion | BQN-wrapped primitives not available; user can't distinguish primitive from derived | Correct CBQN-equivalent approach — not tech debt |
| Monotonic ARR_STORE (no GC) | Simple reference model | Bootstrap alone creates thousands of leaked objects; any REPL session exhausts memory | Acceptable for demo; must fix before shipping |
| Global Mutex stores | Simple to implement | Prevents all multi-threading; contention on any array access | Acceptable; explicitly Out of Scope per PROJECT.md |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| CBQN gen/ bytecode | Assuming gen/runtime1.bqn.c and gen/runtime1x.bqn.c are the same | Use runtime1x when extended provide (40 entries) is available — RBQN uses extended provide, so runtime1x is correct |
| compiler interface | Calling compiler as `c1(compiler, src)` | Compiler is called dyadically: `c2(compiler, env, src)` where env is `⟨runtime, sysval_resolver, var_names, var_depths⟩` |
| formatter initialization | Passing `•Decompose` (sys_idx=1) as the `FmtNum` argument | FmtNum (4th arg to formatter) is a number-formatting function, not Decompose. Use SENTINEL if unavailable — formatter handles absent FmtNum gracefully |
| GPU buffer sizing | Using element count directly for `wgpu::BufferDescriptor.size` | `size` must be in bytes: `len * element_type.byte_size()`. Also: wgpu requires buffer sizes to be a multiple of 4 bytes |
| wgpu pipeline creation | Creating pipeline per-call | Use `PipelineCache` — pipeline compilation is 10-100ms on cold start; always cache by `PipelineKey` |
| provide[2] (log) | Mapping to natural log function | CBQN's `bi_log` is `⋆` (power), not a separate log function. The monadic form of `⋆` computes `e^x`; the runtime defines log as the inverse. Map provide[2] to fruntime[4] (⋆ power) |
| setPrims callback | Calling `c1(setPrims, ⟨decompose, primind⟩)` | Arguments must be in exact order: `⟨bi_decp, bi_primInd⟩` — Decompose first, PrimInd second |
| setInv callback | Calling `c1(setInv, ...)` monadically | setInv is called DYADICALLY: `c2(setInv, bi_setInvSwap, bi_setInvReg)` |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| GPU dispatch for small arrays | Interpreter becomes 100x slower for typical BQN programs (arrays < 10K elements) | Threshold check must be zero-overhead when below threshold — no buffer allocation, no pipeline lookup | Any array smaller than crossover: ~100K for single arithmetic, ~50K for fused chains |
| Unbounded ARR_STORE growth | REPL sessions exhaust memory; bootstrap alone leaks thousands of objects | GC is mandatory before shipping | Bootstrap creates ~1000 objects; 10 minutes of REPL use can exhaust 8GB |
| f64→f32→f64 round-trip on every GPU call | Wrong numerical results accumulate; benchmark shows overhead but results are wrong | Validate with integer arrays first; gate f64 arithmetic on SHADER_F64 | Any single GPU call on f64 data; hidden until precision-sensitive test runs |
| Missing pipeline cache warm-up | First GPU operation takes 50-100ms (shader compilation); UI/REPL feels broken | Pre-warm common pipelines (add, mul, reduce) at context initialization | First call to any new kernel type |
| Sort treating i32 as u32 (sign-bit bug) | Grade/sort of negative numbers returns wrong order | Flip sign bit before radix sort: `x ^ 0x80000000` | Any array with negative integers; `⍋ ¯3‿1‿¯7` gives wrong order |

---

## "Looks Done But Isn't" Checklist

These are features that compile, run, and produce output but are silently wrong.

- [ ] **Runtime bypass:** Verify `•PrimInd "+"` returns 0 (not ¯1) — if wrong, inverse system is broken despite seeming to load
- [ ] **`⁼` (Undo):** Verify `⊐⁼ 0‿1‿0‿2` returns `⟨0 2 3⟩` — simpler cases like `(+˜)⁼ 6` → 3 may work while structural inverses are wrong
- [ ] **`⌾` (Under):** Verify `1⌾(0⊸⊑) 3‿4‿5` returns `⟨1 4 5⟩` — the structural case is much harder than the invertible case
- [ ] **GPU dispatch:** Verify `+/ 100001 ⥊ 1` returns 100001 (not 100000 or a float with rounding error) — GPU path may be active without triggering threshold log
- [ ] **Fill elements:** Verify `5↑"ab"` returns `"ab   "` (3 spaces, not 3 '@' null characters) — fill tracking is wired but fill computation may be wrong
- [ ] **Empty fold identity:** Verify `×´⟨⟩` returns 1, `⌊´⟨⟩` returns ∞ — returns 0 (wrong) or throws (worse) in most incomplete implementations
- [ ] **Character arithmetic:** Verify `"a"+1` returns `"b"` — numeric paths return 98.0 (wrong type) if character promotion is missing
- [ ] **Pervasive under nesting:** Verify `1+⟨2‿3, 4‿5⟩` returns `⟨⟨3 4⟩ ⟨5 6⟩⟩` — non-pervasive paths give shape error or operate on the outer array only
- [ ] **Self-hosting:** Verify compiler compiles its OWN source (`RBQN compile c.bqn`) not just user programs — a compiler that compiles user code but not its own source is not self-hosting

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| PrimInd broken after refactor | LOW | Revert runtime array to use `fruntime` instead of `rtObjRaw`; add assertion to catch future regressions |
| SetInv not called / silently panics | LOW | Remove `catch_unwind` around `setInv` call; let it fail loudly; debug the panic |
| f32 precision causing wrong results | MEDIUM | Add precision guard to `should_use_gpu`; restrict GPU to non-arithmetic or integer-valued ops until SHADER_F64 is implemented |
| Self-hosting bytecode mismatch | HIGH | Revert to CBQN-generated bytecode; diagnose which test fails with RBQN-compiled bytecode before switching |
| GPU deadlock / stale data | MEDIUM | Switch to blocking poll pattern with staging buffers; pool staging buffers to avoid allocation overhead |
| ARR_STORE memory exhaustion | HIGH (long-term) | Short-term: restart process between test runs; long-term: implement refcount GC on BqnArr |
| `⎊` catch corrupting global state | MEDIUM | Disable `⎊` until Result-based errors; add known-broken marker in dispatch; don't test it |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| PrimInd broken after bypass | Phase: Runtime Bypass | `•PrimInd "+" = 0` assertion in compat harness |
| SetPrims/SetInv callback order | Phase: Runtime Bypass | `•Decompose ↕5` returns `⟨2, 6, ↕⟩`-form; `(+˜)⁼ 6` returns 3 |
| GPU f32 precision loss | Phase: GPU Integration | Precision guard in `should_use_gpu`; `prim.bqn` passes with GPU enabled |
| Self-hosting bytecode mismatch | Phase: Self-Hosting | All 13 test files pass with RBQN-compiled bytecode before removing CBQN dep |
| Test suite edge cases (fill, identity, pervasive) | Phase: Test Suite | Run in order: simple → literal → syntax → prim → fill → identity |
| Panic/catch state corruption | Phase: Error Handling | `⎊` disabled or converted to Result before release |
| GPU async/sync deadlock | Phase: GPU Integration | Pool staging buffers before any kernel dispatch wiring |
| Sort sign-bit bug | Phase: GPU Integration | `⍋ ¯3‿1‿¯7` returns correct order `⟨2 1 0⟩` |

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Runtime bypass (Option B) | PrimInd breaks silently | Assert after bootstrap: `•PrimInd "+"` must return 0 |
| Runtime bypass (Option B) | `setInv` swallowed by `catch_unwind` | Remove catch_unwind; verify `⁼` works on at least one primitive |
| Compiler pipeline | `env` argument format wrong | Compiler is dyadic: `c2(compiler, env, src)`; env has 4 slots |
| Test suite | Running tests before compiler works | Need compiler first; use `-nocomp` flag for VM-only tests first |
| Test suite | Treating `fill.bqn` as low priority | Fill failures cascade: `identity.bqn`, `prim.bqn` Take/Drop tests all depend on fills |
| GPU integration | Wiring arithmetic before precision guard | Add guard first; only wire sort/grade initially to prove dispatch path |
| GPU integration | Not pooling staging buffers | Every GPU readback creates a new staging buffer — add to pool before any real usage |
| Self-hosting | Attempting before test suite passes | Never attempt; 13/13 required first |
| Self-hosting | Switching bytecode without diff | Compare RBQN-compiled vs CBQN-compiled gen/ bytecode line-by-line before switching |

---

## Sources

- CBQN source: `src/load.c` — bootstrap sequence, provide array, runtime array construction (lines 460-530). HIGH confidence.
- RBQN progress report: `.planning/quick/3-assess-progress-toward-cbqn-compatible-g/PROGRESS.md` — 19-iteration debug history, exact failure modes. HIGH confidence (direct codebase analysis).
- RBQN bootstrap: `crates/rbqn/src/bootstrap.rs` — current implementation of bypass, setPrims, setInv. HIGH confidence (direct code review).
- RBQN GPU strategy: `.planning/research/gpu-strategy.md` — dispatch thresholds, f64 problem, sort sign-bit bug. MEDIUM confidence.
- RBQN CBQN scope: `.planning/research/cbqn-scope.md` — test suite structure, fill/identity edge cases, inverse system. HIGH confidence (sourced from BQN spec).
- BQN spec — Inferred properties (fills, identity elements): https://mlochbaum.github.io/BQN/spec/inferred.html. HIGH confidence.
- wgpu buffer usage documentation: https://docs.rs/wgpu/latest/wgpu/struct.BufferUsages.html. HIGH confidence.
- GPU sort sign-bit note: `crates/rbqn-gpu/src/kernels/sort.rs` comment "treat as unsigned sort, correct for non-negative values". HIGH confidence (direct code review).

---
*Pitfalls research for: BQN interpreter — runtime bypass, GPU dispatch, self-hosting*
*Researched: 2026-02-23*
