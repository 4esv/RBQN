---
phase: 07-code-simplification-cbqn-parity
plan: 02
subsystem: runtime
tags: [bqn, formatter, error-handling, repr, glyph]

requires:
  - phase: 07-01
    provides: zero-warning workspace, all 1316 tests green

provides:
  - •Fmt/-p mode box-drawing output for rank-2+ arrays
  - •Repr correct string output for numeric arrays (↕5 → "0‿1‿2‿3‿4")
  - Clean error messages without Domain error double-prefix
  - •Glyph returns descriptive strings for non-primitive functions
  - Typed BqnError preserved through panic boundaries

affects:
  - user-facing output
  - error messages
  - CBQN behavioral parity

tech-stack:
  added: []
  patterns:
    - "Native repr function (sys_fn 200) avoids infinite recursion in BQN formatter init"
    - "throw_bqn(e) panics with typed BqnError; panic_to_bqn_error() catches it typed"

key-files:
  created: []
  modified:
    - crates/rbqn/src/bootstrap.rs
    - crates/rbqn-core/src/error.rs
    - crates/rbqn-vm/src/derive.rs
    - crates/rbqn/src/exec.rs
    - crates/rbqn/src/main.rs
    - crates/rbqn-vm/src/modifiers.rs
    - crates/rbqn-vm/src/scope.rs

key-decisions:
  - "Native repr function (sys_fn 200) instead of m_sys_fn(35) for formatter init — m_sys_fn(35) causes infinite recursion since BQN repr calls it back for number formatting"
  - "throw_bqn(BqnError) via panic_any preserves error variant through catch_unwind — eliminates Domain error re-wrapping and double-prefix"
  - "•Glyph returns '(function block)' etc. for non-primitives — matches CBQN sysfn.c behavior"

requirements-completed: [PAR-01, PAR-02, PAR-03]

duration: 24min
completed: 2026-03-02
---

# Phase 7 Plan 02: CBQN Parity Fixes Summary

**Native repr (sys_fn 200) for BQN formatter enables box-drawing for rank-2+ arrays and correct •Repr, typed error panic propagation eliminates Domain error double-prefix**

## Performance

- **Duration:** 24 min
- **Started:** 2026-03-02T21:32:12Z
- **Completed:** 2026-03-02T21:56:34Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- `rbqn -p "2‿3⥊↕6"` now produces box-drawing output matching CBQN (was `[2‿3×…]`)
- `•Repr ↕5` returns `0‿1‿2‿3‿4` matching CBQN (was `‿‿‿‿`)
- `!0` error shows `Error: Assertion error: Assertion failed: 0` (was `Error: Domain error: Assertion error: ...`)
- `•Glyph` returns descriptive strings for non-primitive functions
- All 1316 BQN test suite cases still pass; 21/21 CBQN compat cases still pass

## Task Commits

1. **Task 1: Fix •Fmt and •Repr output for arrays** - `b01cb31` (fix)
2. **Task 2: Fix error message format and •Glyph for non-primitives** - `f53db76` (fix)

## Files Created/Modified

- `crates/rbqn/src/bootstrap.rs` - Pass sys_fn(200) as •Repr to formatter module
- `crates/rbqn-core/src/error.rs` - Add throw_bqn(BqnError) using panic_any for typed panic
- `crates/rbqn-vm/src/derive.rs` - Add native_repr_c1 (sys_fn 200), fix •Glyph, use throw_bqn
- `crates/rbqn/src/exec.rs` - Add panic_to_bqn_error() helper, use in exec_string/compile_string
- `crates/rbqn/src/main.rs` - Use panic_to_bqn_error in exec_repl_line
- `crates/rbqn-vm/src/modifiers.rs` - Replace throw(e.to_string()) with throw_bqn(e)
- `crates/rbqn-vm/src/scope.rs` - Replace throw(e.to_string()) with throw_bqn(e)

## Decisions Made

**Native repr function (sys_fn 200) avoids infinite recursion:**

The BQN formatter (f.bqn) calls `FN num` where FN is the •Repr argument (4th arg to formatter). `FN = m_sys_fn(35)` would call `format_b_repr` which after bootstrap calls `c1(rt.formatter.1, num)` — the BQN repr function — which internally calls `FN num` again via `ReprAtom`. Stack overflow ensues. Solution: create `native_repr_c1` (sys_fn 200) that formats numbers/chars/arrays natively in Rust without routing through the BQN formatter.

**Typed panic propagation eliminates double-prefix:**

`throw(e.to_string())` converts `BqnError::Assert("msg")` to a String `"Assertion error: msg"`, then `exec.rs` wraps it in another `BqnError::Domain`, adding "Domain error: " prefix. Fix: `throw_bqn(e)` uses `panic_any(BqnError)` to preserve the typed error. `panic_to_bqn_error()` downcasts to `BqnError` directly, returning the original variant without re-wrapping.

## Deviations from Plan

**1. [Rule 1 - Bug] m_sys_fn(35) stack overflow during formatter init**
- **Found during:** Task 1 (Fix •Fmt and •Repr)
- **Issue:** Plan said to pass `m_sys_fn(35)` (•Repr) to formatter, but BQN's f.bqn calls FN on numbers in ReprAtom, which recursively calls back through the BQN repr, creating infinite recursion
- **Fix:** Created `native_repr_c1` (sys_fn 200) — pure Rust repr that handles all types natively without routing through BQN formatter
- **Files modified:** crates/rbqn-vm/src/derive.rs, crates/rbqn/src/bootstrap.rs
- **Verification:** Simple scalars, numeric arrays, and rank-2 arrays all format correctly; no stack overflow
- **Committed in:** b01cb31 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug)
**Impact on plan:** Required change — the plan's suggested fix (m_sys_fn(35)) would have caused a stack overflow. The native repr function is the correct solution.

## Issues Encountered

**Infinite recursion investigation:** The BQN formatter's `ReprAtom` calls `FN num` for numbers (not chars as one might expect, since `num < @` is true in BQN's total ordering). This means passing any function that routes back through the BQN-level repr creates infinite recursion. The fix required understanding BQN's type ordering and creating a completely native repr path.

## Next Phase Readiness

- CBQN parity gaps (formatter, errors, glyph) fixed — ready for Plan 03
- All tests green; no regressions introduced

---
*Phase: 07-code-simplification-cbqn-parity*
*Completed: 2026-03-02*

## Self-Check: PASSED

- Files verified: bootstrap.rs, error.rs, derive.rs, exec.rs — all exist
- Commits verified: b01cb31 (Task 1), f53db76 (Task 2) — both in git history
