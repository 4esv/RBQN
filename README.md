# RBQN

A Rust implementation of the [BQN](https://mlochbaum.github.io/BQN/) array language that uses the whole machine: multicore CPU, GPU dispatch, typed arrays, a bytecode VM.

```
cargo install rbqn
```

Or download a binary for macOS, Linux or Windows from [Releases](https://github.com/4esv/RBQN/releases).

```
   +´↕10
45
   3‿3⥊↕9
┌─       
╵ 0 1 2  
  3 4 5  
  6 7 8  
        ┘
   {𝕩≤1 ? 𝕩 ; (𝕊 𝕩-1)+𝕊 𝕩-2}¨ ↕10
⟨ 0 1 1 2 3 5 8 13 21 34 ⟩
```

Multicore, 1e8 elements, `bench/wow/run.sh` against CBQN (12-core Intel Mac, [full output](bench/wow/results/2026-10-24-bench-intel12.md)):

| workload | CBQN | rbqn -t 1 | rbqn -t 12 | speedup vs -t 1 | result agrees |
|---|---|---|---|---|---|
| `+´` 1e8 i32 | 0.030 s (0.27 s) | 0.035 s (0.31 s) | 0.019 s (0.31 s) | 1.83x | yes |
| `(a×b)+c` 1e8 f64 | 1.199 s (3.69 s) | 1.779 s (4.11 s) | 0.569 s (3.06 s) | 3.12x | yes |
| `⌈´` 1e8 f64 | 0.129 s (1.06 s) | 0.299 s (1.30 s) | 0.085 s (1.23 s) | 3.52x | yes |

Cells: median in-process seconds for the operation alone (median whole-process wall seconds).

RBQN passes the official BQN test suite and is self-hosting: it compiles its own compiler, runtime and formatter from the BQN sources, with no other BQN implementation involved. The goal is a BQN that leverages compute as far as it safely can. The language is Marshall Lochbaum's; the implementation goes its own way.

Status: pre-1.0, complete language, no known divergences from the specification. `•file.Open` is unspecified in BQN and absent here.

## What works

- **The language.** Every primitive, modifier and combinator; headers, predicates, multiple bodies, namespaces, `⁼` for the invertible primitives, `⌾` and `⚇`. The official test suite passes in full (13 files, 1316 cases, see [Testing](#testing)). The workspace has 31 `unsafe` sites, each with a `// SAFETY:` comment enforced by `clippy::undocumented_unsafe_blocks` (`bench/unsafe_census.sh`); rbqn-core and rbqn-prim pass under Miri in CI.
- **Self-hosting.** The compiler, runtimes and formatter are compiled by RBQN itself from `c.bqn`, `r0.bqn`, `r1.bqn` and `f.bqn`, embedded as bytecode. A fixpoint check confirms that the compiler compiled by itself produces identical bytecode.
- **Typed arrays.** Arrays are stored unboxed as bits, i8, i16, i32, f64, or 8/16/32-bit characters, squeezed to the narrowest exact type, with boxed arrays only for nested data; the primitives have specialised paths for each.
- **GPU dispatch.** Elementwise arithmetic, reductions, scans, sorting, selection, softmax and matrix multiply have wgpu kernels. Arithmetic, folds, scans and `•math.MatMul` dispatch automatically when a cost model says the GPU wins, using exact integer kernels only; the `•gpu` namespace gives explicit f32 kernels and device-resident arrays with lazy fusion. See [GPU](#gpu).
- **`•FFI`.** The full specification: numeric, pointer (`*`, mutable `&`), array and struct types, `>` and `𝕨`/`𝕩` argument markers, `a` for BQN values, pointer objects with `Read Write Add Sub Cast Field`, calls through libffi on the platform C ABI. `f←"@" •FFI "&"‿"memset"‿"&u8"‿"i32"‿"u64" ⋄ F ⟨0‿0‿0‿0, 7, 3⟩` gives `⟨ 7 7 7 0 ⟩`.
- **Errors with position.** Every error names the primitive and the shapes involved, then the file and line, the source line and a caret under the failing token.
- **Recursion** is bounded by the 512 MB interpreter stack, not a call count. Runaway recursion, and native recursion on deeply nested arrays, raise `Stack overflow` instead of crashing.
- **System values.** `•BQN`, `•Import`, `•Show`, `•Out`, `•Fmt`, `•Repr`, `•Type`, `•Decompose`, `•Glyph`, `•Fill`, `•CurrentError`, `•Hash`, `•HashMap`, `•ParseFloat`, `•FromUTF8`, `•ToUTF8`, `•UnixTime`, `•MonoTime`, `•Delay`, `•SH`, `•Exit`, `•args`, `•path`, `•name`, `•wdpath`, `•state`, `•rand`, `•platform`, `•GroupLen`, `•GroupOrd`, `•FLines`, `•FChars`, `•FBytes`, `•PrimInd`, `•ReBQN`.
- **Namespaces.** `•math` (`Sin` `Cos` `Tan` `ASin` `ACos` `ATan` `Log` `Cbrt` `Hypot` `Erf` `Comb` `MatMul` `Softmax` `Pi`), `•gpu`, `•file` (`Lines` `Chars` `Bytes` `List` `At` `Name` `Parent` `Exists` `Type` `CreateDir` `Remove` `Rename`), `•term` (`RawMode` `CharB` `CharN` `Flush` `OutRaw` `ErrRaw`), `•bit` (`_cast` `_not` `_neg` and the binary operations across widths).
- **A REPL** (`-r`) with history, backslash glyph input (`\r` gives `↑`, the standard BQN keymap) and Tab completion of system values and session names; plus script, expression and file modes.

## Performance

`bench/compare.sh`, hyperfine, mean wall time in ms including process startup, Apple M3 Pro, 2026-10-19, RBQN at commit 9595606. CBQN f4257c14 is shown as a reference point, not a target.

| expression | RBQN | CBQN |
|---|---:|---:|
| `1` (startup) | 4.7 | 3.1 |
| `+´↕10000000` | 5.9 | 6.6 |
| `+´×˜↕10000000` | 21.9 | 20.4 |
| `+´ 2 × ↕5000000` | 5.4 | 7.1 |
| `+´ 5000000⥊1` | 4.5 | 3.4 |
| ``+´ +` ↕5000000`` | 19.3 | 14.4 |
| `+´ {𝕩+1}¨ ↕1000000` | 24.1 | 20.4 |
| `+´ {𝕩×𝕩}¨ ↕1000000` | 23.1 | 21.1 |
| `{𝕩<2 ? 𝕩 ; (𝕊 𝕩-1)+𝕊 𝕩-2} 27` | 48.9 | 34.4 |
| `≠ ⊐ 3000000⥊↕1000` | 8.2 | 5.6 |
| `+´⥊ (↕3000) ×⌜ ↕3000` | 11.9 | 7.4 |
| `+´ (⊢ ⍋⊸⊏ ⊢) 1000000⥊3‿1‿2` | 7.8 | 5.2 |
| `≠ ∾ 100000⥊⟨"ab","cde"⟩` | 5.4 | 4.3 |
| `≠ •Fmt ↕100000` | 6.6 | 25.7 |
| `≠ •Fmt ↕1000000` | 23.1 | 226.3 |

Scalar code through the VM (the fib row) is where the most room remains. `bench/` holds the history of every measured step, and `bench/pgo.sh` builds a profile-guided binary that is 10 to 16% faster on VM-bound code (`bench/pgo.md`).

## GPU

Two layers, kept separate so that nothing outside `•gpu` ever changes a result.

**Automatic dispatch.** Arithmetic, folds, scans, sorts and `•math.MatMul` may run on the GPU. Only integer kernels whose result is exact are used, so the answer is the same as the CPU path; f64 arithmetic stays on the CPU (Metal has no f64). A per-operation cost model compares the modelled GPU time (including any one-time device initialisation not yet paid) with the modelled CPU time and picks the cheaper; `RBQN_GPU_DEBUG=1` prints each decision. `tests/gpu/corpus.txt` runs 87 expressions under forced GPU and CPU-only and requires identical output (`make gpu-corpus`). `make gpu-parity` runs the stranger docs corpus and grid with `RBQN_GPU=force` and `RBQN_GPU=off` on the same binary and requires identical reports; it needs `CBQN_BIN` and `BQN_SRC`, and is run on a machine with a GPU, not in CI. The CBQN parity job in CI is a regression floor, not the goal: the spec is the target, and deliberate divergences beyond CBQN are recorded with a spec citation in `bench/stranger/known_divergences.txt`.

**`•gpu`.** Explicit f32 kernels and device-resident arrays, for code that accepts f32 rounding in exchange for speed.

| function | does |
|---|---|
| `w •gpu.MatMul x` | rank-2 `w` (m×k) times rank-2 `x` (k×n), f32 kernel |
| `•gpu.Softmax x` | softmax of a rank-1 `x`, or of each row of a rank-2 `x`, f32 kernel |
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

## Usage

```
rbqn [options] [file.bqn [arguments]]

Options:
  -f file    execute the contents of the file with all further arguments as •args
  -e code    execute the argument as BQN
  -p code    execute the argument as BQN and print its result pretty-printed
  -o code    execute the argument as BQN and print its raw result
  -M num     accepted for CBQN compatibility; no heap limit is enforced yet
  -r         start the REPL after executing all arguments
  -s         start a silent REPL
  --no-gpu   disable GPU acceleration
  --help     show this help text
  --version  display version information (including `gpu:` adapter name, `off` or `none`)
```

Environment: `RBQN_GPU=off|force` overrides the dispatch decision (`force` bypasses the cost model only); `RBQN_GPU_DEBUG=1` traces it.

## Build

```bash
cargo build --release          # target/release/rbqn
cargo build --release --no-default-features   # CPU-only, no wgpu
```

Rust 1.88 or later (edition 2024). The only dynamic dependencies beyond the C runtime are libffi (`libffi.so.8` on Linux, e.g. `apt install libffi8`; part of the system on macOS) and libgcc_s; `ldd target/release/rbqn` on Linux lists `libffi.so.8 libgcc_s.so.1 libm.so.6 libc.so.6`. The BQN compiler and runtime are embedded as bytecode, so nothing else is needed to build or run. Or install from crates.io: `cargo install rbqn`.

## Architecture

About 30K lines of Rust in five crates:

| Crate | Purpose |
|-------|---------|
| `rbqn` | CLI, REPL, bootstrap, `•Import`/`•BQN`, GPU dispatch decisions |
| `rbqn-core` | Value representation (NaN-boxed `B`), typed arrays, errors |
| `rbqn-prim` | Primitives: arithmetic, structural, search, sort, group |
| `rbqn-vm` | Bytecode VM, blocks and scopes, modifiers, derived functions, system values |
| `rbqn-gpu` | wgpu context, buffer pool, pipeline cache, WGSL kernels |

## Bootstrap

Bytecode files in `crates/rbqn/bins/` are compiled from the BQN sources (`c.bqn`, `r0.bqn`, `r1.bqn`, `f.bqn`) by RBQN itself and loaded with `include_bytes!`.

```bash
# Regenerate bins from updated BQN sources
BQN_SRC=/path/to/BQN/src cargo run -p rbqn --features gen-tools --bin rbqn-gen -- --self

# Verify fixpoint: the compiler compiled by itself produces identical bytecode
BQN_SRC=/path/to/BQN/src cargo run -p rbqn --features gen-tools --bin rbqn-gen -- --self --fixpoint-check
```

## Testing

```bash
cargo test --workspace --release                  # unit and integration tests (•FFI tests compile tests/ffi/lib.c with cc)
BQN_TEST_DIR=/path/to/BQN/test ./test_suite.sh    # official BQN test suite
make gpu-corpus                                   # GPU kernels vs CPU, 87 expressions
```

CI runs all three plus clippy with `-D warnings` on every push, against a pinned BQN revision. The suite result at this commit:

```
simple       PASS|20|0      fill         PASS|62|0
literal      PASS|52|0      header       PASS|156|0
syntax       PASS|156|0     under        PASS|64|0
bytecode     PASS|37|0      undo         PASS|68|0
token        PASS|29|0      namespace    PASS|50|0
identity     PASS|14|0      unhead       PASS|44|0
prim         PASS|564|0
```

## Acknowledgements

**BQN** was designed by [Marshall Lochbaum](https://mlochbaum.github.io/). The language specification, the compiler and runtime sources (`c.bqn`, `r0.bqn`, `r1.bqn`, `f.bqn`) and the test suite are his work, under the ISC License. RBQN embeds bytecode compiled from those sources.

**CBQN**, by [dzaima](https://github.com/dzaima) and the BQN community, provided the first bytecode RBQN ran before it became self-hosting, and its documented internals were reference material for the bytecode format and runtime structure.

- BQN: https://mlochbaum.github.io/BQN/
- BQN community: https://mlochbaum.github.io/BQN/community/
- CBQN: https://github.com/dzaima/CBQN
