---
phase: 04-full-test-suite-green
plan: 07
status: complete
started: 2026-02-27
completed: 2026-02-27
---

## Summary

Reduced prim.bqn from 49 to 9 failures and fill.bqn from 7 to 3 failures through extensive fixes across all primitive clusters.

### Key Changes
- **Deep pick**: Recursive path traversal for boxed index lists in select.rs
- **Search rewrite**: High-rank cell comparison with rank promotion in search.rs
- **Grade/Sort**: Boxed array comparison, mixed type ordering in sort.rs
- **Cells/Insert**: Rank-0 cell semantics, insert fold between rank-0 cells
- **Join**: Mixed-rank array joining with rank promotion
- **Transpose**: Empty permutation identity, computed permutation
- **Slash**: Multi-axis replication with empty dimensions
- **Group**: Scalar dims, multi-axis cell shape, high-rank w
- **Take/Drop**: Multi-axis with more axes than rank, enclosed args
- **Validation**: Should-fail cases for pick, slash, join

### Results
- prim.bqn: 49→9 (-40)
- fill.bqn: 7→3 (-4)

### Remaining prim.bqn failures (9)
- 1 cells-on-atom (bootstrap dependency)
- 4 group edge cases (multi-dim with drop, eval failures)
- 2 grade on complex boxed/scalar data
- 1 prefixes/take interaction
- 1 deep pick with depth modifier

## Self-Check: PASSED (partial — 9 prim + 3 fill remain)
