---
phase: 01-fix-bootstrap-pipeline
plan: 02b
subsystem: vm
tags: [bytecode, vm, opcodes, cbqn-compat]

requires:
  - phase: 01-01
    provides: "Working runtime0 with fork dispatch and typed modifier output"
provides:
  - "Corrected ALIM, ARMO, FN2Oi, SETC opcode implementations"
  - "bqn_merge function for ARMO array merge semantics"
  - "Scope Clone impl compatible with Mutex-based vars"
affects: [01-02a, 01-02c, 01-03]

tech-stack:
  added: []
  patterns: ["CBQN vm.c opcode audit methodology"]

key-files:
  created: []
  modified:
    - crates/rbqn-vm/src/vm.rs
    - crates/rbqn-vm/src/bytecode.rs

key-decisions:
  - "ARMO merge implemented with element-shape validation and typed output"
  - "FN2Oi kept at bytecode level even though RBQN compiler doesn't currently emit it"
  - "SETC separated from SETM in stack metadata to match CBQN sD_m/sC_m tables"

patterns-established:
  - "Opcode audit: compare CBQN bL_m, sD_m, sC_m tables against RBQN bytecode.rs metadata"

requirements-completed: [R-BOOT-4]

duration: 11min
completed: 2026-02-22
---

# Phase 01 Plan 02b: VM Opcode Audit Summary

**Fixed 5 VM opcode discrepancies vs CBQN: ALIM stack leak, ARMO missing merge, FN2Oi wrong bytecode length, SETC/SETCi/SETCv wrong stack metadata, Scope Clone RefCell/Mutex mismatch**

## Performance

- **Duration:** 11 min
- **Started:** 2026-02-22T00:11:34Z
- **Completed:** 2026-02-22T00:22:12Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- Audited all critical VM opcodes against CBQN src/vm.c reference implementation
- Fixed ALIM opcode that was silently dropping values from the stack (consumed 1, produced 0 instead of 0)
- Implemented bqn_merge for ARMO to properly merge arrays into higher-rank results
- Fixed FN2Oi bytecode length from 3 to 5 words (two u64 immediates per CBQN spec)
- Separated SETC from SETM in stack_diff and stack_consumed metadata (SETC pops 2 not 3)
- Fixed Scope Clone impl to use Mutex (matching current struct definition) instead of stale RefCell

## Task Commits

Each task was committed atomically:

1. **Task 1: Audit and fix array-building opcodes** - `88bdefd` (fix)

## Files Created/Modified
- `crates/rbqn-vm/src/vm.rs` - ALIM push-back fix, ARMO bqn_merge, FN2Oi dual-u64, Scope Clone Mutex fix
- `crates/rbqn-vm/src/bytecode.rs` - FN2Oi bc_len=5, SETC/SETCi/SETCv separated from SETM in stack metadata

## Decisions Made
- Implemented bqn_merge with shape validation: when all elements are arrays of the same shape, produces a merged result with leading dimension. Falls back to boxed list for mixed/non-conforming inputs.
- Fixed FN2Oi even though RBQN compiler currently does not emit it (it could be emitted by JIT or future optimization passes).
- SETC stack metadata fix affects compiler stack height calculations -- the compiler was computing max_stack too high for bodies containing SETC.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed Scope Clone impl RefCell/Mutex mismatch**
- **Found during:** Task 1 (build verification)
- **Issue:** vm.rs Clone impl for Scope used `std::cell::RefCell` but scope.rs had been changed to use `std::sync::Mutex`. This prevented compilation.
- **Fix:** Updated Clone impl to use `std::sync::Mutex::new(e.vars.lock().unwrap().clone())`
- **Files modified:** crates/rbqn-vm/src/vm.rs
- **Verification:** Build succeeds
- **Committed in:** 88bdefd (part of task commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Essential for compilation. No scope creep.

## Issues Encountered
None beyond the pre-existing Scope type mismatch documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- VM opcodes now match CBQN semantics for all audited operations
- ARMO merge enables correct handling of multi-dimensional array construction in runtime1
- SETC stack metadata fix ensures compiler stack height calculations are correct
- Ready for plan 02a (bootstrap exec pipeline) and 02c (compiler integration tests)

---
*Phase: 01-fix-bootstrap-pipeline*
*Completed: 2026-02-22*

## Self-Check: PASSED
- vm.rs: FOUND
- bytecode.rs: FOUND
- SUMMARY.md: FOUND
- Commit 88bdefd: FOUND
- Tests: 13 passed, 0 failed
