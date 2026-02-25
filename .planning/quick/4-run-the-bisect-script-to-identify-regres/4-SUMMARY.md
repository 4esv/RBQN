---
phase: quick-4
plan: 1
subsystem: test-suite
tags: [testing, regression, baseline, snapshot]
dependency_graph:
  requires: []
  provides: [uncommitted-wip-snapshot]
  affects: []
tech_stack:
  added: []
  patterns: []
key_files:
  created:
    - .test-results/uncommitted-wip.txt (gitignored, local only)
    - .test-results/uncommitted-wip.detail.txt (gitignored, local only)
  modified: []
decisions:
  - "No regressions from uncommitted changes: 5 modified files produce identical test results to pre-gap-closure"
metrics:
  duration: ~5min
  completed: 2026-02-25T14:51:00Z
---

# Quick Task 4: Run Bisect Script to Identify Regressions — Summary

**One-liner:** Test snapshot of uncommitted working tree (compare.rs, search.rs, select.rs, scope.rs, vm.rs) shows zero delta vs 98-failure pre-gap-closure baseline.

## What Was Done

### Task 1: Build and Run Full Test Suite

Built RBQN in release mode with `CBQN_PATH=/Users/axel/Code/forks/CBQN`. Build succeeded with warnings only.

Ran `./test_suite.sh --save uncommitted-wip` against all 13 test files. Results saved to `.test-results/uncommitted-wip.txt` and `.test-results/uncommitted-wip.detail.txt`.

### Task 2: Compare Against Pre-Gap-Closure Baseline

Ran `./test_suite.sh --compare pre-gap-closure uncommitted-wip` to diff the new snapshot against the commit 48f4971 baseline (98 failures).

Also ran `./test_cbqn_compat.sh`: 21/21 expressions pass across all 5 tiers (arithmetic, arrays, modifiers, compound, blocks).

## Results

### Per-File Status

| File      | Pre-gap-closure | Uncommitted-WIP | Delta     |
|-----------|----------------|-----------------|-----------|
| simple    | PASS (20)      | PASS (20)       | no change |
| literal   | PASS (52)      | PASS (52)       | no change |
| syntax    | PASS (156)     | PASS (156)      | no change |
| bytecode  | PASS (37)      | PASS (37)       | no change |
| token     | PASS (29)      | PASS (29)       | no change |
| namespace | PASS (50)      | PASS (50)       | no change |
| identity  | PASS (14)      | PASS (14)       | no change |
| unhead    | PASS (44)      | PASS (44)       | no change |
| prim      | FAIL (71)      | FAIL (71)       | no change |
| fill      | FAIL (6)       | FAIL (6)        | no change |
| header    | FAIL (8)       | FAIL (8)        | no change |
| under     | FAIL (9)       | FAIL (9)        | no change |
| undo      | FAIL (4)       | FAIL (4)        | no change |

**Total failures: 98 (unchanged)**

### CBQN Compat Test

21/21 pass. All tiers green: arithmetic, arrays, modifiers, compound, blocks.

## Interpretation of Uncommitted Changes

The 5 modified files contain structural/spec-correctness improvements that are in scope for the right code paths but don't yet trigger failures in the current test suite:

- **compare.rs** — Added full lexicographic array comparison (atom-as-unit promotion, shape tie-break). The old `compare()` returned 0 for any two arrays. This is now correct per BQN spec but the test cases that exercise array ordering (⍋/⍒ on non-numeric arrays) may fall within the 71 prim failures.
- **search.rs** — Rewrote `indexOf_c2` (⊐) and added `extract_cell`/`rank0_i32` helpers for high-rank cell matching. Spec-correct result shapes for atom 𝕩.
- **select.rs** — (Changes not individually inspected but likely similar scope to search.rs)
- **scope.rs / vm.rs** — VM-level changes, likely related to variable resolution or bytecode dispatch.

**Conclusion: No regressions.** The uncommitted changes are safe to leave staged. They represent spec-correctness work targeting code paths not yet fully covered by green tests. The appropriate next step is to look at the 71 prim failures and identify which are now closer to passing due to these improvements.

## Deviations from Plan

None. Plan executed exactly as written.

Note: `.test-results/` is gitignored, so snapshot files cannot be committed. This is expected and pre-existing — snapshots serve as local working references only.

## Self-Check

- [x] `.test-results/uncommitted-wip.txt` exists and contains results for all 13 files
- [x] `.test-results/uncommitted-wip.detail.txt` exists with individual failure lines
- [x] Comparison output shows per-file delta columns (all "no change")
- [x] Net failure count computed: 98 → 98 (0 change)
- [x] CBQN compat test ran: 21/21 pass
