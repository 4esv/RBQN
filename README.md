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

Multicore, 1e8 elements, `bench/wow/run.sh` against CBQN (12-core Intel Mac, 2026-10-24, [full output](bench/wow/results/2026-10-24-bench-intel12.md), which does not record the machine's load; a quiet-machine re-measure is #119):

| workload | CBQN | rbqn -t 1 | rbqn -t 12 | speedup vs -t 1 | result agrees |
|---|---|---|---|---|---|
| `+´` 1e8 i32 | 0.030 s (0.27 s) | 0.035 s (0.31 s) | 0.019 s (0.31 s) | 1.83x | yes |
| `(a×b)+c` 1e8 f64 | 1.199 s (3.69 s) | 1.779 s (4.11 s) | 0.569 s (3.06 s) | 3.12x | yes |
| `⌈´` 1e8 f64 | 0.129 s (1.06 s) | 0.299 s (1.30 s) | 0.085 s (1.23 s) | 3.52x | yes |

Cells: median in-process seconds for the operation alone (median whole-process wall seconds).

RBQN passes the official BQN test suite and is self-hosting: it compiles its own compiler, runtime and formatter from the BQN sources, with no other BQN implementation involved. The goal is a BQN that leverages compute as far as it safely can. The language is Marshall Lochbaum's; the implementation goes its own way.

Status: pre-1.0, complete language, no known divergences from the specification. `•file.Open` is unspecified in BQN; here it raises `not yet implemented`.

## What works

- **The language.** Every primitive, modifier and combinator; headers, predicates, multiple bodies, namespaces, `⁼` for the invertible primitives, `⌾` and `⚇`. The official test suite passes in full (13 files, 1316 cases at the pinned BQN revision, see [Testing](#testing)). The workspace has 50 `unsafe` sites (`bench/unsafe_census.sh`); every `unsafe` block carries a `// SAFETY:` comment, enforced by `clippy::undocumented_unsafe_blocks = "deny"`; rbqn-core, rbqn-prim and rbqn-vm pass under Miri in CI (the `miri` job).
- **Self-hosting.** The compiler, runtimes and formatter are compiled by RBQN itself from `c.bqn`, `r0.bqn`, `r1.bqn` and `f.bqn`, embedded as bytecode. A fixpoint check confirms that the compiler compiled by itself produces identical bytecode.
- **Typed arrays.** Arrays are stored unboxed as bits, i8, i16, i32, f64, or 8/16/32-bit characters, squeezed to the narrowest exact type, with boxed arrays only for nested data; the primitives have specialised paths for each.
- **GPU dispatch.** Elementwise arithmetic, reductions, scans, sorting, selection, softmax and matrix multiply have wgpu kernels. Arithmetic, folds, scans, sorts and `•math.MatMul` dispatch automatically, per call, when a cost model calibrated on the machine says the GPU wins, using exact integer kernels only, so the result is the same on either path; the `•gpu` namespace gives explicit f32 kernels and device-resident arrays with lazy fusion. See [GPU](#gpu).
- **`•FFI`.** The full specification: numeric, pointer (`*`, mutable `&`), array and struct types, `>` and `𝕨`/`𝕩` argument markers, `a` for BQN values, pointer objects with `Read Write Add Sub Cast Field`, calls through libffi on the platform C ABI. `f←"@" •FFI "&"‿"memset"‿"&u8"‿"i32"‿"u64" ⋄ F ⟨0‿0‿0‿0, 7, 3⟩` gives `⟨ 7 7 7 0 ⟩`.
- **Errors with position.** Every error names the primitive and the shapes involved, then the file and line, the source line and a caret under the failing token.
- **Recursion** is bounded by the 512 MB interpreter stack, not a call count. Runaway recursion, and native recursion on deeply nested arrays, raise `Stack overflow` instead of crashing.
- **System values.** `•BQN`, `•Import`, `•Show`, `•Out`, `•Fmt`, `•Repr`, `•Type`, `•Decompose`, `•Glyph`, `•Fill`, `•CurrentError`, `•Hash`, `•HashMap` (`Count` `Keys` `Has` `Get` `Set` `Delete` `Values`), `•ns` (`Keys` `Values` `Has` `Get`; `Keys` and `Values` follow definition order), `•ParseFloat`, `•FromUTF8`, `•ToUTF8`, `•UnixTime`, `•MonoTime`, `•Delay`, `•SH`, `•Exit`, `•args`, `•path`, `•name`, `•wdpath`, `•state`, `•rand`, `•MakeRand` (a seed gives the same sequence in every process), `•platform`, `•GroupLen`, `•GroupOrd`, `•FLines`, `•FChars`, `•FBytes`, `•PrimInd`, `•ReBQN`.
- **Namespaces.** `•math` (`Sin` `Cos` `Tan` `ASin` `ACos` `ATan` `Log` `Cbrt` `Hypot` `Erf` `ErfC` `Comb` `Fact` `GCD` `LCM` `MatMul` `Softmax` `Pi` `Sinh` `Cosh` `Tanh` `Asinh` `Acosh` `Atanh` `LogFact` `Sum` `Atan2` `Expm1` `Log1p` `Log2` `Log10`), `•gpu`, `•file` (`Lines` `Chars` `Bytes` `List` `At` `Name` `Parent` `Exists` `Type` `CreateDir` `Remove` `Rename` `path` `Size` `Created` `Modified` `Accessed` `RealPath` `MapBytes`), `•term` (`RawMode` `CharB` `CharN` `Flush` `OutRaw` `ErrRaw`), `•bit` (`_cast` `_not` `_neg` and the binary operations across widths).
- **A REPL** (`-r`, or `rbqn` with no arguments) that prints a header line (version, threads, GPU adapter, `)help`) on a terminal, with history, backslash glyph input (`\r` gives `↑`, the standard BQN keymap), Tab completion of system values and session names, `)help`, `)off` or `)exit` or Ctrl-D to leave, and pasted multi-line programs evaluated line by line (`crates/rbqn/tests/repl_first_contact.rs`); plus script, expression and file modes.

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

**Automatic dispatch.** Arithmetic, folds, scans, sorts and `•math.MatMul` may run on the GPU. Only integer kernels whose result is exact are used, so the answer is the same as the CPU path; f64 arithmetic stays on the CPU (Metal has no f64). A per-operation cost model compares the modelled GPU time (including any one-time device initialisation not yet paid) with the modelled CPU time and picks the cheaper on each call; `RBQN_GPU_DEBUG=1` prints each decision. The model's constants are measured per machine at first device use (small GPU and host workloads timed at two sizes, `crates/rbqn/src/calibrate.rs`) and cached by adapter, driver and thread count under `RBQN_CACHE_DIR` (default: the platform cache directory); `RBQN_GPU_CALIBRATE=1` measures again (`crates/rbqn/tests/gpu_calibrate.rs`). `tests/gpu/corpus.txt` runs 87 expressions under forced GPU and CPU-only and requires identical output (`make gpu-corpus`). `make gpu-parity` runs the stranger docs corpus and grid with `RBQN_GPU=force` and `RBQN_GPU=off` on the same binary and requires identical reports; it needs `CBQN_BIN` and `BQN_SRC`, and is run on a machine with a GPU, not in CI. The CBQN parity job in CI is a regression floor, not the goal: the spec is the target, and deliberate divergences beyond CBQN are recorded with a spec citation in `bench/stranger/known_divergences.txt`.

The device is used only when the cost model expects it to win, and results are identical either way. Median of 3 in-process seconds on an AMD Radeon Pro 5300M (Metal), from `bench/wow/results/2026-10-24-bench-matmul-cpu-166.md` (`bench/wow/run.sh`; that file records the load and foreign CPU of the run, which was shared with other builds: load1 16 at start, foreign CPU peaked at 533%, so the large ratios are the evidence, not the small ones; a quiet-machine re-measure is #119):

<!-- gpu-table -->
| workload | CBQN | rbqn CPU `-t 1` (`RBQN_GPU=off`) | rbqn CPU (`RBQN_GPU=off`) | rbqn GPU (`RBQN_GPU=force`) | rbqn default | default ran on |
|---|---|---|---|---|---|---|
| sort `∧` 1e6 i32 | 0.016 s | 0.033 s | 0.035 s | 0.046 s | 0.035 s | CPU |
| sort `∧` 1e7 i32 | 0.173 s | 0.474 s | 0.510 s | 0.472 s | 0.479 s | CPU |
| sort `∧` 1e8 i32 | 2.462 s | 6.003 s | 5.583 s | 5.310 s | 5.568 s | CPU |
| `•math.MatMul` 1000² (entries below 10) | 0.490 s | 0.198 s | 0.054 s | 0.107 s | 0.056 s | CPU |
| `•math.MatMul` 2000² | 5.575 s | 1.869 s | 0.295 s | 0.805 s | 0.331 s | CPU |
| `•math.MatMul` 4096² | 42.774 s | 15.590 s | 2.569 s | 5.834 s | 2.919 s | CPU |
| `+´˘` 1e3×1e3 i32 | 0.000 s | 0.003 s | 0.003 s | 1.584 s | 0.003 s | CPU |
| `+´˘` 3e3×3e3 i32 | 0.003 s | 0.022 s | 0.019 s | 5.289 s | 0.024 s | CPU |
| `+´˘` 1e4×1e4 i32 | 0.028 s | 0.250 s | 0.254 s | 19.261 s | 0.268 s | CPU |
<!-- /gpu-table -->

`•math.MatMul` and the idiom `+˝∘×⎉1‿∞` run a blocked, packed, multicore f64 kernel on the host (#166): 4096² takes 15.6 s at `-t 1` and 2.6 s on 12 threads against 42.8 s for CBQN's `+˝∘×⎉1‿∞` (a heavily loaded machine; see the results file), up from 704 s for the earlier naive loop (`bench/wow/results/2026-10-24-bench-gpu-rows-radeon5300m.md`). The calibrated model keeps these sizes on the host on this machine; the exact-integer GPU kernel (4096² in 5.8 s when forced here) is chosen only where the model predicts it faster, for example at `-t 1`. Sort and row sums stay on the CPU here. The forced GPU sort (upload and download included) is slower than the host radix sort at 1e6 and about as fast from 1e7 to 1e8, and CBQN's own sort is faster than both (2.462 s at 1e8). There is no batched row-reduce kernel, so a forced `+´˘` runs one reduction per row and takes 19.261 s at 1e4×1e4 against 0.254 s on the host. `RBQN_GPU=force` produces those GPU numbers and `RBQN_GPU_DEBUG=1` prints the model's predicted times next to each decision.

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
- `Tensor` stores f32 when every element is exact in f32, else i32 when every element is an i32 integer, else f32 (rounded). So `•gpu.Fetch •gpu.Tensor x` ≡ `x` for i32 integers and f32-exact floats. An empty `x` stays on the host. `MatMul`/`Softmax` on an i32 tensor convert it through the host (one logged download and upload). Resident device buffers are kept until the process exits (host arrays are freed once nothing references them; the array store is #124).
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
  -t num     use num threads for large array arithmetic (default: RBQN_THREADS or all cores; 1 = serial)
  -r         start the REPL after executing all arguments
  -s         start a silent REPL
  --no-gpu   disable GPU acceleration
  --help     show this help text
  --version  display version information (including `gpu:` adapter name, `off` or `none`)
```

Environment: `RBQN_GPU=off|force` overrides the dispatch decision (`force` bypasses the cost model only); `RBQN_GPU_DEBUG=1` traces it; `RBQN_GPU_CALIBRATE=1` re-measures the cost model and `RBQN_CACHE_DIR` moves its cache; `RBQN_THREADS` sets the thread count.

## Build

```bash
cargo build --release          # target/release/rbqn
cargo build --release --no-default-features   # CPU-only, no wgpu
```

Rust 1.88 or later (edition 2024). The only dynamic dependencies beyond the C runtime are libffi (`libffi.so.8` on Linux, e.g. `apt install libffi8`; part of the system on macOS) and libgcc_s; `ldd target/release/rbqn` on Linux lists `libffi.so.8 libgcc_s.so.1 libm.so.6 libc.so.6`. The BQN compiler and runtime are embedded as bytecode, so nothing else is needed to build or run. Or install from crates.io: `cargo install rbqn`.

## Architecture

About 34K lines of Rust outside `tests/` directories, in five crates:

| Crate | Purpose |
|-------|---------|
| `rbqn` | CLI, REPL, embedding API, bootstrap, `•Import`/`•BQN`, GPU dispatch decisions and calibration |
| `rbqn-core` | Value representation (NaN-boxed `B`, tagged-pointer array handles), typed arrays, errors |
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

CI (`.github/workflows/ci.yml`, on every push to main and every pull request) runs `cargo test --release --workspace` (job `test`, Linux), the official suite and clippy with `-D warnings` (job `build-test`, macOS, against a pinned BQN revision), the docs and grid comparison against a pinned CBQN (job `parity`) and Miri (job `miri`); `make gpu-corpus` and `make gpu-parity` run on a machine with a GPU. The suite result at this commit:

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
