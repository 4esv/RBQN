# RBQN Project State

## Current Milestone
M1: Core Language Completeness

## Current Phase
01-fix-bootstrap-pipeline — Plan 01 COMPLETE, continuing to Plan 02

## Last Session
2026-02-21 — Completed 01-01-PLAN.md (fork dispatch fix + typed modifier output)

## Decisions Made
- Fork c1: `(f g h) x = (f x) g (h x)` — fixed broken 3-step dispatch in derive.rs
- `results_to_arr` helper: modifier outputs use typed arrays (numeric/char) not Boxed
- Integration tests in `crates/rbqn-vm/tests/` (binary crate has no lib.rs)

## Implementation Status

### Bootstrap Pipeline
| Stage | Status | Notes |
|-------|--------|-------|
| Build script (parse CBQN gen) | DONE | Embedded bytecode from CBQN |
| Provide array (40 slots) | DONE | All 23 basic + 17 extended filled |
| Runtime0 execution | DONE | 13/13 integration tests pass; fork and each bugs fixed |
| Runtime1 execution | PARTIAL | Loads and runs but needs validation against fixed runtime0 |
| Compiler loading | PARTIAL | Loads but requires runtime1 to be fully correct |
| Formatter loading | SKIPPED | Optional, loads if embedded bytecode present |

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
- Runtime1/compiler correctness depends on runtime0 being correct (now unblocked)
