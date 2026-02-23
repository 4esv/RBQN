# RBQN Project State

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-02-23 — Milestone v2.0 started

## Decisions Made
- Assessment recommends bypassing runtime0 overrides (Option B) over debugging VM execution (Option A), given 19 failed fix iterations
- Scrapped M1 roadmap — replanning from scratch with Option B strategy
- Full CBQN test suite compatibility required (all 13 files)
- GPU integration is in-scope for this milestone, not deferred

## Accumulated Context

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
| Cells | ˘ | DONE |
| Each | ¨ | DONE |
| Table | ⌜ | DONE |
| Undo | ⁼ | NOT IMPLEMENTED |
| Fold | ´ | DONE |
| Insert | ˝ | DONE |
| Scan | ` | DONE |
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

### Known Technical Details
- Fork monadic dispatch: correctly computes `(f x) g (h x)` in derive.rs
- results_to_arr helper: modifier outputs use typed arrays (numeric/char) not Boxed
- compile_block: full CBQN-compatible body pair processing with fixup remapping
- Arc::make_mut scope cloning was root cause of runtime1 crashes — fixed with Mutex<Vec<B>> interior mutability
- Repr placeholder uses B::SENTINEL instead of m_sys_fn(1)

## Blockers
- Runtime override pipeline broken: 3/21 CBQN compat tests pass (14%). Strategy shift to Option B (bypass overrides) approved.

### Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 1 | Start GPU-accelerated ML primitives | 2026-02-21 | 1997575 | [1-start-gpu-accelerated-ml-primitives](./quick/1-start-gpu-accelerated-ml-primitives/) |
| 2 | Add tracing to compiler-used primitives | 2026-02-23 | 386b409 | [2-add-tracing-to-compiler-used-primitives-](./quick/2-add-tracing-to-compiler-used-primitives-/) |
| 3 | Assess progress toward CBQN-compatible GPU-accelerated BQN | 2026-02-23 | 8b3da9e | [3-assess-progress-toward-cbqn-compatible-g](./quick/3-assess-progress-toward-cbqn-compatible-g/) |
