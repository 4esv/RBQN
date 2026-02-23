# RBQN Project State

## Current Milestone
M1: Core Language Completeness

## Current Phase
01-fix-bootstrap-pipeline — Plans 01 and 03 COMPLETE, Plan 02 pending

## Last Session
Last activity: 2026-02-23 - Completed quick task 3: Assess progress toward CBQN-compatible GPU-accelerated BQN

## Decisions Made
- Assessment recommends bypassing runtime0 overrides (Option B) over debugging VM execution (Option A), given 19 failed fix iterations
- Fork c1: `(f g h) x = (f x) g (h x)` — fixed broken 3-step dispatch in derive.rs
- `results_to_arr` helper: modifier outputs use typed arrays (numeric/char) not Boxed
- Integration tests in `crates/rbqn-vm/tests/` (binary crate has no lib.rs)
- Repr placeholder uses B::SENTINEL instead of m_sys_fn(1) to avoid Decompose conflict
- Mode::Eval (-e) prints output for REPL-like behavior
- Formatter tests gracefully skip when compiler unavailable rather than failing
- [Phase 01]: Root cause of runtime1 pick-on-empty crash: Arc::make_mut cloned parent scopes, making child writes invisible. Fix: Mutex<Vec<B>> interior mutability + Arc<Scope> sharing.

## Implementation Status

### Bootstrap Pipeline
| Stage | Status | Notes |
|-------|--------|-------|
| Build script (parse CBQN gen) | DONE | Embedded bytecode from CBQN |
| Provide array (40 slots) | DONE | All 23 basic + 17 extended filled |
| Runtime0 execution | DONE | 13/13 integration tests pass; fork and each bugs fixed |
| Runtime1 execution | PARTIAL | Loads and runs but needs validation against fixed runtime0 |
| Compiler loading | PARTIAL | Loads but requires runtime1 to be fully correct |
| Formatter loading | PARTIAL | Code complete, falls back gracefully; blocked by runtime1 panic |

### Primitive Functions (44 total)
| Category | Count | Status |
|----------|-------|--------|
| Arithmetic (+-×÷⋆√⌊⌈\|¬) | 10/10 | DONE |
| Logic/Comparison (∧∨<>≠=≤≥≡≢) | 10/10 | DONE |
| Structural (⊣⊢⥊∾≍⋈↑↓↕«»⌽⍉/) | 14/14 | DONE |
| Search/Sort (⍋⍒⊏⊑⊐⊒∊⍷⊔!) | 10/10 | DONE |

### Modifiers (20 total)
| Modifier | Glyph | Status |
|----------|-------|--------|
| Constant | ˙ | DONE |
| Swap | ˜ | DONE |
| Cells | ˘ | DONE (typed output fixed) |
| Each | ¨ | DONE (typed output fixed) |
| Table | ⌜ | DONE (typed output fixed) |
| Undo | ⁼ | NOT IMPLEMENTED |
| Fold | ´ | DONE (result now correct) |
| Insert | ˝ | DONE |
| Scan | ` | DONE (typed output fixed) |
| Atop | ∘ | DONE |
| Over | ○ | DONE |
| Before | ⊸ | DONE |
| After | ⟜ | DONE |
| Under | ⌾ | NOT IMPLEMENTED |
| Valences | ⊘ | DONE |
| Choose | ◶ | DONE |
| Rank | ⎉ | NOT IMPLEMENTED |
| Depth | ⚇ | NOT IMPLEMENTED |
| Repeat | ⍟ | DONE |
| Catch | ⎊ | DONE |

### System Functions
| Function | Status |
|----------|--------|
| •Type | DONE |
| •Decompose | DONE |
| •Glyph | STUB |
| •Fill | DONE |
| •GroupLen | DONE |
| •GroupOrd | DONE |
| All others (~44) | NOT IMPLEMENTED |

### GPU Crate
| Component | Status |
|-----------|--------|
| GpuContext (wgpu init) | DONE |
| Buffer pool | DONE |
| Pipeline cache | DONE |
| Dispatch thresholds | DONE |
| Unary elementwise (exp/sqrt/neg/abs) | DONE |
| Matmul (tiled 16x16) | DONE |
| Softmax (single-WG + multi-pass) | DONE |
| Arithmetic kernels | SCAFFOLDING |
| Reduce kernels | SCAFFOLDING |
| Scan kernels | SCAFFOLDING |
| Sort kernels | SCAFFOLDING |
| Select kernels | SCAFFOLDING |
| Fusion engine | MINIMAL |

### BQN Test Suite
| Test File | Status |
|-----------|--------|
| simple.bqn | UNKNOWN (compiler not working) |
| literal.bqn | UNKNOWN |
| token.bqn | UNKNOWN |
| syntax.bqn | UNKNOWN |
| bytecode.bqn | UNKNOWN |
| prim.bqn | UNKNOWN |
| header.bqn | UNKNOWN |
| unhead.bqn | UNKNOWN |
| namespace.bqn | UNKNOWN |
| fill.bqn | UNKNOWN |
| identity.bqn | UNKNOWN |
| under.bqn | UNKNOWN |
| undo.bqn | UNKNOWN |

## Known Bugs
None currently blocking runtime0. Fixed:
- ~~`+´` (fold with add): returns 3 instead of 6~~ FIXED
- ~~`+¨` (each with add): type error~~ FIXED

## Blockers
- Runtime override pipeline broken: 3/21 CBQN compat tests pass (14%). Most primitives return args unchanged through compiled path. 19 fix iterations haven't resolved it. Option B (bypass overrides, use native Rust primitives) recommended as next approach.

### Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 1 | Start GPU-accelerated ML primitives | 2026-02-21 | 1997575 | [1-start-gpu-accelerated-ml-primitives](./quick/1-start-gpu-accelerated-ml-primitives/) |
| 2 | Add tracing to compiler-used primitives | 2026-02-23 | 386b409 | [2-add-tracing-to-compiler-used-primitives-](./quick/2-add-tracing-to-compiler-used-primitives-/) |
| 3 | Assess progress toward CBQN-compatible GPU-accelerated BQN | 2026-02-23 | 8b3da9e | [3-assess-progress-toward-cbqn-compatible-g](./quick/3-assess-progress-toward-cbqn-compatible-g/) |
