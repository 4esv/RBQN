# RBQN

A Rust implementation of the [BQN](https://mlochbaum.github.io/BQN/) array language.

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

## Bootstrap

RBQN ships with pre-compiled bytecode committed to the repository. This eliminates the CBQN build dependency from normal users.

**Bootstrap chain:** CBQN compiles the BQN compiler source files (`c.bqn`, `r0.bqn`, `r1.bqn`, `f.bqn`) into bytecode. That bytecode is stored as `.bin` files in `crates/rbqn/src/embedded/` and committed to the repo. RBQN loads them via `include_bytes!` at compile time — no CBQN needed to build.

**Self-hosting verification:** RBQN can compile `c.bqn` (the BQN compiler source) using its own compiler. Run with `--verify`:

```bash
CBQN_PATH=/path/to/cbqn BQN_SRC=/path/to/BQN/src \
  cargo run --bin rbqn-gen --features gen-tools -- --verify
```

This bootstraps RBQN from the committed `.bin` files, then compiles the BQN compiler source, confirming RBQN can reproduce its own compiler.

**Developer workflow:** To regenerate `.bin` files from updated CBQN sources:

```bash
CBQN_PATH=/path/to/cbqn cargo run --bin rbqn-gen --features gen-tools
# Commit the updated .bin files
git add crates/rbqn/src/embedded/*.bin && git commit
```

The `rbqn-gen` tool requires `CBQN_PATH` pointing to a built CBQN directory. It is not compiled during normal `cargo build` — only when the `gen-tools` feature is enabled.

## Version

`rbqn --version` shows the version and bytecode source:

```
rbqn 0.1.0
bytecode: cbqn
```

The `bytecode` field is `cbqn` for CBQN-bootstrapped bytecode (the default) and `rbqn-self` for self-compiled bytecode.
