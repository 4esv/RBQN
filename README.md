# RBQN

A Rust implementation of the [BQN](https://mlochbaum.github.io/BQN/) array language with experimental GPU dispatch.

> **Experimental.** Built primarily with Claude. Passes the full official BQN test suite and runs 2-6x slower than [CBQN](https://github.com/dzaima/CBQN) on most array workloads (see [Performance](#performance)). Use CBQN for anything serious.

## What works

- **Self-hosting**: compiles its own BQN compiler, runtime, and formatter. No external BQN implementation needed to build. Fixpoint-verified (round 2 produces identical bytecode).
- **All primitives**: arithmetic, comparison, structural (`⥊↑↓↕⌽⍉`), search/sort (`⊐⊒∊⍋⍒`), modifiers (`˘¨⌜´˝`), combinators (`∘○⊸⟜⊘◶⍟`), etc.
- **System values**: `•BQN`, `•Show`, `•Out`, `•Fmt`, `•Type`, `•Decompose`, `•Glyph`, `•Fill`, `•CurrentError`, `•args`, `•Exit`
- **`•math` namespace**: `Sin`, `Cos`, `Tan`, `ASin`, `ACos`, `ATan`, `Log`, `Cbrt`, `Hypot`, `Erf`, `Comb`, `MatMul`, `Softmax`, `Pi`
- **`•gpu` namespace**: `MatMul`, `Softmax`, `Available`, explicit f32 kernels, and `Tensor`/`Fetch` for device-resident arrays (see [`•gpu`](#gpu))
- **`•file` namespace**: `Lines`, `Chars`, `Bytes`, `List`, `At`, `Name`, `Parent`, `Exists`, `Type`, `CreateDir`, `Remove`, `Rename`
- **`•term` namespace**: `RawMode`, `CharB`, `CharN`, `Flush`, `OutRaw`, `ErrRaw`
- **`•bit` namespace**: `_cast`, `_not`, `_neg`, and binary bit operations across widths
- **Other**: `•Import`, `•ParseFloat`, `•Hash`, `•FromUTF8`, `•ToUTF8`, inverse (`⁼`) for many primitives, headers/predicates, block bodies, REPL
- **GPU kernels** (wgpu): elementwise arithmetic, reductions, prefix scan, sort, gather, softmax, matmul. Auto-dispatches to GPU above size thresholds (~10M+ elements on Apple Silicon). Fused kernel support.

## What doesn't

- Slower than CBQN: 2-6x on array workloads, up to ~14x on per-element paths like `•Fmt` of large arrays (startup is ~15 ms vs CBQN's ~3 ms)
- No REPL completion or history
- `•FFI` is stubbed but non-functional
- `•file.Open` not implemented
- Error messages are often unhelpful
- No namespace support beyond the built-in ones
- Known divergences from CBQN: rank>1 fold returns a value instead of erroring, `⌊`/`⌈` with NaN follow Rust `min`/`max`, large floats print with all digits instead of `1e308`
- Error messages: the parts scripts match on follow CBQN (no `Domain error:`-style kind prefix, `Stack overflow`, `Mapping: Expected equal shape prefix (…)` for arithmetic, `Assertion error` / the `!` message, compiler messages without position). Accepted divergence: per-primitive wording (e.g. `⊑` out-of-bounds, `↕`, `⌽`, `>`, `⊏` messages), values shown in Rust notation (`[1, 2]` instead of `⟨1, 2⟩`), and no source position or caret under the error
- Recursion depth: 4096 nested block evaluations, as CBQN (`{𝕊⍟(𝕩<4094) 𝕩+1} 0` is the deepest that succeeds in both). Accepted divergence: CBQN halves the limit when the recursion goes through `¨`, RBQN does not. Native recursion on deeply nested arrays (comparison, hashing, sorting, arithmetic) raises `Stack overflow` before the 512 MB interpreter stack runs out, where CBQN can segfault

## Performance

Measured with `bench/compare.sh` (hyperfine, mean ms, Apple Silicon). Before/after files are in `bench/`.

| expression | CBQN ms | RBQN ms | ratio |
|---|---:|---:|---:|
| `+´↕10000000` | 6.5 | 24.9 | 3.8x |
| `+´ {𝕩+1}¨ ↕1000000` | 20.0 | 96.8 | 4.8x |
| `{𝕩<2 ? 𝕩 ; (𝕊 𝕩-1)+𝕊 𝕩-2} 27` | 33.9 | 120.7 | 3.6x |
| `≠ ⊐ 3000000⥊↕1000` | 5.6 | 32.8 | 5.9x |
| `+´⥊ (↕3000) ×⌜ ↕3000` | 7.4 | 30.3 | 4.1x |
| ``+´ +` ↕5000000`` | 13.5 | 29.0 | 2.1x |
| `≠ •Fmt ↕1000000` | 223.5 | 3087.3 | 13.8x |

Before the October 2026 performance work these rows were 10-200x slower than CBQN (`bench/baseline.md`). The remaining gap is mostly per-call scope allocation and boxing in `¨`; see the git log for `perf(...)` commits.

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

## •gpu

Explicit f32 GPU kernels, for code that accepts f32 rounding in exchange for speed. Nothing outside `•gpu` changes results: automatic GPU dispatch (arithmetic, folds, scans, sorts, `•math.MatMul`) runs integer (i32/i64) kernels only where the result is exact and falls back to the f64 CPU path otherwise; `tests/gpu/corpus.txt` checks it against the CPU.

| function | does |
|---|---|
| `w •gpu.MatMul x` | rank-2 `w` (m×k) times rank-2 `x` (k×n), f32 kernel |
| `•gpu.Softmax x` | softmax of a rank-1 `x`, or of each row of a rank-2 `x` (one dispatch per row), f32 kernel |
| `•gpu.Available @` | 1 if a GPU device exists, else 0 |
| `•gpu.Tensor x` | upload numeric `x` once; the result stays on the device |
| `•gpu.Fetch x` | download a resident `x` (any other value is returned unchanged) |

Contract:

- Inputs are numeric arrays of any element type; every element is rounded to f32 before upload (values beyond f32 range become `∞`).
- `MatMul` and `Softmax` take resident or host arguments and return resident results, so `•gpu.Fetch a •gpu.MatMul b •gpu.MatMul c` uploads each input once and downloads once. Any other primitive (and `•Show`/`•Fmt`) downloads a resident value on first use and keeps the host copy; `≢`, `≠` and `=` read the shape without a download.
- `Tensor` stores f32 when every element is exact in f32, else i32 when every element is an i32 integer, else f32 (rounded). So `•gpu.Fetch •gpu.Tensor x` ≡ `x` for i32 integers and f32-exact floats. An empty `x` stays on the host. `MatMul`/`Softmax` on an i32 tensor convert it through the host (one logged download and upload). Resident buffers are never freed (like host arrays today).
- Arithmetic on a resident value (`+ - × ÷ ⌊ ⌈ |`, monadic or dyadic, with a number or a same-shape resident operand) is f32 and stays lazy. The chain runs as one kernel when `Fetch`, `MatMul`, `Softmax` or any other primitive needs it, and `+´ ×´ ⌊´ ⌈´` on a resident list runs it inside the reduce kernel: `+´ 1+2×•gpu.Tensor x` is one dispatch with nothing downloaded except the sum. A host-array operand downloads, as before.
- Downloaded values are ordinary arrays holding the f32 values. They differ from `•math.MatMul` / `•math.Softmax` (exact f64, CPU) by rounding: `(2‿2⥊0.1) •gpu.MatMul 2‿2⥊3` gives `0.6000000238418579` where `•math.MatMul` gives `0.6000000000000001`.
- No size threshold and no exactness gate: every call dispatches, so small inputs pay the transfer cost.
- With no device (a CPU-only build via `--no-default-features`, `--no-gpu`, `RBQN_GPU=off`, or no adapter) `MatMul`, `Softmax` and `Tensor` error with `no GPU available`, and `Available` returns 0.
- `RBQN_GPU_DEBUG=1` logs each `upload`, `download` and `(f32, resident)` dispatch.

## Install

```bash
cargo install rbqn
```

No external dependencies required. The BQN compiler and runtime are embedded as bytecode.

An optional profile-guided build (`bench/pgo.sh`, two steps, not available via `cargo install`) is up to 16% faster on interpreter-bound code; see `bench/pgo.md`.

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
