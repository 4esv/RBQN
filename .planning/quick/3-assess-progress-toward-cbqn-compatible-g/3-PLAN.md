---
phase: quick-03
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - .planning/quick/3-assess-progress-toward-cbqn-compatible-g/PROGRESS.md
autonomous: true
requirements: []
must_haves:
  truths:
    - "PROGRESS.md exists with comprehensive status of RBQN vs CBQN compatibility"
    - "Document covers all 5 dimensions: working features, broken features, GPU status, gap analysis, next steps"
    - "Assessment is grounded in actual test results, not just STATE.md documentation"
  artifacts:
    - path: ".planning/quick/3-assess-progress-toward-cbqn-compatible-g/PROGRESS.md"
      provides: "Complete progress assessment document"
      min_lines: 150
  key_links: []
---

<objective>
Produce a comprehensive PROGRESS.md assessing RBQN's current state toward being a 1:1 CBQN-compatible, GPU-accelerated Rust BQN implementation that could be shared with Marshall.

Purpose: Give a clear-eyed view of where RBQN stands, what works, what's broken, and what the path forward looks like.
Output: .planning/quick/3-assess-progress-toward-cbqn-compatible-g/PROGRESS.md
</objective>

<execution_context>
@/Users/axel/.claude/get-shit-done/workflows/execute-plan.md
@/Users/axel/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@.planning/ROADMAP.md
@.planning/codebase/ARCHITECTURE.md
@.planning/codebase/CONCERNS.md
</context>

<tasks>

<task type="auto">
  <name>Task 1: Write PROGRESS.md assessment document</name>
  <files>.planning/quick/3-assess-progress-toward-cbqn-compatible-g/PROGRESS.md</files>
  <action>
Write a comprehensive progress assessment document covering these sections:

**1. Executive Summary**
One-paragraph honest assessment of where RBQN stands. Key facts:
- 3/21 compatibility test expressions pass (14% pass rate)
- Native primitive layer works (13/13 unit tests pass) but the compiled BQN expression pipeline is broken
- The compiler loads and runs but runtime0/1 override functions produce wrong results for most primitives
- Only `+` dyadic, literal lists, and monadic `{block}` work end-to-end
- GPU crate is scaffolding only (2072 lines, 15 WGSL shaders) but zero integration with the interpreter

**2. What Works Today**
Based on actual testing (not just STATE.md claims):
- Build pipeline: compiles cleanly, CBQN bytecode embedding works
- Bootstrap stages: runtime0, runtime1, compiler, and formatter all LOAD without crashing
- Native primitives: All 44 functions implemented with c1/c2, 13/13 integration tests pass when called directly
- Modifiers: 16/20 implemented natively (fold, each, scan, table, cells, swap, atop, over, before, after, valences, choose, repeat, catch, constant, insert)
- VM: Stack machine executes bytecodes, handles SETH/PRED, block definitions, scoping
- Formatter: Falls back gracefully, produces basic output for scalars and simple arrays
- `cargo run -- -p '1+1'` => 2 (correct)
- `cargo run -- -p '⟨1,2,3⟩'` => correct list display
- `cargo run -- -p '{𝕩+1}5'` => 6 (correct -- monadic blocks with + work)

**3. What's Broken (Critical)**
The compiled expression pipeline has a fundamental issue -- runtime0/1 override functions don't work correctly:
- Most dyadic primitives return `w` (left argument) instead of the computed result: `2*3=2`, `5-3=5`, `10/2=10`
- Monadic primitives return `x` unchanged: `-3=3` (should be -3), `floor 3.7=3.7` (should be 3), `|neg5=neg5` (should be 5)
- Only `+` works correctly (both monadic identity and dyadic addition)
- ALL modifier expressions fail with "Interpreting non-1-modifier as 1-modifier" -- the runtime's modifier functions are mistyped or misrouted
- String operations crash with "Type error: !x: x must be 0 or 1"
- `2{w*x}3 = 2` (dyadic block returns w, not w*x -- same * bug propagated)

Root cause hypothesis: Runtime0 executes and produces 24 functions, but most of these override functions are broken. They were derived from CBQN's BQN-written runtime, and the VM isn't executing the BQN runtime source correctly. The native Rust primitives (which DO work) are being shadowed by broken runtime0 overrides.

**4. Missing Language Features**
From CBQN compatibility perspective:
- Modifiers not implemented: Undo (inverseprim), Under (inverterprim), Rank (cell-based), Depth (depth-based) -- 4/20 missing
- System functions: Only 6/~50 implemented (Type, Decompose, Glyph, Fill, GroupLen, GroupOrd). Missing: BQN, ReBQN, Show, Out, Import, FChars, FLines, FBytes, math.*, file.*, SH, Exit, platform, etc.
- No garbage collection (memory grows monotonically)
- REPL has no state persistence between lines
- Fill tracking not implemented
- DYNM opcode returns SENTINEL (dynamic variable mutation broken)
- Monadic shifts (laquo, raquo) have no c1 implementation

**5. GPU Acceleration Status**
The rbqn-gpu crate (2072 lines total: 1315 Rust + 757 WGSL):
- Infrastructure DONE: GpuContext (wgpu init), buffer pool, pipeline cache, dispatch thresholds
- Kernels with real implementations: unary elementwise (exp/sqrt/neg/abs), matmul (tiled 16x16), softmax (single-WG + multi-pass)
- Kernels as scaffolding only: arithmetic binary ops, reduce, scan, select, sort
- Integration with interpreter: ZERO -- rbqn-gpu is listed as a Cargo dependency of rbqn-prim but never imported or called
- No benchmarks, no tests, no way to actually trigger GPU execution from BQN code

**6. Gap Analysis: Path to CBQN Drop-In**
Organize into tiers by dependency order:

Tier 0 (Blocking everything): Fix the runtime0/1 override pipeline
- The fact that `*`, `-`, `floor`, `|`, `div`, `range`, `reshape`, `reverse`, and ALL modifiers are broken through the compiled pipeline means nothing beyond trivial `+` arithmetic works
- This is the Phase 1 Plan 02 work that's been iterated on 19 times (02a through 02s summaries) but still not resolved
- Until this is fixed, Phases 2 and 3 are unreachable

Tier 1 (Language completeness): After fixing the pipeline
- Implement Undo, Under, Rank, Depth modifiers
- Implement ~44 missing system functions (prioritize: BQN, Show, Out, Import, file I/O, math)
- Fix fill tracking, DYNM, monadic shifts
- Pass 13 official BQN test files

Tier 2 (Self-hosting): Remove CBQN build dependency
- Compile BQN compiler source using RBQN's compiler
- Embed compiled bytecode, remove CBQN_PATH requirement

Tier 3 (GPU integration): Actually wire GPU to interpreter
- Design dispatch strategy: when does array op go to GPU vs CPU?
- Implement typed array transfer: BqnArr ArrData -> GpuBuffer
- Wire hot primitives: +, *, /, reshape, reduce, scan, sort, grade
- Benchmarking infrastructure

Tier 4 (Production quality): For Marshall/sharing
- Garbage collection (currently objects never freed)
- Error handling via Result instead of panic
- REPL state persistence
- CI/CD pipeline
- Documentation
- Performance: eliminate HashMap lookup per value access, stop cloning arrays

**7. Codebase Metrics**
- Total Rust: ~12,500 lines (core: 824, vm: 4373, prim: 3278, main: 1988, gpu: 1315)
- GPU shaders: 757 lines WGSL across 15 files
- Integration tests: 13 (all passing, but only test native primitives, not compiled BQN)
- CBQN compat test: 3/21 pass (14%)
- Commits in Phase 1: ~20+ fix iterations on Plan 02

**8. Prioritized Next Steps**
1. **[CRITICAL] Fix runtime0/1 override dispatch** -- This is the single blocker. Most primitives work natively but the BQN-written runtime overrides are producing wrong results. Consider: (a) debugging why the VM misexecutes runtime0 BQN source, or (b) bypassing runtime0 overrides entirely and using native Rust primitives directly (since all 44 are implemented).
2. **Run official BQN test suite** once compiler works -- establishes a quantitative correctness baseline
3. **Implement missing modifiers** (Undo, Under, Rank, Depth) -- needed for many BQN idioms
4. **Implement core system functions** (BQN, Show, Out, file I/O) -- needed for real programs
5. **Wire GPU crate** to interpreter -- the GPU code exists but is completely disconnected
6. **Add GC** -- without it, any non-trivial program leaks memory indefinitely

**9. Honest Assessment for Sharing with Marshall**
RBQN is not yet ready to share as a CBQN drop-in. The fundamental issue is that while all the architecture is in place (NaN-boxing, bytecode VM, 4-stage bootstrap, 64 native primitives, GPU scaffolding), the critical compiler/runtime pipeline that transforms BQN source into executable results is broken for everything except `+`. This means RBQN currently functions as an expensive identity function for most BQN expressions.

The path forward requires resolving the Tier 0 blocker first. Once the compiler pipeline works, RBQN has a solid foundation to build on -- the native primitives are correct, the VM opcodes are implemented, and the GPU infrastructure is ready to be wired in.

Estimated effort to reach "demo-ready for Marshall":
- Fix compiler pipeline: Unknown (19 iterations haven't solved it -- may need a different approach)
- Pass basic test suite: 2-4 weeks after pipeline fix
- Wire GPU for demo: 1-2 weeks
- Total: Highly dependent on the pipeline fix

Estimated effort to reach "CBQN drop-in":
- All of the above, plus self-hosting, GC, system functions, official test suite: 2-4 months
  </action>
  <verify>
    - File exists at .planning/quick/3-assess-progress-toward-cbqn-compatible-g/PROGRESS.md
    - File contains all 9 sections
    - File is at least 150 lines
    - Claims match actual test results (3/21 compat, 13/13 native, etc.)
  </verify>
  <done>PROGRESS.md exists with all sections, grounded in actual test data, providing honest assessment of RBQN status</done>
</task>

</tasks>

<verification>
- PROGRESS.md exists and is comprehensive
- All claims verified against actual cargo run/test output
- Document is structured for readability
</verification>

<success_criteria>
- PROGRESS.md written with all 9 sections
- Assessment based on real test data, not optimistic STATE.md
- Clear tier-based gap analysis
- Honest "ready to share?" conclusion
</success_criteria>

<output>
After completion, create `.planning/quick/3-assess-progress-toward-cbqn-compatible-g/3-SUMMARY.md`
</output>
