# Roadmap: RBQN

## Milestones

- ✅ **v2.0 — CBQN Drop-in with GPU Acceleration** — Phases 1–7 (shipped 2026-03-03)
- ✅ **v3.0 — True Self-Hosting & CBQN Parity** — Phases 8–10 (shipped 2026-03-03)

## Phases

<details>
<summary>✅ v2.0 — CBQN Drop-in with GPU Acceleration (Phases 1–7) — SHIPPED 2026-03-03</summary>

- [x] Phase 1: Runtime Bypass and Working Pipeline (23 plans) — completed 2026-02-24
- [x] Phase 2: Test Baseline (3 plans) — completed 2026-02-24
- [x] Phase 3: Language Completeness (5 plans) — completed 2026-02-24
- [x] Phase 4: Full Test Suite Green (9 plans) — completed 2026-02-27
- [x] Phase 5: GPU Integration (6 plans) — completed 2026-02-28
- [x] Phase 6: Self-Hosting (3 plans) — completed 2026-03-01
- [x] Phase 7: Code Simplification & CBQN Parity (4 plans) — completed 2026-03-03

Full details: `.planning/milestones/v2.0-ROADMAP.md`

</details>

---

### ✅ v3.0 — True Self-Hosting & CBQN Parity (Shipped 2026-03-03)

**Milestone Goal:** RBQN with zero dependency on CBQN at any point — RBQN compiles its own bootstrap bytecode — plus full behavioral parity fixing all known stubs and edge cases.

- [x] **Phase 8: Complete Self-Hosting** - RBQN compiles all .bin files; build chain requires no CBQN tool
- [x] **Phase 9: CBQN Behavioral Parity** - All known system function stubs and edge cases fixed
- [x] **Phase 10: Quality & Validation** - Portable test infrastructure and systematic parity corpus

## Phase Details

### Phase 8: Complete Self-Hosting
**Goal**: RBQN compiles its own .bin files and the build chain requires no CBQN at any point
**Depends on**: Nothing (first phase of v3.0)
**Requirements**: SELF3-01, SELF3-02, SELF3-03, SELF3-04, SELF3-05, QUAL-03
**Success Criteria** (what must be TRUE):
  1. `cargo build` and `cargo install rbqn` complete successfully with CBQN_PATH unset and no CBQN on PATH
  2. `rbqn-gen --self` regenerates all four .bin files (runtime0.bin, runtime1x.bin, compiler.bin, formatter.bin) using RBQN with no CBQN_PATH required
  3. All 1316 official BQN tests pass using only the RBQN-compiled .bin files committed to the repo
  4. The RBQN-compiled .bin files are committed and replace all CBQN-compiled bytecode; no CBQN-compiled file remains in the repo
  5. The SELF3-02 human verification is documented: swap test result recorded and RBQN-compiled .bin committed
**Plans**: 4 plans

Plans:
- [x] 08-01-PLAN.md — Move .bin files to git-lfs bins/ dir; update build.rs with CBQN_PATH deprecation warning
- [x] 08-02-PLAN.md — Implement rbqn-gen --self mode; fix r1.bqn compilation; compile all 4 bins without CBQN_PATH
- [x] 08-03-PLAN.md — Fixpoint swap test; compile and commit RBQN-self bins; verify 1316 tests pass
- [x] 08-04-PLAN.md — Document SELF3-02 verification (VERIFICATION.md); update README with CBQN acknowledgement

### Phase 9: CBQN Behavioral Parity
**Goal**: All known system function stubs and behavioral gaps are fixed to match CBQN 1:1
**Depends on**: Phase 8
**Requirements**: PAR3-01, PAR3-02, PAR3-03, PAR3-04, PAR3-05, PAR3-06
**Success Criteria** (what must be TRUE):
  1. `•bit._and`, `•bit._or`, `•bit._xor`, `•bit._not` and all 9 bit modifiers return results matching CBQN for integer array inputs
  2. `•term.RawMode`, `•term.CharB`, `•term.Flush`, `•term.CharN`, `•term.OutRaw`, `•term.ErrRaw` work matching CBQN terminal I/O behavior
  3. `•math.Comb`, `•math.LCM`, `•math.GCD` monadic forms error (matching CBQN); `w •math.Comb x` computes C(w,x)
  4. `•state` returns `(path, name, args)` triple; `•CurrentError` returns the current error inside catch blocks
  5. `•FFI "type"‿"libname"‿"funcname"` loads a shared library and calls a C function (basic pattern working)
**Plans**: 5 plans

Plans:
- [x] 09-01-PLAN.md — Fix math monadic errors (Comb/LCM/GCD), fix Comb argument order, fix state return value
- [x] 09-02-PLAN.md — Implement term namespace (6 fields: Flush, RawMode, CharB, CharN, OutRaw, ErrRaw)
- [x] 09-03-PLAN.md — Implement bit namespace (9 1-modifier operations matching CBQN)
- [x] 09-04-PLAN.md — Implement CurrentError with catch handler plumbing
- [x] 09-05-PLAN.md — Implement FFI basic pattern (load shared library, call C functions)

### Phase 10: Quality & Validation
**Goal**: Test infrastructure is portable and a systematic parity corpus validates RBQN matches CBQN
**Depends on**: Phase 9
**Requirements**: QUAL-01, QUAL-02
**Success Criteria** (what must be TRUE):
  1. `test_suite.sh` and `test_cbqn_compat.sh` run correctly on a fresh checkout without editing any paths
  2. A parity validation script runs CBQN and RBQN on the same BQN corpus and produces a diff report of any output divergences
  3. The parity corpus covers all system functions implemented in v3.0 (•bit, •term, •math edge cases, •state, •CurrentError)
**Plans**: 2 plans

Plans:
- [x] 10-01-PLAN.md — Fix hardcoded paths in test_suite.sh and test_cbqn_compat.sh (env-var based, clear errors)
- [x] 10-02-PLAN.md — Write parity validation script and BQN corpus covering v3.0 system functions

## Progress

**Execution Order:** 8 → 9 → 10

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Runtime Bypass | v2.0 | 23/23 | Complete | 2026-02-24 |
| 2. Test Baseline | v2.0 | 3/3 | Complete | 2026-02-24 |
| 3. Language Completeness | v2.0 | 5/5 | Complete | 2026-02-24 |
| 4. Full Test Suite Green | v2.0 | 9/9 | Complete | 2026-02-27 |
| 5. GPU Integration | v2.0 | 6/6 | Complete | 2026-02-28 |
| 6. Self-Hosting | v2.0 | 3/3 | Complete | 2026-03-01 |
| 7. Code Simplification & CBQN Parity | v2.0 | 4/4 | Complete | 2026-03-03 |
| 8. Complete Self-Hosting | v3.0 | 4/4 | Complete | 2026-03-03 |
| 9. CBQN Behavioral Parity | v3.0 | 5/5 | Complete | 2026-03-03 |
| 10. Quality & Validation | v3.0 | 2/2 | Complete | 2026-03-03 |
