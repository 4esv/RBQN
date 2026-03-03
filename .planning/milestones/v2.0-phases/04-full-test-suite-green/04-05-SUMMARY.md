---
phase: 04-full-test-suite-green
plan: 05
status: partial
started: 2026-02-24
completed: 2026-02-24
---

# Plan 04-05: Final Sweep

## Results

### Test Suite Status (post-phase 4)
| Suite | Result | Change |
|-------|--------|--------|
| simple | 100% (72/72) | maintained |
| literal | 100% (50/50) | maintained |
| syntax | 100% (108/108) | maintained |
| bytecode | 100% (35/35) | maintained |
| token | 100% (29/29) | maintained |
| namespace | 100% (50/50) | 26→50 (plan 01) |
| identity | 100% (14/14) | 12→14 (plan 04) |
| unhead | 100% (44/44) | 27→44 (plan 03) |
| fill | 88.7% (55/62) | 34→55 |
| header | 94.2% (147/156) | ~145→147 |
| undo | 76.5% (13/17) | 0→13 (plan 03) |
| under | 84.4% (54/64) | ~55→54 |
| prim | 86.0% (485/564) | 458→485 |

### Total: 1156/1295 (89.3%), up from ~1080/1295 (83.4%)

## What Was Fixed (Plan 05)
- High-rank cell comparison for search monads (⊐, ∊, ⊒)
- prim.bqn: 82→79 failures

## Remaining Failures (110 total)
- **prim.bqn (79)**: Deep pick with boxed indices, high-rank grade/sort, high-rank group, join edge cases, high-rank select, repeat (⍟) complex, scan edge cases
- **under.bqn (10)**: Structural inversion patterns (F⌾G where G involves ⊏, ≍, <, ↑, ⊑)
- **header.bqn (9)**: Stack overflow in modifier headers, predicate edge cases
- **fill.bqn (7)**: Fill through nested operations (windows, transpose of windows, sort fills)
- **undo.bqn (5)**: <⁼, ×˜⁼, <⌜⁼, compound inverse patterns

## System Stubs (Not Started)
SYS-22..26 (•FFI, •bit, •term, •ns, •HashMap) were not implemented. These are not tested by the official suite.

## Commits
- `5886a5a`: fix(04-05): high-rank cell comparison for search monads

## Deviation from Plan
- Plan targeted 0 failures across all 13 files. Achieved 0 in 8/13 files.
- Remaining 110 failures require deep implementation work across Under structural inversion, deep pick, high-rank grade, and other complex features.
- System function stubs deferred — not tested by official suite.
