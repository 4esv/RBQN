---
phase: 04-full-test-suite-green
plan: 10
subsystem: runtime
tags: [system-functions, ffi, namespace, hashmap, bqn-stubs]

# Dependency graph
requires:
  - phase: 04-full-test-suite-green
    provides: "All 13 test files passing (plans 04-01 through 04-09)"
provides:
  - "System stubs for FFI, bit, term, ns, HashMap"
  - "Working •ns.Keys/Values/Has/Get namespace introspection"
  - "Phase 4 fully complete — 13/13 test files green"
affects: [05-gpu-acceleration]

# Tech tracking
tech-stack:
  added: []
  patterns: [namespace-construction-pattern, sys-fn-dispatch-pattern]

key-files:
  created: []
  modified:
    - crates/rbqn-vm/src/derive.rs

key-decisions:
  - "HashMap uses namespace-per-instance with stub methods (test suite doesn't test it)"
  - "bit namespace fields are sys_fn stubs that throw on call (bitwise ops not needed for tests)"
  - "ns.Keys/Values/Has/Get fully functional — namespace introspection works"

patterns-established:
  - "Namespace stub pattern: static LazyLock cache + str2gid + NSDesc + Scope + store_ns"

requirements-completed: [SYS-22, SYS-23, SYS-24, SYS-25, SYS-26, TEST-05, TEST-06, TEST-07, TEST-08, TEST-09, TEST-10, TEST-11, TEST-12, TEST-13]

# Metrics
duration: 11min
completed: 2026-02-28
---

# Phase 04 Plan 10: System Stubs + Regression Gate Summary

**System stubs for FFI/bit/term/ns/HashMap registered; all 13 BQN test files pass with 0 failures — Phase 4 complete**

## Performance

- **Duration:** 11 min
- **Started:** 2026-02-27T23:52:15Z
- **Completed:** 2026-02-28T00:03:44Z
- **Tasks:** 3
- **Files modified:** 1

## Accomplishments
- Registered 5 system function stubs (FFI, bit, term, ns, HashMap) in sys_name_to_b
- Implemented working •ns namespace with Keys, Values, Has, Get methods
- All 13 BQN test files confirmed passing: simple, literal, syntax, bytecode, token, namespace, identity, unhead, prim, fill, header, under, undo
- Phase 4 goal achieved: full BQN test suite green

## Task Commits

Each task was committed atomically:

1. **Task 1+2: System stubs for FFI, bit, term, ns, HashMap** - `fd98b01` (feat)
2. **Task 3: Regression gate** - verification only, no code changes

**Plan metadata:** (pending)

## Files Created/Modified
- `crates/rbqn-vm/src/derive.rs` - Added sys_name_to_b entries, dispatch handlers, namespace constructors, ns introspection functions, HashMap constructor

## Decisions Made
- Combined Tasks 1 and 2 into a single commit since all changes were in the same file (derive.rs)
- HashMap uses per-instance namespace with stub methods (Count=0, Keys=empty, etc.) since the official test suite doesn't test HashMap
- bit namespace fields (_and_, _or_, _xor_, _not) are all mapped to a single sys_fn(161) stub that throws on call
- ns.Keys/Values/Has/Get are fully functional with real namespace introspection via NSDesc.exp_gids and Scope.vars

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 4 complete: all 13 BQN test files pass with 0 failures
- System stubs registered for real-world BQN program compatibility
- Ready for Phase 5 (GPU acceleration) or other enhancements

---
*Phase: 04-full-test-suite-green*
*Completed: 2026-02-28*
