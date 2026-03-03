---
phase: 06-self-hosting
plan: 03
subsystem: infra
tags: [self-hosting, bqn-compiler, bytecode, serialization, rbqn-gen, wire-format]

requires:
  - phase: 06-self-hosting
    plan: 02
    provides: "rbqn-gen --verify mode, compile_string() + compiler_output_to_owned() + encode_owned_bytecode() stubs"

provides:
  - "rbqn-gen --verify produces real self-compiled .bin files (not 49-byte placeholders)"
  - "Self-compiled compiler.bin and formatter.bin pass SELF-02 behavioral equivalence (13/13 tests)"
  - "compile_string() + compiler_output_to_owned() + encode_owned_bytecode() fully wired into --verify"

affects: [self-hosting, SELF-02, bytecode-serialization]

tech-stack:
  added: []
  patterns:
    - "iarrs[0] = empty array convention: must match CBQN wire format (NOT bc data)"
    - "c.bqn wrapping: use {func‿mod1‿mod2←𝕩\\nbody} (𝕩 arg forces imm=0 non-immediate block)"
    - "compiler_output_to_owned: separate bc from iarrs, start iarrs with empty [] placeholder"

key-files:
  created: []
  modified:
    - crates/rbqn/src/exec.rs
    - crates/rbqn/src/bin/rbqn_gen.rs

key-decisions:
  - "iarrs[0] must be empty [] not bc: CBQN wire format uses iarrs[0]=[] as placeholder for no-bodies blocks"
  - "c.bqn wrapping uses 𝕩 not embedded glyphs: {func‿mod1‿mod2←𝕩} forces imm=0 so bootstrap can c1(compgen,glyphs)"
  - "body serialization: only store [bcOffset, varCount] in iarrs (not 4-element mixed array from compiler)"
  - "Self-compiled bytes intentionally smaller than CBQN: RBQN uses Provide+Runtime refs vs CBQN's Runtime-only"

patterns-established:
  - "Self-compilation gap closure pattern: use 𝕩 in wrapper to control immediateness of compiled block"
  - "Wire format validation: compare iarrs[0] length (must be 0) and block IArr vs Info types"

requirements-completed: [SELF-01, SELF-02, SELF-03, SELF-04, SELF-05]

duration: 45min
completed: 2026-03-01
---

# Phase 06 Plan 03: Self-Hosted Bytecode Serialization Summary

**Real RBQN-compiled .bin files (67953+14876 bytes) replace 49-byte placeholders and pass all 13 test files — SELF-02 behavioral equivalence confirmed**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-03-01T11:00:00Z
- **Completed:** 2026-03-01T11:45:00Z
- **Tasks:** 2 (Task 1 pre-committed at 55ac2e4, Task 2 at 1f4b4c5)
- **Files modified:** 2

## Accomplishments

- Self-compiled compiler.bin (67953 bytes) and formatter.bin (14876 bytes) produced by RBQN's own compiler
- All 13/13 BQN test files pass with self-compiled .bin files swapped in (SELF-02)
- All 13/13 BQN test files pass with CBQN-generated .bin files restored (no regression)
- cargo build -p rbqn succeeds without CBQN_PATH (SELF-04/05 not regressed)
- Identified and fixed two critical bugs in the serialization approach

## Task Commits

1. **Task 1: Add compile_string(), compiler_output_to_owned(), encode_owned_bytecode()** - `55ac2e4` (feat)
2. **Task 2: Wire real bytecode serialization into rbqn-gen --verify** - `1f4b4c5` (feat)

## Files Created/Modified

- `crates/rbqn/src/exec.rs` - Fixed compiler_output_to_owned: iarrs[0]=[], body extraction correct
- `crates/rbqn/src/bin/rbqn_gen.rs` - Fixed c.bqn wrapping (𝕩 arg), wired compile_string path

## Decisions Made

- **iarrs[0] = empty array**: The CBQN wire format uses iarrs[0] as a shared empty array placeholder for blocks with no monadic body list. My initial implementation stored `output.bc` at iarrs[0], which caused all block iarrs indices to be off-by-1 and bloated the iarrs section with the full 14K-element bc.

- **c.bqn wrapping uses 𝕩**: Initial wrapping embedded concrete glyph literals (`{func‿mod1‿mod2 ← ⟨"+-...", ...⟩`), causing BQN to mark the block as immediate (imm=1) since the body doesn't reference `𝕩`. Bootstrap calls `c1(compgen, glyphs_b)` which requires the block to be non-immediate (imm=0). Fixed by using `{func‿mod1‿mod2←𝕩\nbody}` which forces imm=0.

- **Body extraction: [bcOffset, varCount] only**: The BQN compiler returns bodies as 4-element mixed arrays `[bcOffset, varCount, namesArr, depthsArr]`. compile_all only reads elements [0] and [1], so we serialize only those two as a 2-element i32 array.

- **Self-compiled size ~half of CBQN**: Expected behavior — RBQN uses Provide(n)+Runtime(n) references while CBQN's compiler produces Runtime(n)-only references. Different object table layouts, same semantics.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] iarrs[0] was bc array instead of empty placeholder**
- **Found during:** Task 2 (SELF-02 behavioral test — all 13 tests crashed)
- **Issue:** compiler_output_to_owned pushed `output.bc` into iarrs[0], causing block references to be off and the iarrs table to contain a 14K-element array that confused the decoder
- **Fix:** Changed iarrs initialization to `iarrs.push(vec![])` — iarrs[0] = empty array per CBQN convention
- **Files modified:** crates/rbqn/src/exec.rs
- **Verification:** Decoded .bin shows iarrs[0]=[] (len=0), matching CBQN wire format
- **Committed in:** 1f4b4c5 (Task 2 commit)

**2. [Rule 1 - Bug] c.bqn wrapping embedded glyphs as constants (imm=1 block)**
- **Found during:** Task 2 (comparing block structures: CBQN blocks[1]=imm=0, self=imm=1)
- **Issue:** `{func‿mod1‿mod2 ← ⟨"glyphs"⟩ body}` produces an immediate block because no `𝕩` reference. Bootstrap's `c1(compgen, glyphs_b)` expects a non-immediate function block.
- **Fix:** Changed wrapping to `{func‿mod1‿mod2←𝕩\nbody}` — `𝕩` usage forces imm=0
- **Files modified:** crates/rbqn/src/bin/rbqn_gen.rs
- **Verification:** blocks[1] now has imm=0 in decoded output; bootstrap c1(compgen,glyphs) succeeds
- **Committed in:** 1f4b4c5 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (Rule 1 — bugs in serialization approach)
**Impact on plan:** Both bugs prevented SELF-02 from passing. Fixes were localized and non-architectural.

## Issues Encountered

- The bootstrap panic "compiler initialization panicked: Domain error: This block cannot be called with these arguments" was the symptom pointing to both bugs. Diagnosed by comparing decoded CBQN vs self-compiled .bin structures.
- Self-compiled .bin files are intentionally smaller than CBQN-generated: RBQN uses mixed Provide+Runtime object references, CBQN uses Runtime-only. Both decode correctly and produce equivalent behavior.
- r0.bqn and r1.bqn still cannot be compiled (known limitation: assignment destructuring patterns not supported). This is pre-existing and expected.

## Next Phase Readiness

- SELF-01: c.bqn compiles via RBQN's own compiler (confirmed in 06-02, reinforced here)
- SELF-02: Behavioral equivalence confirmed — all 13 tests pass with self-compiled bytecode
- SELF-03, SELF-04, SELF-05: No regression (zero-dependency build still works)
- Phase 06 gap closure complete: the 49-byte placeholder gap is fully closed
- Self-hosting verification: RBQN can compile its own compiler source and the result is behaviorally equivalent

---
*Phase: 06-self-hosting*
*Completed: 2026-03-01*
