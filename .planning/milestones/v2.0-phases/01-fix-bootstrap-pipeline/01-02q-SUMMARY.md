# Phase 1 Plan 02q: Fix Under Inverse Assertion

Delegate Under (⌾) to BQN runtime's implementation for structural correctness.

## Changes

### Problem
During compiler initialization (`c1(compgen, glyphs)`), two "Assertion error: ⁼: Inverse failed" errors fired. The root cause: our native Under implementation used a naive `G⁻¹(F(G(x)))` approach that fails for structural G functions (e.g. `mask⊸/`). The BQN runtime's roundtrip assertion check caught the incorrect inverse and panicked.

After the assertion errors were caught by `catch_unwind`, the code fell through to broken structural Under helpers that produced wrong results, causing subsequent rank/shape errors.

### Solution
Follow CBQN's architecture (load.c line 506, md2.c lines 284-291):

1. **Extract `rt_under`**: After runtime1 loads, extract the BQN-defined Under function from `rtObjRaw[57]` and store it in a global.

2. **Delegate to BQN runtime**: When the BQN runtime's Under is available, our native Under creates `m_md2d(rt_under, f, g)` and calls it. The BQN runtime's Under handles both computational (`G⁻¹(F(G(x)))` with correct assertion) and structural cases (mask-based selection, index-based selection, etc.).

3. **Dyadic Under**: Follow CBQN's pattern: `w F⌾G x` is transformed to `(G(w)⊸F)⌾G x` -- bind `G(w)` as left argument of F using Before (⊸), then do monadic Under.

4. **Remove dead code**: Deleted ~250 lines of broken structural Under helpers (`detect_mask_replicate`, `detect_idx_select`, `structural_under_replicate`, `structural_under_select`, `structural_under_generic`).

### Key Insight
CBQN uses virtual dispatch (`fn_uc1`) where each function type knows how to handle being used as G in Under. The default handler (`def_fn_uc1`) falls back to the BQN runtime's Under implementation (`rt_under`). We replicate this fallback pattern.

## Files Modified

| File | Change |
|------|--------|
| `crates/rbqn-vm/src/modifiers.rs` | Rewrote Under (⌾) to delegate to BQN runtime; added `set_rt_under`/`get_rt_under` globals; removed broken structural Under helpers |
| `crates/rbqn/src/bootstrap.rs` | Extract `rt_under` from `rtObjRaw[57]` after runtime1 loads |

## Commits

| Hash | Description |
|------|-------------|
| 483d4ea | fix(01-02): delegate Under to BQN runtime for structural correctness |

## Verification
- "Assertion error: ⁼: Inverse failed" no longer appears during `cargo run -- -e '1+1'`
- Compiler initialization (`compgen(glyphs)`) succeeds without assertion errors
- All core/prim/vm unit tests pass
- Net code reduction: -192 lines (62 added, 254 removed)

## Remaining Issues
- "Type error: Expected number" in `choose_c1` (◶) during compilation of '1+1' -- separate issue, not related to Under
