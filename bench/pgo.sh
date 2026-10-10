#!/bin/bash
# Profile-guided release build of rbqn (issue #22).
# Usage: bench/pgo.sh   -> writes target/pgo/release/rbqn (target/release is left alone)
# Needs: rustup component add llvm-tools
# Training run: every bench/compare.sh expression, every tests/parity line, and the test suite.
set -eu
cd "$(dirname "$0")/.."
ROOT=$PWD
PROF="$ROOT/target/pgo/profiles"
SYSROOT=$(rustc --print sysroot)
PROFDATA=$(ls "$SYSROOT"/lib/rustlib/*/bin/llvm-profdata 2>/dev/null | head -1)
[ -x "$PROFDATA" ] || { echo "llvm-profdata missing: rustup component add llvm-tools" >&2; exit 1; }
rm -rf "$PROF"; mkdir -p "$PROF"

# 1. Instrumented build.
GEN=(-Cprofile-generate="$PROF")
RUSTFLAGS="${GEN[*]}" cargo build --release -p rbqn --target-dir target/pgo/gen
BIN=target/pgo/gen/release/rbqn

# 2. Training run.
sed -n "/^EXPRS=(/,/^)/p" bench/compare.sh | sed -n "s/^  '\(.*\)'$/\1/p" | while IFS= read -r x; do
  "$BIN" -p "$x" >/dev/null 2>&1 || true
done
cat tests/parity/*.txt | grep -v '^#' | grep -v '^$' | while IFS= read -r x; do
  "$BIN" -p "$x" >/dev/null 2>&1 || true
done
RUSTFLAGS="${GEN[*]}" cargo test --release --workspace --target-dir target/pgo/gen -q >/dev/null 2>&1 || true

# 3. Merge and rebuild with the profile.
"$PROFDATA" merge -o "$PROF/merged.profdata" "$PROF"
RUSTFLAGS="-Cprofile-use=$PROF/merged.profdata" cargo build --release -p rbqn --target-dir target/pgo
echo "PGO binary: target/pgo/release/rbqn"
