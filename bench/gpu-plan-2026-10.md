# GPU plan: what we try, how we know to bail, how we know it works

Companion to `bench/leads-2026-10.md`. Baselines on HEAD 958f2ed, M3 Pro,
hyperfine -N -r 5, ms. Bare startup is 6.9; `a←↕1e7 ⋄ ≠ a` is 10.8.

## Monkey test, 2026-10-09: can it sing?

Question: does a chain of *exact* integer ops on data that originates in the
interpreter beat the CPU end to end, Metal having no f64? Probe:
`scratchpad/dispatchbench/src/monkey.rs`, W2 shape `+´ (a×a)+(a×a)` over
`↕n`, i64 kernels (SHADER_INT64), result asserted equal to the CPU i64 sum,
10 reps, ms. "Resident" means `a` already on the device; "end to end" means
host Vec<i32> → upload → kernels → 8-byte readback.

| | 1M | 10M |
|---|---:|---:|
| CPU optimal Rust, fused chain+sum (no interpreter) | 0.31 | 2.20 |
| CPU Rust, 3 materialized passes (interpreter shape) | 0.67 | 9.34 |
| rbqn today | 10.8 (incl 6.9 startup) | 50.3 |
| GPU unfused resident (sq, add, reduce, finish) | 0.75 | 3.25 |
| GPU fused resident (one map-reduce + finish) | 0.42 | 0.55 |
| GPU iota-on-device + fused | 0.51 | 0.86 |
| GPU end to end, mapped upload + fused + readback | 0.52 | 1.41 |
| GPU end to end, mapped upload + unfused + readback | 0.85 | 3.75 |

**It sings.** At 10M, end to end from CPU-origin data, the exact GPU chain
beats optimal hand-written CPU Rust by 1.6x, beats the interpreter-shaped CPU
by 6.6x, and beats rbqn today by 35x. Resident and fused it is 4x optimal
CPU. At 1M optimal CPU still wins by 1.7x but interpreter-shaped CPU loses,
so the break-even against what an interpreter can realistically do is ~1M.

**Two API facts made the earlier floors disappear** (`src/floor.rs`, 20 reps):

| | ms |
|---|---:|
| `device.poll(Maintain::Wait)` on an empty submit | 1.43 |
| same, spinning on `Maintain::Poll` until `on_submitted_work_done` | 0.025 |
| 8-byte readback: staging copy + map + Wait (rbqn today) | 1.30 |
| 8-byte readback: MAPPABLE_PRIMARY_BUFFERS, map directly, spin | 0.006-0.011 |
| upload 40 MB: `queue.write_buffer` + Wait (rbqn today) | 6.0-8.0 |
| upload 40 MB: persistent STORAGE|MAP_WRITE, map + memcpy + unmap | 0.93-1.75 |
| CPU memcpy 40 MB | 1.28 |

`Maintain::Wait` sleeps; `write_buffer` stages and blits. Both are what
rbqn_gpu does on every op. Replacing them is the first pedestal and it also
fixes today's per-op path for free.

**Conditions on the verdict**, each a kill switch:
1. Integer-only exactness. Non-integer-producing ops stay CPU. If the product
   wants float math on the GPU, that is a separate decision (f32, lossy).
2. Fusion is required to beat *optimal* CPU; unfused resident (3.25) only
   ties it. Unfused still beats interpreter-shaped CPU 3x, so the pedestal
   order is residency first, fusion second, and the win is real at each step.
3. Spin-poll burns a core during GPU work. Fine for a single-threaded
   interpreter; revisit if rayon enters.
4. Not tested: arrays above 16.7M (2D grid exists in rbqn already), rank>1,
   boxed data (never goes to GPU), programs mixing many CPU-only prims (the
   readback counter decides).

**Pedestal order revised by the test:** Step 1 is now "spin-poll sync,
mappable readback, persistent mapped upload" (S, applies to the existing
per-op path, expect today's 1.6 ms/op at 1M to drop under 0.3). Then Steps
0, 2, 3, 5, 4, 6, 7 as written; Step 4 (shared encoder) moves after fusion
because the sync cost it was meant to amortise is gone.

## Monkey per path, 2026-10-09

Each path's hardest unknown, probed before any pedestal. All probes in the
session scratchpad unless noted; numbers are this M3 Pro.

| path | monkey (the question) | probe | verdict |
|---|---|---|---|
| GPU chain, exact ints | beats CPU end to end without f64? | `monkey.rs`, i64 kernels, W2 shape at 10M | **sings**: 1.41 ms end to end vs 2.2 optimal CPU vs 50 rbqn (table above) |
| GPU sync/upload | is the 1.5 ms/op floor real? | `floor.rs` | **no**: `Maintain::Wait` 1.4 ms → spin 0.025; `write_buffer` 6-8 ms/40 MB → mapped 0.9-1.7 |
| lazy device values at registry | does anything bypass `get_arr`? | grep for `ARR_STORE` and `>> 3` id decodes outside arrstore.rs | **clear**: the 10 direct decodes (vm.rs:209,225,456; typed.rs:21; derive.rs) are all function/derived ids, never arrays. Spare payload bit: only bit 0 (merge) is used |
| GPU sort (W5 271 ms) | can the existing radix kernel hit ~10 ms at 10M? | `sortb.rs`, `sortd.rs` on `radix_sort_u32` | **broken**: output is a permutation but not sorted at n=1000, 1e5, 1e6, 1e7; 105 ms at 10M vs CPU `sort_unstable` 44. Both GPU sort tests in gpu_kernels.rs are `#[ignore]`. Also `sort_i32` downloads, converts on CPU, re-uploads, twice. Kernel rewrite (per-workgroup stable scatter), not threshold tuning; literature target ~1 G elem/s on M1 Max |
| block call path (fib 120 ms) | is the cost allocation + hash lookups, as assumed? | `/usr/bin/sample` 3 s on fib 33, self time by symbol | **assumption wrong**: `eval_bc` self = 71% of samples; `Scope::new` 2%, malloc+free ~7%, `try_get_derived` 1.6%, `build_pscs` 1.2%, TLS 1.2%. The single-alloc-scope pedestal caps at ~10%. The cost is inside the inlined dispatch loop; needs an instruction-level look (Instruments) before any pedestal |
| CPU reshape/deshape | none; S and measured (14.7 + 4.9 ms) | | **go** |
| CPU widening sum/scan | none; S | | **go** |
| multicore via rayon | does a second core help on bandwidth-bound loops? | `par.rs`, 11 threads | **reductions only**: sum 2.5x at 10M, 2.0x at 1M, 0.3x at 1e5; materialized map 1.2x at 10M, slower below. Threshold ≥1M, reductions only |
| `•Fmt` native fast path | does f.bqn output have surprises for int arrays? | 7 edge cases vs installed CBQN | **small**: identical modulo the already-documented ≥1e15 divergence (rbqn prints all digits, CBQN `1e15`). Rank-1 ints <1e15: `⟨ a b c ⟩` with `¯`. Rank-2 uses box drawing; keep to rank 1 |
| compiler bin regen (`⟨⟩.field` fix) | is the regen pipeline reproducible from the repo? | 3 attempts | **yes, with a missing step**: `src/preprocessed/{r0,r1}.bqn` must be generated by `cd BQN/src && bqn -e '•Out (⟨"0"⟩ •Import "pr.bqn").src'` (and "1"); `bqn src/pr.bqn 0` prints nothing. Then `BQN_SRC=… rbqn-gen --self`. Result: only `bins/compiler.bin` changed, runtime/formatter byte-identical, reproducer now returns 10, suite no change. Left uncommitted in the working tree; the preprocessed step is undocumented and should go in rbqn_gen.rs's header |
| i64 exactness guard | cost of min/max tracking | reduce at 10M ≈ 0.3-0.5 ms incl. sync (from `monkey.rs`) | **cheap** |
| subgroup reduce | is reduce already bandwidth-bound? | fused map-reduce 10M in 0.55 ms incl. sync ≈ 80+ GB/s of 150 | **polish**, ≤2x headroom; after everything else |

## Hard facts that shape every step

- **Metal has no f64.** Adapter on this machine: SHADER_F64 false, SHADER_INT64
  true, SUBGROUP true (size 4..64), MAPPABLE_PRIMARY_BUFFERS true,
  TIMESTAMP_QUERY true, max buffer 10.7 GB. BQN numbers are f64, so the GPU
  can be *exact* only for integer-valued data: i32 kernels, i64 for sums,
  products and scans. Anything producing non-integers (÷, √, ⋆, •math) stays
  on the CPU or is lossy. "GPU BQN" means integer BQN plus whatever f32 the
  user explicitly accepts. This is the first thing to decide, not discover.
- **Per-op readback is the cost, not dispatch.** Measured
  (`scratchpad/dispatchbench`, 1M i32 add, 100 reps): submit+readback per op
  1.6 ms; resident, one wait: 0.09-0.15; one encoder: 0.06-0.10. At 10M:
  6.3 / 0.93 / 0.89 vs CPU add+sum 1.9. Device init 9-15 ms warm, 215 cold;
  first pipeline compile 2-4 ms.
- **Arrays are immutable `Arc<BqnArr>` in a never-freed thread-local registry**
  (arrstore.rs:51). Every array read goes through `get_arr` (206 call sites);
  `ArrData` is matched at ~205 sites across four crates. So a lazy device
  value is cheapest as a *registry-level* thing, not an `ArrData` variant.
- Current GPU hooks: arith (array-array, same shape), fused chains, fold,
  scan, grade, sort, matmul, softmax. All upload → dispatch → block → download
  → convert to F64 → squeeze (gpu_runtime.rs:49-69: two extra passes per op).
- Today's test suite: `BQN_TEST_DIR=~/Code/forks/BQN/test ./test_suite.sh`.

## Workload set (CPU today, ms)

| id | expression | CPU | note |
|---|---|---:|---|
| W1 | `a←↕1e7 ⋄ ≠ a+a` | 41.4 | single elementwise; 30 ms above baseline = 3 ns/elem, slow on CPU too |
| W2 | `a←↕1e7 ⋄ +´ (a×a)+(a×a)` | 50.3 | 3-op chain + reduce; products exceed i32 → needs i64 kernels |
| W3 | `a←↕1e7 ⋄ +´ 1+2×3+4×a` | 85.9 | 4 scalar ops + reduce, the fusion case |
| W4 | `a←↕1e7 ⋄ ⌈´ +\` a` | 41.9 | scan + reduce; sums exceed i32 → i64 |
| W5 | `a←1e7⥊↕1000 ⋄ ≠ ∧a` | 271.3 | sort 10M; GPU radix sort exists but sits behind 100M threshold |
| W6 | `a←1e6⥊↕1000 ⋄ ≠ ∧a` | 34.1 | sort 1M |
| W7 | `a←↕1e6 ⋄ +´ (a×a)+(a×a)` | 10.8 | 1M chain: the break-even probe |
| W0 | README rows via `bench/compare.sh` | | must not regress |

Success for the whole program: W2-W5 at ≤ 1/3 of CPU, W7 at ≤ CPU, W0 flat,
test suite identical. Resident GPU add at 10M measured 0.9 ms/op, so a 3-op
chain + reduce + one readback of a scalar should land near 10.8 + 3 + 1 ≈ 15
ms for W2; that is the target, not "faster than now".

## Steps, in order, each a separate unit with its own proof

### Step 0. Instrumentation (S)
Add to RBQN_GPU_DEBUG=1 a per-run summary: dispatches, submits, readbacks,
bytes up, bytes down, device-init ms, pipeline compiles. Also `RBQN_GPU=force`
(threshold 0) and `RBQN_GPU=off`.
- **Proof:** the counters on W1-W7 match hand counts.
- **Why first:** every later bail decision reads these numbers.

### Step 1. Pay init once, early (S)
Create the device and precompile arith/reduce/scan pipelines on a background
thread at startup when the program is a file or `-e` longer than a threshold,
or always and measure. Persist nothing yet.
- **Proof:** `+\` ↕10000000` drops from 38.8 to within 1 ms of the CPU 26.8;
  bare `1` stays within 0.5 ms of 6.9 (init must be off the critical path).
- **Bail:** if init on a thread adds >1 ms to bare startup or races the
  thread-local registry, defer init to first use and accept the 12 ms once.

### Step 2. Lazy device values at the registry (M, the lever)
A `B` with ARR_TAG and a spare low bit (merge uses bit 0; use bit 1) whose
registry entry is `Pending { buf: Arc<GpuBuffer>, shape, elem }`. `get_arr`
on it downloads, converts, inserts the materialised `BqnArr`, returns it:
zero changes at the 206 `get_arr` sites or the 205 `ArrData` sites. GPU-aware
hooks check the registry for a device buffer *before* calling `get_arr`.
Shape and element type live host-side so `≠ ≢ =` never sync. Buffers freed
by `Arc` drop into the pool. Counter: readbacks per run (Step 0).
- **Proof:** W2 and W3 with `RBQN_GPU=force`: dispatches = ops, readbacks = 1
  (the final scalar), wall time ≤ 20 ms. W1 ≤ CPU.
- **Bail:** if a correctness fix needs touching more than ~20 of the
  `get_arr` sites (meaning the registry indirection is leaking), stop and do
  the `ArrData::Gpu` variant properly instead (L). If hidden readbacks on W0
  rows push any row >10% slower, lower the dispatch threshold back until flat.
- **Trap to pre-empt:** `+´` returns a scalar; return a 1-element pending
  value and let the comparison/print force it.

### Step 3. i64 integer kernels and exactness guard (M)
Add i64 variants for add/sub/mul, reduce and scan (SHADER_INT64 confirmed).
Keep the existing bound checks (`gpu_sum_fits_i32` etc.) but widen to "result
is an integer within ±2^53" using min/max tracked per device value, computed
by a reduce kernel when unknown. Non-integer-producing ops fall back to CPU
(forces readback; counted).
- **Proof:** W2 and W4 exact-equal to CPU results over a mutation set:
  `↕n` for n in {1e6, 1e7, 3e7}, negatives, and values near 2^31 and 2^53;
  the suite passes. Mutation-test the guard: flip the bound and confirm the
  test that catches it fails.
- **Bail:** if i64 kernels on M3 Pro run slower than 2x the i32 rate for
  elementwise (measure W2 with force), the integer story is reduce/scan-only
  and elementwise stays i32 with CPU fallback on overflow.

### Step 4. One open encoder, flush on sync (M)
Record all dispatches into a shared encoder; submit on readback, every N
dispatches (start 64), or X MB of pending outputs (start 512 MB). Uploads go
through `queue.write_buffer` before the submit that needs them.
- **Proof:** dispatchbench pattern C numbers reproduced inside rbqn: W3
  submits = 1, dispatches = 5, time within 15% of Step 2's result (the win is
  small here; this step exists for many-op programs and for Step 5).
- **Bail:** if Metal shows no difference between B and C patterns inside rbqn
  (<5% on a 50-op synthetic chain), skip it; keep per-op submits.

### Step 5. Lazy elementwise tree → one kernel (M)
Elementwise ops on device values build an expression node instead of
dispatching. Materialise (compile a WGSL kernel keyed by tree shape+dtypes,
scalars as uniforms, cache it) on: non-elementwise consumer, fan-out >1,
readback, or 32 nodes. Inline the tree into the reduce/scan load.
- **Proof:** W3 dispatches drop from 5 to 1 (fused map+reduce) and time
  within 2 ms of W1 (one pass over memory). Kernel cache hit on the second
  run of the same expression in a REPL session.
- **Bail:** if WGSL compile per distinct tree is >5 ms and typical programs
  produce >10 distinct trees, cap fusion to chains ≤4 and precompile those.

### Step 6. Thresholds from measurement (S)
With Steps 1-5 in, sweep n in {1e5, 3e5, 1e6, 3e6, 1e7} for elementwise,
reduce, scan, sort and set each threshold to the measured crossover × 1.5.
Expect ~1M for a chain, higher for a lone op. Sort: fix the per-pass
histogram readback in kernels/sort.rs (4 blocking syncs today) first, then
expect W5 at ~10-20 ms.
- **Proof:** W0 flat; W5 ≤ 90 ms (3x) and ideally ≤ 30 ms; W7 ≤ 10.8.

### Step 7. Subgroup reduce/scan (S-M), segmented folds (M)
Only after 6. Proof: reduce at 10M within 1.2x of memcpy bandwidth
(measure with TIMESTAMP_QUERY, which is available).

## Global bail criteria

- Test suite diff is non-empty at the end of any step: the step is not done.
- Any W0 row regresses >10% with GPU on, and Step 0 shows the cause is a
  hidden readback that cannot be avoided: that primitive loses its GPU path.
- After Step 3, if the integer-only constraint means fewer than half the
  README rows and workload set can use the GPU, write that down as the
  product boundary and decide whether f32 mode (`RBQN_GPU=f32`, lossy,
  opt-in) is wanted. Do not slide into it silently.

## What done looks like

`bench/compare.sh` extended with W1-W7, run before and after each step, the
delta pasted into the PR. Step 2 is the first point where the plan is
validated or killed: W2 at ≤ 20 ms with one readback, or we are not building
a GPU array language on this stack.
