# Roadmap: RBQN

## Milestones

- ✅ **v2.0 — CBQN Drop-in with GPU Acceleration** — Phases 1–7 (shipped 2026-03-03)
- 🚧 **v3.0 — True Self-Hosting & CBQN Parity** — Phases 8–10 (in progress)

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

### 🚧 v3.0 — True Self-Hosting & CBQN Parity (In Progress)

**Milestone Goal:** RBQN with zero dependency on CBQN at any point — RBQN compiles its own bootstrap bytecode — plus full behavioral parity fixing all known stubs and edge cases.

- [ ] **Phase 8: Complete Self-Hosting** - RBQN compiles all .bin files; build chain requires no CBQN tool
- [ ] **Phase 9: CBQN Behavioral Parity** - All known system function stubs and edge cases fixed
- [ ] **Phase 10: Quality & Validation** - Portable test infrastructure and systematic parity corpus

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
**Plans**: TBD

Plans:
- [ ] 08-01: Implement `rbqn-gen --self` mode — drive .bin compilation through RBQN pipeline, no CBQN_PATH
- [ ] 08-02: Compile and commit compiler.bin and formatter.bin from RBQN; verify test suite still passes
- [ ] 08-03: Compile and commit runtime0.bin and runtime1x.bin from RBQN; verify test suite still passes
- [ ] 08-04: Document SELF3-02 verification; update CI/build docs; close QUAL-03

### Phase 9: CBQN Behavioral Parity
**Goal**: All known system function stubs and behavioral gaps are fixed to match CBQN 1:1
**Depends on**: Phase 8
**Requirements**: PAR3-01, PAR3-02, PAR3-03, PAR3-04, PAR3-05, PAR3-06
**Success Criteria** (what must be TRUE):
  1. `•bit.And`, `•bit.Or`, `•bit.Xor`, `•bit.Not`, `•bit.Shift` return results matching CBQN for integer array inputs
  2. `•term.RawMode`, `•term.CharB`, `•term.Flush` work matching CBQN terminal I/O behavior
  3. `•math.Comb n` returns the correct binomial coefficient C(n,2) and `•math.LCM n` returns the correct LCM value
  4. `•state` returns meaningful execution state rather than SENTINEL; `•CurrentError` returns the current error inside catch blocks rather than SENTINEL
  5. `•FFI "type"‿"libname"‿"funcname"` loads a shared library and calls a C function (basic pattern working)
**Plans**: TBD

Plans:
- [ ] 09-01: Implement `•bit` namespace — And, Or, Xor, Not, Shift matching CBQN
- [ ] 09-02: Implement `•term` namespace — RawMode, CharB, Flush
- [ ] 09-03: Fix `•math.Comb` monadic and `•math.LCM` monadic
- [ ] 09-04: Fix `•state` and `•CurrentError` to return real values
- [ ] 09-05: Implement `•FFI` basic pattern (stretch: full type coverage)

### Phase 10: Quality & Validation
**Goal**: Test infrastructure is portable and a systematic parity corpus validates RBQN matches CBQN
**Depends on**: Phase 9
**Requirements**: QUAL-01, QUAL-02
**Success Criteria** (what must be TRUE):
  1. `test_suite.sh` and `test_cbqn_compat.sh` run correctly on a fresh checkout without editing any paths
  2. A parity validation script runs CBQN and RBQN on the same BQN corpus and produces a diff report of any output divergences
  3. The parity corpus covers all system functions implemented in v3.0 (•bit, •term, •math edge cases, •state, •CurrentError)
**Plans**: TBD

Plans:
- [ ] 10-01: Fix hardcoded paths in test_suite.sh and test_cbqn_compat.sh (relative/env-based)
- [ ] 10-02: Write parity validation script and BQN corpus; run against CBQN, document results

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
| 8. Complete Self-Hosting | v3.0 | 0/4 | Not started | - |
| 9. CBQN Behavioral Parity | v3.0 | 0/5 | Not started | - |
| 10. Quality & Validation | v3.0 | 0/2 | Not started | - |
