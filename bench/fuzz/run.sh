#!/bin/sh
# Usage: bench/fuzz/run.sh <expr|fmt|bytes> <seconds> [extra libFuzzer flags]
# -fork=1: every eval leaks about 1 MB of arrays (the interpreter never frees what the
# self-hosted compiler allocates), so a single process hits the 2 GB limit within about
# 2000 execs. Fork mode restarts the child between jobs; OOM/timeout artifacts are kept
# (-ignore_timeouts=0 makes a 2 s hang fatal; -ignore_ooms=1 keeps the parent alive when a job outgrows 2 GB, the first long run died on that after 67 min). ASAN_OPTIONS=detect_leaks=0 (and the flag): LeakSanitizer (Linux) would flag the
# deliberate array leak on the first exec. Replay an oom-* artifact alone before filing it.
set -e
PATH="$HOME/.cargo/bin:$PATH" # rustup shims first: `cargo +nightly` needs them
export ASAN_OPTIONS="detect_leaks=0${ASAN_OPTIONS:+:$ASAN_OPTIONS}"
cd "$(dirname "$0")/../.."
t=$1; secs=$2; shift 2
exec cargo +nightly fuzz run "$t" -- -fork=1 -ignore_timeouts=0 -max_total_time="$secs" \
  -timeout=2 -rss_limit_mb=2048 -detect_leaks=0 -ignore_ooms=1 "$@"
