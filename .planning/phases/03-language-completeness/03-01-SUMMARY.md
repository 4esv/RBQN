---
phase: 03-language-completeness
plan: 01
status: complete
started: 2026-02-24
completed: 2026-02-24
duration: 22min
---

## Summary

Fixed modifier semantics (Undo, Under, Rank, Depth) and primitive edge cases (fill propagation, identity elements, pervasive extension) to improve hard test file pass rates.

## What Was Built

### Task 1: Undo inverse table and Under structural patterns
- Fixed `native_inverse_reg` in derive.rs: corrected +⁼→√, removed ×⁼, fixed ⋆⁼→ln, fixed √⁼
- Expanded `try_structural_under` in modifiers.rs for ⌽, ⍉, k⊸↑, k⊸↓, < patterns

### Task 2: Fill propagation, fold identity, Rank/Depth fixes
- Fill propagation through structural ops (↑, «, », >, ⥊)
- Fold identity elements for empty arrays (+→0, ×→1, ⌊→∞, etc.)
- Rank/Depth modifier audit and fixes

### Task 3: Pervasive extension, depth, repeat, char arithmetic
- Pervasive monad/dyad recurse through nested boxed arrays
- Character arithmetic support
- Depth modifier wiring
- Repeat modifier fixes

## Test Results

| File | Before | After | Delta |
|------|--------|-------|-------|
| prim | 409/564 (73%) | 426/564 (75.5%) | +17 |
| fill | 29/62 (47%) | 36/62 (58.1%) | +7 |
| identity | 8/14 (57%) | 12/14 (85.7%) | +4 |
| under | 51/64 (80%) | 53/64 (82.8%) | +2 |
| undo | 30/68 (44%) | 43/68 (63.2%) | +13 |
| **Overall** | **860/1116 (77%)** | **890/1116 (79.7%)** | **+30** |

## Commits

- `61afcd2` fix(03-01): fix Undo inverse table and structural Under patterns
- `677523c` fix(03-01): fill propagation, fold identity, insert on empty axis
- `fece6f1` fix(03-01): pervasive extension, depth, repeat, char arithmetic

## Deviations

None significant. All three tasks completed as planned.

## Key Files

### key-files.created
- None (modifications only)

### key-files.modified
- crates/rbqn-vm/src/derive.rs
- crates/rbqn-vm/src/modifiers.rs
- crates/rbqn-prim/src/structural.rs
- crates/rbqn-prim/src/arith_monad.rs
- crates/rbqn-prim/src/arith_dyad.rs
