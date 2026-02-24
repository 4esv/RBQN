---
phase: 02-test-baseline
plan: 01
subsystem: testing
tags: [bqn-test-harness, file-namespace, integration-test]

requires:
  - phase: 01-fix-bootstrap-pipeline
    provides: working compiler pipeline, system function dispatch, •Out/•BQN/•Fmt
provides:
  - •file namespace with Lines and List methods
  - Official BQN test harness integration (this.bqn runs against RBQN)
  - Baseline pass/fail counts for all 13 test files
  - harness.rs integration test suite
affects: [02-02, 02-03, all future correctness work]

tech-stack:
  added: []
  patterns: [synthetic NS object construction via store_ns for system namespaces]

key-files:
  created:
    - crates/rbqn/tests/harness.rs
  modified:
    - crates/rbqn-vm/src/derive.rs
    - crates/rbqn-vm/src/namespace.rs
    - crates/rbqn-prim/src/slash.rs

key-decisions:
  - "CWD-relative file paths for •file.Lines/List (no script-relative resolution yet)"
  - "Synthetic NS via store_ns with LazyLock caching for •file namespace"
  - "Run harness per-file in integration tests to avoid stack overflow on recursive header cases"

patterns-established:
  - "Synthetic namespace pattern: LazyLock<Mutex<Option<B>>> for cached system namespace objects"
  - "Harness integration: run from BQN/test/ dir, derive BQN path from CBQN_PATH sibling"

requirements-completed: [TEST-01, TEST-02, TEST-03, TEST-04]

duration: ~45min (across 2 agent attempts)
completed: 2026-02-24
---

# Plan 02-01: Wire BQN Test Harness Summary

**•file namespace (Lines + List) implemented, official BQN test harness running with 77% baseline (1014/1316 passing)**

## Performance

- **Duration:** ~45 min (across 2 agent attempts due to context exhaustion)
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Implemented •file.Lines (reads text file → array of BQN strings) and •file.List (lists directory → sorted array of names)
- Built •file as synthetic namespace object using store_ns infrastructure
- Official BQN test harness (this.bqn) runs against RBQN and reports results for all 13 test files
- Created comprehensive harness.rs integration test suite with per-file tests

## Baseline Results (2026-02-24)

| File | Pass/Total | Rate |
|------|-----------|------|
| simple | 20/20 | 100% |
| literal | 52/52 | 100% |
| token | 29/29 | 100% |
| syntax | 152/156 | 97% |
| bytecode | 36/37 | 97% |
| header | ~145/156 | 93% |
| under | 51/64 | 80% |
| prim | 409/564 | 73% |
| unhead | 27/44 | 61% |
| identity | 8/14 | 57% |
| namespace | 26/50 | 52% |
| fill | 29/62 | 47% |
| undo | 30/68 | 44% |
| **Total** | **~1014/1316** | **77%** |

## Task Commits

1. **Task 1: Implement •file namespace** - `b9d5b80` (feat)
2. **Task 2: Run harness + integration tests** - `b4467d6`, `10d959b` (fix: slash/indices + classify)

## Files Created/Modified
- `crates/rbqn-vm/src/derive.rs` - •file namespace construction, file_lines_c1, file_list_c1
- `crates/rbqn-vm/src/namespace.rs` - NS infrastructure used by •file
- `crates/rbqn-prim/src/slash.rs` - Slash/indices fixes for harness compatibility
- `crates/rbqn/tests/harness.rs` - Integration test suite with per-file harness tests

## Decisions Made
- CWD-relative paths for •file (sufficient for Phase 2, script-relative deferred)
- Per-file harness runs in tests to avoid stack overflow on recursive header cases
- Harness derives BQN test dir from CBQN_PATH sibling directory

## Deviations from Plan
- Required fixing monadic classify (⊐) and slash/indices primitives for harness to run — these were blocking bugs discovered during harness execution

## Issues Encountered
- Stack overflow on recursive header test cases when running all 13 files together — mitigated by per-file test execution
- Agent context exhaustion (2 attempts needed)

## Next Phase Readiness
- Harness infrastructure complete, baseline established
- Plans 02-02 and 02-03 can now target specific test file failures

---
*Phase: 02-test-baseline*
*Completed: 2026-02-24*
