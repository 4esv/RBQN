---
phase: quick-02
plan: 01
subsystem: rbqn-vm
tags: [tracing, debugging, primitives, compiler]
key-files:
  modified:
    - crates/rbqn-vm/src/vm.rs
    - crates/rbqn-vm/src/derive.rs
decisions:
  - "Use LazyLock<bool> for env-var gate so the check is done once at startup with zero per-call cost"
  - "PRIM_GLYPHS const string for glyph lookup avoids calling get_runtime() in trace hot path"
  - "Use eprintln! (not vm_trace_push) for PRIM_TRACE output — immediate stderr flush, no ring-buffer loss"
  - "Made DERIVED_STORE pub so vm.rs fmt helpers can read derived kinds for fun/md display"
metrics:
  duration: "~15 minutes"
  completed: "2026-02-23"
  tasks: 2
  files: 2
---

# Quick Task 2: Add Tracing to Compiler-Used Primitives — Summary

**One-liner:** Env-var-gated `[PRIM/SYS/BLOCK c1/c2]` traces via eprintln to stderr, showing BQN glyph, input values/shapes, and output for every primitive dispatch.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add trace formatting helpers and env-var gate to vm.rs | 16dfae1 | crates/rbqn-vm/src/vm.rs, crates/rbqn-vm/src/derive.rs |
| 2 | Add result tracing to c1/c2 NativeFn and SysFn paths | 386b409 | crates/rbqn-vm/src/derive.rs |

## What Was Built

### vm.rs additions

- `PRIM_TRACE_ENABLED: LazyLock<bool>` — checks `RBQN_PRIM_TRACE` env var once at startup
- `prim_trace_enabled() -> bool` — public accessor, branch-predicted to false when unset
- `fmt_b_short(b: B) -> String` — concise formatting: numbers, chars, `arr([shape] ElType [first3])`, `fun(prim=N)`, `·`
- `fmt_b_detail(b: B) -> String` — verbose formatting: up to 10 elements, char arrays as quoted strings
- `fmt_b_scalar(b: B) -> String` — private helper, non-recursive element display

### derive.rs additions

- `PRIM_GLYPHS` const — all 64 BQN primitive glyphs in fruntime order
- `DERIVED_STORE` made pub — allows vm.rs helpers to inspect derived kinds
- `[PRIM c1]` / `[PRIM c2]` traces — NativeFn paths log glyph, args before call, result after
- `[SYS c1]` / `[SYS c2]` traces — SysFn paths log sys index, args, result
- `[BLOCK c1]` / `[BLOCK c2]` traces — FunBlock paths log block id, arg, result

## Usage

```bash
# See all primitive traces
CBQN_PATH=/path/to/CBQN RBQN_PRIM_TRACE=1 cargo run -- -e "1+1" 2>trace.log

# Find the < operation that crashes
grep '\[PRIM c2\].*<' trace.log

# Find GroupLen calls (sys=22)
grep '\[SYS\].*sys=22' trace.log

# Run without tracing (zero overhead)
CBQN_PATH=/path/to/CBQN cargo run -- -e "1+1"
```

## Sample Trace Output

```
[PRIM c1] < x='∾'
[PRIM c1] < -> arr([] B ['∾'])
[PRIM c2] w='+' = x='∾' (prim_idx=15)
[PRIM c2] = -> 0
[SYS c1] sys=22 x=arr([17] I8 [9,11,14,...])
[SYS c1] sys=22 -> arr([37] I32 [0,0,0,0,0,0,0,0,0,1,...])
[BLOCK c1] id=137 x=arr([28] B ['+',0,'-',...])
[BLOCK c1] id=137 -> arr([2] B [arr(...),arr(...)])
```

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

- `crates/rbqn-vm/src/vm.rs` — modified, verified
- `crates/rbqn-vm/src/derive.rs` — modified, verified
- Commit 16dfae1 — exists (`feat(quick-02): add prim_trace_enabled, fmt_b_short, fmt_b_detail`)
- Commit 386b409 — exists (`feat(quick-02): add RBQN_PRIM_TRACE tracing to all c1/c2 dispatch paths`)
- `cargo check -p rbqn-vm` — passes with only pre-existing warnings
- `RBQN_PRIM_TRACE=1 cargo run -- -e "1+1"` — shows `[PRIM/SYS/BLOCK]` trace lines
- Without `RBQN_PRIM_TRACE` — no trace lines appear
