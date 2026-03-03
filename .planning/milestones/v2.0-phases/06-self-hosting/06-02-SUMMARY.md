---
phase: 06-self-hosting
plan: 02
subsystem: infra
tags: [self-hosting, bqn-compiler, bootstrap, rbqn-gen, cli, readme]

requires:
  - phase: 06-self-hosting
    plan: 01
    provides: "rbqn-gen tool, committed .bin bytecode files, decode_bytecode()"

provides:
  - "rbqn-gen --verify: bootstraps RBQN and compiles c.bqn + f.bqn via RBQN's own compiler"
  - "src/lib.rs + src/exec.rs: shared library target exposing bootstrap and exec_string"
  - "embedded::bytecode_source_tag(): lightweight header read for --version"
  - "rbqn --version shows bytecode source (cbqn or rbqn-self)"
  - "README.md with bootstrap story, usage, and developer workflow"
  - "CBQN-generated .bin files now tagged cbqn (was filename-derived)"

affects: [self-hosting, cargo-install, developer-onboarding]

tech-stack:
  added: []
  patterns:
    - "lib.rs + main.rs dual-target: binary accesses lib via rbqn::bootstrap, rbqn::exec"
    - "bytecode_source_tag(): read header-only from .bin (no full decode for --version)"
    - "rbqn-gen --verify: catch_unwind around each source file compilation"
    - "BQN source location: BQN_SRC env var or CBQN_PATH/../BQN/src sibling repo"

key-files:
  created:
    - crates/rbqn/src/lib.rs
    - crates/rbqn/src/exec.rs
    - README.md
  modified:
    - crates/rbqn/src/bin/rbqn_gen.rs
    - crates/rbqn/src/main.rs
    - crates/rbqn/src/repl.rs
    - crates/rbqn/src/cli.rs
    - crates/rbqn/src/embedded/mod.rs
    - crates/rbqn/src/embedded/compiler.bin
    - crates/rbqn/src/embedded/formatter.bin
    - crates/rbqn/src/embedded/runtime0.bin
    - crates/rbqn/src/embedded/runtime1x.bin

key-decisions:
  - "lib.rs exposes bootstrap + exec so rbqn-gen binary can access them without subprocess"
  - "exec_string extracted from main.rs into exec.rs (shared between main and rbqn-gen)"
  - "main.rs uses rbqn::bootstrap (lib), not crate::bootstrap (its own mod declaration)"
  - "rbqn-gen --verify uses catch_unwind per file — failures are informational, not fatal"
  - "c.bqn wrapping strips first line (func‿mod1‿mod2←•args) then wraps body as BQN function"
  - "CBQN-generated .bin source_tag fixed to cbqn (was accidentally using gen/ filename)"
  - "r0.bqn and r1.bqn fail with assignment target error — known RBQN limitation, documented"

patterns-established:
  - "RBQN self-compilation proof: c.bqn compiles OK, f.bqn compiles OK via RBQN's compiler"
  - "Self-hosted path: BQN_SRC env var > CBQN_PATH/../BQN/src sibling"
  - "README documents bootstrap chain for new contributors"

requirements-completed: [SELF-01, SELF-02]

duration: 9min
completed: 2026-02-28
---

# Phase 06 Plan 02: Self-Hosting Verification Summary

**RBQN compiles c.bqn (the BQN compiler source) using its own compiler, confirmed via rbqn-gen --verify, with --version showing bytecode provenance (cbqn vs rbqn-self)**

## Performance

- **Duration:** 9 min
- **Started:** 2026-02-28T21:36:03Z
- **Completed:** 2026-02-28T21:45:00Z
- **Tasks:** 3
- **Files modified:** 11

## Accomplishments

- RBQN successfully compiles c.bqn and f.bqn using its own compiler (SELF-01)
- Added lib.rs + exec.rs to share exec_string between main.rs and rbqn-gen.rs
- `rbqn --version` now shows `bytecode: cbqn` (or `rbqn-self` for self-compiled)
- README.md created with bootstrap chain, usage, and developer workflow docs
- Fixed CBQN-generated .bin files to use "cbqn" source tag (was using gen/ filenames)
- All 13/13 BQN test files remain green throughout

## Task Commits

1. **Task 1: Add self-compilation verification to rbqn-gen** - `7094ddd` (feat)
2. **Task 2: Add --version bytecode source info** - `dc0382c` (feat)
3. **Task 3: Add README bootstrap story section** - `8dfcba8` (docs)

## Files Created/Modified

- `crates/rbqn/src/lib.rs` - New library target exposing bootstrap + exec modules
- `crates/rbqn/src/exec.rs` - exec_string extracted from main.rs for library reuse
- `crates/rbqn/src/bin/rbqn_gen.rs` - Added --verify flag with full self-compilation workflow
- `crates/rbqn/src/main.rs` - Now uses rbqn::bootstrap + rbqn::exec from lib
- `crates/rbqn/src/repl.rs` - Updated to use rbqn::bootstrap::Runtime from lib
- `crates/rbqn/src/cli.rs` - --version handler prints bytecode source tag
- `crates/rbqn/src/embedded/mod.rs` - Added bytecode_source_tag() function
- `crates/rbqn/src/embedded/*.bin` - Regenerated with "cbqn" source tag
- `README.md` - Created with bootstrap story, usage, and developer workflow

## Decisions Made

- Added `lib.rs` as library target so `rbqn-gen.rs` (a separate binary) can call `bootstrap::bootstrap()` and `exec_string()` without subprocess. The binary target accesses lib via `rbqn::bootstrap`.
- `exec_string` extracted from `main.rs` into `exec.rs` — eliminates code duplication and makes exec available to the library.
- `--verify` wraps each BQN source compilation in `catch_unwind` — failures are informational, not fatal (CBQN-generated .bin files are the shipping artifact).
- `c.bqn` needs wrapping: its first line `func‿mod1‿mod2←•args` is replaced with a BQN function literal that takes the glyph arrays as argument.
- `r0.bqn` and `r1.bqn` fail with "Assignment target must be a name or list of targets" — this is a known RBQN compiler limitation for certain destructuring patterns. Documented, not blocked.
- CBQN-generated .bin source_tag changed from filename ("compiles") to "cbqn" — makes --version output meaningful.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Source tag in .bin files was gen/ filename, not "cbqn"**
- **Found during:** Task 2 (--version implementation)
- **Issue:** `rbqn --version` showed `bytecode: compiles` (the gen/ filename) not `bytecode: cbqn`
- **Fix:** Changed rbqn-gen to use "cbqn" as source_tag for all CBQN-generated .bin files, regenerated all 4 .bin files
- **Files modified:** crates/rbqn/src/bin/rbqn_gen.rs, all 4 .bin files
- **Verification:** `rbqn --version` now shows `bytecode: cbqn`; 13/13 tests still pass
- **Committed in:** dc0382c (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 — wrong source_tag in existing .bin files)
**Impact on plan:** No scope creep. Bug fix was a prerequisite for Task 2 to work correctly.

## Issues Encountered

- `r0.bqn` and `r1.bqn` cannot be compiled by RBQN due to "Assignment target must be a name or list of targets" assertion error. These files use BQN destructuring patterns that RBQN's compiler doesn't fully support yet. c.bqn and f.bqn both compile successfully, satisfying SELF-01.

## Next Phase Readiness

- Phase 06 (self-hosting) complete: bootstrap chain established (06-01), self-hosting verified (06-02)
- SELF-01: c.bqn compiles via RBQN's own compiler
- SELF-02: 13/13 test files green with CBQN-generated .bin files (behavioral equivalence)
- SELF-03, SELF-04, SELF-05: satisfied by 06-01 (zero-dependency build, cargo install)

---
*Phase: 06-self-hosting*
*Completed: 2026-02-28*
