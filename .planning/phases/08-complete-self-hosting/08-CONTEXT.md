# Phase 8: Complete Self-Hosting - Context

**Gathered:** 2026-03-03
**Status:** Ready for planning

<domain>
## Phase Boundary

RBQN compiles its own `.bin` files via `rbqn-gen --self`. After this phase, `cargo build` and `cargo install rbqn` complete with no `CBQN_PATH` set and no CBQN on PATH. All four bytecode files (runtime0.bin, runtime1x.bin, compiler.bin, formatter.bin) are RBQN-compiled and committed. No CBQN-compiled bytecode remains in the repo.

</domain>

<decisions>
## Implementation Decisions

### Developer workflow
- A Make/cargo script handles `.bin` regeneration locally and is reused by CI (`rbqn-gen --self` under the hood)
- `cargo build` (dev profile): recompiles bins if BQN source is newer than committed bins
- `cargo build --release`: always uses committed bins — no recompile, reproducible/release builds
- If a PR changes BQN source but doesn't update bins: CI emits a warning but does **not** block the PR

### CBQN_PATH lifecycle
- `CBQN_PATH` is **deprecated with a warning**: if set, build prints "CBQN_PATH no longer needed — use `rbqn-gen --self` to regenerate bins"
- CI explicitly **unsets** `CBQN_PATH` so the self-hosting guarantee is machine-verifiable
- Not removed from the codebase yet — deprecation first

### Verification rigor
- Full verification suite required before Phase 8 can be called done:
  1. All 1316 official BQN tests pass with RBQN-compiled bins
  2. RBQN-compiled bins must be **literally byte-for-byte identical to CBQN-compiled bins** (hard requirement — a byte diff, not just semantic equivalence). Rationale: if RBQN correctly executes `c.bqn` (the BQN compiler), it must produce the exact same bytecode output as CBQN executing `c.bqn` against the same source. Any difference indicates a bug in RBQN. A reference copy of the CBQN-compiled bins must be preserved and diffed against the RBQN output.
  3. Swap test runs automatically until fixpoint: compile with RBQN → recompile with those bins → repeat until round N and round N-1 bins are identical; report iteration count
- Verification results published as CI artifact (not committed to repo)
- Goal: results shareable with Marshall and Dzaima as proof of correctness

### Committed .bin strategy
- `.bin` files moved to **git-lfs** (top-level `bins/` directory)
  - `bins/runtime0.bin`, `bins/runtime1x.bin`, `bins/compiler.bin`, `bins/formatter.bin`
- When CI regenerates and verifies bins: **commits directly to main**
- If `bins/` files are missing at build time (e.g., `git lfs pull` not run): **fail with a clear error message** — no fallback bootstrap, no auto-download

### Claude's Discretion
- Internal implementation of `rbqn-gen --self` compilation pipeline
- Exact Makefile/cargo script structure
- CI job names and step ordering
- Deprecation warning exact wording

</decisions>

<specifics>
## Specific Ideas

- Verification must be compelling enough to share directly with Marshall Lochbaum and Dzaima — the BQN authors who both serve as reference implementation and spec validators
- CBQN deserves acknowledgement in README as a direct reference (on par with the BQN spec and official test suite), not just a footnote
- The fixpoint swap test (automate until stable) is the gold standard proof of self-hosting correctness

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 08-complete-self-hosting*
*Context gathered: 2026-03-03*
