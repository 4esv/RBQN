# RBQN

A Rust implementation of the [BQN](https://mlochbaum.github.io/BQN/) array language, built to let BQN use the whole machine: typed arrays, a bytecode VM, and GPU dispatch for array operations.

RBQN passes the official BQN test suite and is self-hosting: it compiles its own compiler, runtime and formatter from the BQN sources, with no other BQN implementation involved. The goal is a BQN that leverages compute as far as it safely can. The language is Marshall Lochbaum's; the implementation goes its own way.

Status: pre-1.0. The language is complete; the system namespaces (`•file`, `•FFI`, `•term`) are partial. See [Limitations](#limitations).

## What works

- **The language.** Every primitive, modifier and combinator; headers, predicates, multiple bodies, namespaces, `⁼` for the invertible primitives, `⌾` and `⚇`. The official test suite passes in full (13 files, 1316 cases, see [Testing](#testing)).
- **Self-hosting.** The compiler, runtimes and formatter are compiled by RBQN itself from `c.bqn`, `r0.bqn`, `r1.bqn` and `f.bqn`, embedded as bytecode. A fixpoint check confirms that the compiler compiled by itself produces identical bytecode.
- **Typed arrays.** Arrays are stored unboxed as bits, i8, i16, i32, f64, or 8/16/32-bit characters, squeezed to the narrowest exact type, with boxed arrays only for nested data; the primitives have specialised paths for each.
- **GPU dispatch.** Elementwise arithmetic, reductions, scans, sorting, selection, softmax and matrix multiply have wgpu kernels. Arithmetic, folds, scans and `•math.MatMul` dispatch automatically when a cost model says the GPU wins, using exact integer kernels only; the `•gpu` namespace gives explicit f32 kernels and device-resident arrays with lazy fusion. See [GPU](#gpu).
- **System values.** `•BQN`, `•Import`, `•Show`, `•Out`, `•Fmt`, `•Repr`, `•Type`, `•Decompose`, `•Glyph`, `•Fill`, `•CurrentError`, `•Hash`, `•HashMap`, `•ParseFloat`, `•FromUTF8`, `•ToUTF8`, `•UnixTime`, `•MonoTime`, `•Delay`, `•SH`, `•Exit`, `•args`, `•path`, `•name`, `•wdpath`, `•state`, `•rand`, `•platform`, `•GroupLen`, `•GroupOrd`, `•FLines`, `•FChars`, `•FBytes`, `•PrimInd`, `•ReBQN`.
- **Namespaces.** `•math` (`Sin` `Cos` `Tan` `ASin` `ACos` `ATan` `Log` `Cbrt` `Hypot` `Erf` `Comb` `MatMul` `Softmax` `Pi`), `•gpu`, `•file` (`Lines` `Chars` `Bytes` `List` `At` `Name` `Parent` `Exists` `Type` `CreateDir` `Remove` `Rename`), `•term` (`RawMode` `CharB` `CharN` `Flush` `OutRaw` `ErrRaw`), `•bit` (`_cast` `_not` `_neg` and the binary operations across widths).
- **A REPL** (`-r`), and script, expression and file modes.

## Limitations

- `•FFI` is declared but not functional.
- `•file.Open` is not implemented.
- The REPL has no completion or history.
- Error messages name the primitive and the shapes involved but carry no source position.
- Recursion depth is bounded by the 512 MB interpreter stack, not a call count. Native recursion on deeply nested arrays (comparison, hashing, sorting, arithmetic) raises `Stack overflow` rather than crashing.
- Large floats print with full precision (`1e308×10` gives `∞`, `1e308` prints all digits).
- GPU sort and grade are gated at 4M elements, where the kernel beats the CPU counting sort. The radix kernel is wrong on small inputs and must stay behind that gate.
- GPU kernels are f32 or integer. Float arithmetic that must stay f64 never dispatches automatically.

## Performance

`bench/compare.sh`, hyperfine, mean wall time in ms including process startup, Apple M3 Pro, 2026-10-19, RBQN at commit 28629e0. CBQN f4257c14 is shown as a reference point, not a target.

| expression | RBQN | CBQN |
|---|---:|---:|
| `1` (startup) | 4.8 | 3.4 |
| `+´↕10000000` | 6.7 | 7.9 |
| `+´×˜↕10000000` | 24.2 | 22.1 |
| `+´ 2 × ↕5000000` | 5.9 | 8.2 |
| `+´ 5000000⥊1` | 5.7 | 3.4 |
| ``+´ +` ↕5000000`` | 22.0 | 15.8 |
| `+´ {𝕩+1}¨ ↕1000000` | 25.0 | 21.5 |
| `+´ {𝕩×𝕩}¨ ↕1000000` | 25.5 | 27.8 |
| `{𝕩<2 ? 𝕩 ; (𝕊 𝕩-1)+𝕊 𝕩-2} 27` | 55.7 | 36.1 |
| `≠ ⊐ 3000000⥊↕1000` | 8.7 | 6.2 |
| `+´⥊ (↕3000) ×⌜ ↕3000` | 13.0 | 8.2 |
| `+´ (⊢ ⍋⊸⊏ ⊢) 1000000⥊3‿1‿2` | 8.5 | 6.3 |
| `≠ ∾ 100000⥊⟨"ab","cde"⟩` | 6.3 | 4.7 |
| `≠ •Fmt ↕100000` | 7.3 | 27.0 |
| `≠ •Fmt ↕1000000` | 25.8 | 244.9 |

Scalar code through the VM (the fib row) is where the most room remains. `bench/` holds the history of every measured step, and `bench/pgo.sh` builds a profile-guided binary that is 10 to 16% faster on VM-bound code (`bench/pgo.md`).

## GPU

Two layers, kept separate so that nothing outside `•gpu` ever changes a result.

**Automatic dispatch.** Arithmetic, folds, scans, sorts and `•math.MatMul` may run on the GPU. Only integer kernels whose result is exact are used, so the answer is the same as the CPU path. A per-operation cost model compares the modelled GPU time (including any one-time device initialisation not yet paid) with the modelled CPU time and picks the cheaper; `RBQN_GPU_DEBUG=1` prints each decision. `tests/gpu/corpus.txt` runs 87 expressions under forced GPU and CPU-only and requires identical output (`make gpu-corpus`).

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
  -M num     set maximum heap size to num megabytes
  -r         start the REPL after executing all arguments
  -s         start a silent REPL
  --no-gpu   disable GPU acceleration
  --help     show this help text
  --version  display version information
```

Environment: `RBQN_GPU=off|force` overrides the dispatch decision; `RBQN_GPU_DEBUG=1` traces it.

## Build

```bash
cargo build --release          # target/release/rbqn
cargo build --release --no-default-features   # CPU-only, no wgpu
```

Rust 1.85 or later (edition 2024), no external dependencies. The BQN compiler and runtime are embedded as bytecode, so nothing else is needed to build or run. RBQN is not yet on crates.io.

## Architecture

About 29K lines of Rust in five crates:

| Crate | Purpose |
|-------|---------|
| `rbqn` | CLI, REPL, bootstrap, `•Import`/`•BQN`, GPU dispatch decisions |
| `rbqn-core` | Value representation (NaN-boxed `B`), typed arrays, errors |
| `rbqn-prim` | Primitives: arithmetic, structural, search, sort, group |
| `rbqn-vm` | Bytecode VM, blocks and scopes, modifiers, derived functions, system values |
| `rbqn-gpu` | wgpu context, buffer pool, pipeline cache, WGSL kernels |

## Bootstrap

Bytecode files in `bins/` are compiled from the BQN sources (`c.bqn`, `r0.bqn`, `r1.bqn`, `f.bqn`) by RBQN itself and loaded with `include_bytes!`.

```bash
# Regenerate bins from updated BQN sources
BQN_SRC=/path/to/BQN/src cargo run -p rbqn --features gen-tools --bin rbqn-gen -- --self

# Verify fixpoint: the compiler compiled by itself produces identical bytecode
BQN_SRC=/path/to/BQN/src cargo run -p rbqn --features gen-tools --bin rbqn-gen -- --self --fixpoint-check
```

## Testing

```bash
cargo test --workspace --release                  # unit and integration tests
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
