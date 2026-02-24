# Roadmap: RBQN v2.0 — CBQN Drop-in with GPU Acceleration

## Overview

RBQN has the architecture in place: five crates, NaN-boxed values, a working VM, 44 primitives, and a GPU crate with kernel stubs. The runtime bypass (Option B) is already implemented in bootstrap.rs — Phase 1 verifies it is correct before the compiler pipeline can be trusted. That unblocks the test harness (Phase 2), language completeness work (Phase 3), and the final push to all 13 test files green (Phase 4). Once correct, the GPU differentiator gets wired in (Phase 5) and self-hosting removes the CBQN build dependency entirely (Phase 6).

## Milestone: v2.0 — CBQN Drop-in with GPU Acceleration

## Phases

- [x] **Phase 1: Runtime Bypass and Working Pipeline** - Verify Option B bypass, get compiler executing arbitrary BQN, REPL working (completed 2026-02-24)
- [x] **Phase 2: Test Baseline** - Wire official test harness, establish pass/fail count, pass simple/literal/syntax/bytecode (completed 2026-02-24)
- [x] **Phase 3: Language Completeness** - All missing modifiers, primitive edge cases, essential system functions (completed 2026-02-24)
- [ ] **Phase 4: Full Test Suite Green** - Drive all 13 official test files to 0 failures
- [ ] **Phase 5: GPU Integration** - Wire rbqn-gpu into primitive dispatch with precision guards and benchmarks
- [ ] **Phase 6: Self-Hosting** - Compile own bytecode, embed in binary, ship via cargo install

## Phase Details

### Phase 1: Runtime Bypass and Working Pipeline
**Goal**: Users can execute arbitrary BQN programs via CLI and REPL with native Rust primitives serving as the runtime
**Depends on**: Nothing (first phase)
**Requirements**: PIPE-01, PIPE-02, PIPE-03, PIPE-04, PIPE-05, PIPE-06, PIPE-07, PIPE-08, SYS-01, SYS-02, SYS-03, SYS-04, SYS-05
**Success Criteria** (what must be TRUE):
  1. `rbqn -e '1+1'` outputs `2` and `rbqn -e '+´1‿2‿3'` outputs `6`
  2. `•PrimInd "+"` returns `0` (not ¯1), confirming bypass correctly passes fruntime not rtObjRaw
  3. REPL maintains variable state: `a←5` entered on one line is accessible on the next
  4. `•Out "hello"` prints to stdout; `•BQN "1+1"` evaluates to `2`; `•Exit 0` terminates
  5. Runtime1, compiler, and formatter all load without panicking
**Plans:** 23/5 plans complete

Plans:
- [x] 01-01-PLAN.md — Fix compiler/VM correctness bugs and verify bypass pipeline (PIPE-01..06)
- [x] 01-02-PLAN.md — Implement critical system functions: •BQN, •Out, •Fmt, •Exit, environment (SYS-01..05)
- [x] 01-03-PLAN.md — Formatter loading, exec_string pipeline, fallback formatting
- [ ] 01-04-PLAN.md — [gap closure] Fix string literal compilation and dyadic/fold ops through compiler (PIPE-01..06, PIPE-08, SYS-01..02)
- [ ] 01-05-PLAN.md — [gap closure] REPL variable persistence and Phase 1 verification tests (PIPE-07, PIPE-08, SYS-03..05)

### Phase 2: Test Baseline
**Goal**: Official BQN test harness wired and running, with the four structurally-simplest test files passing
**Depends on**: Phase 1
**Requirements**: TEST-01, TEST-02, TEST-03, TEST-04
**Success Criteria** (what must be TRUE):
  1. Running the test harness reports a pass/fail count across all 13 official test files without crashing
  2. `simple.bqn` passes: basic arithmetic, assignment, and simple conditionals all correct
  3. `literal.bqn` passes: number literals, character literals, and string literals all parse and evaluate correctly
  4. `syntax.bqn` and `bytecode.bqn` pass: block syntax and compiled bytecode correctness verified
**Plans:** 3/3 plans complete

Plans:
- [ ] 02-01-PLAN.md — Wire official test harness: implement •file namespace (Lines, List), run baseline
- [ ] 02-02-PLAN.md — Fix simple.bqn (20 tests) and bytecode.bqn (37 tests) to 0 failures
- [ ] 02-03-PLAN.md — Fix literal.bqn (69 tests) and syntax.bqn (244 tests) to 0 failures

### Phase 3: Language Completeness
**Goal**: All 20 modifiers work, primitive edge cases match spec, and essential system functions exist — everything the hard test files require
**Depends on**: Phase 2
**Requirements**: MOD-01, MOD-02, MOD-03, MOD-04, PRIM-01, PRIM-02, PRIM-03, PRIM-04, SYS-06, SYS-07, SYS-08, SYS-09, SYS-10, SYS-11, SYS-12, SYS-13, SYS-14, SYS-15, SYS-16, SYS-17, SYS-18, SYS-19, SYS-20, SYS-21
**Success Criteria** (what must be TRUE):
  1. `+⁼9` returns `3` (Undo with sqrt inverse); all 14 spec-required inverse mappings work
  2. `⌽⌾(1⊸↑) "abc"` returns `"abc"` (Under structural mode for ⌽, ⍉, k⊸⊏, k⊸↑, k⊸↓)
  3. `f⎉1` applies f to rank-1 cells; `f⚇2` applies f at depth 2 (Rank and Depth correct)
  4. Fill propagation: `5↑1‿2‿3` pads with correct fill; `«` and `»` shift with correct fill element
  5. `•FChars "f.bqn"` reads a file; `•math.Sin π÷2` returns `1`; `•Out`, `•SH`, `•Import` all work
**Plans:** 5/5 plans complete

Plans:
- [x] 03-01-PLAN.md — Fix modifier semantics (Undo, Under, Rank, Depth) and primitive completeness (fill, identity, pervasive)
- [x] 03-02-PLAN.md — File I/O system functions, Import with caching, introspection, utility functions
- [x] 03-03-PLAN.md — Math/rand/platform namespaces, time functions, shell execution, •_while_ modifier
- [x] 03-04-PLAN.md — [gap closure] Fix Undo gaps (sys 203 c2, dyadic +⁼) and Under functional-k patterns (MOD-01, MOD-02)
- [x] 03-05-PLAN.md — [gap closure] Fix prim structural edge cases — prim 81.2% (458/564), overall 84.7% excl. namespace (PRIM-04)

### Phase 4: Full Test Suite Green
**Goal**: All 13 official BQN test files pass with 0 failures
**Depends on**: Phase 3
**Requirements**: TEST-05, TEST-06, TEST-07, TEST-08, TEST-09, TEST-10, TEST-11, TEST-12, TEST-13, SYS-22, SYS-23, SYS-24, SYS-25, SYS-26
**Success Criteria** (what must be TRUE):
  1. `prim.bqn` passes: all 44 primitive functions return spec-correct output
  2. `fill.bqn`, `identity.bqn`, `under.bqn`, and `undo.bqn` all pass
  3. `header.bqn`, `unhead.bqn`, `namespace.bqn`, and `token.bqn` all pass
  4. All 13 test files report 0 failures; test harness exits 0
**Plans:** 5 plans

Plans:
- [ ] 04-01-PLAN.md — Fix namespace export (compile_block NSDesc from body_arr[2]/[3]) and header namespace tests
- [ ] 04-02-PLAN.md — Fix prim gap clusters: table scalar args, group multi-dim, slash atoms, reshape auto-dim
- [ ] 04-03-PLAN.md — Wire block header inverse bodies and implement missing native inverses (⌽⁼, ⍉⁼, +`⁼)
- [ ] 04-04-PLAN.md — Fix fill propagation through structural ops, identity elements, remaining prim clusters
- [ ] 04-05-PLAN.md — Fix Under gaps, remaining test failures, add system stubs (•FFI, •bit, •term, •ns, •HashMap)

### Phase 5: GPU Integration
**Goal**: Arrays above 50K elements transparently dispatch to GPU for arithmetic, sort/grade, fold, and scan — with measurable speedup and no correctness regression
**Depends on**: Phase 4
**Requirements**: GPU-01, GPU-02, GPU-03, GPU-04, GPU-05, GPU-06, GPU-07, GPU-08, GPU-09, GPU-10
**Success Criteria** (what must be TRUE):
  1. `+´ 1e5⥊1` dispatches to GPU (visible via debug flag) and returns the correct result `100000`
  2. `⍋ 1e5⥊↕100` sorts on GPU; output matches CPU sort exactly for integer-valued arrays
  3. Precision guard blocks GPU dispatch for arrays with values above 2^24; falls back to CPU silently
  4. Criterion benchmark shows measurable speedup vs CPU for arrays >100K elements on Apple Silicon
  5. All 13 test files still pass after GPU wiring (correctness not regressed)
**Plans**: TBD

Plans:
- [ ] 05-01: TBD

### Phase 6: Self-Hosting
**Goal**: `cargo install rbqn` works with no CBQN installed and no CBQN_PATH environment variable
**Depends on**: Phase 5
**Requirements**: SELF-01, SELF-02, SELF-03, SELF-04, SELF-05
**Success Criteria** (what must be TRUE):
  1. RBQN compiles `c.bqn` (the BQN compiler source, ~3000 lines) without crashing
  2. RBQN-generated bytecode matches CBQN-generated bytecode for all 13 test files
  3. `cargo install rbqn` completes on a machine with no CBQN installed
  4. All 13 test files pass using the embedded (RBQN-compiled) bytecode with CBQN_PATH unset
**Plans**: TBD

Plans:
- [ ] 06-01: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Runtime Bypass and Working Pipeline | 0/3 | Complete    | 2026-02-24 |
| 2. Test Baseline | 0/3 | Complete    | 2026-02-24 |
| 3. Language Completeness | 5/5 | Complete   | 2026-02-24 |
| 4. Full Test Suite Green | 0/5 | In Progress | - |
| 5. GPU Integration | 0/? | Not started | - |
| 6. Self-Hosting | 0/? | Not started | - |
