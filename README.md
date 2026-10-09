# RBQN

A Rust implementation of the [BQN](https://mlochbaum.github.io/BQN/) array language with experimental GPU dispatch.

> **Experimental / Archived.** Built primarily with Claude as a fun experiment. Slower than [CBQN](https://github.com/dzaima/CBQN) and unlikely to receive further updates. Use CBQN for anything serious.

## What works

- **Self-hosting**: compiles its own BQN compiler, runtime, and formatter. No external BQN implementation needed to build. Fixpoint-verified (round 2 produces identical bytecode).
- **All primitives**: arithmetic, comparison, structural (`⥊↑↓↕⌽⍉`), search/sort (`⊐⊒∊⍋⍒`), modifiers (`˘¨⌜´˝`), combinators (`∘○⊸⟜⊘◶⍟`), etc.
- **System values**: `•BQN`, `•Show`, `•Out`, `•Fmt`, `•Type`, `•Decompose`, `•Glyph`, `•Fill`, `•CurrentError`, `•args`, `•Exit`
- **`•math` namespace**: `Sin`, `Cos`, `Tan`, `ASin`, `ACos`, `ATan`, `Log`, `Cbrt`, `Hypot`, `Erf`, `Comb`, `MatMul`, `Softmax`, `Pi`
- **`•file` namespace**: `Lines`, `Chars`, `Bytes`, `List`, `At`, `Name`, `Parent`, `Exists`, `Type`, `CreateDir`, `Remove`, `Rename`
- **`•term` namespace**: `RawMode`, `CharB`, `CharN`, `Flush`, `OutRaw`, `ErrRaw`
- **`•bit` namespace**: `_cast`, `_not`, `_neg`, and binary bit operations across widths
- **Other**: `•Import`, `•ParseFloat`, `•Hash`, `•FromUTF8`, `•ToUTF8`, inverse (`⁼`) for many primitives, headers/predicates, block bodies, REPL
- **GPU kernels** (wgpu): elementwise arithmetic, reductions, prefix scan, sort, gather, softmax, matmul. Auto-dispatches to GPU above size thresholds (~10M+ elements on Apple Silicon). Fused kernel support.

## What doesn't

- Slower than CBQN across the board
- No REPL completion or history
- `•FFI` is stubbed but non-functional
- `•file.Open` not implemented
- Error messages are often unhelpful
- No namespace support beyond the built-in ones
- Edge cases in lesser-used primitives: passes most but not all of the BQN test suite

## Usage

```
rbqn [options] [file.bqn [arguments]]

Options:
  -e code    execute BQN code
  -p code    execute and pretty-print result
  -o code    execute and print raw output
  -f file    execute file with remaining args as •args
  -r         start REPL after executing arguments
  --version  show version and bytecode source
  --help     show help
```

## Install

```bash
cargo install rbqn
```

No external dependencies required. The BQN compiler and runtime are embedded as bytecode.

## Architecture

~24K lines of Rust across five crates:

| Crate | Purpose |
|-------|---------|
| `rbqn` | CLI, REPL, bootstrap, `•Import`/`•BQN` |
| `rbqn-core` | Value types, NaN-boxed `B` representation, arrays, errors |
| `rbqn-prim` | Primitive implementations (arithmetic, structural, search/sort) |
| `rbqn-vm` | Bytecode VM, block compilation, modifiers, derived functions, system values |
| `rbqn-gpu` | wgpu context, buffer pool, pipeline cache, WGSL kernels |

## Bootstrap

RBQN is self-hosting. Bytecode files in `bins/` are compiled from the BQN source files (`c.bqn`, `r0.bqn`, `r1.bqn`, `f.bqn`) and loaded via `include_bytes!` at compile time.

```bash
# Regenerate bins from updated BQN sources
BQN_SRC=/path/to/BQN/src cargo run -p rbqn --features gen-tools --bin rbqn-gen -- --self

# Verify fixpoint
BQN_SRC=/path/to/BQN/src cargo run -p rbqn --features gen-tools --bin rbqn-gen -- --self --fixpoint-check
```

## Acknowledgements

**BQN** was designed by [Marshall Lochbaum](https://mlochbaum.github.io/). The language spec, compiler source (`c.bqn`, `r0.bqn`, `r1.bqn`, `f.bqn`), and reference test suite are his work and are licensed under the ISC License. RBQN embeds bytecode compiled from those source files. The language itself is Marshall's creation.

**CBQN** is the reference C implementation of BQN, written by [dzaima](https://github.com/dzaima) with contributions from the BQN community. RBQN was originally bootstrapped from CBQN-compiled bytecode and is now self-hosting. CBQN's internals, the bytecode format, provide array layout, and runtime structure documented in `load.c`, were essential reference material throughout development.

- BQN: https://mlochbaum.github.io/BQN/
- CBQN: https://github.com/dzaima/CBQN
- BQN community: https://mlochbaum.github.io/BQN/community/
