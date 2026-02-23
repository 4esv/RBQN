---
phase: quick-02
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/rbqn-vm/src/derive.rs
  - crates/rbqn-vm/src/vm.rs
autonomous: true
requirements: [QUICK-02]

must_haves:
  truths:
    - "Running with RBQN_PRIM_TRACE=1 prints every primitive c1/c2 call with args and results"
    - "Running without RBQN_PRIM_TRACE prints no extra trace output (zero overhead)"
    - "Trace output shows array shapes, element types, and first few elements for arrays"
    - "The crash location is identifiable from the trace: which primitive produced wrong shape"
  artifacts:
    - path: "crates/rbqn-vm/src/derive.rs"
      provides: "Prim trace in c1/c2 NativeFn + SysFn dispatch paths"
    - path: "crates/rbqn-vm/src/vm.rs"
      provides: "Trace helper functions for formatting B values"
  key_links:
    - from: "crates/rbqn-vm/src/derive.rs"
      to: "crates/rbqn-vm/src/vm.rs"
      via: "trace formatting helpers"
      pattern: "trace_b_value|trace_enabled"
---

<objective>
Add environment-variable-gated tracing to all primitive dispatches (c1/c2 in derive.rs)
so that running `RBQN_PRIM_TRACE=1 cargo run -- -e "1+1"` prints every primitive call
with its arguments and return value, enabling identification of which operation produces
the wrong result that causes the compiler's shape mismatch crash.

Purpose: The BQN compiler crashes on `𝕨<𝕩` with shapes [2] vs [1]. The primitives work
individually but some BQN logic in the compiler produces wrong intermediate results.
Tracing every primitive call with inputs AND outputs will reveal which operation diverges
from expected behavior.

Output: Modified derive.rs with traced c1/c2 paths; vm.rs with trace formatting helpers.
</objective>

<execution_context>
@/Users/axel/.claude/get-shit-done/workflows/execute-plan.md
@/Users/axel/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@crates/rbqn-vm/src/derive.rs
@crates/rbqn-vm/src/vm.rs
@crates/rbqn-prim/src/dispatch.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add trace formatting helpers and env-var gate to vm.rs</name>
  <files>crates/rbqn-vm/src/vm.rs</files>
  <action>
Add a thread-local bool `PRIM_TRACE_ENABLED` initialized from `std::env::var("RBQN_PRIM_TRACE").is_ok()` on first access (use `std::sync::LazyLock<bool>` or `std::sync::OnceLock<bool>` for the env check, then store in thread-local for fast access).

Add a public function `pub fn prim_trace_enabled() -> bool` that returns this flag.

Add a public function `pub fn fmt_b_short(b: B) -> String` that formats a B value for trace output:
- f64: show the number (e.g. "3", "¯2.5")
- c32: show the char (e.g. "'+'")
- arr: show "arr(shape=[...], el={I32|F64|C8|C16|C32|B}, first3=[...])" where first3 shows first 3 elements formatted recursively (truncate at depth 1 to avoid infinite recursion)
- fun/md1/md2: show "fun(prim=idx)" or "fun(block)" or "md1(prim=idx)" etc by checking DerivedKind
- SENTINEL: show "·"
- other: show hex tag

Add a public function `pub fn fmt_b_detail(b: B) -> String` that gives more detail for arrays:
- Shows full shape, element type, and up to 10 elements
- For char arrays, shows the string content (up to 60 chars)
- This is for tracing results where we need to see more data

Keep these lightweight — they should only be called when tracing is enabled.
  </action>
  <verify>
`cargo check -p rbqn-vm 2>&1 | tail -5` shows no errors. The new functions compile.
  </verify>
  <done>
vm.rs has `prim_trace_enabled()`, `fmt_b_short()`, and `fmt_b_detail()` functions that compile cleanly.
  </done>
</task>

<task type="auto">
  <name>Task 2: Add result tracing to c1/c2 NativeFn and SysFn paths in derive.rs</name>
  <files>crates/rbqn-vm/src/derive.rs</files>
  <action>
In the `c1` function, in the `DerivedKind::NativeFn` match arm (around line 471):
- After the existing `vm_trace_push` call, add a block gated on `crate::vm::prim_trace_enabled()`
- Before calling the primitive, log: `[PRIM c1] {glyph} x={fmt_b_short(x)}`
- After getting the result, log: `[PRIM c1] {glyph} → {fmt_b_detail(result_b)}`
- Use `eprintln!` directly (not vm_trace_push) so output goes to stderr immediately

In the `c2` function, in the `DerivedKind::NativeFn` match arm (around line 583):
- Same pattern: gate on `prim_trace_enabled()`
- Before: `[PRIM c2] w={fmt_b_short(w)} {glyph} x={fmt_b_short(x)}`
- After: `[PRIM c2] {glyph} → {fmt_b_detail(result_b)}`
- Include the prim_idx in the trace for cross-referencing

In `dispatch_sys_c1` and `dispatch_sys_c2`:
- At the entry of each function, if `prim_trace_enabled()`:
  - Log: `[SYS c1] sys={idx} x={fmt_b_short(x)}` / `[SYS c2] sys={idx} w={fmt_b_short(w)} x={fmt_b_short(x)}`
- These are important because the compiler heavily uses •GroupLen (22), •GroupOrd (23), •Type (0)

Also add result tracing to the FunBlock and Md1D/Md2D paths in c1/c2:
- Gate on `prim_trace_enabled()`
- For FunBlock: `[BLOCK c1] id={id} x={fmt_b_short(x)}` and after: `[BLOCK c1] id={id} → {fmt_b_short(result)}`
- This helps trace which BQN-defined function returns a bad result

Important: the trace must NOT use get_runtime() in the hot path for getting the glyph — that allocates a new Vec every call. Instead, use the `prim_idx` to index into a static glyph string. Define a const at the top of derive.rs:
```rust
const PRIM_GLYPHS: &str = "+-×÷⋆√⌊⌈|¬∧∨<>≠=≤≥≡≢⊣⊢⥊∾≍⋈↑↓↕«»⌽⍉/⍋⍒⊏⊑⊐⊒∊⍷⊔!˙˜˘¨⌜⁼´˝`∘○⊸⟜⌾⊘◶⎉⚇⍟⎊";
```
Use `PRIM_GLYPHS.chars().nth(prim_idx)` for glyph lookup (only when tracing is enabled, so allocation cost doesn't matter).

Do NOT modify the existing vm_trace_push calls — those serve a different purpose (crash trace ring buffer). The new tracing is additive via eprintln.
  </action>
  <verify>
Run `CBQN_PATH=/Users/axel/Code/forks/CBQN cargo build 2>&1 | tail -5` to verify compilation.
Then run `CBQN_PATH=/Users/axel/Code/forks/CBQN RBQN_PRIM_TRACE=1 cargo run -- -e "1+1" 2>&1 | head -100` to verify trace output appears.
Then run `CBQN_PATH=/Users/axel/Code/forks/CBQN cargo run -- -e "1+1" 2>&1 | head -20` to verify NO trace output without the env var.
  </verify>
  <done>
Running with RBQN_PRIM_TRACE=1 shows per-primitive traces with arguments and results. Running without shows no extra output. The trace clearly shows primitive names, input shapes/values, and output shapes/values so the divergent operation can be identified.
  </done>
</task>

</tasks>

<verification>
1. `CBQN_PATH=/Users/axel/Code/forks/CBQN cargo build` succeeds
2. `CBQN_PATH=/Users/axel/Code/forks/CBQN RBQN_PRIM_TRACE=1 cargo run -- -e "1+1" 2>trace.log` produces trace.log with primitive call traces
3. `grep '\[PRIM c2\].*<' trace.log` shows the `<` operation that crashes, and the preceding traces show what produced the mismatched shapes
4. Without RBQN_PRIM_TRACE, no trace lines appear
</verification>

<success_criteria>
- Every NativeFn c1/c2 call is traced with input values/shapes AND output values/shapes
- SysFn calls (Type, GroupLen, GroupOrd) are traced
- FunBlock calls show entry/exit with argument and result summaries
- Tracing is zero-cost when RBQN_PRIM_TRACE is not set
- The trace from compiling "1+1" shows enough detail to identify which operation before the `<` crash produced wrong shapes
</success_criteria>

<output>
After completion, create `.planning/quick/2-add-tracing-to-compiler-used-primitives-/2-SUMMARY.md`
</output>
