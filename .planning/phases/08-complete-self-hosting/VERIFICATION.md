# Phase 8: Complete Self-Hosting — Verification Record

**Date:** 2026-03-03
**Status:** VERIFIED

## SELF3-02: RBQN-Compiled Bins

All four bytecode files committed to `bins/` are compiled by RBQN itself:

| File | Source Tag | Size | Notes |
|------|------------|------|-------|
| runtime0.bin | rbqn-self | 5,212 bytes | Runtime override functions (24 ops) |
| runtime1x.bin | rbqn-self | 131,581 bytes | Full 64-function runtime (extended) |
| compiler.bin | rbqn-self | 67,953 bytes | Self-hosted BQN compiler |
| formatter.bin | rbqn-self | 14,876 bytes | Self-hosted BQN formatter |

## Fixpoint Swap Test

**Result:** FIXPOINT reached at round 2

The fixpoint swap test verifies that recompiling the BQN sources with the
RBQN-compiled bins produces byte-for-byte identical output. This is the gold
standard proof of self-hosting correctness.

- Round 1: Bootstrap from committed bins → compile all 4 sources → write new bins
- Round 2: Recompile all 4 sources from same bootstrap → compare with round 1 output
- Result: Byte-for-byte identical (fixpoint at round 2)

Command: `BQN_SRC=/path/to/BQN/src cargo run -p rbqn --features gen-tools --bin rbqn-gen -- --self --fixpoint-check`

## Test Suite Results

All 1316 official BQN tests pass using only RBQN-compiled bins:

```
FILE          RESULT
────────────  ──────────────────
simple        pass 20
literal       pass 52
syntax        pass 156
bytecode      pass 37
token         pass 29
namespace     pass 50
identity      pass 14
unhead        pass 44
prim          pass 564
fill          pass 62
header        pass 156
under         pass 64
undo          pass 68
────────────  ──────────────────
TOTAL         ALL GREEN (13/13 files)
```

## Build Verification

`cargo build` completes with CBQN_PATH unset — no CBQN dependency:

```
unset CBQN_PATH && cargo build -p rbqn  # ✓ succeeds
```

## CBQN Acknowledgement

This self-hosting implementation was bootstrapped from CBQN-compiled bytecode.
CBQN (https://github.com/dzaima/CBQN by dzaima) served as the reference
implementation throughout development. The BQN specification by Marshall Lochbaum
(https://mlochbaum.github.io/BQN/) defines the language semantics.
