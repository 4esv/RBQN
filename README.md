# RBQN

A Rust implementation of the [BQN](https://mlochbaum.github.io/BQN/) array language that attempts to get the GPU involved.

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

RBQN is self-hosting. The BQN compiler, runtime, and formatter are compiled by RBQN itself. No external BQN implementation is needed to build.

Bytecode files in `bins/` are compiled from the BQN source files (`c.bqn`, `r0.bqn`, `r1.bqn`, `f.bqn`) and loaded via `include_bytes!` at compile time.

**Fixpoint verification:** RBQN's self-compilation is deterministic — recompiling the BQN sources with RBQN-compiled bins produces byte-for-byte identical output (fixpoint at round 2).

```bash
BQN_SRC=/path/to/BQN/src cargo run -p rbqn --features gen-tools --bin rbqn-gen -- --self --fixpoint-check
```

**Developer workflow:** To regenerate bins from updated BQN sources:

```bash
BQN_SRC=/path/to/BQN/src cargo run -p rbqn --features gen-tools --bin rbqn-gen -- --self
git add bins/ && git commit
```

## Acknowledgements

**BQN** was designed by [Marshall Lochbaum](https://mlochbaum.github.io/). The language spec, compiler source (`c.bqn`, `r0.bqn`, `r1.bqn`, `f.bqn`), and reference test suite are his work and are licensed under the ISC License. RBQN embeds bytecode compiled from those source files — the language itself is Marshall's creation.

**CBQN** is the reference C implementation of BQN, written by [dzaima](https://github.com/dzaima) with contributions from the BQN community. RBQN was originally bootstrapped from CBQN-compiled bytecode and is now self-hosting. CBQN's internals — the bytecode format, provide array layout, and runtime structure documented in `load.c` — were essential reference material throughout development.

- BQN: https://mlochbaum.github.io/BQN/
- CBQN: https://github.com/dzaima/CBQN
- BQN community: https://mlochbaum.github.io/BQN/community/

## Version

`rbqn --version` shows the version and bytecode source:

```
rbqn 0.1.0
bytecode: cbqn
```

The `bytecode` field is `rbqn-self` for self-compiled bytecode (the default) or `cbqn` for legacy CBQN-bootstrapped bytecode.
