# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-23)

**Core value:** Correct BQN execution with identical behavior to CBQN
**Current focus:** Phase 7 — Code Simplification & CBQN Parity (in progress)

## Current Position

Phase: 7 of 7 (Code Simplification & CBQN Parity) — IN PROGRESS
Plan: 1 of 3 in Phase 7 complete (07-01 complete)
Status: 07-01 complete — zero-warning workspace build, all 1316 tests still pass
Last activity: 2026-03-02 - Completed 07-01: eliminated all 209 clippy + 39 compiler warnings

Progress: [██████████] 100% (Phase 6 complete + Phase 7 started)

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
| Phase 04 P10 | 11min | 3 tasks | 1 files |
| Phase 05 P02 | 6min | 2 tasks | 7 files |
| Phase 05-gpu-integration P06 | 7 | 1 tasks | 3 files |
| Phase 05 P04 | 3min | 1 tasks | 3 files |
| Phase 05 P05 | 59min | 2 tasks | 4 files |
| Phase 06 P01 | 6min | 2 tasks | 9 files |
| Phase 06 P02 | 9 | 3 tasks | 11 files |
| Phase 06 P03 | 45min | 2 tasks | 2 files |
| Phase 07 P01 | 35min | 2 tasks | 13 files |

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
- [04-10]: HashMap uses namespace-per-instance with stub methods (test suite doesn't test it)
- [04-10]: ns.Keys/Values/Has/Get fully functional — namespace introspection works via NSDesc.exp_gids
- [Phase 04]: HashMap uses namespace-per-instance with stub methods (test suite doesn't test it)
- [Phase 04]: ns.Keys/Values/Has/Get fully functional — namespace introspection works via NSDesc.exp_gids
- [05-01]: GpuRuntime stored as OnceLock<Option<GpuRuntime>> — None when --no-gpu or no adapter found
- [05-01]: Precision guard uses 2^24 (16_777_216.0) as f32 safe integer limit
- [05-01]: sort_i32 sign-bit fix is CPU-side XOR pass (download, XOR, upload) — simpler than GPU shader pass
- [05-02]: OnceLock<fn-pointer> pattern for cross-crate GPU hooks avoids circular dependency (rbqn-prim can't depend on rbqn)
- [05-02]: GPU arith only dispatches array-array (matching shape) — scalar-array skipped (non-commutative op complexity)
- [05-02]: GPU argsort uses radix sort on GPU for keys + HashMap VecDeque index reconstruction on CPU for stability
- [05-02]: Sort/grade GPU threshold 500K (5x base) due to GPU-CPU roundtrip cost for argsort
- [05-03]: prim_idx mapping uses PRIM_GLYPHS order (0=+, 2=×, 6=⌊, 7=⌈) not provide-array order — plan had wrong indices
- [05-03]: scan shader propagate must use idx/512 for block_sums index — wid.x maps to 256-element workgroups but scan blocks are 512 elements
- [05-03]: gpu_scan only dispatches + (prim_idx 0); other scan ops fall back to CPU
- [Phase 05-06]: MatMul threshold M*K+K*N>=50000 avoids GPU overhead for small matrices; Softmax threshold n>=256 matches single workgroup size
- [Phase 05-06]: f32 GPU compute for matmul/softmax: ML primitives accept f32 precision, interface stays f64
- [05-04]: GPU_FUSED_HOOK explicit API chosen over auto-fusion: BQN evaluator calls c2 one at a time with no lookahead — true expression-level fusion requires VM-level analysis, documented as future work
- [05-04]: try_fused_arith uses string op names (add/sub/mul/div/scalar_add/scalar_mul) matching existing gpu_op_name convention in arith_dyad.rs
- [05-05]: Metal dispatch overhead on Apple Silicon is ~1.5ms constant — GPU slower than CPU at all tested sizes (50K-500K); crossover requires ~5-40M elements depending on op
- [05-05]: Updated dispatch thresholds to benchmark-validated crossover × 2: arith 30M, reduce 80M, scan 10M, sort 100M
- [06-01]: ObjectEntry::Str changed from &'static [u32] to Vec<u32> — required for OwnedBytecode decode path
- [06-01]: gen-tools feature NOT in defaults — cargo install rbqn only installs rbqn binary, not rbqn-gen
- [06-01]: bootstrap.rs updated alongside embedded/mod.rs in Task 1 (plan said Task 2) due to type change requiring immediate rebuild
- [Phase 06-02]: lib.rs exposes bootstrap + exec so rbqn-gen binary can call bootstrap::bootstrap() and exec_string() without subprocess
- [Phase 06-02]: CBQN-generated .bin source_tag fixed to cbqn (was accidentally gen/ filename like 'compiles')
- [Phase 06-02]: rbqn-gen --verify: catch_unwind per file, c.bqn and f.bqn compile OK, r0/r1 fail on assignment destructuring (documented, not blocked)
- [06-03]: iarrs[0] must be empty [] not bc — CBQN wire format uses iarrs[0]=[] as shared placeholder for no-bodies block lists
- [06-03]: c.bqn wrapping uses {func‿mod1‿mod2←𝕩} (𝕩 arg forces imm=0) — embedded glyph constants cause imm=1 which breaks bootstrap c1(compgen,glyphs)
- [06-03]: Self-compiled .bin sizes (~half of CBQN) expected — RBQN uses mixed Provide+Runtime obj refs vs CBQN's Runtime-only; same semantics

- [07-01]: `return error::throw(...)` triggers both unreachable_code and diverging_sub_expression simultaneously — drop `return` to fix both
- [07-01]: Dead code removal (270 lines): deep_pick_one/deep_pick/group_multi_axis_scalar_x/md2d_inverse_reg were fully unreachable

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

### Roadmap Evolution

- Phase 7 added: Code Simplification & CBQN Parity

## Session Continuity

Last session: 2026-03-02
Stopped at: Completed 07-01-PLAN.md (zero-warning workspace, all 1316 tests green — ready for Phase 7 Plans 02 and 03)
Resume file: None
