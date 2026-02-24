---
phase: 03-language-completeness
plan: 04
subsystem: vm
tags: [bqn, inverse, undo, under, modifiers, derive]

# Dependency graph
requires:
  - phase: 03-language-completeness
    provides: "modifier infrastructure, try_structural_under, native_inverse_reg table"
provides:
  - "sys 203 c2 dispatch: w√⁼x = x^w (dyadic sqrt-inverse)"
  - "Dyadic +⁼ char arithmetic: 3+⁼'d' = 'a' via runtime fallthrough"
  - "Functional-k Under: F⌾((2÷˜≠)⊸↑) evaluates k on x before take/drop"
affects: [language-completeness, undo-tests, under-tests]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "native_inverse_reg returns None to let BQN runtime handle complex inverse cases"
    - "Functional-k detection: left_op.is_fun() check before c1(left_op, x) evaluation"

key-files:
  created: []
  modified:
    - crates/rbqn-vm/src/derive.rs
    - crates/rbqn-vm/src/modifiers.rs
    - crates/rbqn-prim/src/structural.rs
    - crates/rbqn-prim/src/select.rs

key-decisions:
  - "Remove native +⁼ shortcut from native_inverse_reg: returning + breaks dyadic char arithmetic (3+⁼'d'='g' not 'a'); fall through to BQN runtime which returns -˜ as dyadic inverse"
  - "Functional-k Under: check left_op.is_fun() before using as take/drop count; evaluate c1(left_op,x) to get numeric k when callable"
  - "sys 203 c2 dispatch uses pow_c2(x, xa, w, wa) with args swapped: w√⁼x = x^w not w^x"

patterns-established:
  - "Inverse dispatch: native shortcut table bypasses runtime only when shortcut is correct for ALL arities (monadic and dyadic)"
  - "Structural Under: detect callable left_op before passing to take/drop"

requirements-completed: [MOD-01, MOD-02]

# Metrics
duration: 26min
completed: 2026-02-24
---

# Phase 03 Plan 04: Modifier Gap Closure Summary

**Dyadic sqrt-inverse (sys 203 c2), dyadic +⁼ char arithmetic via runtime fallthrough, and functional-k Under evaluation for shape-derived take/drop patterns**

## Performance

- **Duration:** ~26 min
- **Started:** 2026-02-24T15:00:00Z
- **Completed:** 2026-02-24T15:26:27Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- sys 203 c2 dispatch: `2√⁼8 = 64` (`x^w` using pow_c2 with args swapped)
- Dyadic +⁼ char arithmetic: `3+⁼"d" = "a"` by removing native shortcut, letting BQN runtime resolve inverse of `+` as `-˜`
- Functional-k Under: `⌽⌾((2÷˜≠)⊸↑) "abcdef" = "cbadef"` — k is evaluated on x before take
- Fixed pre-existing structural.rs compile errors (rotate_along_axis rank type cast)

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix Undo gaps — sys 203 c2 dispatch and dyadic +⁼ char fallthrough** - `4965217` (fix)
2. **Task 2: Expand structural Under for functional-k** - `dd0f906` (fix)

Pre-existing work also committed:
- `7148dc1` fix(prims): group/search/slash/sort correctness improvements from prior session

## Files Created/Modified
- `crates/rbqn-vm/src/derive.rs` - Add case 203 to dispatch_sys_c2; remove native +⁼ shortcut; fix LazyInvReg c2 fallthrough
- `crates/rbqn-vm/src/modifiers.rs` - Functional-k detection in try_structural_under
- `crates/rbqn-prim/src/structural.rs` - Fix rank type cast (u8→usize) in rotate_along_axis
- `crates/rbqn-prim/src/select.rs` - first_cell_c1 rank-1 handling (pre-existing)

## Decisions Made
- Removed `native_inverse_reg(0)` shortcut that returned `+` as inverse of `+`. This was correct for monadic `+⁼x = x` (identity) but broke dyadic `w+⁼x` for char arithmetic. The BQN runtime handles both arities correctly — monadic still returns `+⁼9 = 9` because the runtime knows `+⁻¹ = +` for real numbers.
- sys 203 c2 uses `pow_c2(x, xa, w, wa)` — arguments are swapped because `w√⁼x = x^w` (raise x to power w), not `w^x`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fix rotate_along_axis rank type mismatch**
- **Found during:** Task 1 (build phase)
- **Issue:** `rotate_along_axis` used `arr.rank()` (returns `u8`) as `Vec` index and loop bound — type mismatch caused compile errors
- **Fix:** Cast `arr.rank() as usize` at top of function; fix comparison `rotations.len() != arr.rank() as usize`
- **Files modified:** crates/rbqn-prim/src/structural.rs
- **Verification:** `cargo build` passes with no errors
- **Committed in:** `4965217` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking compile error)
**Impact on plan:** Necessary to unblock compilation. Pre-existing work from a prior session that hadn't been committed.

## Issues Encountered
- The BQN runtime's `⁼` block goes through BQN bytecode, not through our `LazyInvReg` Rust dispatch. This meant the `+⁼` fix had to be at the `native_inverse_reg` level (removing the shortcut) rather than in `LazyInvReg c2`. Traced via `RBQN_PRIM_TRACE=1`.
- The plan's expected value "dcbaef" for `⌽⌾((2÷˜≠)⊸↑)"abcdef"` was incorrect — both CBQN and our implementation return "cbadef" (reverse first 3 chars, not 4).

## Next Phase Readiness
- MOD-01 and MOD-02 gaps closed
- sys 203 dyadic dispatch complete (both c1 and c2)
- Structural Under handles literal-k and functional-k take/drop patterns
- BQN runtime's Under handles ⊏⎉1 and other complex patterns via fallthrough

## Self-Check: PASSED

- FOUND: 03-04-SUMMARY.md
- FOUND: crates/rbqn-vm/src/derive.rs
- FOUND: crates/rbqn-vm/src/modifiers.rs
- FOUND commit 4965217 (Task 1)
- FOUND commit dd0f906 (Task 2)
- FOUND commit 7148dc1 (pre-existing work)

---
*Phase: 03-language-completeness*
*Completed: 2026-02-24*
