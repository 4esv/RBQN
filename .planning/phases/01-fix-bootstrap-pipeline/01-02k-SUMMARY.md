# Phase 01 Plan 02k: Fix cells modifier for rank-1 arrays

Fix ˘ (cells) modifier on rank-1 arrays and add cell result merging.

## Root Cause

The ˘ (cells) modifier had two bugs:

1. **Rank-1 dispatch**: `cells_c1`/`cells_c2` with rank-1 arrays called `c2(f, w, x)` on the whole array instead of applying F to each element. In BQN, cells of a rank-1 array are individual rank-0 atoms.

2. **Result assembly**: Cell results were assembled with `results_to_arr` which produces boxed arrays. For ˘, uniform cell results must be merged into a higher-rank array (shape = lead + cell_shape).

This caused the compiler init crash: `0 ≍˘ arr[7]` was computed as `0 ≍ arr[7]` (scalar vs [7] shape mismatch) instead of mapping `0 ≍ elem` over each element to produce a [7,2] numeric array. The downstream `+` then saw [7] + [2,7] and errored.

## Investigation Path

1. Added shape tracing to c2 dispatch -- confirmed [7] + [2,7] shape mismatch
2. Verified in CBQN that [7] + [2,7] is a legitimate error (leading-axis prefix agreement)
3. Added modifier tracing -- identified the modifier as ˘ (prim_idx=46) with operand ≍ (prim_idx=24)
4. Confirmed in CBQN: `0 ≍˘ ↕7` produces shape [7,2], not an error
5. Found cells_c2 returned `c2(f, w, x)` directly for rank-1, bypassing per-element dispatch
6. Fixed cells to iterate elements for rank-1, added merge_cells_result for proper shape assembly

## Changes

- `crates/rbqn-vm/src/modifiers.rs`:
  - `cells_c1`: Handle rank-1 arrays by iterating elements
  - `cells_c2`: Handle rank-1 arrays with element iteration, supporting atom w, matching-length w, and higher-rank w
  - `merge_cells_result`: New function that merges uniform cell results into higher-rank arrays (used only by ˘)
  - `results_to_arr`: Unchanged -- used by ¨ and ⌜ which produce boxed results

## Verification

- Compiler init (`compgen(glyphs)`) completes successfully
- All 13 rbqn-vm tests pass
- Bootstrap passes all stages (runtime1, setPrims, setInv, compiler, formatter)

## Commit

- `3bf6027`: fix(01-02k): fix cells modifier for rank-1 arrays and cell merging
