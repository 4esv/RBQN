---
phase: quick-4
plan: 1
type: execute
wave: 1
depends_on: []
files_modified: []
autonomous: true
requirements: []

must_haves:
  truths:
    - "Current test suite results are captured as a fresh baseline against uncommitted working tree"
    - "Regression points are identified by comparing current results against the pre-gap-closure snapshot"
    - "A clear report shows which test files regressed, improved, or stayed the same, with individual failure-level diffs"
  artifacts: []
  key_links: []
---

<objective>
Run the test suite scripts to establish current state and identify any regression points in the uncommitted working tree changes.

Purpose: There are 6 uncommitted file changes (compare.rs, search.rs, select.rs, scope.rs, vm.rs) that may have introduced regressions or improvements. We need to measure the delta against the last known baseline (pre-gap-closure, commit 48f4971, 98 total failures).

Output: Test results comparison showing per-file and per-test regression/improvement deltas.
</objective>

<execution_context>
@/Users/axel/.claude/get-shit-done/workflows/execute-plan.md
@/Users/axel/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@test_suite.sh
@test_cbqn_compat.sh
</context>

<tasks>

<task type="auto">
  <name>Task 1: Build and run full test suite, save as new snapshot</name>
  <files></files>
  <action>
1. Build RBQN in release mode with CBQN_PATH set:
   `CBQN_PATH=/Users/axel/Code/forks/CBQN cargo build --release --manifest-path=/Users/axel/Code/forks/RBQN/Cargo.toml`

2. Run the full test suite and save results as a named snapshot capturing current uncommitted state:
   `cd /Users/axel/Code/forks/RBQN && ./test_suite.sh --save uncommitted-wip`
   This runs all 13 test files (simple, literal, syntax, bytecode, token, namespace, identity, unhead, prim, fill, header, under, undo) and saves results to `.test-results/uncommitted-wip.txt` and `.test-results/uncommitted-wip.detail.txt`.

3. Capture the full terminal output showing per-file pass/fail counts.
  </action>
  <verify>
- `.test-results/uncommitted-wip.txt` exists and contains results for all 13 files
- `.test-results/uncommitted-wip.detail.txt` exists with individual failure lines
  </verify>
  <done>Fresh test results captured for current working tree state</done>
</task>

<task type="auto">
  <name>Task 2: Diff against pre-gap-closure baseline to identify regressions</name>
  <files></files>
  <action>
1. Compare the new snapshot against the pre-gap-closure baseline:
   `cd /Users/axel/Code/forks/RBQN && ./test_suite.sh --compare pre-gap-closure uncommitted-wip`
   This shows per-file delta (regressions/fixes) and individual test-level new failures vs fixed failures.

2. Also run the quick compat test to check basic arithmetic/array/modifier/compound/block tiers:
   `cd /Users/axel/Code/forks/RBQN && ./test_cbqn_compat.sh`

3. Summarize findings:
   - Which files improved (fewer failures)?
   - Which files regressed (more failures)?
   - Which files stayed the same?
   - List specific new regressions (test expressions that newly fail)
   - List specific fixes (test expressions that newly pass)
   - Net delta in total failures (was 98 at pre-gap-closure)
  </action>
  <verify>
- Comparison output shows per-file delta columns
- Individual regression/fix lines are listed
- Net failure count change is computed
  </verify>
  <done>Clear regression report showing exactly what improved and what regressed relative to the 48f4971 (98 failures) baseline, with specific test expressions identified</done>
</task>

</tasks>

<verification>
- Build succeeds without errors
- All 13 test files are exercised (no skips, no timeouts killing the run)
- Comparison output is parseable and shows meaningful deltas
- If header.bqn still causes stack overflow, note it but don't let it block other results
</verification>

<success_criteria>
- Test suite ran to completion on current uncommitted working tree
- Regression/improvement delta is quantified per file and per individual test
- User has actionable information about which changes helped and which hurt
</success_criteria>

<output>
After completion, create `.planning/quick/4-run-the-bisect-script-to-identify-regres/4-SUMMARY.md`
</output>
