#!/bin/sh
# Usage: bench/fuzz/run.sh <expr|fmt|bytes> <seconds> [extra libFuzzer flags]
# -fork=1: every eval leaks about 1 MB of arrays (the interpreter never frees what the
# self-hosted compiler allocates), so a single process hits the 2 GB limit within about
# 2000 execs. Fork mode restarts the child between jobs; OOM/timeout artifacts are kept
# (-ignore_timeouts=0 makes a 2 s hang fatal). Replay an oom-* artifact alone before filing it.
set -e
cd "$(dirname "$0")/../.."
t=$1; secs=$2; shift 2
exec cargo +nightly fuzz run "$t" -- -fork=1 -ignore_timeouts=0 -max_total_time="$secs" \
  -timeout=2 -rss_limit_mb=2048 "$@"
