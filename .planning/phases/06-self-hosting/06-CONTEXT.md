# Phase 6: Self-Hosting - Context

**Gathered:** 2026-02-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Remove the CBQN build dependency. Compile BQN's compiler (c.bqn) using RBQN, embed the resulting bytecode as .bin files, and make `cargo install rbqn` work with no CBQN installed. A separate `rbqn-gen` binary handles bytecode regeneration for developers.

</domain>

<decisions>
## Implementation Decisions

### Bytecode embedding
- Store pre-compiled bytecode as committed binary files in `src/embedded/` (e.g. runtime0.bin, runtime1x.bin, compiler.bin, formatter.bin)
- Embed all four stages: runtime0, runtime1x, compiler, formatter — fully standalone
- `include_bytes!` loads them at compile time
- Separate `rbqn-gen` binary (e.g. `cargo run --bin rbqn-gen`) requires CBQN_PATH, outputs .bin files to commit

### Build script behavior
- build.rs always uses embedded .bin files — never parses CBQN gen/ files
- Drop all gen/ file parsing from build.rs; that logic moves to `rbqn-gen` only
- CBQN_PATH is only relevant to `rbqn-gen`, not to the main build
- If .bin files are missing, build.rs hard-errors with clear message: "Missing embedded bytecode. Run `cargo run --bin rbqn-gen` with CBQN_PATH to regenerate."
- `rbqn --version` shows version and bytecode source info; normal startup is silent

### Install experience
- GPU support (wgpu) always included — matches Phase 5 decision, no feature flag
- `cargo install rbqn` installs only the `rbqn` binary — `rbqn-gen` is a dev tool, not user-facing
- No MSRV promise — target latest stable Rust
- README section explaining the self-hosting bootstrap story (how RBQN bootstrapped from CBQN)

### Claude's Discretion
- Verification strategy for bytecode matching (exact byte match vs behavioral equivalence)
- .bin file format (raw bytecode arrays or structured with headers)
- Whether `rbqn-gen` uses RBQN itself or CBQN to compile — pick what's most reliable for bootstrapping

</decisions>

<specifics>
## Specific Ideas

- CBQN is a dev-time bootstrapping tool only, not a runtime or build dependency for end users
- The bootstrap chain: CBQN compiles c.bqn → bytecode .bin files committed to repo → RBQN loads them via include_bytes!
- Goal is `cargo install rbqn` with zero external dependencies beyond Rust toolchain

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 06-self-hosting*
*Context gathered: 2026-02-28*
