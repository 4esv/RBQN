---
phase: 04-full-test-suite-green
plan: 03
subsystem: testing
tags: [bqn, inverse, transpose, scan, undo]

requires:
  - phase: 04-02
    provides: prim test improvements
  - phase: 04-01
    provides: bootstrap and runtime infrastructure

provides:
  - Native ⍉⁼ (inverse transpose) for all ranks including rank>2
  - Native +`⁼ (scan inverse) via ScanInv DerivedKind
  - Fixed +` scan for rank>1 arrays (major-cell semantics)
  - Dyadic ⋆⁼ (log_w(x)) via sys_fn 202 c2
  - Dyadic +˜⁼ (x-w) via sys_fn 204 c2
  - Fixed /⁼ for unsorted index arrays
  - Partial permutation support in dyadic ⍉ (reorder_c2)
  - undo.bqn reduced from 18 to 5 failures

affects: [future test phases, undo modifier, scan modifier, transpose]

tech-stack:
  added: []
  patterns:
    - ScanInv DerivedKind intercepts inv_reg(F`) before BQN runtime
    - sys_fn dispatch pattern for internal inverse functions (200-205 range)
    - transpose_with_perm shared helper for ⍉ and ⍉⁼

key-files:
  created: []
  modified:
    - crates/rbqn-prim/src/structural.rs
    - crates/rbqn-prim/src/slash.rs
    - crates/rbqn-vm/src/derive.rs
    - crates/rbqn-vm/src/modifiers.rs

key-decisions:
  - "ScanInv DerivedKind: intercept inv_reg(F`) natively to avoid BQN runtime issues with rank>1 arrays"
  - "⍉⁼ permutation: 1‿2‿...‿(r-1)‿0 (axis i → pos i+1 mod r), moving last axis to first"
  - "⍉ monadic rank>2 fix: use (r-1)‿0‿1‿...‿(r-2) permutation (move first axis to last)"
  - "Partial permutation extension: fill remaining target positions with unused source axes in order"
  - "Remove ⊣⁼ from native table: dyadic w⊣⁼x has no inverse per BQN spec"
  - "Remove √˜⁼ native shortcut: let BQN runtime handle complex inversion"

requirements-completed: [TEST-08, TEST-13]

duration: 90min
completed: 2026-02-24
---

# Phase 04 Plan 03: Native Inverses Summary

**BQN inverse operators implemented natively — ⍉⁼, +`⁼, ⋆⁼ dyadic, +˜⁼ dyadic — reducing undo.bqn from 18 to 5 failures**

## Performance

- **Duration:** ~90 min
- **Completed:** 2026-02-24
- **Tasks:** 1 (Task 2 only — Task 1 was pre-completed)
- **Files modified:** 4

## Accomplishments
- Implemented `⍉⁼` (inverse transpose) for rank>2 using correct permutation, plus dyadic `w⍉⁼x` with full/partial permutation inversion
- Fixed `⍉` monadic for rank>2 (was implementing full reverse, now moves first axis to last per BQN spec)
- Fixed `+`` (scan) for rank>1 arrays to operate on major cells
- Implemented native `F`⁼` (scan inverse) via new `ScanInv` DerivedKind
- Added dyadic `⋆⁼` (log_w(x)) and `+˜⁼` (x-w) missing c2 implementations
- Fixed `/⁼` to use actual max value (not just last element) for unsorted arrays
- Added partial permutation support in `reorder_c2` (CBQN extension rule)
- undo.bqn: 18 → 5 failures (-13), target was ≤5 ✓

## Task Commits

1. **Task 2: Native inverses** - `99d74ed` (feat)

## Files Created/Modified
- `crates/rbqn-prim/src/structural.rs` - Added transpose_inv_c1, transpose_with_perm, fixed transpose_c1 rank>2, added partial permutation extension in reorder_c2
- `crates/rbqn-prim/src/slash.rs` - Fixed indices_inverse_c1 to use actual max (not last element)
- `crates/rbqn-vm/src/derive.rs` - Added ScanInv DerivedKind, m_scan_inv constructor, c1/c2 dispatch, sys_fn 202/204/205 c2 implementations, fixed ⊣⁼/√˜⁼ handling
- `crates/rbqn-vm/src/modifiers.rs` - Fixed scan_c1/c2 for rank>1, added scan_inv_c1/scan_inv_c2

## Decisions Made
- ScanInv is a new DerivedKind rather than a sys_fn: it needs to store the operand function F and call inv_reg(F) at execution time
- `⍉` rank>2 was using full axis-reverse (wrong); fixed to use CBQN's "move first axis to last" semantics
- Partial permutation extension for `reorder_c2`: CBQN allows `p` with length < rank(x); missing target positions filled with remaining source axes in order
- Removed `⊣⁼` and `√˜⁼` from native table to let BQN runtime handle correctly

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed scan for rank>1 arrays**
- **Found during:** Task 2 (scan inverse investigation)
- **Issue:** `scan_c1`/`scan_c2` iterating over all elements instead of major cells for rank>1 arrays
- **Fix:** Rewrote to detect rank and use cell-based iteration for rank>1
- **Files modified:** crates/rbqn-vm/src/modifiers.rs
- **Verification:** `+` 3‿4⥊↕12` matches CBQN output
- **Committed in:** 99d74ed

**2. [Rule 1 - Bug] Fixed ⍉ monadic for rank>2**
- **Found during:** Task 2 (⍉⁼ rank>2 investigation)
- **Issue:** `transpose_c1` used full axis-reverse for rank>2, but BQN spec requires moving first axis to last
- **Fix:** Added rank>2 case using `transpose_with_perm` with correct permutation
- **Files modified:** crates/rbqn-prim/src/structural.rs
- **Verification:** `≢⍉ 2‿3‿4⥊↕24 = ⟨3,4,2⟩` matches CBQN
- **Committed in:** 99d74ed

**3. [Rule 2 - Missing Critical] Added partial permutation support in reorder_c2**
- **Found during:** Task 2 (⍉⁼ dyadic test)
- **Issue:** `reorder_c2` rejected permutations shorter than rank; CBQN allows them
- **Fix:** Extended partial permutations with uncovered target positions
- **Files modified:** crates/rbqn-prim/src/structural.rs
- **Verification:** `2‿1⍉2‿3‿1‿4⥊↕24` works, prim tests improved 94→93
- **Committed in:** 99d74ed

**4. [Rule 2 - Missing Critical] Added dyadic ⋆⁼ (sys_fn 202 c2)**
- **Found during:** Task 2 (⋆⁼⌜ test with ∞)
- **Issue:** sys_fn 202 had no c2 dispatch, failing on `w⋆⁼x = log_w(x)`
- **Fix:** Implemented dispatch_sys_c2 case 202 computing ln(x)/ln(w) with array support
- **Files modified:** crates/rbqn-vm/src/derive.rs
- **Committed in:** 99d74ed

**5. [Rule 2 - Missing Critical] Added dyadic +˜⁼ (sys_fn 204 c2)**
- **Found during:** Task 2 (swap inverse test for +‿×‿∧)
- **Issue:** sys_fn 204 had no c2 dispatch for `w(+˜)⁼x = x-w`
- **Fix:** Implemented dispatch_sys_c2 case 204 using sub_c2(x, xa, w, wa)
- **Files modified:** crates/rbqn-vm/src/derive.rs
- **Committed in:** 99d74ed

---

**Total deviations:** 5 auto-fixed (3 Rule 1 bugs, 2 Rule 2 missing)
**Impact on plan:** All necessary for correctness. Fixed pre-existing bugs in ⍉ and scan.

## Issues Encountered
- `√˜⁼` dyadic (16√˜⁼2=4): BQN runtime still returns 65536; root cause is that `inv_reg(√˜)` returns `⋆˜` which computes `2^16`. Complex to fix without deeper runtime analysis — deferred.
- `∧˜⁼` swap inverse: BQN runtime handles ∧ specially; still 1 test failing.

## Remaining Failures (5/68)
1. `(<⁼<)¨⊸≡` — unbox inverse on boxed arrays (complex box/unbox chain)
2. `{6(𝕎˜⁼≡𝕎⁼)𝕩}⟜¯0.8‿0‿3¨ +‿×‿∧` — ∧˜⁼ doesn't match ∧⁼ (BQN runtime returns 0 instead of 1)
3. `+´∊⟨√,×˜⁼,∧˜⁼⟩{𝕎𝕩}⌜0‿2‿∞` — membership test returns 3 not 1 (∊ identity issue)
4. `16√˜⁼2` — √˜⁼ dyadic returns 65536 not 4
5. `(∾˜ ≡ ·(<⌜⁼∾<¨⁼)<¨)` — table/each inverse on chars

## Next Phase Readiness
- undo.bqn at 5 failures (≤5 target met)
- unhead.bqn at 0 failures
- No regressions in simple/syntax/token/literal/bytecode/namespace
- Ready for phase 04-04

---
*Phase: 04-full-test-suite-green*
*Completed: 2026-02-24*
