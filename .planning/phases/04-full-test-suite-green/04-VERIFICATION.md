---
phase: 04-full-test-suite-green
verified: 2026-02-24T20:30:00Z
status: gaps_found
score: 4/13 test files at 0 failures (phase goal: all 13 at 0 failures)
gaps:
  - truth: "prim.bqn passes: all 44 primitive functions return spec-correct output"
    status: failed
    reason: "79 failures remain across deep pick, high-rank grade/sort/group, join edge cases, repeat, scan edge cases"
    artifacts:
      - path: "crates/rbqn-prim/src/structural.rs"
        issue: "Fill propagation exists but 7 nested-operation edge cases missing"
      - path: "crates/rbqn-prim/src/group.rs"
        issue: "Multi-dim group partial — high-rank cases and ¯1 exclusion incomplete"
    missing:
      - "Deep pick (⊑) with boxed index lists"
      - "High-rank grade (⍋/⍒) cell comparison"
      - "High-rank group (⊔) edge cases"
      - "Join (∾) atom args"
      - "Repeat (⍟) complex patterns"
      - "Scan edge cases (non-rank-1)"
      - "Progressive ⊒ (occurrence count) for high-rank"

  - truth: "fill.bqn passes with 0 failures"
    status: failed
    reason: "7 failures remain in fill propagation through nested operations (windows, transpose-of-windows, sort fills)"
    artifacts:
      - path: "crates/rbqn-prim/src/structural.rs"
        issue: "Fill propagation exists for reshape/transpose/shift/take/join but complex nested operations incomplete"
    missing:
      - "Fill through windows operation"
      - "Fill through sort (⍋/⍒ applied to fill-bearing arrays)"
      - "Fill propagation through nested take+transpose chains"

  - truth: "header.bqn passes with 0 failures"
    status: failed
    reason: "Stack overflow in modifier header dispatch kills the process; at least 9 visible failures before crash"
    artifacts:
      - path: "crates/rbqn-vm/src/compiler.rs"
        issue: "SETH dispatch for multi-header modifier blocks causes infinite recursion on retry"
    missing:
      - "Stack overflow fix for recursive modifier headers (2{⟨a,b⟩_r:a+b;a _r: a‿a _r})"
      - "Header literal number matching ({2:4;𝕩})"
      - "Destructuring in function header ({1‿b:b;𝕊:𝕩})"
      - "Modifier header with operand args ({4 3 _𝕣_ 2 1:...})"
      - "Multi-header dyadic block with 𝕨=𝕊"
      - "Predicate ? in immediate block"

  - truth: "under.bqn passes 64/64 (0 failures)"
    status: failed
    reason: "10 failures remain: structural inversion patterns involving ⊏⎉N, ↑⌾, ⊑⌾ not handled"
    artifacts:
      - path: "crates/rbqn-vm/src/modifiers.rs"
        issue: "try_structural_under covers <, ⊸↑, ⊸↓, ⊸⊏ but missing ⊏⎉N, ≍ solo, ⌽⌾(1↓4↑⊢) patterns"
    missing:
      - "Under with ⊏⎉N (select-at-rank)"
      - "Under with ≍ (solo)"
      - "F⌾(k⊸↑) where k from derived expression (not literal)"
      - "Structural inversion of 1↓4↑⊢ chain"

  - truth: "undo.bqn passes with 0 failures"
    status: failed
    reason: "4 failures remain: <⁼<, ∧˜⁼, √˜⁼, <⌜⁼∾<¨⁼ compound patterns"
    artifacts:
      - path: "crates/rbqn-vm/src/derive.rs"
        issue: "Compound inverse patterns not handled: unbox inverse on boxed arrays, √˜⁼ dyadic wrong"
    missing:
      - "Unbox inverse (<⁼<) on boxed arrays"
      - "√˜⁼ dyadic returning correct value (currently returns 65536 instead of 4)"
      - "∧˜⁼ swap inverse"
      - "Table/each inverse on chars (<⌜⁼∾<¨⁼)"

  - truth: "SYS-22..26 system stubs exist (•FFI, •bit, •term, •ns, •HashMap)"
    status: failed
    reason: "No system function stubs were implemented — sysfn.rs has no entries for these"
    artifacts:
      - path: "crates/rbqn-prim/src/sysfn.rs"
        issue: "No •FFI, •bit, •term, •ns, •HashMap stubs exist"
    missing:
      - "•FFI stub (throw not-supported)"
      - "•bit namespace with _and_, _or_, _xor_, _not"
      - "•term namespace with RawMode, CharB, Flush"
      - "•ns namespace with Keys, Values, Has, Get"
      - "•HashMap constructor with Get, Has, Keys"
---

# Phase 4: Full Test Suite Green — Verification Report

**Phase Goal:** All 13 official BQN test files pass with 0 failures
**Verified:** 2026-02-24T20:30:00Z
**Status:** GAPS FOUND
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | prim.bqn passes: all 44 primitive functions spec-correct | FAILED | 79 failures (485/564) |
| 2 | fill.bqn passes with 0 failures | FAILED | 7 failures (55/62) |
| 3 | identity.bqn passes with 0 failures | VERIFIED | 14/14, all passed |
| 4 | under.bqn passes 64/64 (0 failures) | FAILED | 10 failures (54/64) |
| 5 | header.bqn passes with 0 failures | FAILED | Stack overflow + ≥9 visible failures |
| 6 | unhead.bqn passes 44/44 (0 failures) | VERIFIED | 44/44, all passed |
| 7 | namespace.bqn passes 50/50 (0 failures) | VERIFIED | 50/50, all passed |
| 8 | undo.bqn passes with 0 failures | FAILED | 4 failures (13/17) |
| 9 | token.bqn passes 29/29 (0 failures) | VERIFIED | 29/29, all passed |
| 10 | simple/literal/syntax/bytecode pass 100% | VERIFIED | 294/294, all passed |
| 11 | All 13 test files report 0 failures | FAILED | Only 8/13 files at 0 failures |
| 12 | Test harness exits 0 | FAILED | header.bqn causes stack overflow (non-zero exit) |
| 13 | SYS-22..26 system stubs exist | FAILED | No stubs implemented |

**Score:** 4/13 truths fully verified (8 files pass, 5 files still failing)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rbqn-vm/src/compiler.rs` | NSDesc from body_arr[2]/[3] | VERIFIED | `build_ns_desc` function exists and is wired |
| `crates/rbqn-vm/src/namespace.rs` | str2gid, NSDesc struct | VERIFIED | Both present and functional |
| `crates/rbqn-vm/src/derive.rs` | InvBlock + ScanInv DerivedKind | VERIFIED | Both variants present with c1/c2 dispatch |
| `crates/rbqn-vm/src/block.rs` | inv_x_body, inv_w_body fields | VERIFIED | Both fields exist in Body struct |
| `crates/rbqn-vm/src/modifiers.rs` | Under dispatch + table scalar | VERIFIED | try_structural_under exists; partial coverage |
| `crates/rbqn-prim/src/structural.rs` | Fill propagation for structural ops | PARTIAL | Fill set in reshape/transpose/shift but 7 edge cases missing |
| `crates/rbqn-prim/src/group.rs` | Multi-dimensional group | PARTIAL | group_indices_multidim exists but high-rank edge cases broken |
| `crates/rbqn-prim/src/slash.rs` | Slash on atoms | VERIFIED | Atom right args and empty left handled |
| `crates/rbqn-prim/src/sysfn.rs` | SYS-22..26 stubs | MISSING | No •FFI, •bit, •term, •ns, •HashMap entries exist |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `compiler.rs` | `namespace.rs` | str2gid in build_ns_desc | WIRED | `crate::namespace::str2gid` called at lines 68, 127, 519, 546 |
| `compiler.rs` | `vm.rs` | ns_desc consumed at RETD | WIRED | ns_desc field set in Body, consumed at RETD opcode |
| `derive.rs` | `block.rs` | inv_x_body checked before BQN runtime | WIRED | Lines 178, 223, 325 check inv_x_body |
| `derive.rs` | `vm.rs` | InvBlock c1 calls exec_block_with_body | WIRED | DerivedKind::InvBlock dispatched at line 787 |
| `modifiers.rs` | `derive.rs` | Under dispatch calls try_structural_under | WIRED | try_structural_under called from under_c1 at line 1088 |
| `sysfn.rs` | `derive.rs` | sys_idx dispatch for new system stubs | NOT_WIRED | No SYS-22..26 entries registered anywhere |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| TEST-05 | 04-02, 04-04, 04-05 | Pass prim.bqn | BLOCKED | 79 failures remain (485/564) |
| TEST-06 | 04-05 | Pass token.bqn | SATISFIED | 29/29 all passed |
| TEST-07 | 04-01 | Pass header.bqn | BLOCKED | Stack overflow + ≥9 failures |
| TEST-08 | 04-03 | Pass unhead.bqn | SATISFIED | 44/44 all passed |
| TEST-09 | 04-01 | Pass namespace.bqn | SATISFIED | 50/50 all passed |
| TEST-10 | 04-04 | Pass fill.bqn | BLOCKED | 7 failures remain (55/62) |
| TEST-11 | 04-04 | Pass identity.bqn | SATISFIED | 14/14 all passed |
| TEST-12 | 04-05 | Pass under.bqn | BLOCKED | 10 failures remain (54/64) |
| TEST-13 | 04-03 | Pass undo.bqn | BLOCKED | 4 failures remain (13/17) |
| SYS-22 | 04-05 | •FFI stub | BLOCKED | Not implemented |
| SYS-23 | 04-05 | •bit namespace | BLOCKED | Not implemented |
| SYS-24 | 04-05 | •term namespace | BLOCKED | Not implemented |
| SYS-25 | 04-05 | •ns namespace | BLOCKED | Not implemented |
| SYS-26 | 04-05 | •HashMap | BLOCKED | Not implemented |

Requirements satisfied: TEST-06, TEST-08, TEST-09, TEST-11 (4/14)
Requirements blocked: TEST-05, TEST-07, TEST-10, TEST-12, TEST-13, SYS-22..26 (10/14)

### Anti-Patterns Found

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| `04-05-SUMMARY.md` | "System Stubs (Not Started)" explicitly documents non-implementation | Blocker | SYS-22..26 requirements unmet |
| `header.bqn` test run | Stack overflow — `thread 'main' has overflowed its stack` | Blocker | Process kills with non-zero exit; masks true failure count |
| `04-04-SUMMARY.md` | "Did not achieve fill.bqn 0 failures" and "Did not achieve prim.bqn ≤20" | Blocker | Plan's own targets missed |
| `04-05-SUMMARY.md` | "Plan targeted 0 failures ... Achieved 0 in 8/13 files" | Blocker | Phase goal explicitly unmet |

### Human Verification Required

None — all failures are quantifiable via test output. The stack overflow in header.bqn is programmatically detectable.

### Gaps Summary

Phase 4 made substantial progress — 8 of 13 test files now pass at 100%, up from an estimated 5-6 at phase start. However, the phase goal of "all 13 test files at 0 failures" was not achieved.

**Root causes of remaining failures:**

1. **prim.bqn (79 failures):** The four targeted clusters (table, group, slash, reshape) were only partially fixed. The actual problem is deeper: deep pick with boxed index lists, high-rank grade/sort, high-rank group outer-product semantics, and complex prim interactions were not implemented. Plan 04-02 fixed only 11 of the planned 40+ failures.

2. **fill.bqn (7 failures):** Fill propagation infrastructure was added, but 7 complex nested-operation edge cases (windows fills, sort fills, transpose-of-windows fills) were explicitly left as "too complex" in the plan 04-04 deviation notes.

3. **header.bqn (≥9 failures + stack overflow):** Stack overflow from recursive modifier headers is a hard crash, not a controlled test failure. It prevents the harness from completing the file. The SETH/body dispatch infinite recursion bug was acknowledged but deferred in plan 04-01 and never addressed in 04-05.

4. **under.bqn (10 failures):** The structural under patterns for ⊏⎉N (select-at-rank), ≍ solo, and complex chained structural inverses were not implemented.

5. **undo.bqn (4 failures):** Compound inverse patterns (<⁼<, ∧˜⁼, √˜⁼ dyadic) remain broken.

6. **SYS-22..26:** All five system function stubs were explicitly deferred in plan 04-05 with the note "not tested by official suite." They remain entirely absent from the codebase.

**Files that do pass (achieved):** simple, literal, syntax, bytecode, token, namespace, identity, unhead — all 100%.

---

_Verified: 2026-02-24T20:30:00Z_
_Verifier: Claude (gsd-verifier)_
