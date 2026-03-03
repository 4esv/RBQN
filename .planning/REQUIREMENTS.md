# Requirements: RBQN v3.0

**Defined:** 2026-03-03
**Core Value:** Correct BQN execution with identical behavior to CBQN

## v3.0 Requirements

### SELF3: True Self-Hosting (No CBQN Bytecode)

The committed `.bin` files should be compiled by RBQN itself — not by CBQN. No CBQN tool should be needed at any point in the development chain.

- [ ] **SELF3-01**: RBQN compiles compiler.bin and formatter.bin from BQN source using its own pipeline; output committed to repo replacing CBQN-compiled versions
- [ ] **SELF3-02**: RBQN compiles runtime0.bin and runtime1x.bin from BQN source (r0.bqn, r1.bqn); RBQN-compiled versions committed to repo
- [ ] **SELF3-03**: All 1316 official BQN tests pass using only RBQN-compiled .bin files (no CBQN-compiled fallback)
- [ ] **SELF3-04**: `rbqn-gen --self` (or equivalent) regenerates all .bin files using RBQN, no CBQN_PATH required
- [ ] **SELF3-05**: `cargo build` and `cargo install rbqn` complete with CBQN_PATH unset and no CBQN on PATH

### PAR3: CBQN Behavioral Parity

Fix all known behavioral gaps to achieve 1:1 parity with CBQN.

- [ ] **PAR3-01**: •FFI loads shared libraries and calls C functions from BQN — basic `•FFI"type"‿"libname"‿"funcname"` pattern working
- [ ] **PAR3-02**: •bit namespace fully implemented: And, Or, Xor, Not, Shift (matching CBQN behavior)
- [ ] **PAR3-03**: •term namespace: RawMode, CharB, Flush working (matching CBQN terminal I/O behavior)
- [ ] **PAR3-04**: •math.Comb monadic returns correct binomial coefficient C(n,2); •math.LCM monadic returns correct LCM value
- [ ] **PAR3-05**: •state returns meaningful value (current BQN execution state) rather than SENTINEL
- [ ] **PAR3-06**: •CurrentError returns current error value inside catch blocks rather than SENTINEL

### QUAL: Quality & Infrastructure

- [ ] **QUAL-01**: test_suite.sh and test_cbqn_compat.sh use relative paths — portable for any contributor, not hardcoded to /Users/axel/
- [ ] **QUAL-02**: Parity validation script: run CBQN and RBQN on the same BQN corpus, diff outputs, report divergences
- [ ] **QUAL-03**: SELF-02 human verification documented as complete: swap test passes, rbqn-compiled .bin committed

## Future Requirements

### Performance
- **PERF-01**: SIMD-accelerated primitive dispatch for CPU path
- **PERF-02**: Type-specialized array dispatch (avoid boxing for homogeneous arrays)
- **PERF-03**: Array squeeze (compress storage for small-range integers)

### Advanced
- **ADV-01**: Multi-threaded execution (requires removing global Mutex stores)
- **ADV-02**: Custom GPU kernel API for user-defined compute shaders
- **ADV-03**: JIT compilation for hot BQN functions

## Out of Scope

| Feature | Reason |
|---------|--------|
| Multi-threaded execution | Global mutex stores require architectural change — deferred |
| Pure-Rust bootstrap (no BQN source files) | Rewriting compiler/runtime in pure Rust is a separate major effort |
| Custom GPU kernel API | Internal optimization only |
| JIT compilation | Correctness first |
| IDE/editor integration | Not part of CLI drop-in |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| SELF3-01 | Phase 8 | Pending |
| SELF3-02 | Phase 8 | Pending |
| SELF3-03 | Phase 8 | Pending |
| SELF3-04 | Phase 8 | Pending |
| SELF3-05 | Phase 8 | Pending |
| QUAL-03 | Phase 8 | Pending |
| PAR3-01 | Phase 9 | Pending |
| PAR3-02 | Phase 9 | Pending |
| PAR3-03 | Phase 9 | Pending |
| PAR3-04 | Phase 9 | Pending |
| PAR3-05 | Phase 9 | Pending |
| PAR3-06 | Phase 9 | Pending |
| QUAL-01 | Phase 10 | Pending |
| QUAL-02 | Phase 10 | Pending |

**Coverage:**
- v3.0 requirements: 14 total
- Mapped to phases: 14
- Unmapped: 0

---
*Requirements defined: 2026-03-03*
