# RBQN Requirements

## Requirement Categories

### R-BOOT: Bootstrap Pipeline
| ID | Requirement | Status | Priority |
|----|------------|--------|----------|
| R-BOOT-1 | Provide array (40 native functions) correctly populated | Validated | Critical |
| R-BOOT-2 | Runtime0 executes and produces ~24 core runtime functions | Partial (7/9) | Critical |
| R-BOOT-3 | Runtime1 executes using runtime0 output, produces 64-value runtime | Partial | Critical |
| R-BOOT-4 | SetPrims callback enables •Decompose and •Glyph | Not started | Critical |
| R-BOOT-5 | SetInv callback sets up inverse tables for ⁼ and ⌾ | Not started | Critical |
| R-BOOT-6 | Compiler loads and can compile BQN source strings | Partial | Critical |
| R-BOOT-7 | Formatter loads and provides •Fmt and •Repr | Partial (fallback works, self-hosted blocked by runtime1) | High |
| R-BOOT-8 | REPL mode with variable persistence across evaluations | Not started | Medium |

### R-PRIM: Primitive Functions (44 functions)
| ID | Requirement | Status | Priority |
|----|------------|--------|----------|
| R-PRIM-1 | All 10 arithmetic functions (+-×÷⋆√⌊⌈\|¬) monadic + dyadic | Validated | Critical |
| R-PRIM-2 | All 10 comparison functions (∧∨<>≠=≤≥≡≢) monadic + dyadic | Validated | Critical |
| R-PRIM-3 | All 14 structural functions (⊣⊢⥊∾≍⋈↑↓↕«»⌽⍉/) monadic + dyadic | Validated | Critical |
| R-PRIM-4 | All 10 search/sort/group functions (⍋⍒⊏⊑⊐⊒∊⍷⊔!) monadic + dyadic | Validated | Critical |
| R-PRIM-5 | Pervasive extension (deep array arithmetic) | Not started | Critical |
| R-PRIM-6 | Fill element propagation (all rules per spec) | Partial | High |
| R-PRIM-7 | Identity elements for fold on empty arrays | Not started | High |
| R-PRIM-8 | Character arithmetic (char±num, char-char) | Validated | Critical |

### R-MOD: Modifiers (20 modifiers)
| ID | Requirement | Status | Priority |
|----|------------|--------|----------|
| R-MOD-1 | 1-modifiers: ˙ ˜ ˘ ¨ ⌜ ´ ˝ ` | Validated | Critical |
| R-MOD-2 | 2-modifiers: ∘ ○ ⊸ ⟜ ⊘ ◶ ⍟ ⎊ | Validated | Critical |
| R-MOD-3 | Undo (⁼) — inverse resolution for all required functions/modifiers | Not started | Critical |
| R-MOD-4 | Under (⌾) — structural and invertible under | Not started | Critical |
| R-MOD-5 | Rank (⎉) — apply function at specified rank | Not started | Critical |
| R-MOD-6 | Depth (⚇) — apply function at specified depth | Not started | Critical |

### R-VM: Virtual Machine
| ID | Requirement | Status | Priority |
|----|------------|--------|----------|
| R-VM-1 | All 31 standard opcodes executing correctly | Validated | Critical |
| R-VM-2 | 18 optimized internal opcodes (EXTO, FN1Ci, SETNi, etc.) | Validated | Critical |
| R-VM-3 | Block definition and closure creation (DFND) | Validated | Critical |
| R-VM-4 | Train construction (TR2D, TR3D, TR3O) | Validated | Critical |
| R-VM-5 | Header matching and predicates (SETH, PRED) | Validated | Critical |
| R-VM-6 | Namespace support (FLDO, FLDM, ALIM, RETD) | Validated | Critical |
| R-VM-7 | Multiple body dispatch (monadic/dyadic, case headers) | Validated | High |

### R-SYS: System Functions (~50 functions)
| ID | Requirement | Status | Priority |
|----|------------|--------|----------|
| R-SYS-1 | •Type, •Decompose, •Glyph, •Fill | Partial | Critical |
| R-SYS-2 | •BQN / •ReBQN (eval) | Not started | Critical |
| R-SYS-3 | •Show, •Out, •Fmt, •Repr | Not started | High |
| R-SYS-4 | •Import (file loading with caching) | Not started | High |
| R-SYS-5 | •FChars, •FLines, •FBytes (file read) | Not started | High |
| R-SYS-6 | •file namespace (full: path, At, List, Bytes, Chars, Lines, Open, etc.) | Not started | High |
| R-SYS-7 | •args, •path, •name, •wdpath, •state | Not started | High |
| R-SYS-8 | •math namespace (trig, combinatorial, Cbrt, Hypot, Erf, GCD, etc.) | Not started | Medium |
| R-SYS-9 | •UnixTime, •MonoTime, •Delay | Not started | Medium |
| R-SYS-10 | •MakeRand / •rand (Range, Deal, Subset) | Not started | Medium |
| R-SYS-11 | •HashMap | Not started | Medium |
| R-SYS-12 | •platform namespace (os, cpu.arch, bqn.impl, etc.) | Not started | Medium |
| R-SYS-13 | •SH (shell execution) | Not started | Medium |
| R-SYS-14 | •FFI (foreign function interface) | Not started | Low |
| R-SYS-15 | •bit namespace (bitwise operations) | Not started | Low |
| R-SYS-16 | •term namespace (terminal I/O) | Not started | Low |
| R-SYS-17 | •_while_ (loop modifier) | Not started | Low |
| R-SYS-18 | •ns namespace (introspection) | Not started | Low |
| R-SYS-19 | •Exit, •ParseFloat, •Hash, •Cmp | Not started | Low |
| R-SYS-20 | •FromUTF8, •ToUTF8, •CurrentError | Not started | Low |

### R-GPU: GPU Acceleration
| ID | Requirement | Status | Priority |
|----|------------|--------|----------|
| R-GPU-1 | GPU context initialization (wgpu, Metal/Vulkan/DX12) | Validated | High |
| R-GPU-2 | Buffer pool with size-bucketed reuse | Partial | High |
| R-GPU-3 | Threshold-gated dispatch (CPU below ~50K elements) | Scaffolding | High |
| R-GPU-4 | Element-wise arithmetic kernels (add, sub, mul, div for f32/i32) | Scaffolding | High |
| R-GPU-5 | Reduction kernels (sum, min, max, and, or) | Scaffolding | High |
| R-GPU-6 | Scan kernels (prefix sum, Blelloch) | Scaffolding | High |
| R-GPU-7 | Sort kernels (radix sort) | Scaffolding | Medium |
| R-GPU-8 | Kernel fusion for chained element-wise operations | Minimal | Medium |
| R-GPU-9 | f64 support via SHADER_F64 feature detection + f32 fallback | Not started | High |
| R-GPU-10 | VM integration (dispatch to GPU from primitive evaluation) | Not started | High |
| R-GPU-11 | Benchmark harness with wgpu-profiler | Not started | Medium |

### R-TEST: Testing & Verification
| ID | Requirement | Status | Priority |
|----|------------|--------|----------|
| R-TEST-1 | Pass simple.bqn (basic arithmetic, operations) | Not started | Critical |
| R-TEST-2 | Pass literal.bqn (number/string/char literals) | Not started | Critical |
| R-TEST-3 | Pass syntax.bqn (assignment, trains, blocks, lists) | Not started | Critical |
| R-TEST-4 | Pass bytecode.bqn (VM opcode behavior) | Not started | Critical |
| R-TEST-5 | Pass prim.bqn (all primitive operations) | Not started | Critical |
| R-TEST-6 | Pass token.bqn (tokenizer edge cases) | Not started | High |
| R-TEST-7 | Pass header.bqn (block headers, pattern matching) | Not started | High |
| R-TEST-8 | Pass unhead.bqn (headerless multi-body blocks) | Not started | High |
| R-TEST-9 | Pass namespace.bqn (creation, access, mutation) | Not started | High |
| R-TEST-10 | Pass fill.bqn (fill element propagation) | Not started | Medium |
| R-TEST-11 | Pass identity.bqn (fold/insert identity elements) | Not started | Medium |
| R-TEST-12 | Pass under.bqn (⌾ operation) | Not started | Medium |
| R-TEST-13 | Pass undo.bqn (⁼ operation) | Not started | Medium |

### R-SELF: Self-Hosting
| ID | Requirement | Status | Priority |
|----|------------|--------|----------|
| R-SELF-1 | Compile BQN compiler source using RBQN's own compiler | Not started | Milestone |
| R-SELF-2 | Eliminate CBQN_PATH build-time dependency | Not started | Milestone |
| R-SELF-3 | Embed pre-compiled bytecode in binary | Not started | Milestone |
