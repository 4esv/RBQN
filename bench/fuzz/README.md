# Fuzzing

Three cargo-fuzz (libFuzzer) targets in `fuzz/`, all through the embed API (`rbqn::Bqn`):

| target | input | checks |
|---|---|---|
| `expr` | bytes pick a primitive, modifier and argument shapes from the lists in `bench/stranger/grid.py` (`bench/fuzz/check_lists.py` fails if they drift), one to three statements, argument nesting up to depth 2 | each statement runs under `⎊`, the last through `•Fmt`; a Rust panic (anything that is not a `BqnError`), an abort or a run over 2 s is a crash |
| `fmt` | rank 0 to 2 arrays of arbitrary f64 or char data | `•Fmt`, and `•BQN •Repr x` must equal `x` |
| `bytes` | arbitrary bytes as a string | `•BQN` (tokenizer, parser, compiler, run) must only raise BQN errors; inputs containing `•` are skipped so random text cannot reach `•Exit` or `•file` |

The embed API turns every panic into a BQN error. The fuzz crate installs a panic hook after
warm-up that aborts on any payload that is not a `BqnError`, so a Rust panic in a primitive is a crash.

## Run

```
cargo install cargo-fuzz                      # once; needs the nightly toolchain
bench/fuzz/run.sh expr 120                    # smoke, what CI runs (fmt and bytes: 60)
bench/fuzz/run.sh expr 86400 2>&1 | grep --line-buffered -v NEW_FUNC > bench/fuzz/logs/$(date +%F).txt   # long run
```

`run.sh` uses `-fork=1`: every eval leaks about 1 MB (the interpreter does not free what the
self-hosted compiler allocates) and costs about 40 ms, so a single process reaches the 2 GB limit
after about 2000 execs. Fork mode restarts the child between jobs. It also means an `oom-*` file
can be a leak and not a bug: replay it alone (`fuzz/target/*/release/expr oom-...`); only one
that still fails alone is real.

Crash artifacts land in `fuzz/artifacts/<target>/`. Minimise with
`cargo +nightly fuzz tmin <target> <file>`, fix at the root or file an issue with the minimised
input, and add the case to `crates/rbqn/tests/fuzz_regressions.rs`.

## Logs

`bench/fuzz/logs/<date>.txt`: the libFuzzer output of each long run (without `NEW_FUNC` lines),
with a header giving the commit and command.
