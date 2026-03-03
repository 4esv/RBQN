---
phase: "01"
plan: "02d"
subsystem: "rbqn-prim, rbqn-core"
tags: [char-arithmetic, pervasive, bootstrap]
key-files:
  modified:
    - crates/rbqn-prim/src/arith_dyad.rs
    - crates/rbqn-core/src/array.rs
decisions:
  - "Used char_num_class dispatch pattern to cleanly separate char/num/array type combinations"
  - "Added c32_iter/is_char_arr/is_num_arr helpers to BqnArr for reuse by other pervasive ops"
metrics:
  completed: "2026-02-21"
  tasks-completed: 1
  tasks-total: 1
---

# Phase 01 Plan 02d: Character Arithmetic for + and - Summary

Pervasive character arithmetic: char+num, num+char -> char; char-num -> char; char-char -> number. All scalar/array combos handled.

## What Changed

### BqnArr helpers (array.rs)
- `c32_iter()`: Extract code points from C8/C16/C32/Boxed char arrays
- `is_char_arr()`: Type test for character array data
- `is_num_arr()`: Type test for numeric array data

### Dyadic + rewrite (arith_dyad.rs)
- `char_num_class(v, va)` classifies each operand as 'c' (char), 'n' (num), or 'o' (other)
- `add_c2` dispatches: num+num -> standard path, char+num or num+char -> `pervasive_char_add`, char+char -> type error
- `pervasive_char_add` handles all 4 scalar/array combinations, producing C32 result arrays with bounds checking

### Dyadic - rewrite (arith_dyad.rs)
- `sub_c2` dispatches: num-num -> standard path, char-num -> `pervasive_char_sub_num` (-> char), char-char -> `pervasive_char_sub_char` (-> number), num-char -> type error
- Both helpers handle all 4 scalar/array combinations with proper shape checking

## Verification

- Compiler init no longer panics with "Type error: +: Unexpected argument types"
- All char+number scalar operations in the VM trace succeed (0x7ff1 + various f64 tags)
- Compiler init now gets further, hitting an unrelated v_set destructuring mismatch

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | ef74d15 | fix(01-02d): add pervasive character arithmetic to + and - |

## Deviations from Plan

None - plan executed exactly as written.

## Self-Check: PASSED
