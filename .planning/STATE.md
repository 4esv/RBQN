# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-23)

**Core value:** Correct BQN execution with identical behavior to CBQN
**Current focus:** Phase 4 — Full Test Suite Green

## Current Position

Phase: 4 of 6 (Full Test Suite Green)
Plan: 2 of 5 in current phase (04-02 complete)
Status: Phase 4 active — prim.bqn reduced from 106 to 95 failures
Last activity: 2026-02-25 - Completed quick task 4: Run the bisect script to identify regression points

Progress: [████████░░] 74%

## Performance Metrics

**Velocity:**
- Total plans completed: 7
- Average duration: 10 min
- Total execution time: ~1.55 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01 | 3 | 38min | 13min |
| 02 | 3 | 20min | 7min |
| 03 | 5 | ~72min | ~14min |

**Recent Trend:**
- Last 6 plans: 03-01 (~15min), 03-02 (~15min), 03-03 (~35min), 03-04 (~12min), 03-05 (~35min), 04-01 (~45min)
- Trend: Steady (averaging ~26min recently)

*Updated after each plan completion*

## Accumulated Context

### Decisions

- [Pre-planning]: Option B (bypass runtime0 overrides) chosen after 19 failed debug iterations — native Rust primitives already correct
- [Pre-planning]: Scrapped M1 roadmap entirely; replanned for v2.0 with GPU scope included
- [Pre-planning]: Full CBQN test suite (13 files) required; GPU integration in-scope this milestone
- [Pre-planning]: Runtime bypass already implemented in bootstrap.rs — Phase 1 is verification, not new implementation
- [01-02]: System functions use global Mutex<Option<SysRuntime>> for •BQN re-evaluation state
- [01-02]: Environment values resolved at lookup time, not as callable functions
- [01-02]: •Out/•BQN tests blocked on pre-existing compiler string literal bug
- [01-04]: Root cause of string bug was in build.rs C literal parser, not in VM execution
- [01-04]: sort_up/sort_down need full row extraction for rank>1, not single-element get
- [01-05]: REPL persistence uses compiler varNames/varDepths (depth=-1) rather than CBQN-style in-place scope mutation
- [01-05]: Search primitives (⊐ ⊒ ∊ ⍷) required deep_equal fix for nested array comparison
- [02-02]: v_get and v_get_move need array handling for SETM with list targets (destructuring modify-assign)
- [02-02]: syntax.bqn improved from 152 to 153 as collateral benefit of v_get array fix
- [02-03]: ARMM merge targets use bit-0 marker in array payload to distinguish from LSTM list targets
- [02-03]: Rank-1 merge-destructuring wraps elements as rank-0 unit arrays (CBQN m_unit semantics)
- [03-02]: Dyadic •FChars/•FBytes write variants share sys_idx with monadic reads (52/53) — arity distinguishes
- [03-02]: •ImportCache uses B::SENTINEL as circular-import sentinel; canonical path as cache key via fs::canonicalize
- [03-02]: •ParseFloat normalizes ¯→- then handles ∞ and π as special string cases before f64 parse
- [03-03]: CBQN compiler strips underscores from modifier names — "_while_" → "while", "_fillBy_" → "fillby"
- [03-03]: Math functions use sys indices 1100-1130 to avoid collision with existing sys 100 (system resolver)
- [03-03]: BQN namespace field names are lowercased by compiler: "PI" → "pi" at access time
- [Phase 03]: native_inverse_reg removes + shortcut so BQN runtime handles dyadic +⁼ char arithmetic: 3+⁼'d'='a' via runtime -˜ inverse
- [Phase 03]: sys 203 c2 dispatch: w√⁼x = x^w implemented via pow_c2(x, xa, w, wa) with swapped args
- [Phase 03]: Functional-k Under: detect left_op.is_fun() in try_structural_under, evaluate c1(left_op,x) to get numeric k before take/drop
- [03-05]: ⊏ scalar select from rank-1 returns rank-0 cell (not plain scalar) — enables (<'c')≡2⊏"abc"
- [03-05]: ⍉ diagonal (duplicate perm values) is valid BQN; only gaps in new axes are errors
- [03-05]: ⍋/⍒ bins validate x.trailing_shape == w.cell_shape (not x.cell_shape == w.cell_shape)
- [04-01]: CBQN omits ALIM for same-name ⟨a⟩←ns — uses body_arr[2] slot→name mapping at runtime
- [04-01]: ALIAS_TAG (0x7FF5) NaN-boxes gid+depth+pos for explicit rename ⟨b⇐a⟩←ns
- [04-01]: build_ns_desc creates NSDesc when export_mask array present (even empty) to handle {⇐}
- [04-01]: Namespace destructuring only in v_set ARRAY branch to prevent plain ns←val from triggering extraction
- [04-02]: table_c2 uses match on (w.is_atom(), x.is_atom()) — scalar_as_unit caused bootstrap panic via rank-0 pick
- [04-02]: reshape modes 0=exact,1=floor+cycle,2=ceil+cycle,3=ceil+pad; ⌊ and ↑ registered as statics at bootstrap
- [04-02]: group multi-dim uses per-dimension matching Cartesian outer-product (NOT parallel flat-position indexing)
- [04-02]: f¨/f˘ on scalars return rank-0 arrays (BQN semantics), not plain scalars
- [Phase 04-full-test-suite-green]: ScanInv DerivedKind intercepts inv_reg(F') natively to avoid BQN runtime issues with rank>1 arrays
- [Phase 04-full-test-suite-green]: ⍉ rank>2: move first axis to last (not full reverse); ⍉⁼ rank>2: move last axis to first using permutation 1‥(r-1)‥0

### Pending Todos

None yet.

### Blockers/Concerns

- PrimInd regression risk: `•PrimInd "+"` must return 0 not ¯1 after bypass; assert immediately in Phase 1
- `setInv`/`setPrims` wrapped in `catch_unwind` in bootstrap.rs — panics are silently swallowed; remove before Phase 1 complete
- GPU f32 precision: all GPU kernels use f32, BQN semantics are f64 — precision guard required before any GPU arithmetic dispatch in Phase 5
- GPU staging buffers: current `GpuBuffer::storage()` lacks `MAP_READ`; pool staging buffers before any Phase 5 kernel wiring

### Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 4 | Run the bisect script to identify regression points | 2026-02-25 | 01a4147 | [4-run-the-bisect-script-to-identify-regres](./quick/4-run-the-bisect-script-to-identify-regres/) |

## Session Continuity

Last session: 2026-02-25
Stopped at: Completed quick-4 (test snapshot of uncommitted-wip — no regressions vs pre-gap-closure baseline, 98 failures unchanged)
Resume file: None
