---
phase: 06-self-hosting
plan: 01
subsystem: infra
tags: [binary-format, embedded-bytecode, cbqn, build-system, rust]

requires:
  - phase: 05-gpu-integration
    provides: "All 13 BQN test files green (baseline)"

provides:
  - "rbqn-gen binary (gen-tools feature) that parses CBQN gen/ and writes .bin files"
  - "Four committed .bin files: runtime0.bin, runtime1x.bin, compiler.bin, formatter.bin"
  - "OwnedBytecode struct + decode_bytecode() in embedded/mod.rs"
  - "Simplified build.rs (~24 lines, presence check only)"
  - "bootstrap.rs wired to decode_bytecode() at startup"

affects: [self-hosting, cargo-install]

tech-stack:
  added: [gen-tools feature flag, RBQN binary wire format]
  patterns:
    - "RBQN binary format: magic(4) + version(u32) + source_tag + bc + iarrs + objs + blocks + bodies"
    - "Obj tags: 0=Provide, 1=Runtime, 2=RuntimePrev, 3=Float, 4=Char, 5=Str, 6=IArr"
    - "Block tags: 0=IArr, 1=Info(u8 typ + u32 iarrs0_idx + u32 data_idx)"
    - "gen-tools feature gates rbqn-gen binary, never compiled during normal cargo build"

key-files:
  created:
    - crates/rbqn/src/bin/rbqn_gen.rs
    - crates/rbqn/src/embedded/runtime0.bin
    - crates/rbqn/src/embedded/runtime1x.bin
    - crates/rbqn/src/embedded/compiler.bin
    - crates/rbqn/src/embedded/formatter.bin
  modified:
    - crates/rbqn/build.rs
    - crates/rbqn/src/embedded/mod.rs
    - crates/rbqn/src/bootstrap.rs
    - crates/rbqn/Cargo.toml

key-decisions:
  - "ObjectEntry::Str changed from &'static [u32] to Vec<u32> — required for OwnedBytecode decode path"
  - "gen-tools feature NOT in defaults — cargo install rbqn only installs rbqn binary"
  - "Placeholder .bin files created via Python to satisfy include_bytes! before rbqn-gen runs"
  - "bootstrap.rs updated in Task 1 (not Task 2) since ObjectEntry type change forced immediate rebuild"

patterns-established:
  - "RBQN wire format: simple TLV binary, always little-endian, validated by magic+version"
  - "rbqn-gen tool: CBQN_PATH required, finds gen/ dir same way as old build.rs"
  - "decode_bytecode: cursor-based reader, panics on invalid magic/version (format invariant)"

requirements-completed: [SELF-03, SELF-04, SELF-05]

duration: 6min
completed: 2026-02-28
---

# Phase 06 Plan 01: Self-Hosting Bootstrap Summary

**CBQN build dependency eliminated: four committed .bin files replace 707-line C-parser build.rs, enabling `cargo install rbqn` with zero external dependencies**

## Performance

- **Duration:** 6 min
- **Started:** 2026-02-28T21:27:03Z
- **Completed:** 2026-02-28T21:33:32Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments

- Moved entire CBQN gen/ parser (~500 lines) from build.rs into `rbqn_gen.rs` dev binary
- Defined RBQN binary wire format and `encode_bytecode()` / `decode_bytecode()` functions
- Replaced 707-line `build.rs` with 24-line presence check
- Committed 4 `.bin` files (runtime0=9KB, runtime1x=224KB, compiler=126KB, formatter=28KB)
- All 13/13 BQN test files remain green with no CBQN_PATH set

## Task Commits

1. **Task 1: Binary format, rbqn-gen binary, and embedded loader** - `8d6720f` (feat)
2. **Task 2: Simplify build.rs and wire bootstrap to OwnedBytecode** - `20f6b96` (feat)

## Files Created/Modified

- `crates/rbqn/src/bin/rbqn_gen.rs` - Dev tool: CBQN gen/ parser + encode_bytecode
- `crates/rbqn/src/embedded/mod.rs` - include_bytes! statics, OwnedBytecode, decode_bytecode()
- `crates/rbqn/src/embedded/runtime0.bin` - Pre-compiled runtime0 bytecode (9,355 bytes)
- `crates/rbqn/src/embedded/runtime1x.bin` - Pre-compiled runtime1x bytecode (224,478 bytes)
- `crates/rbqn/src/embedded/compiler.bin` - Pre-compiled compiler bytecode (125,732 bytes)
- `crates/rbqn/src/embedded/formatter.bin` - Pre-compiled formatter bytecode (27,640 bytes)
- `crates/rbqn/build.rs` - Simplified to ~24 lines (presence check + rerun-if-changed)
- `crates/rbqn/src/bootstrap.rs` - Uses decode_bytecode() instead of generated statics
- `crates/rbqn/Cargo.toml` - Added gen-tools feature, [[bin]] rbqn-gen with required-features

## Decisions Made

- `ObjectEntry::Str` changed from `&'static [u32]` to `Vec<u32>` to support owned decode; `bootstrap.rs` updated in same PR since it broke compilation
- Python script used to create minimal placeholder `.bin` files before `rbqn-gen` was run, satisfying `include_bytes!`
- `gen-tools` feature not in defaults so `cargo install rbqn` only installs the `rbqn` binary
- build.rs keeps `#[allow(dead_code)]` on `EmbeddedBytecode` stub — removal deferred as it's harmless

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] bootstrap.rs updated in Task 1 (plan said Task 2)**
- **Found during:** Task 1 (embedded/mod.rs rewrite)
- **Issue:** Removing `ObjectEntry`'s `Copy` derive (required for `Vec<u32>`) and removing `include!()` broke bootstrap.rs compilation immediately
- **Fix:** Updated bootstrap.rs to use `OwnedBytecode` and reference patterns during Task 1 to keep the build green
- **Files modified:** crates/rbqn/src/bootstrap.rs
- **Verification:** `cargo build -p rbqn` passes after mod.rs changes
- **Committed in:** `20f6b96` (Task 2 commit, logically still correct)

---

**Total deviations:** 1 auto-fixed (Rule 1 - ordering deviation, not scope change)
**Impact on plan:** No scope creep. Both tasks' changes are present as specified; only the sequencing was merged since compilation required it.

## Issues Encountered

None — plan executed smoothly. The build.rs→rbqn_gen.rs migration was straightforward since the parser functions were already isolated.

## Next Phase Readiness

- Phase 06 Plan 01 complete: zero-dependency build path established
- `cargo install rbqn` now works without CBQN_PATH (requirements SELF-03, SELF-04, SELF-05 satisfied)
- Next: Phase 06 Plan 02 (if it exists) or phase completion

---
*Phase: 06-self-hosting*
*Completed: 2026-02-28*
