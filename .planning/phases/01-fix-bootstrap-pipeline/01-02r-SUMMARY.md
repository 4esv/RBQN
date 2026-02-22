---
phase: "01"
plan: "02r"
subsystem: primitives
tags: [join, rank, structural]
key-files:
  modified:
    - crates/rbqn-prim/src/structural.rs
decisions:
  - "Promote lower-rank arg by prepending 1 to shape, matching BQN spec for dyadic join"
metrics:
  completed: "2026-02-22"
---

# Phase 01 Plan 02r: Fix Dyadic Join Rank Handling Summary

Implemented BQN-compliant rank promotion in dyadic `∾` (join to). Previously required equal ranks, now handles rank difference of 1 by treating the lower-rank array as a single cell (prepending 1 to its shape).

## Changes

### `crates/rbqn-prim/src/structural.rs` - `join_to_c2`

**Before:** Multi-rank path required `warr.rank() == xarr.rank()`, erroring on any mismatch.

**After:** Three cases for the array-array branch:
1. Both rank 1: simple vector concatenation (fast path)
2. Equal ranks: concatenate along first axis with trailing shape check
3. Rank differs by 1: promote lower-rank arg by prepending `[1]` to shape, then concatenate

Also improved atom-array branches: replaced `Nyi` errors with proper `Rank` errors for atom + rank>1 cases.

## Verification

- Build succeeds
- Compiler compilation now passes the `∾` rank error that was blocking it
- The next error encountered is a different bug (shape mismatch in `×` arithmetic), confirming the join fix unblocked further progress

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | 5447095 | fix(01-02): handle rank-differs-by-1 in dyadic join |

## Self-Check: PASSED
