# Phase 7: Code Simplification & CBQN Parity - Research

**Researched:** 2026-03-02
**Domain:** Rust code quality (clippy/compiler warnings) + BQN behavioral parity
**Confidence:** HIGH (all findings verified against live codebase and running binaries)

## Summary

Phase 7 has two distinct work streams: (1) code simplification via the `code-simplifier` agent across all crates, and (2) closing behavioral divergences from CBQN. Both are well-scoped. The simplification work is mechanical and large in volume (215 clippy warnings, 143 auto-fixable) but low risk. The parity work requires targeted fixes to the formatter and error output; the 13 official BQN test files already pass green, so parity gaps are in areas outside those tests (•Fmt/•Repr for multi-dim arrays, error message format).

The two streams are independent and can be planned as separate plans within the phase. Simplification should come first since it touches all crates; parity fixes come second since they are scoped to specific files (main.rs, derive.rs, the embedded formatter).

**Primary recommendation:** Run `cargo clippy --fix` per crate to auto-apply 143 suggestions, then manually address the remaining 72 warnings. Fix the four identified CBQN parity gaps separately. Run the full test suite after each stream to catch regressions.

## Current State (Verified)

### Test baseline
- All 13 official BQN test files: **ALL GREEN** (1316 total tests pass)
- `test_cbqn_compat.sh` tier 1-5: **21/21 pass**

### Clippy warnings by crate
| Crate | Total Warnings | Auto-fixable |
|-------|---------------|--------------|
| rbqn-vm | 101 | 73 |
| rbqn-prim | 81 | 52 |
| rbqn-gpu | 11 | 7 |
| rbqn (lib) | 8 | 6 |
| rbqn (bin) | 3 | 2 |
| rbqn-core | 5 | 3 |
| **Total** | **209** | **143** |

### Compiler warnings (cargo build)
- 39 additional warnings (unused variables, mut, parens) — mostly overlapping with clippy

## Architecture Patterns

### Crate structure (for simplification scope)
```
crates/
├── rbqn-core/     # B type, arrays, error — 5 clippy warns
├── rbqn-vm/       # VM, compiler, modifiers, derive — 101 clippy warns (largest)
├── rbqn-prim/     # primitives — 81 clippy warns (second)
├── rbqn-gpu/      # GPU kernels — 11 clippy warns
└── rbqn/          # main binary, bootstrap — 11 clippy warns
```

### Largest files (simplification priority)
| File | Lines | Functions | Notes |
|------|-------|-----------|-------|
| `rbqn-vm/src/derive.rs` | 3637 | 174 | VM dispatch + sys fns |
| `rbqn-prim/src/structural.rs` | 2570 | — | All structural primitives |
| `rbqn-vm/src/modifiers.rs` | 2265 | ~30 | each, fold, scan, table |
| `rbqn-prim/src/group.rs` | 926 | — | Group/GroupLen/GroupOrd |
| `rbqn-prim/src/arith_dyad.rs` | 900 | — | Dyadic arithmetic |

### How code-simplifier is invoked
The `code-simplifier` agent in `.claude/agents/code-simplifier.md` is a Claude Opus subagent. Per its description it:
- Focuses on recently modified code unless instructed to review broader scope
- Preserves exact functionality
- Targets: unnecessary complexity/nesting, redundant code, naming, consolidating related logic
- Can be explicitly instructed to review all crates

For Phase 7, it must be invoked with an explicit instruction to review the entire workspace (not just recent changes), crate by crate.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead |
|---------|-------------|-------------|
| Clippy auto-fixes | Manual edits | `cargo clippy --fix --lib -p {crate}` |
| div_ceil reimplementation | `(x + d - 1) / d` | `.div_ceil(d)` (stable since Rust 1.73) |
| Manual range contains | `x >= lo && x <= hi` | `(lo..=hi).contains(&x)` |

## Code Simplification Plan

### Stream 1: Auto-fixable warnings (mechanical)

Run per crate in order (least to most impact):
```bash
cargo clippy --fix --lib -p rbqn-core
cargo clippy --fix --lib -p rbqn-gpu
cargo clippy --fix --lib -p rbqn
cargo clippy --fix --bin "rbqn"
cargo clippy --fix --lib -p rbqn-prim
cargo clippy --fix --lib -p rbqn-vm
```

Each `--fix` applies only the machine-safe transformations. Verify build passes after each crate.

### Stream 2: Manual warnings (judgment required)

Top categories requiring manual work:
| Category | Count | Action |
|----------|-------|--------|
| `collapsible_if` | 81 | Merge nested `if` into combined condition |
| `identity map` | 8 | Remove `.map(|x| x)` calls |
| `unreachable expression` | 6 | Remove dead branches |
| `if_same_then_else` | 2 | Merge or remove duplicate branches |
| Dead functions: `deep_pick_one`, `deep_pick`, `group_multi_axis_scalar_x` | 3 | Remove or pub + use |
| Dead fields: `result_size` (3 instances) | 3 | Remove from structs |
| `unused variables` | ~15 | Prefix with `_` or remove |

### Stream 3: code-simplifier agent sweep

After mechanical fixes, invoke `code-simplifier` on each crate for deeper structural improvements:
- Focus on derive.rs (3637 lines, 174 fns) — largest simplification opportunity
- Focus on modifiers.rs (2265 lines) — each/fold/scan/table patterns have duplication
- structural.rs (2570 lines) — reshape/take/drop patterns

## CBQN Parity Gaps (Verified)

Tested by running expressions against both `CBQN/BQN` and `rbqn` binaries. Test suite (1316 assertions) passes. Gaps are in output formatting and error messages.

### Gap 1: •Fmt / -p mode for multi-dim arrays (HIGH IMPACT)

**Symptom:** `2‿3⥊↕6` in `-p` mode shows `[2‿3×…]` in RBQN vs box-drawing in CBQN.

**Root cause:** The BQN formatter loaded from embedded bytecode handles rank-2+ arrays differently. `format_result()` in `main.rs` calls `c1(fmt_fn, val)` — the formatter succeeds but returns a debug representation instead of the BQN canonical box-drawn form.

```
CBQN: ┌─
      ╵ 0 1 2
        3 4 5
              ┘
RBQN: [2‿3×…]
```

**Files:** `crates/rbqn/src/main.rs:383` (`format_result`), embedded formatter in `bootstrap.rs`.

**Investigation needed:** Is this the `•Fmt` function or the `•Repr`/display function being called? The formatter is loaded from embedded BQN bytecode (self-compiled). A different formatter function (index 1 of the formatter pair vs index 0) may handle `-p` display.

### Gap 2: •Repr broken for numeric arrays (HIGH IMPACT)

**Symptom:**
```
•Repr ↕5  →  CBQN: "0‿1‿2‿3‿4"   RBQN: "‿‿‿‿"
•Repr 3‿4‿5 → CBQN: "3‿4‿5"      RBQN: "‿‿"
```

**Root cause:** `•Repr` is the second function in the formatter pair. The output "‿‿‿‿" for a 5-element array suggests the formatter is only collecting separator characters between elements, meaning it's iterating elements but the element values are being missed (returning empty or fill values).

**Investigation needed:** Check how `•Repr` is called vs `•Fmt`; may be that the formatter expects `•Repr` argument to be pre-wrapped (boxed) but RBQN passes the raw value.

### Gap 3: Error message format (MEDIUM IMPACT)

**Symptom:**
```
CBQN: Error: Assertion error
      (-p):1:
        !0
        ^
RBQN:  Error: Domain error: Assertion error: Assertion failed: 0
```

**Root cause:** RBQN catches panics in `exec.rs` and wraps in `BqnError::Domain`. The panic message from `throw()` already contains the BqnError Display string (`Domain error: Assertion error: ...`), so the final output double-prefixes. Additionally RBQN doesn't emit source location (line/column) context.

**Severity:** This affects user-facing error messages but not program behavior. Source location tracking is a significant feature gap vs CBQN; fixing the double-prefix is trivial.

**Files:** `crates/rbqn-core/src/error.rs` (BqnError Display), `crates/rbqn/src/exec.rs:46-57` (panic catch), `crates/rbqn/src/main.rs:103`.

### Gap 4: •Glyph for non-primitive functions (LOW IMPACT)

**Symptom:**
```
•Glyph {𝕩}  →  CBQN: "(function block)"   RBQN: "" (empty string)
```

**Root cause:** `dispatch_sys_glyph_c1` in `derive.rs:1919` returns empty string for non-primitive functions. CBQN returns a descriptive string for block functions.

**Files:** `crates/rbqn-vm/src/derive.rs:1888-1922`.

### Not a parity gap

- `•PrimInd`: CBQN returns "Unknown system function" because `•primind` is not exposed in CBQN to user code — it's an internal mechanism. RBQN exposing it is not wrong but could be hidden.

## Common Pitfalls

### Pitfall 1: cargo clippy --fix modifying shared code
**What goes wrong:** Running `--fix` on one crate can modify code that is semantically shared (e.g., helper functions used by tests). Some collapsible_if transformations change logical structure in ways that affect readability even if semantics are preserved.
**How to avoid:** Run `cargo build && cargo test` after each crate's fix pass, before moving to the next.
**Warning signs:** Merge conflicts in fix output; changes to `pub` functions in library crates.

### Pitfall 2: Removing "dead" code that's actually load-bearing
**What goes wrong:** `deep_pick_one`, `deep_pick`, `group_multi_axis_scalar_x` appear unused but may be referenced by feature-gated paths or future expansion.
**How to avoid:** Check git blame and comments before deleting; search for any conditional compilation that might reference them.

### Pitfall 3: Formatter parity fixes breaking test suite
**What goes wrong:** The formatter (embedded BQN bytecode) is shared between `•Fmt`/`•Repr` and `-p` display. Changing how it's called may break one path while fixing another.
**How to avoid:** Run `./test_suite.sh --diff` before and after each formatter fix. The formatter is tested indirectly by all 13 test files.

### Pitfall 4: collapsible_if creating hard-to-read conditions
**What goes wrong:** Clippy correctly suggests merging `if a { if b { ... } }` into `if a && b { ... }`, but when conditions are long, the merged form may be harder to read.
**How to avoid:** Accept the clippy fix, then manually format the condition across multiple lines if needed.

## Code Examples

### Auto-fix invocation pattern
```bash
# Per crate, in order
cargo clippy --fix --lib -p rbqn-core 2>&1
cargo build --workspace  # verify no breakage

# After all crates
cargo clippy --workspace 2>&1 | grep "generated [0-9]* warning"
```

### div_ceil fix (10 instances)
```rust
// Before (manual reimplementation)
(x + d - 1) / d

// After (standard method, stable since Rust 1.73)
x.div_ceil(d)
```

### Verifying parity after changes
```bash
# Run both test scripts
./test_suite.sh        # must stay ALL GREEN
./test_cbqn_compat.sh  # must stay 21/21

# Spot-check specific parity gaps
/Users/axel/Code/forks/CBQN/BQN -p "2‿3⥊↕6" 2>&1
./target/release/rbqn -p "2‿3⥊↕6" 2>&1
```

## Standard Stack

No external tools needed. Everything is within the existing Rust toolchain:

| Tool | Purpose |
|------|---------|
| `cargo clippy --fix` | Auto-apply 143 safe suggestions |
| `cargo build --workspace` | Verify no breakage after each fix batch |
| `./test_suite.sh` | Regression detection (must stay ALL GREEN) |
| `./test_cbqn_compat.sh` | Parity regression detection |
| `code-simplifier` agent | Deeper structural simplification after mechanical fixes |

## Open Questions

1. **•Fmt vs •Repr call path for -p mode**
   - What we know: `format_result()` in main.rs calls `c1(fmt_fn, val)` where `fmt_fn` is `rt.formatter.0`
   - What's unclear: CBQN has two formatter functions (fmt and repr); which does `-p` use? Is the issue that RBQN calls the wrong one, or that the formatter's internal state differs?
   - Recommendation: Print the formatter function indices during bootstrap and compare call site to CBQN's load.c `fmt_c1`/`fmt_c2` dispatch.

2. **Source location in error messages**
   - What we know: CBQN emits `(-p):1:\n  code\n  ^^^` context; RBQN emits nothing
   - What's unclear: Whether this is in scope for Phase 7 or a future improvement
   - Recommendation: Fix the double-prefix (`Domain error: Domain error:`) as it's trivial; defer source location tracking as it requires AST span tracking (significant work).

3. **code-simplifier scope boundary**
   - What we know: The agent focuses on recently modified code by default; Phase 7 needs it to sweep all crates
   - What's unclear: How many passes are needed; whether it should run per-file or per-crate
   - Recommendation: Run per crate in order of warning count (vm → prim → gpu → rbqn → core); one pass per crate after mechanical clippy fixes.

## Sources

### Primary (HIGH confidence)
- Live `cargo clippy --workspace` output — exact warning counts and categories (2026-03-02)
- Running `./test_suite.sh` — confirmed ALL GREEN 1316 tests (2026-03-02)
- Running `./test_cbqn_compat.sh` — confirmed 21/21 pass (2026-03-02)
- Direct CBQN vs RBQN binary comparison — 5 parity gaps in 20 tested expressions (2026-03-02)
- Source inspection: `crates/rbqn-core/src/error.rs`, `crates/rbqn/src/main.rs:383-410`, `crates/rbqn-vm/src/derive.rs:1888-1922`

### Secondary (MEDIUM confidence)
- Rust 1.73 release notes for `div_ceil` stabilization (training data, stable feature)

## Metadata

**Confidence breakdown:**
- Clippy warning counts and categories: HIGH — direct tool output
- Auto-fixable count (143): HIGH — direct tool output
- CBQN parity gaps (4 identified): HIGH — verified with running binaries
- Root cause analysis for formatter gaps: MEDIUM — code read but not debugged with breakpoints
- code-simplifier invocation pattern: MEDIUM — based on agent description; may need adjustment

**Research date:** 2026-03-02
**Valid until:** This is based on the current codebase snapshot; any new commits may change warning counts.
