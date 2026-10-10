# PGO build (issue #22)

`bench/pgo.sh` builds a profile-guided `target/pgo/release/rbqn` on top of the release profile
(LTO fat, codegen-units=1). Plain `cargo install rbqn` and `cargo build --release` do not get it.

```bash
rustup component add llvm-tools   # provides llvm-profdata
bench/pgo.sh                      # instrumented build -> training run -> profile-use build (~2 min)
target/pgo/release/rbqn -p '+´↕10'
```

Training run: every `bench/compare.sh` expression, every `tests/parity` line, then
`cargo test --release --workspace` built with the same instrumentation.

## Result (2026-10-10, Apple Silicon, rustc 1.98.1)

Median of 21 interleaved runs per binary (base, PGO, held-out PGO alternated per iteration).
The machine was shared (load avg ~8), so absolute ms are inflated; deltas are the signal.
"Held-out" is the same build trained on parity + tests only, without the bench expressions.

| expression | base ms | pgo ms | Δ | pgo held-out ms | Δ |
|---|---:|---:|---:|---:|---:|
| `1` | 9.3 | 9.4 | +1% | 9.2 | -1% |
| `+´↕10000000` | 10.8 | 10.5 | -3% | 10.6 | -2% |
| `+´×˜↕10000000` | 35.8 | 35.6 | -1% | 35.7 | -0% |
| `+´ {𝕩+1}¨ ↕1000000` | 36.3 | 34.0 | -6% | 33.3 | -8% |
| `{𝕩<2 ? 𝕩 ; (𝕊 𝕩-1)+𝕊 𝕩-2} 25` | 27.8 | 24.4 | -12% | 26.8 | -4% |
| `≠ ⊐ 3000000⥊↕1000` | 13.6 | 12.8 | -5% | 15.2 | +12% |
| `+´⥊ (↕3000) ×⌜ ↕3000` | 25.4 | 25.3 | -0% | 27.2 | +7% |
| `+´ (⊢ ⍋⊸⊏ ⊢) 1000000⥊3‿1‿2` | 13.6 | 13.2 | -3% | 13.4 | -2% |
| `≠ •Fmt ↕100000` | 11.9 | 11.4 | -4% | 12.3 | +3% |
| `+´ +` ↕5000000` | 36.3 | 36.4 | +0% | 35.0 | -4% |
| `+´ 2 × ↕5000000` | 12.8 | 12.9 | +1% | 13.3 | +4% |
| `≠ ∾ 100000⥊⟨"ab","cde"⟩` | 11.9 | 11.0 | -8% | 11.2 | -6% |
| `+´ 5000000⥊1` | 11.1 | 10.4 | -6% | 10.8 | -3% |
| `≠ •Fmt ↕1000000` | 31.5 | 28.8 | -9% | 32.5 | +3% |
| `{𝕩<2 ? 𝕩 ; (𝕊 𝕩-1)+𝕊 𝕩-2} 27` | 75.8 | 66.1 | -13% | 76.2 | +0% |
| `+´ {𝕩×𝕩}¨ ↕1000000` | 77.7 | 67.0 | -14% | 62.6 | -19% |

geomean Δ: pgo -5.2%, pgo held-out -1.4%  (median of 21 interleaved runs per binary)

An earlier two-order `bench/compare.sh` run (base vs PGO, 5 runs each) agreed: geomean −5.7%,
fib 27 −16%, `•Fmt ↕1000000` −14%; hyperfine at 30 runs: fib 27 1.19±0.02× and
`•Fmt ↕1000000` 1.18±0.02× faster.

Correctness: PGO and base binaries print identical output on all bench and parity expressions
(491 lines); `cargo test --release --workspace` passes.

## Verdict

The gain depends on the training set. Trained on the bench, PGO gives ~5% geomean and 10–16% on
interpreter-bound rows (recursive blocks, `¨` with a block, `•Fmt`). Trained without it, the
geomean (−1.4%) is within noise and some rows regress (`⊐` +12%). Array kernels (`+´`, `×⌜`,
`+``) do not move either way. The build is worth using when the workload looks like the training
run; it is not a default, and it is not wired into `cargo install`.
