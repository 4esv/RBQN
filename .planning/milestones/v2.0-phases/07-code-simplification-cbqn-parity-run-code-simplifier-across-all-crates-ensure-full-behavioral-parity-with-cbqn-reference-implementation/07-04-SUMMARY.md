---
phase: 07-code-simplification-cbqn-parity
plan: "04"
subsystem: vm
tags: [bqn, derive, glyph, system-functions, primitives, cbqn-parity]

# Dependency graph
requires:
  - phase: 07-code-simplification-cbqn-parity
    provides: "native_prim_idx helper in derive.rs (from 07-03)"
provides:
  - "Fixed dispatch_sys_glyph_c1 returning correct glyph types for SysFn and NativeFn"
  - "sys_fn_name helper mapping sys_idx to •Name strings"
  - "Scalar char (m_c32) return for primitive glyphs matching CBQN semantics"
affects: [formatter, repr, glyph-display]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "sys_fn_name: static dispatch table for sys_idx → •Name string"
    - "B::m_c32 for scalar char returns (CBQN-compatible glyph type)"

key-files:
  created: []
  modified:
    - crates/rbqn-vm/src/derive.rs

key-decisions:
  - "sys_fn_name uses a match table (not array) because sys_idx values are sparse (gaps up to 200)"
  - "SysFn check uses same id extraction pattern as native_prim_idx: (x.0 & 0xFFFFFFFFFFFF) >> 3"
  - "NativeFn glyph returns first char via chars().next() — BQN prim glyphs are single Unicode codepoints"

patterns-established:
  - "sys_fn_name pattern: add new sys_idx entries as new system functions are added"

requirements-completed: [PAR-03]

# Metrics
duration: 5min
completed: 2026-03-02
---

# Phase 7 Plan 04: Glyph Gap Closure Summary

**Fixed •Glyph for system functions and primitives: '-p '•Glyph +'' now outputs '•Glyph+' matching CBQN byte-for-byte via sys_fn_name table and scalar char return**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-03-02T00:00:00Z
- **Completed:** 2026-03-02T00:05:00Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Added `sys_fn_name` helper mapping 30+ sys_idx values to their •Name display strings
- Fixed `dispatch_sys_glyph_c1` to handle `SysFn` values by returning the •Name string (e.g. "•Glyph", "•Fmt", "•Out")
- Fixed `NativeFn` glyph return type from `str_to_b` (char array) to `B::m_c32` (scalar char) matching CBQN type semantics
- All 1316 BQN tests green, 21/21 CBQN compat tests pass, PAR-03 preserved

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix dispatch_sys_glyph_c1 for SysFn and NativeFn values** - `c1d5de2` (fix)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `crates/rbqn-vm/src/derive.rs` - Added sys_fn_name helper and updated dispatch_sys_glyph_c1

## Decisions Made
- `sys_fn_name` uses sparse match table rather than dense array — sys_idx values have large gaps (0..200 non-contiguous), match is cleaner and more maintainable
- SysFn id extraction reuses the same `(x.0 & 0xFFFFFFFFFFFF) >> 3` pattern already in native_prim_idx
- NativeFn glyph takes `chars().next().unwrap_or('+')` — all BQN primitive glyphs are single Unicode codepoints

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Phase 7 gap-closure plan complete: PAR-03 verified, all tests green
- All Phase 7 plans (07-01 through 07-04) complete

## Self-Check: PASSED
- `crates/rbqn-vm/src/derive.rs` — FOUND
- `07-04-SUMMARY.md` — FOUND
- Commit `c1d5de2` — FOUND

---
*Phase: 07-code-simplification-cbqn-parity*
*Completed: 2026-03-02*
