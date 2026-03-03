---
phase: 04-full-test-suite-green
plan: 01
subsystem: compiler
tags: [bqn, namespace, compiler, vm, bytecode]

# Dependency graph
requires:
  - phase: 03-language-completeness
    provides: working VM with RETD opcode and NS store infrastructure
provides:
  - NSDesc population from body_arr[2]/[3] in compile_block
  - Namespace destructuring ⟨a⟩←ns via all_var_gids slot mapping
  - Empty namespace {⇐} creates valid NSDesc
  - ALIAS_TAG NaN-boxing for explicit field rename ⟨b⇐a⟩←ns
affects: [04-02, 04-03, 04-05]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "NSDesc built from body_arr[2]/[3] in compile_block body loop"
    - "all_var_gids Vec<i32> in Body for slot→GID mapping at runtime"
    - "ALIAS_TAG (0x7FF5) NaN-boxing encodes gid+depth+pos in 48 bits"
    - "v_set list branch handles is_nsp() source separately from array source"

key-files:
  created: []
  modified:
    - crates/rbqn-core/src/value.rs
    - crates/rbqn-core/src/lib.rs
    - crates/rbqn-vm/src/compiler.rs
    - crates/rbqn-vm/src/block.rs
    - crates/rbqn-vm/src/scope.rs
    - crates/rbqn-vm/src/vm.rs

key-decisions:
  - "ALIM bytecode creates ALIAS_TAG B value (NaN-boxed gid+depth+pos) not a heap object"
  - "⟨a⟩←ns uses all_var_gids slot lookup (not ALIM) because CBQN omits ALIM for same-name extraction"
  - "build_ns_desc creates NSDesc when export_mask array present, even if empty (enables {⇐} → empty namespace)"
  - "Namespace destructuring only triggered in ARRAY branch of v_set, not plain VAR branch (prevents ns←ns_val from triggering extraction)"

patterns-established:
  - "NSDesc offset: off = (ty==0?0:ty==1?2:3) + (imm?0:3) matches CBQN ns.c"
  - "body.all_var_gids[slot] = GID for that variable, populated from body_arr[2] + nameList"

requirements-completed: [TEST-09, TEST-07]

# Metrics
duration: 45min
completed: 2026-02-24
---

# Phase 4 Plan 1: Namespace Export Fix Summary

**Namespace export and destructuring fully working: build_ns_desc reads body_arr[2]/[3], ⟨a⟩←ns uses per-slot GID lookup, empty {⇐} creates valid namespace**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-02-24T18:40:00Z
- **Completed:** 2026-02-24T19:29:20Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- namespace.bqn: 50/50 (all passed, up from 26/50)
- header.bqn: failures reduced from ~11+overflow to 7+overflow (4 fewer)
- prim.bqn: improved from 458/564 to ~470/564 (namespace fix collateral benefit)
- No regressions in simple, literal, syntax, bytecode, token (294/294)
- `{a⇐5}.a` returns 5, `{a⇐5, b⇐10}.b` returns 10 as verified

## Task Commits

1. **Task 1: Build NSDesc from body_arr[2]/[3] in compile_block** - `78172e7` (feat)
2. **Task 2: Fix header.bqn namespace-dependent tests** - no separate commit (namespace fix in Task 1 handled all namespace-related header failures; remaining failures are non-namespace bugs deferred to 04-05)

## Files Created/Modified
- `crates/rbqn-core/src/value.rs` - Added ALIAS_TAG (0x7FF5) and is_alias/alias_gid/alias_depth/alias_pos methods
- `crates/rbqn-core/src/lib.rs` - Exported ALIAS_TAG
- `crates/rbqn-vm/src/compiler.rs` - Added build_ns_desc, build_all_var_gids helpers; integrated into compile_block body loop
- `crates/rbqn-vm/src/block.rs` - Added all_var_gids field to Body struct
- `crates/rbqn-vm/src/scope.rs` - v_set: namespace destructuring in ARRAY branch using all_var_gids; ALIAS branch for ⟨b⇐a⟩←ns
- `crates/rbqn-vm/src/vm.rs` - ALIM handler creates ALIAS_TAG B value instead of passing VAR ref unchanged

## Decisions Made

- **CBQN omits ALIM for same-name extraction**: Discovered that `⟨a⟩←ns` does NOT emit ALIM in CBQN bytecode. ALIM is only for renamed fields like `⟨b⇐a⟩←ns`. The variable name lookup uses `body.all_var_gids[slot]` at runtime.
- **ALIAS_TAG NaN-boxing**: Since CBQN uses heap-allocated FldAlias structs, we use ALIAS_TAG (0x7FF5) with payload encoding `gid(16)|depth(16)|pos(16)` in lower 48 bits.
- **Empty namespace**: When `body_arr` has 4 elements and export_mask is an empty array, still create NSDesc (previously returned None which prevented `{⇐}` from returning a namespace).
- **Namespace destructuring only in list context**: Moving the `x.is_nsp()` check from the VAR branch to the ARRAY branch prevents `ns←namespace_val` from incorrectly triggering field extraction.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] CBQN emits no ALIM for ⟨a⟩←ns — different mechanism required**
- **Found during:** Task 1 (testing after initial ALIM-based fix)
- **Issue:** The plan assumed ALIM would be emitted for `⟨a⟩←ns`. Inspection of actual CBQN bytecode showed ALIM is only used for renamed fields `⟨b⇐a⟩←ns`. For same-name extraction, the compiler relies on runtime lookup of variable names via body_arr[2].
- **Fix:** Added `all_var_gids: Vec<i32>` to Body struct, populated in compile_block from body_arr[2]. v_set uses `pscs[d].body.all_var_gids[p]` for field GID lookup in list context.
- **Files modified:** compiler.rs, block.rs, scope.rs
- **Committed in:** 78172e7

**2. [Rule 1 - Bug] Namespace destructuring triggered on plain assignment ns←namespace_val**
- **Found during:** Task 1 (debugging wrong field extraction error)
- **Issue:** Initial implementation put namespace-extraction check in the VAR branch of v_set. When assigning a namespace to a plain variable (`ns←namespace_val`), it incorrectly tried to extract a field by variable name.
- **Fix:** Moved namespace-extraction logic from VAR branch to ARRAY branch. Plain VAR assignment of namespace values now works correctly.
- **Files modified:** scope.rs
- **Committed in:** 78172e7

---

**Total deviations:** 2 auto-fixed (both Rule 1 bugs discovered during implementation)
**Impact on plan:** Both fixes were essential for correctness. Discovered through test execution rather than upfront.

## Issues Encountered

- Stack overflow in `2{⟨a,b⟩_r:a+b;a _r: a‿a _r}` — recursive modifier self-call with array operand. Even with 128MB stack, infinite recursion. Root cause: SETH dispatch for multi-header modifier blocks re-enters with original operand instead of updated operand. Deferred to 04-05.

## Remaining header.bqn Failures (Deferred to 04-05)

See `deferred-items.md` for details:
1. Predicate `?` in immediate block should fail
2. Header with literal number matching `{2:4;𝕩}`
3. Destructuring in function header `{1‿b:b;𝕊:𝕩}`
4. Strand type matching in header
5. Multi-header dyadic block with 𝕨=𝕊
6. Modifier header with operand args `{4 3 _𝕣_ 2 1:...}`
7. Recursive modifier stack overflow

## Next Phase Readiness
- namespace.bqn fully green (50/50)
- header.bqn at ~149/156 (7 failures + overflow from ~145/156)
- Ready for 04-02: Block header inverse body dispatch (unhead.bqn)

---
*Phase: 04-full-test-suite-green*
*Completed: 2026-02-24*
