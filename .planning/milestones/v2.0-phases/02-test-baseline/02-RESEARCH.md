# Phase 2: Test Baseline - Research

**Researched:** 2026-02-23
**Domain:** BQN test harness integration, variable resolution bugs, file system syscalls
**Confidence:** HIGH — findings from direct codebase inspection and live binary testing

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TEST-01 | Pass simple.bqn | 20 tests: basic arithmetic (÷, √, ⌊, |, ¬, ∧, ∨), variables (←), blocks ({𝕩}), lexical scope. Variable assignment (`a←5⋄a`) currently broken — Phase 1 blocker. |
| TEST-02 | Pass literal.bqn | 44 tests: number literal parsing (π, ∞, e-notation, ¯ prefix), char literals ('a'), string literals ("ab"), escape sequences. Also 25 error-expected tests (!). Tests the TOKENIZER, not runtime. |
| TEST-03 | Pass syntax.bqn | 156 tests: trains (×˜, ⊸, ○, ⍟), define (←), modify (↩), lists (‿, ⟨⟩), blocks with headers, Nothing (·). 88 error-expected tests. Tests the COMPILER output. |
| TEST-04 | Pass bytecode.bqn | 37 tests: pure VM opcode semantics — PUSH, SETN, VARO, VARM, FN1C, FN2C, DFND, LSTO, LSTM, FN1O, FN2O, MD1C, MD2C, TR2D, TR3D, TR3O, SETM, SETC — no primitives. |

</phase_requirements>

---

## Summary

Phase 2 has two separate work streams: (1) wire the official BQN test harness so it runs against RBQN and reports pass/fail counts without crashing, and (2) fix whatever failures the four target test files expose until all pass.

The official test harness is `/Users/axel/Code/forks/BQN/test/this.bqn`. It uses: `•args`, `•BQN`, `•file.Lines`, `•file.List`, `•Out`, `•Repr`, and the `⎊` (catch) modifier. Of these, `•file.Lines` and `•file.List` are completely unimplemented in RBQN. All other harness dependencies are already implemented and working. Since `•file.Lines` and `•file.List` partially overlap with Phase 3's SYS-06/07 requirements, implementing them in Phase 2 is the correct move — they are blocking the harness.

The four target test files use no system functions themselves. Their failures will come from (a) remaining Phase 1 compiler bugs (variable assignment is currently broken), and (b) specific VM or primitive correctness issues that Phase 1 doesn't cover. The most critical pre-condition is Phase 1 completion: `a←5⋄a` must work before ANY tests in `simple.bqn` or `bytecode.bqn` can pass.

**Primary recommendation:** Phase 2 = [wire harness] + [fix variable resolution] + [iterate on test failures]. Do NOT start Phase 2 until Phase 1 is verified complete with the success criteria from ROADMAP.md.

---

## Current Binary State (2026-02-23)

Verified by running the current `target/debug/rbqn` binary with `CBQN_PATH` set:

| Expression | Expected | Actual | Status |
|-----------|----------|--------|--------|
| `1+1` | `2` | `2` | PASS |
| `¬15÷20` | `0.25` | `15` | FAIL (wrong result) |
| `√5` | `2.2360...` | panics | FAIL |
| `a←5⋄a` | `5` | Assertion error: Undefined identifier | FAIL |
| `a‿b←7‿2⋄a` | `7` | Assertion error: Undefined identifier | FAIL |
| `{𝕩}3` | `3` | `3` | PASS |
| `{a←1⋄{a←2}⋄a}` | `1` | Assertion error: Undefined identifier | FAIL |
| `4{𝔽}` | `4` | `4` | PASS |
| `a←3⋄a{𝕩}↩8⋄a` | `8` | Assertion error: Undefined identifier | FAIL |

**These are Phase 1 failures, not Phase 2 work.** Phase 2 cannot start until Phase 1 is complete.

---

## Standard Stack

### Core

| Component | Version | Purpose | Why Standard |
|-----------|---------|---------|--------------|
| `/Users/axel/Code/forks/BQN/test/this.bqn` | Current | Official BQN test harness | The authoritative pass/fail oracle; "wiring the test harness" means running this file |
| `/Users/axel/Code/forks/BQN/test/cases/*.bqn` | Current | 13 BQN test case files | Official spec test files — 4 of them are Phase 2 targets |
| `CBQN_PATH=/Users/axel/Code/forks/CBQN cargo run` | Current | RBQN build and run | Required for bootstrap bytecode |
| `#[test]` / `cargo test` | Rust built-in | Regression tests for fixes | Already established pattern from Phase 1 |

### Test Case File Inventory

| File | Total Tests | Error Cases (!) | Notes |
|------|-------------|-----------------|-------|
| `simple.bqn` | 20 | 0 | Basic arithmetic + variables + blocks |
| `literal.bqn` | 44 | 25 | Number/char/string literal parsing |
| `syntax.bqn` | 156 | 88 | Trains, define, modify, lists, blocks, Nothing |
| `bytecode.bqn` | 37 | 0 | Pure VM opcode tests, no primitives |
| All 13 files | ~450 | ~200 | Phase 2 harness must report on all without crashing |

### Test Format

The test format is simple: lines from the cases files are one of:
- `expected_val % code` — evaluate `code`, compare result to `expected_val`
- `! % code` — evaluate `code`, expect it to throw an error
- `bqn_expression` (no %) — evaluate the expression, expect it to return truthy (1)
- Comment lines (`#...`) and blank lines are ignored

The harness reads files via `•file.Lines`, strips comments, runs each via `•BQN`, compares using `≡` match.

---

## Architecture Patterns

### Work Stream 1: Wire the Harness

The harness `this.bqn` is run as: `rbqn test/this.bqn` from the BQN repo root, or `rbqn test/this.bqn simple literal syntax bytecode` to run only the 4 target files.

The harness uses `⎊` (catch) to catch failures — this is already implemented in RBQN (`modifiers.rs:1194`).

**Only two missing pieces for harness operation:**

#### •file.Lines implementation

`•file.Lines path` → reads the file at `path`, returns a rank-1 array of strings (one per line, newline stripped).

The harness calls it as: `•file.Lines "cases/simple.bqn"` — a relative path. RBQN must resolve this relative to the current working directory (not the script's directory, since the harness itself doesn't use relative-to-script paths for this).

```rust
// In dispatch_sys_c1 (idx 50 or similar new idx):
// x is a B char array (the path string)
// Returns: rank-1 B array of rank-1 char arrays (array of strings)
fn file_lines_c1(x: B) -> B {
    let path = b_to_string(x);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| rbqn_core::error::throw(format!("•file.Lines: {e}")));
    let lines: Vec<B> = content.lines()
        .map(|line| {
            let chars: Vec<u32> = line.chars().map(|c| c as u32).collect();
            crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
        })
        .collect();
    crate::vm::b_vec_to_arr(lines)
}
```

#### •file.List implementation

`•file.List path` → lists directory entries, returns rank-1 array of filename strings (basenames only, sorted).

```rust
fn file_list_c1(x: B) -> B {
    let path = b_to_string(x);
    let mut entries: Vec<String> = std::fs::read_dir(&path)
        .unwrap_or_else(|e| rbqn_core::error::throw(format!("•file.List: {e}")))
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    entries.sort();
    let result: Vec<B> = entries.iter().map(|name| {
        let chars: Vec<u32> = name.chars().map(|c| c as u32).collect();
        crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
    }).collect();
    crate::vm::b_vec_to_arr(result)
}
```

#### •file as a namespace

In CBQN, `•file` is a namespace object. RBQN must implement this.

The compiler generates:
1. `SYSV <file_idx>` → resolves to a namespace B value at runtime
2. `FLDO <lines_gid>` → looks up the `lines` field on the namespace

The simplest approach: when sys_name_to_b("file") is called, build a synthetic NS object:

```rust
fn make_file_namespace() -> B {
    use crate::namespace::{NS, NSDesc, store_ns, str2gid};
    use crate::scope::Scope;
    use std::sync::Arc;

    let lines_gid = str2gid("lines");
    let list_gid  = str2gid("list");
    // Add more fields as needed (at, chars, bytes, etc.)

    let desc = Arc::new(NSDesc {
        var_am: 2,
        exp_gids: vec![lines_gid, list_gid],
    });

    // Create a scope with the functions in the right slots
    let lines_fn = m_sys_fn(FILE_LINES_IDX);  // new sys_idx
    let list_fn  = m_sys_fn(FILE_LIST_IDX);   // new sys_idx

    // NOTE: This requires a Scope that holds these values
    // The easiest approach: use a dedicated Body with no bytecode
    // and pre-populate the vars slot with the functions
    let body = Body { ... };  // placeholder empty body
    let sc = Arc::new(Scope::new(body, None, 2, &[lines_fn, list_fn]));

    store_ns(NS { desc, sc })
}
```

**Alternative (simpler):** Add `"file"` to sys_name_to_b with a lazy-initialized static B value. The FLDO opcode then dispatches on NSP tag to the NS's scope. This is the cleanest approach given the existing NS infrastructure.

### Work Stream 2: Fix Test Failures

After Phase 1 is complete and the harness runs, fix failures in this order:

1. **simple.bqn** (20 tests, 0 error cases) — lowest hanging fruit; exercises `÷`, `√`, `¬`, `∧`, `∨`, blocks with lexical scoping. If Phase 1 is really complete, this should pass almost entirely.

2. **bytecode.bqn** (37 tests, 0 error cases) — pure VM opcode tests, no primitives. All failures are VM correctness issues.

3. **literal.bqn** (44 tests, 25 error cases) — number/literal parsing. Error cases need `⎊` to work correctly. The 25 `!` cases test that invalid literals throw.

4. **syntax.bqn** (156 tests, 88 error cases) — the hardest. Tests trains (×˜, ⊸, ○, ⍟), define/modify, Nothing (·), lists, blocks with headers. The 88 error cases all need correct error propagation.

### Test Iteration Pattern

```bash
# Run the official harness on the 4 target files:
cd /Users/axel/Code/forks/BQN
CBQN_PATH=/path/to/CBQN /path/to/rbqn test/this.bqn simple literal syntax bytecode

# Run all 13 files (for pass/fail count criterion):
CBQN_PATH=/path/to/CBQN /path/to/rbqn test/this.bqn

# Debug a single failing test:
CBQN_PATH=/path/to/CBQN /path/to/rbqn -p 'a←5⋄a'

# Check what CBQN produces for the same expression:
/path/to/CBQN/BQN -p 'a←5⋄a'
```

### Regression Test Pattern

Keep Phase 1's integration test pattern. Add tests for each fixed case in `tests/` to prevent regressions. Run with `--test-threads=1` due to global store singletons.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Test case parser | Custom Rust parser for `.bqn` test format | Run `this.bqn` directly | The harness already handles the format; a separate parser creates a maintenance burden |
| File path resolution | Custom relative path logic | `std::path::Path::new(path)` | Already handles `.` and relative paths correctly |
| Namespace construction | New NS representation | Existing `store_ns(NS { desc, sc })` infrastructure | NS infrastructure already handles FLDO lookup correctly |
| BQN repr comparison | Custom value comparator | `≡` (match) used by the harness in BQN | The harness compares via BQN's own `≡`, so RBQN's implementation of `≡` must be correct |

---

## Common Pitfalls

### Pitfall 1: Starting Phase 2 Before Phase 1 Is Complete

**What goes wrong:** The test harness will run but `simple.bqn` will have ~15/20 failures (including all test cases that use variable assignment). The failures look like Phase 2 bugs but are actually Phase 1 bugs.

**How to avoid:** Gate Phase 2 on ALL Phase 1 success criteria from ROADMAP.md:
1. `rbqn -e '1+1'` outputs `2` ← already works
2. `rbqn -e '+´1‿2‿3'` outputs `6` ← UNKNOWN, needs verification
3. `•PrimInd "+"` returns `0` ← UNKNOWN
4. REPL variable persistence ← UNKNOWN
5. `a←5` accessible on next REPL line ← must mean variable assignment works globally

**Warning sign:** If `rbqn -p 'a←5⋄a'` outputs anything other than `5`, Phase 1 is not complete.

### Pitfall 2: •file Namespace Path Resolution

**What goes wrong:** The harness runs from the BQN repo's `test/` directory and calls `•file.Lines "cases/simple.bqn"`. If RBQN resolves this relative to the binary location or the RBQN source directory, it will look in the wrong place.

**How to avoid:** Use the *current working directory* for relative path resolution. `std::fs::read_dir(path)` and `std::fs::read_to_string(path)` already do this correctly — don't add any path prefix.

**CBQN note:** In CBQN, `•file` carries a "base path" that functions like `•path` for relative resolution. For Phase 2, always-use-cwd is correct because the harness is called from the BQN repo root.

### Pitfall 3: •Repr Must Produce BQN-Valid Output

**What goes wrong:** The harness uses `•Repr≠c` to print the test count. If `•Repr` returns a wrong format (e.g., `"20"` instead of `20`), the output line will be malformed. The harness also uses `•Repr⎊"{…}"` to format test results.

**How to avoid:** `•Repr` is already implemented in derive.rs. It calls the formatter's repr function, falling back to `format_b_repr`. Verify: `rbqn -p '•Repr 42'` should output `42`, not `"42"`. Numbers should not be quoted.

### Pitfall 4: Error Cases Need Correct Error Propagation

**What goes wrong:** `literal.bqn` and `syntax.bqn` have many `!` tests (25 and 88 respectively). These expect the BQN code to THROW an error. The harness catches it with `Exec⎊0` — if RBQN returns 0 instead of throwing, the test sees "evaluation failed" as the result and reports the test as PASS. But if RBQN throws the wrong error (or panics in an uncaught way), the test may also misbehave.

**How to avoid:** Verify `⎊` (catch) works correctly: `rbqn -p '"should error"⊸!⎊1 _12'` should return `1`. The `_12` is an invalid token that should throw; if `⎊` catches it, the result is `1`.

**Known status:** `⎊` is implemented in `modifiers.rs:1194` via `catch_unwind`. This should work if panics correctly propagate through `catch_unwind`. The key concern: nested `catch_unwind` calls are fine in Rust.

### Pitfall 5: •file.List Sort Order

**What goes wrong:** The harness calls `•file.List "cases"` and iterates the results. If the order is wrong, tests run in a different order (not a correctness issue but can confuse debugging).

**How to avoid:** Sort the directory listing alphabetically. `std::fs::read_dir` returns in arbitrary order on most filesystems. Always sort before returning.

### Pitfall 6: syntax.bqn Tests That Use Trains

**What goes wrong:** `syntax.bqn` includes `16 % 3(2×+⍟(1+1))2` which uses `⍟` (repeat/power modifier). If `⍟` is not correctly implemented for non-integer counts, this test fails.

**How to avoid:** Check each syntax.bqn expression against CBQN output before claiming the test passes. Use `CBQN/BQN -p '3(2×+⍟(1+1))2'` to get expected output.

---

## Code Examples

### Test Harness Invocation

```bash
# From the BQN repo test directory, run this.bqn against RBQN:
cd /Users/axel/Code/forks/BQN
CBQN_PATH=/Users/axel/Code/forks/CBQN /path/to/rbqn test/this.bqn simple literal syntax bytecode
# Expected output: "Running N tests: simple literal syntax bytecode\nAll passed!"
# Or: "Running N tests: simple literal syntax bytecode\nX failed!"

# Run all 13 files:
CBQN_PATH=/Users/axel/Code/forks/CBQN /path/to/rbqn test/this.bqn
```

### Adding •file to the sys_name_to_b Table

**File:** `crates/rbqn-vm/src/derive.rs`

```rust
// In sys_name_to_b():
"file" => make_file_namespace(),

// New function (could live in a file_sys.rs module):
static FILE_NS: std::sync::LazyLock<std::sync::Mutex<Option<B>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

fn make_file_namespace() -> B {
    let mut guard = FILE_NS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(cached) = *guard {
        return cached;
    }
    // Build the namespace with Lines and List
    let ns_b = build_file_ns();
    *guard = Some(ns_b);
    ns_b
}
```

### •file.Lines Implementation

**File:** `crates/rbqn-vm/src/derive.rs` (or new `file_sys.rs`)

```rust
fn file_lines_c1(path_b: B) -> B {
    let path = b_to_string(path_b);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| rbqn_core::error::throw(format!("•file.Lines: cannot read {path}: {e}")));
    let lines: Vec<B> = content.lines()
        .map(|line| {
            let chars: Vec<u32> = line.chars().map(|c| c as u32).collect();
            crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
        })
        .collect();
    crate::vm::b_vec_to_arr(lines)
}
```

### •file.List Implementation

```rust
fn file_list_c1(path_b: B) -> B {
    let path = b_to_string(path_b);
    let mut entries: Vec<String> = std::fs::read_dir(&path)
        .unwrap_or_else(|e| rbqn_core::error::throw(format!("•file.List: cannot list {path}: {e}")))
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    entries.sort();
    let result: Vec<B> = entries.iter().map(|name| {
        let chars: Vec<u32> = name.chars().map(|c| c as u32).collect();
        crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
    }).collect();
    crate::vm::b_vec_to_arr(result)
}
```

### Test Case Breakdown — What Each File Exercises

**simple.bqn** (20 tests):
- Arithmetic: `+`, `-`, `×`, `÷`, `⌊`, `√`, `|` (monad)
- Logic: `¬`, `∧`, `∨`
- Identity: `⊢`, `⊣`
- Variables: `←` (define), `,` (separator), `⋄` (separator)
- Blocks: `{𝕩}` (lambda), nested blocks with lexical scope
- Train use: `(÷2)+(÷3)+(÷6)` uses fork semantics implicitly
- **Key test:** `{a←1⋄{a←2}⋄a}` tests that inner block does NOT affect outer scope → requires correct EXTO/EXTM or variable depth resolution

**bytecode.bqn** (37 tests):
- PUSH/RETN/POPS: basic stack ops
- SETN/SETU/VARO/VARM: variable set/get
- FN1C/FN2C: function calls
- DFND: block definition
- LSTO/LSTM: list construction and multi-assign
- NOTM: nothing (compiled to ADDU SENTINEL by compiler.rs)
- FN1O/FN2O: optional-dispatch calls
- MD1C/MD2C: modifier application
- TR2D/TR3D: 2-train and 3-train (atop, fork)
- TR3O: 3-train with optional
- SETM/SETC: modify-assign `↩` forms
- **Key test:** `a‿b←2‿1⋄a‿b{𝕩‿𝕨}↩4⋄a` — multi-assign SETM

**literal.bqn** (44 tests, 25 error cases):
- Numbers: `0`, `12`, `12_3`, `¯12`, `1.2e1`, `π`, `∞`, `πe4`
- Characters: `' '`, `'"'`, `'𝕩'`
- Strings: `"ab"`, `"a""""b"`, escape sequences
- Error cases: `_12` (invalid), `5.` (invalid), `.5` (invalid), `¯` (bare), `4¯2` (two minuses)
- Uses `≡` (match) for comparison: `'a'‿'b' ≡ "ab"` — this is a BQN expression that returns 1 if equal

**syntax.bqn** (156 tests, 88 error cases):
- Trains: `(⌈-)4+÷2`, `√5(+×-)4` (fork), `4((¬=)∧¬=1+-)6` (complex fork)
- Swapped/selfed: `×˜-3`, `3-˜4`, `4-˜○÷2`
- Repeat: `3(2×+⍟(1+1))2`, `2+⍟3-1`
- Error cases for syntax: `()`, `+∘∘-`, `(+)∘`, etc.
- Nothing (·): `(1+·)-4`, `·⋄1`, `(·-⊑)¯2‿3`
- Define: `F←-⋄F 3`, `DiffSq←+×((-))`, destructure `a‿b‿·←↕3`
- Modify: `n←2⋄n↩3`, `x←4⋄x-↩1`, `a‿b←2‿0⋄a‿b+↩2`
- Lists: `⟨0,1⟩`, `2‿+‿-‿1`, `f←0⊑⟨×,-⟩`
- High-rank arrays: `[,0,1,2]`, `{[𝕩]≡≍𝕩}"ab"`, destructure `[a⋄b]←↕2‿3`
- Blocks with headers: `2{𝕩÷𝕨}6`, `{q←𝕩⋄{(q∧q)+(𝕩∨𝕩)}¬q}÷4`, `24 % 1{(0⊸<)◶⟨𝕗,(𝕗×𝕩)_𝕣⟩ 𝕩-1}4` (recursion via 𝕣)
- Nesting errors: `(`, `⟩⟨`, `{{`, etc.

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|-----------------|--------|
| No test harness | Official `this.bqn` run via `rbqn` | Pass/fail count across all 13 files |
| Manual expression testing (test_cbqn_compat.sh) | Official BQN test cases (450+ tests) | CBQN-compatible correctness standard |
| No file system syscalls | `•file.Lines` + `•file.List` via synthetic namespace | Enables harness and real BQN scripts |

---

## Open Questions

1. **Does Phase 1 include variable assignment?**
   - What we know: `a←5⋄a` currently fails with "Assertion error: Undefined identifier". This is an "Assertion error" from INSIDE the BQN compiler (the compiler itself is calling `!` assertion). This means the compiler is producing wrong bytecode for assignments, or the VM is incorrectly executing the assignment bytecode.
   - What's unclear: Is this the SAME issue as the "0 pick empty" compiler error mentioned in Phase 1 summaries? It may be a different manifestation.
   - Recommendation: Before starting Phase 2 planning, verify Phase 1 success criterion 3 ("REPL maintains variable state: `a←5` entered on one line is accessible on the next"). If this doesn't work, Phase 1 is not done.

2. **What does •file.Lines return for empty files vs files with trailing newlines?**
   - What we know: The harness reads test files which have trailing newlines. BQN's `this.bqn` filters with `(0<≠)◶0‿('#'≠⊑)¨⊸/` — first filtering empty strings (0<≠) then filtering comment lines.
   - What's unclear: Does `std::str::lines()` in Rust strip the trailing empty line from files ending in `\n`?
   - Recommendation: `str::lines()` in Rust does NOT include a trailing empty string for files ending in `\n`. This matches the expected behavior. Use `content.lines()` directly.

3. **Does •file namespace need path-relative resolution?**
   - What we know: CBQN's `•file` namespace stores a base path for relative resolution. `•file.Lines "cases/simple.bqn"` in the harness (run from BQN repo root) uses CWD-relative paths.
   - What's unclear: Does any test file use `•file.Lines` or `•Import` with a path relative to the script's own location?
   - Recommendation: For Phase 2, implement CWD-relative paths only. The 4 target test files (`simple.bqn`, `literal.bqn`, `syntax.bqn`, `bytecode.bqn`) use NO system functions — only the harness `this.bqn` does. So CWD-relative is sufficient for all Phase 2 work.

4. **Which opcodes are exercised by syntax.bqn that might not be in bytecode.bqn?**
   - What we know: `syntax.bqn` tests trains, modify-assign, blocks with `𝕣` (recursion), and header patterns like `{;𝕨}`.
   - What's unclear: Whether SETH/PRED (header opcodes) are exercised by syntax.bqn. The header section uses `;` separators which compile to SETH.
   - Recommendation: syntax.bqn almost certainly exercises SETH. Verify SETH is correctly implemented in vm.rs before expecting syntax.bqn to pass.

5. **Is the harness run from the BQN repo root or the test/ subdirectory?**
   - What we know: `this.bqn` uses `•file.Lines "cases/"⊸∾` — relative path from the repo root's `test/` directory. The README says "run from the project root directory" for some things, but `this.bqn` specifically uses relative paths from wherever it's called.
   - Recommendation: Run as `cd /path/to/BQN && rbqn test/this.bqn`. The CWD will be `/path/to/BQN`, so `•file.Lines "test/cases/simple.bqn"` needs to work. Actually — `this.bqn` calls `•file.Lines "cases/"⊸∾` where `"cases/"` is a function that prepends `"cases/"` to the filename. So if called from the `test/` directory, relative path is `"cases/simple.bqn"`. If called from the repo root, it would be `"test/cases/simple.bqn"`. This matters.
   - **Confirmed approach:** Run from the `test/` directory: `cd /path/to/BQN/test && rbqn this.bqn simple literal syntax bytecode`.

---

## Sources

### Primary (HIGH confidence)
- Direct inspection of `/Users/axel/Code/forks/BQN/test/this.bqn` — harness dependencies confirmed
- Direct inspection of `/Users/axel/Code/forks/BQN/test/cases/simple.bqn`, `literal.bqn`, `syntax.bqn`, `bytecode.bqn` — test counts and formats verified
- Direct inspection of `crates/rbqn-vm/src/derive.rs` — sys_name_to_b confirmed no "file" entry
- Live binary testing: `rbqn -p 'a←5⋄a'` confirms Phase 1 incomplete
- Direct inspection of `crates/rbqn-vm/src/namespace.rs` — NS struct and store_ns confirmed
- Direct inspection of `crates/rbqn-vm/src/modifiers.rs:1194` — ⎊ catch confirmed implemented
- Direct inspection of `/Users/axel/Code/forks/CBQN/src/builtins/sysfn.c:895` — •file.List confirmed

### Secondary (MEDIUM confidence)
- Phase 1 summary `01-02s-SUMMARY.md` — confirms compiler still fails after 20 plans
- Phase 1 summary `01-03-SUMMARY.md` — confirms formatter done but compiler blocked
- BQN `README.txt` — run from `test/` directory confirmed

### Tertiary (LOW confidence)
- The inference that syntax.bqn exercises SETH — based on reading the test content; not verified by running

---

## Metadata

**Confidence breakdown:**
- Test harness dependencies (what's needed): HIGH — verified by reading this.bqn and testing live
- •file.Lines/•file.List implementation approach: HIGH — straightforward file I/O, existing NS infrastructure
- Phase 1 completion requirement: HIGH — live binary shows variable assignment broken
- Which specific test cases will fail: MEDIUM — depends on what Phase 1 actually fixes
- syntax.bqn SETH requirement: LOW — inferred from test content, not verified

**Research date:** 2026-02-23
**Valid until:** After Phase 1 is complete (re-evaluate then — current binary state will change significantly)
