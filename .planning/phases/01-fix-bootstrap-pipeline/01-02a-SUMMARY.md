---
phase: 01-fix-bootstrap-pipeline
plan: 02a
subsystem: vm
tags: [scope, interior-mutability, runtime1, bootstrap]

requires:
  - phase: 01-01
    provides: "Working runtime0 with fork dispatch and typed modifier output"
  - phase: 01-02b
    provides: "VM opcode fixes including Mutex-based scope vars"
provides:
  - "Verified runtime1 completes without panicking"
  - "Confirmed scope variable isolation bug root cause and fix"
  - "Working tree consistency restored (stale RefCell code replaced with committed Mutex code)"
affects: [01-02c]

tech-stack:
  added: []
  patterns: ["Interior mutability via Mutex<Vec<B>> for shared parent scope variables"]

key-files:
  created: []
  modified:
    - crates/rbqn-vm/src/scope.rs
    - crates/rbqn-vm/src/vm.rs
    - crates/rbqn-vm/src/derive.rs
    - crates/rbqn-vm/src/block.rs
    - crates/rbqn-vm/src/namespace.rs

key-decisions:
  - "Root cause: Arc::make_mut cloned parent scopes on write, so child block modifications were invisible to parent. Fix: Mutex<Vec<B>> for vars + passing Arc<Scope> (not clone) to child blocks."
  - "Plan 02a overlapped with already-committed 02b fix (88bdefd). Working tree had stale pre-Mutex files causing the crash."

patterns-established:
  - "Scope vars use interior mutability (Mutex) to enable shared parent-child scope access"
  - "exec_block/exec_block_with_args take Arc<Scope> to share parent scopes without cloning"

requirements-completed: [R-BOOT-3]

duration: 14min
completed: 2026-02-22
---

# Phase 01 Plan 02a: Runtime1 Crash Trace and Fix Summary

**Traced runtime1 pick-on-empty crash to scope variable isolation via Arc::make_mut cloning; fix already committed in 02b (88bdefd); verified runtime1 now completes**

## Performance

- **Duration:** 14 min
- **Started:** 2026-02-22T00:11:34Z
- **Completed:** 2026-02-22T00:25:34Z
- **Tasks:** 1
- **Files modified:** 5 (restored to match HEAD)

## Accomplishments
- Traced the exact bytecode path causing `c2 prim=pick w=0 x_ia=0` crash via RBQN_VM_TRACE
- Identified root cause: inner FunBlock wrote to parent scope var (depth=2, pos=5) but Arc::make_mut cloned the scope, so outer block (depth=1, pos=5) still read the uninitialized empty array
- Confirmed fix: Mutex<Vec<B>> interior mutability + Arc<Scope> sharing (no clone) enables parent scope visibility of child writes
- Verified runtime1 now runs to completion (crash moved to compiler init stage, a separate issue)
- All 13 existing integration tests pass

## Root Cause Analysis

The crash path was:
1. Inner FunBlock executes `SETU` at `pscs[2].vars[5] = arr(ia=9)` (reshape result)
2. `Arc::make_mut(&mut pscs[2])` clones the scope because multiple Arcs reference it
3. Inner block returns
4. Outer block executes `VARO d=1 p=5` and reads uninitialized `arr(ia=0)` (empty harr default)
5. `0 pick arr(ia=0)` panics: "Index 0 out of bounds for length 0"

The fix (Mutex + Arc sharing) ensures `pscs[2]` in the child IS the same physical scope as `pscs[1]` in the parent.

## Task Commits

The core fix was already committed in plan 02b:

1. **Task 1: Trace and fix runtime1 crash** - `88bdefd` (fix, from plan 02b)

No new commit needed -- working tree was restored to match HEAD.

## Files Created/Modified
- `crates/rbqn-vm/src/scope.rs` - Mutex<Vec<B>> for vars, var_get/var_set methods
- `crates/rbqn-vm/src/vm.rs` - exec_block takes Arc<Scope>, pscs no longer mut, var_get/var_set calls
- `crates/rbqn-vm/src/derive.rs` - FunBlock/Md1Block/Md2Block dispatch passes psc.clone() (Arc share)
- `crates/rbqn-vm/src/block.rs` - eval_fun_block passes psc directly (no clone)
- `crates/rbqn-vm/src/namespace.rs` - NS::get_by_gid uses vars.lock()

## Decisions Made
- Root cause confirmed via targeted VM bytecode tracing (RBQN_VM_TRACE env var)
- No new commit since fix was already in 88bdefd; working tree consistency restored

## Deviations from Plan

Plan specified adding tracing, finding bug, and fixing it. The tracing was added and the bug was found, but the fix turned out to already exist in a prior commit (02b). The working tree had stale files that masked the fix.

---

**Total deviations:** 0 auto-fixed
**Impact on plan:** Plan objective achieved (runtime1 no longer crashes on pick-on-empty). Fix attribution goes to commit 88bdefd.

## Issues Encountered
- Working tree had inconsistent file versions (scope.rs had old RefCell code while HEAD had Mutex). This caused the runtime1 crash despite the fix being committed. Restored working tree to match HEAD.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Runtime1 completes successfully, producing 64-element runtime array
- Compiler initialization now fails (separate bug: type error in `+` with char array argument)
- Ready for plan 02c (compiler integration tests) which will address the compiler stage

---
*Phase: 01-fix-bootstrap-pipeline*
*Completed: 2026-02-22*

## Self-Check: PASSED
- scope.rs: FOUND (matches HEAD, Mutex-based)
- vm.rs: FOUND (matches HEAD, Arc<Scope> signatures)
- derive.rs: FOUND (matches HEAD, psc.clone() calls)
- block.rs: FOUND (matches HEAD, psc passed directly)
- namespace.rs: FOUND (matches HEAD, vars.lock())
- Commit 88bdefd: FOUND (contains the core fix)
- Tests: 13 passed, 0 failed
- Runtime1: completes without panic
