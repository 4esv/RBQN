---
phase: 01-fix-bootstrap-pipeline
plan: 04
subsystem: compiler
tags: [bqn-compiler, tokenizer, string-literals, sort, nan-boxing, bytecode-parsing]

requires:
  - phase: 01-fix-bootstrap-pipeline (01-02)
    provides: System functions (Out, BQN, PrimInd) registered in VM
provides:
  - Working string literal compilation through BQN compiler pipeline
  - Working character literal compilation (single-quote)
  - Correct higher-rank sort (monadic sort up/down on 2D+ arrays)
  - Dyadic arithmetic and fold through compiler pipeline
affects: [01-05-gap-closure, phase-02-conformance]

tech-stack:
  added: []
  patterns:
    - "strip_prefix/strip_suffix over trim_*_matches for exact delimiter removal"
    - "apply_row_permutation helper for higher-rank array operations"

key-files:
  created: []
  modified:
    - crates/rbqn/build.rs
    - crates/rbqn-prim/src/sort.rs
    - crates/rbqn-core/src/array.rs
    - crates/rbqn-prim/src/group.rs
    - crates/rbqn-prim/src/search.rs
    - crates/rbqn-vm/src/derive.rs
    - crates/rbqn-vm/src/modifiers.rs
    - crates/rbqn-vm/src/vm.rs
    - crates/rbqn/src/bootstrap.rs
    - crates/rbqn/src/cli.rs
    - crates/rbqn/src/main.rs
    - crates/rbqn/src/repl.rs

key-decisions:
  - "Root cause of string bug was in build.rs C literal parser, not in VM execution"
  - "sort_up/sort_down need full row extraction for rank>1, not single-element get"

patterns-established:
  - "Use strip_prefix/strip_suffix for C literal parsing to avoid greedy quote stripping"

requirements-completed: [PIPE-01, PIPE-02, PIPE-03, PIPE-04, PIPE-05, PIPE-06, PIPE-08, SYS-01, SYS-02]

duration: 18min
completed: 2026-02-23
---

# Phase 01 Plan 04: Compiler Pipeline Gap Closure Summary

**Fixed string/char literal compilation (build.rs C escape parser) and higher-rank sort, enabling "hello", Out, BQN with string args**

## Performance

- **Duration:** 18 min
- **Started:** 2026-02-23T23:49:13Z
- **Completed:** 2026-02-24T00:07:00Z
- **Tasks:** 2 (across 2 commits by 2 agents)
- **Files modified:** 2 (this session)

## Accomplishments
- String literals now compile and execute correctly (`"hello"` outputs `"hello"`)
- Character literals work (`'a'` outputs `'a'`)
- `•Out "hello"` prints to stdout
- `•BQN "1+1"` returns 2 (self-hosted evaluation works)
- Dyadic arithmetic through compiler: `3x4` = 12, `10-3` = 7
- Fold modifier through compiler: `+´1‿2‿3` = 6, `x´2‿3‿4` = 24
- Higher-rank sort (monadic) correctly permutes full rows, not single elements

## Task Commits

Each task was committed atomically:

1. **Task 2: Fix dyadic arithmetic and modifier resolution** - `e3aa1a3` (fix) - by previous agent
2. **Task 1: Fix string literal compilation** - `830025c` (fix) - this session

## Files Created/Modified
- `crates/rbqn/build.rs` - Fixed parse_c32_char: added `\"` escape case, switched from trim_end_matches to strip_suffix for exact single-quote removal
- `crates/rbqn-prim/src/sort.rs` - Added apply_row_permutation helper; fixed sort_up_c1 and sort_down_c1 to extract full rows for higher-rank arrays

## Decisions Made
- Root cause was in build.rs, not VM: the C literal parser `parse_c32_char` was missing the `\"` escape sequence, causing both `'` (codepoint 39) and `"` (codepoint 34) to be loaded as null (codepoint 0). This meant the BQN compiler's tokenizer couldn't detect string/char delimiters.
- The `trim_end_matches('\'')` call greedily strips ALL trailing quotes, corrupting `U'\''` (single-quote literal). Replaced with `strip_suffix('\'')` which removes exactly one.
- Higher-rank sort bug was a secondary issue discovered during debugging: sort_up_c1 called `arr.get(row_idx)` which gets a flat element, not a full row. Fixed with apply_row_permutation that copies cell_size elements per row.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed higher-rank sort (monadic sort up/down)**
- **Found during:** Task 1 (debugging string literal crash)
- **Issue:** sort_up_c1 and sort_down_c1 called arr.get(idx) which gets flat elements, but grade returns row indices. For a 2x2 array, this created a 2-element result with shape [2,2], causing "index out of bounds" panics when the malformed array was later accessed.
- **Fix:** Added apply_row_permutation helper that extracts cell_size elements per row index. Updated both sort_up_c1 and sort_down_c1 to use it.
- **Files modified:** crates/rbqn-prim/src/sort.rs
- **Verification:** `sort([2x2] I8 [6,6,1,5])` now returns `[2x2] I8 [1,5,6,6]` correctly
- **Committed in:** 830025c (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** The sort bug was blocking the string compilation path — the BQN compiler's tokenizer uses monadic sort on 2D arrays during character classification. Fixing it was necessary to reach the actual root cause.

## Issues Encountered
- The "Double subjects (missing ‿?)" error had two layers: (1) a sort crash on 2D arrays was caught by the BQN runtime's error handler, masking the real error, and (2) the actual root cause was in build.rs C literal parsing, not in any VM opcode. Deep trace analysis through 71K+ primitive calls was needed to isolate that both quote characters were being loaded as null bytes.
- `•PrimInd "+"` returns 64 (not found) because our native PrimInd expects a function value, not a string. The BQN runtime's setPrims wrapper would handle string-to-function conversion, but we bypass runtime wrappers (using native fruntime). This is a pre-existing limitation, not a regression.
- Variable assignment (`a←5 ⋄ a`) fails with "Undefined identifier" — also pre-existing, not caused by our changes.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- String literals, character literals, and basic arithmetic all work through the full compiler pipeline
- System functions •Out, •BQN work with string arguments
- Ready for plan 01-05 (remaining gap closure items)
- Known limitations: variable assignment, •PrimInd with string args

---
*Phase: 01-fix-bootstrap-pipeline*
*Completed: 2026-02-23*
