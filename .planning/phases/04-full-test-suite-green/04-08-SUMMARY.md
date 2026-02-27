---
phase: 04-full-test-suite-green
plan: 08
status: complete
started: 2026-02-27
completed: 2026-02-27
---

## Summary

Fixed all under.bqn failures (8→0). Implemented complete Under (⌾) structural inversion covering all test patterns.

### Key Changes
- **Under enclose-cells** (`F⌾(<˘)`): Detect <˘ pattern, inverse is >˘
- **Under enclose** (`F⌾<`): Inverse is > (merge)
- **Non-injective select**: Validate duplicate indices in `F⌾(k⊸⊏)`, throw on conflict
- **Under pick paths**: Scatter modified values back to nested index positions
- **Under take with computed k**: Evaluate functional left arg for `F⌾(k⊸↑)`
- **Under rank-modified select** (`F⌾(k⊏⎉r⊢)`): Decompose ⎉ modifier, scatter at rank
- **Chain error**: Detect ambiguous structural inverse for `F⌾(1↓4↑⊢)`, throw error
- **˘⁼ restriction**: Only `<˘⁼ = >˘` is valid, fixed undo.bqn regression

### Results
- under.bqn: 8→0 ✓
- undo.bqn: maintained at 0 (regression caught and fixed)

## Self-Check: PASSED
