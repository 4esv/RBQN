#!/bin/bash
# Multicore bench (#104). Median wall time of 3 runs per workload on CBQN,
# rbqn -t 1 and rbqn default threads; prints a Markdown table.
# Each cell: median in-process op time (median whole-process wall time).
# Env: CBQN_BIN (default $HOME/Code/forks/CBQN/BQN), RBQN_BIN (default target/release/rbqn), REPS (3).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
CBQN_BIN=${CBQN_BIN:-$HOME/Code/forks/CBQN/BQN}
RBQN_BIN=${RBQN_BIN:-$here/../../target/release/rbqn}
export CBQN_BIN RBQN_BIN REPS=${REPS:-3} PROG=$here/multicore.bqn
exec python3 -I - <<'PY'
import os, statistics, subprocess, time
cb, rb, reps, prog = os.environ["CBQN_BIN"], os.environ["RBQN_BIN"], int(os.environ["REPS"]), os.environ["PROG"]
threads = subprocess.run([rb, "--version"], capture_output=True, text=True).stdout
nthreads = [l.split(": ")[1] for l in threads.splitlines() if l.startswith("threads:")][0]
work = [("`+´` 1e8 i32", "sum"), ("`(a×b)+c` 1e8 f64", "fma"), ("`⌈´` 1e8 f64", "max")]
cols = [("CBQN", [cb]), ("rbqn -t 1", [rb, "-t", "1"]), (f"rbqn -t {nthreads}", [rb, "-t", nthreads])]
def med(cmd, w):
    """Median in-process compute seconds and median wall seconds over reps runs."""
    ts, walls, val = [], [], None
    for _ in range(reps):
        t0 = time.perf_counter()
        r = subprocess.run(cmd + [prog, w], capture_output=True, text=True)
        walls.append(time.perf_counter() - t0)
        if r.returncode: return None, None, r.stderr.strip()[:60]
        lines = r.stdout.split("\n")
        val = lines[0].strip()
        ts.append(float([l for l in lines if l.startswith("t=")][0][2:]))
    return statistics.median(ts), statistics.median(walls), val
print("| workload | " + " | ".join(c for c, _ in cols) + " | speedup vs -t 1 | result agrees |")
print("|---|" + "---|" * (len(cols) + 2))
for label, w in work:
    res = [med(c, w) for _, c in cols]
    cells = [f"{t:.3f} s ({wl:.2f} s)" if t is not None else f"error: {o}" for t, wl, o in res]
    ok = all(t is not None for t, _, _ in res)
    sp = f"{res[1][0] / res[2][0]:.2f}x" if ok else "-"
    agree = "yes" if ok and len({o for _, _, o in res}) == 1 else "NO"
    print(f"| {label} | " + " | ".join(cells) + f" | {sp} | {agree} |")
print()
print("Cells: median in-process seconds for the operation alone (median whole-process wall seconds).")
