# RBQN Roadmap

## Milestone 1: Core Language Completeness
**Goal:** RBQN can compile and execute arbitrary BQN programs, passing the official test suite.

---

### Phase 1: Fix Bootstrap Pipeline
**Goal:** Runtime0 → Runtime1 → Compiler → Formatter all execute correctly.

**Why first:** Everything downstream depends on a working compiler. Runtime0 has 2 failing tests (+´ returns wrong result, +¨ type error) that cascade into runtime1 failures.

**Requirements:** R-BOOT-2, R-BOOT-3, R-BOOT-4, R-BOOT-5, R-BOOT-6, R-BOOT-7

**Key tasks:**
1. Debug and fix +´ (fold) — likely accumulator or operand dispatch issue
2. Debug and fix +¨ (each) — type error in array element dispatch
3. Verify runtime0 produces correct ~24 core functions
4. Execute runtime1 end-to-end, verify 64-value runtime output
5. Implement SetPrims callback (Decompose + PrimInd)
6. Implement SetInv callback (inverse tables)
7. Load compiler, verify it can compile simple BQN expressions
8. Load formatter, verify •Fmt and •Repr work

**Success criteria:** `CBQN_PATH=/path/to/CBQN cargo run -- -e '1+1'` outputs `2`

**Plans:** 3 plans

Plans:
- [ ] 01-01-PLAN.md — Fix fork/each bugs and verify runtime0 correctness
- [ ] 01-02-PLAN.md — Wire setPrims/setInv, runtime1, and compiler end-to-end
- [ ] 01-03-PLAN.md — Load formatter and verify •Fmt/•Repr output

---

### Phase 2: Missing Modifiers
**Goal:** Implement the 4 missing modifiers required for full language support.

**Why second:** Many BQN programs and the test suite depend on these modifiers. Under and Undo are needed for the inverse system.

**Requirements:** R-MOD-3, R-MOD-4, R-MOD-5, R-MOD-6

**Key tasks:**
1. Implement Rank (⎉) — apply function at specified cell rank
2. Implement Depth (⚇) — apply function at specified depth
3. Implement Undo (⁼) — inverse resolution tables for arithmetic, structural, and modifier inverses
4. Implement Under (⌾) — structural under (insert transformed values back) + invertible under (G⁼ applied)

**Success criteria:** All 20 modifiers dispatch correctly; inverse table covers all required arithmetic/structural/modifier inverses

---

### Phase 3: Essential System Functions
**Goal:** Implement system functions needed to run real BQN programs and the test suite.

**Requirements:** R-SYS-1 through R-SYS-7

**Key tasks:**
1. Complete •Glyph (map all primitives to glyph characters)
2. Implement •BQN / •ReBQN (eval BQN strings)
3. Implement •Show, •Out (stdout output)
4. Implement •Fmt, •Repr (value formatting — may come from formatter bytecode)
5. Implement •args, •path, •name, •wdpath, •state
6. Implement •FChars, •FLines, •FBytes
7. Implement •Import (file loading with module caching)
8. Implement •file namespace basics (path, At, List, Bytes, Chars, Lines)

**Success criteria:** Can load and execute multi-file BQN projects using •Import

---

### Phase 4: Primitive Correctness & Fill System
**Goal:** All primitives are spec-compliant including edge cases.

**Requirements:** R-PRIM-5, R-PRIM-6, R-PRIM-7

**Key tasks:**
1. Implement pervasive extension (deep array arithmetic — +‿-‿× on nested arrays)
2. Complete fill element propagation rules per BQN spec
3. Implement identity elements for fold/insert on empty arrays
4. Audit all 44 functions against BQN spec for edge cases
5. Fix any monadic gaps (shift nudge forms, etc.)

**Success criteria:** Pass prim.bqn, fill.bqn, identity.bqn test files

---

### Phase 5: Pass BQN Test Suite
**Goal:** Run and pass all 13 official BQN test files.

**Requirements:** R-TEST-1 through R-TEST-13

**Key tasks:**
1. Set up test runner (clone mlochbaum/BQN, run test/cases/*.bqn via RBQN)
2. Run simple.bqn, literal.bqn, syntax.bqn — fix failures
3. Run bytecode.bqn — fix VM edge cases
4. Run prim.bqn — fix remaining primitive issues
5. Run header.bqn, unhead.bqn — fix block dispatch edge cases
6. Run namespace.bqn — fix namespace edge cases
7. Run token.bqn — fix tokenizer/compiler issues
8. Run fill.bqn, identity.bqn, under.bqn, undo.bqn — fix remaining issues
9. Create CI pipeline to run test suite on every commit

**Success criteria:** All 13 test files pass with zero failures

---

### Phase 6: GPU Kernel Implementation
**Goal:** Complete GPU compute kernels and integrate with VM dispatch.

**Requirements:** R-GPU-4 through R-GPU-11

**Key tasks:**
1. Complete element-wise arithmetic kernels (add, sub, mul, div, pow, min, max for f32/i32)
2. Complete reduction kernels (sum, product, min, max, and, or)
3. Complete scan kernels (prefix sum, prefix product)
4. Complete sort kernel (radix sort for grade up/down)
5. Implement f64 feature detection — use SHADER_F64 when available, CPU fallback otherwise
6. Integrate GPU dispatch into primitive evaluation path (threshold check → GPU or CPU)
7. Build benchmark harness with wgpu-profiler for threshold tuning
8. Tune thresholds per operation category based on benchmarks
9. Implement kernel fusion for 2-3 chained element-wise operations

**Success criteria:** GPU path activates for arrays > threshold, benchmarks show measurable speedup, all tests still pass

---

## Milestone 2: System Function Completeness
**Goal:** All CBQN system functions implemented — full drop-in replacement.

### Phase 7: Math & Random
**Requirements:** R-SYS-8, R-SYS-10

**Key tasks:**
1. •math namespace (Cbrt, Log2, Log10, Log1p, Expm1, Hypot, trig functions, Fact, LogFact, Comb, Erf, ErfC, GCD, LCM, Sum)
2. •MakeRand / •rand (Range, Deal, Subset)

### Phase 8: OS Integration
**Requirements:** R-SYS-9, R-SYS-12, R-SYS-13

**Key tasks:**
1. •UnixTime, •MonoTime, •Delay
2. •platform namespace (os, environment, cpu.arch, bqn.impl, bqn.implVersion)
3. •SH (shell execution — returns exit_code, stdout, stderr)

### Phase 9: Data Structures & Extended I/O
**Requirements:** R-SYS-6, R-SYS-11, R-SYS-15, R-SYS-16

**Key tasks:**
1. •HashMap (Count, Keys, Values, Has, Get, Set, Delete)
2. •file namespace completion (Open, CreateDir, Remove, Rename, etc.)
3. •bit namespace (bitwise NOT, AND, OR, XOR, NEG, ADD, SUB, MUL, CAST)
4. •term namespace (Flush, RawMode, CharB, CharN)

### Phase 10: FFI & Embedding
**Requirements:** R-SYS-14, R-SYS-17 through R-SYS-20

**Key tasks:**
1. •FFI (foreign function interface — type system, pointer objects, struct support)
2. •_while_ (loop modifier)
3. •ns namespace (introspection)
4. •Exit, •ParseFloat, •Hash, •Cmp
5. •FromUTF8, •ToUTF8, •CurrentError
6. C API for embedding (bqn_init, bqn_eval, bqn_call1/2)

---

## Milestone 3: Self-Hosting
**Goal:** RBQN compiles itself without CBQN dependency.

### Phase 11: Self-Hosted Compiler
**Requirements:** R-SELF-1, R-SELF-2, R-SELF-3

**Key tasks:**
1. Compile BQN compiler source (c.bqn) using RBQN's loaded compiler
2. Verify output bytecode matches CBQN-generated bytecode
3. Embed compiled bytecode in binary (checked into repo)
4. Remove CBQN_PATH build-time requirement
5. Verify bootstrap chain works with embedded bytecode

---

## Milestone 4: Performance & GPU Optimization
**Goal:** Competitive with CBQN on CPU, faster on GPU-suitable workloads.

### Phase 12: CPU Performance
**Key tasks:**
1. SIMD-accelerated primitives (packed operations on typed arrays)
2. Type-specialized dispatch (avoid f64 conversions for integer arrays)
3. Array squeeze optimization (automatic type narrowing)
4. Profile hot paths and optimize allocation patterns

### Phase 13: Advanced GPU
**Key tasks:**
1. Subgroup operations for faster reductions (when hardware supports)
2. Advanced kernel fusion (reduction + map, scan + map)
3. Lazy GPU-resident arrays (keep data on GPU across operations)
4. Async GPU dispatch (batch command encoders, non-blocking submission)
5. Multi-pass optimization for compound array expressions

---

## Phase Dependency Graph

```
Phase 1 (Bootstrap) ──→ Phase 2 (Modifiers) ──→ Phase 3 (System Fns)
                                                       │
                                                       ▼
                         Phase 4 (Prim Correctness) ──→ Phase 5 (Test Suite)
                                                              │
                              Phase 6 (GPU Kernels) ◄─────────┘
                                     │
              ┌────────────────────┬─┴──────────────────┐
              ▼                    ▼                      ▼
    Phase 7 (Math)       Phase 8 (OS)          Phase 9 (Data/IO)
              │                    │                      │
              └────────────────────┴──────────────────────┘
                                   │
                                   ▼
                         Phase 10 (FFI/Embed)
                                   │
                                   ▼
                        Phase 11 (Self-Hosting)
                                   │
                          ┌────────┴────────┐
                          ▼                  ▼
                Phase 12 (CPU Perf)   Phase 13 (GPU Adv)
```

## Notes
- Phases 1-5 are the critical path to a working BQN implementation
- Phase 6 (GPU) can partially overlap with Phases 3-5 since GPU kernels are independent of language features
- Phases 7-10 can be worked in parallel after Phase 5
- Phase 11 requires all system functions needed by the compiler itself
- Phases 12-13 are pure optimization with no functional changes
