---
phase: 03-language-completeness
plan: 02
subsystem: vm
tags: [bqn, system-functions, file-io, import, utf8, rust]

requires:
  - phase: 03-01
    provides: modifier semantics and primitive edge case fixes

provides:
  - File I/O system functions (•FChars, •FBytes, •FLines) with read and write modes
  - Expanded •file namespace (Lines, List, Chars, Bytes, At, Name, Parent, Exists, Type, CreateDir, Rename, Remove, Open)
  - •Import with result caching and circular import detection
  - Utility functions: •ParseFloat (BQN number format), •Hash, •FromUTF8, •ToUTF8, •Cmp
  - •CurrentError stub registered

affects: [03-03, phase-4-gpu]

tech-stack:
  added: []
  patterns:
    - "sys_idx range 50-80 reserved for file I/O and utility system functions"
    - "resolve_path() centralizes •path-relative path resolution for all file ops"
    - "IMPORT_CACHE uses B::SENTINEL as circular-import sentinel value"
    - "Namespace fields registered with str2gid in order matching scope init array"

key-files:
  created: []
  modified:
    - crates/rbqn-vm/src/derive.rs

key-decisions:
  - "Dyadic write variants (•FChars/•FBytes) share sys_idx with monadic reads (52/53) — dispatch distinguishes by arity"
  - "•file.Chars (63) and •file.Bytes (64) are separate sys_idx from •FChars/•FBytes (52/53) to allow independent namespace wiring"
  - "•ImportCache uses canonical path as key (via fs::canonicalize) to prevent duplicate loading of symlinked files"
  - "•ParseFloat normalizes ¯ to - before parsing; handles ∞ and π as special cases inline"
  - "•Cmp uses type-priority ordering (number < char < array < fun < md1 < md2 < ns) matching BQN total order"

requirements-completed: [SYS-06, SYS-07, SYS-08, SYS-09, SYS-10, SYS-11, SYS-12, SYS-13, SYS-14, SYS-15]

duration: 8min
completed: 2026-02-24
---

# Phase 03 Plan 02: System Functions — File I/O, Import, Utilities Summary

**13-field •file namespace, •FChars/•FBytes read+write, •Import with caching, and •ParseFloat/•ToUTF8/•FromUTF8/•Cmp utility functions added to dispatch_sys_c1/c2**

## Performance

- **Duration:** 8 min
- **Started:** 2026-02-24T12:47:42Z
- **Completed:** 2026-02-24T12:55:11Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- File I/O: •FChars reads files as char arrays, •FBytes as byte arrays, both support dyadic write mode
- Expanded •file namespace from 2 fields (Lines, List) to 13 fields including At, Name, Parent, Exists, Type, CreateDir, Rename, Remove
- •Import loads BQN files, caches results by canonical path, detects circular imports via SENTINEL
- Utility functions: •ParseFloat handles BQN number format (¯, ∞, π), •ToUTF8/•FromUTF8 for UTF-8 conversion, •Cmp for total ordering
- All 8 harness tests continue to pass (no regressions)

## Task Commits

1. **Task 1: File I/O system functions and file namespace expansion** - `f2821bb` (feat)
2. **Task 2: Import caching, introspection, and utility functions** - `866bf5b` (feat)

**Plan metadata:** pending final commit

## Files Created/Modified
- `crates/rbqn-vm/src/derive.rs` - All system function implementations, IMPORT_CACHE global, expanded make_file_namespace()

## Decisions Made
- Dyadic write variants share the same sys_idx as monadic reads (52/53) — c1 reads, c2 writes
- FILE_NS lazy cache invalidated by rebuilding would cause races; kept it cached once (namespace is stateless)
- •CurrentError returns SENTINEL for now — full implementation requires catch modifier error capture integration

## Deviations from Plan

None - plan executed exactly as written. The sys_idx ranges for file ops (50-65) and utilities (70-80) matched the plan specification.

## Issues Encountered
- `Body::new` and `Scope::new` take `u16` for `var_am`, not `i32` — fixed by using explicit `u16` literals for the expanded 13-field namespace

## Next Phase Readiness
- SYS-06 through SYS-15 all implemented, enabling test files that use •FChars, •Import, •ParseFloat
- •CurrentError requires catch modifier context integration — deferred to phase 4 or later
- •file.Open returns NYI error — acceptable for current test coverage

---
*Phase: 03-language-completeness*
*Completed: 2026-02-24*
