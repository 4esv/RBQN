#!/bin/bash
# Multicore (#104) and GPU (#125, #146) bench. Median wall time of 3 runs per workload on CBQN,
# rbqn -t 1 and rbqn default threads; prints a Markdown table.
# Each cell: median in-process op time (median whole-process wall time).
# Env: CBQN_BIN (default $HOME/Code/forks/CBQN/BQN), RBQN_BIN (default target/release/rbqn), REPS (3).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
CBQN_BIN=${CBQN_BIN:-$HOME/Code/forks/CBQN/BQN}
RBQN_BIN=${RBQN_BIN:-$here/../../target/release/rbqn}
export CBQN_BIN RBQN_BIN REPS=${REPS:-3} PROG=$here/multicore.bqn GPROG=$here/gpu.bqn
echo "uptime (start): $(uptime)"
echo "load1 before run: $(uptime | sed 's/.*averages: //' | awk '{print $1}' | tr -d ,)"
# Interference evidence: max CPU% of any process other than rbqn/BQN/this script's own tools, sampled every 2 s.
samp=$(mktemp); echo 0 > "$samp"
( while :; do
    m=$(ps -Ao pcpu,comm | awk 'NR>1{n=$2; sub(/.*\//,"",n); if(n!~/^(rbqn|BQN|python3|Python|bash|zsh|sh|ps|awk|sleep)$/ && $1>m) m=$1} END{print m+0}')
    p=$(cat "$samp"); awk -v a="$m" -v b="$p" 'BEGIN{print (a>b?a:b)}' > "$samp.n" && mv "$samp.n" "$samp"
    sleep 2
  done ) &
sampler=$!
python3 -I - <<'PY'
import os, statistics, subprocess, time
cb, rb, reps, prog = os.environ["CBQN_BIN"], os.environ["RBQN_BIN"], int(os.environ["REPS"]), os.environ["PROG"]
gprog = os.environ["GPROG"]
threads = subprocess.run([rb, "--version"], capture_output=True, text=True).stdout
nthreads = [l.split(": ")[1] for l in threads.splitlines() if l.startswith("threads:")][0]
work = [("`+´` 1e8 i32", "sum"), ("`(a×b)+c` 1e8 f64", "fma"), ("`⌈´` 1e8 f64", "max")]
cols = [("CBQN", [cb]), ("rbqn -t 1", [rb, "-t", "1"]), (f"rbqn -t {nthreads}", [rb, "-t", nthreads])]
def med(cmd, w, env=None, prog=prog):
    """Median in-process compute seconds and median wall seconds over reps runs."""
    ts, walls, val = [], [], None
    for _ in range(reps):
        t0 = time.perf_counter()
        r = subprocess.run(cmd + [prog, w], capture_output=True, text=True, env={**os.environ, **(env or {})})
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
# GPU rows (#125): `+´ (x×x) + 2×x` over 1e8 i32.
print()
print(subprocess.run([rb, "--version"], capture_output=True, text=True).stdout.strip().replace("\n", "; "))
gcols = [("CBQN", [cb], "auto", {}),
         ("rbqn CPU (`RBQN_GPU=off`)", [rb], "auto", {"RBQN_GPU": "off"}),
         ("rbqn GPU (automatic)", [rb], "auto", {}),
         ("rbqn `•gpu` explicit (f32)", [rb], "tensor", {})]
def counts(cmd, w, env):
    """Upload/download lines and the summary counters from one RBQN_GPU_DEBUG=1 run."""
    r = subprocess.run(cmd + [gprog, w], capture_output=True, text=True, env={**os.environ, **env, "RBQN_GPU_DEBUG": "1"})
    err = r.stderr.splitlines()
    summ = [l for l in err if l.startswith("gpu: dispatches=")]
    return (sum(l.startswith("[gpu] upload") for l in err), sum(l.startswith("[gpu] download") for l in err), summ[-1] if summ else "-")
print()
print("| pipeline `+´ (x×x)+2×x`, x 1e8 i32 | in-process s (wall s) | result | uploads | download lines | summary line |")
print("|---|---|---|---|---|---|")
for label, c, w, env in gcols:
    t, wl, o = med(c, w, env, gprog)
    cell = f"{t:.3f} ({wl:.2f})" if t is not None else f"error: {o}"
    if c[0] == cb:
        print(f"| {label} | {cell} | {o} | n/a | n/a | n/a |")
    else:
        u, d, summ = counts(c, w, env)
        print(f"| {label} | {cell} | {o} | {u} | {d} | `{summ}` |")
print()
print("Explicit row: exactly one `[gpu] upload` line; the sum is read back as one scalar (readbacks=1 in the summary), so there is no `[gpu] download` line. The f32 sum is not guaranteed equal to the exact one.")

# GPU rows where the device can win (#146): sort, matmul, row sums, each at three sizes.
# CBQN has no •math.MatMul, so its matmul cell is the BQN expression `w +˝∘× ⎉1‿∞ x` (rbqn runs the same kernel for both spellings, #166); its •rand stream
# differs from rbqn's, so only the three rbqn columns are compared for agreement.
print()
print("| workload | CBQN | rbqn CPU `-t 1` (`RBQN_GPU=off`) | rbqn CPU (`RBQN_GPU=off`) | rbqn GPU (`RBQN_GPU=force`) | rbqn default | default ran on | model gpu / cpu ms | rbqn modes agree |")
print("|---|---|---|---|---|---|---|---|---|")
grows = [("sort `∧` 1e6 i32", "sort", "1e6", 3), ("sort `∧` 1e7 i32", "sort", "1e7", 3), ("sort `∧` 1e8 i32", "sort", "1e8", 3),
         ("`•math.MatMul` 1000² (entries below 10)", "matmul", "1000", 3), ("`•math.MatMul` 2000²", "matmul", "2000", 3),
         ("`•math.MatMul` 4096²", "matmul", "4096", 3),
         ("`+´˘` 1e3×1e3 i32", "rows", "1000", 3), ("`+´˘` 3e3×3e3 i32", "rows", "3000", 3), ("`+´˘` 1e4×1e4 i32", "rows", "1e4", 3)]
def gmed(cmd, w, arg, env, reps):
    ts, walls, val = [], [], None
    for _ in range(reps):
        t0 = time.perf_counter()
        r = subprocess.run(cmd + [gprog, w, arg], capture_output=True, text=True, env={**os.environ, **env})
        walls.append(time.perf_counter() - t0)
        if r.returncode: return None, r.stderr.strip()[:60]
        lines = r.stdout.split("\n")
        val = lines[0].strip()
        ts.append(float([l for l in lines if l.startswith("t=")][0][2:].replace("¯", "-")))
    return statistics.median(ts), val
def gdebug(w, arg):
    r = subprocess.run([rb, gprog, w, arg], capture_output=True, text=True, env={**os.environ, "RBQN_GPU_DEBUG": "1"})
    err = r.stderr.splitlines()
    summ = [l for l in err if l.startswith("gpu: dispatches=")]
    disp = int(summ[-1].split("dispatches=")[1].split()[0]) if summ else 0
    dec = [l for l in err if "decide sort" in l or "decide matmul" in l]
    model = "-"
    if dec:
        m = dec[-1]
        model = m.split(" gpu=")[1].split(" (")[0].replace("ms cpu=", " / ").replace("ms", "")
    return ("GPU" if disp > 0 else "CPU"), model
for label, w, arg, off_reps in grows:
    cells, vals = [], []
    for name, cmd, env, reps in [("cb", [cb], {}, 3), ("t1", [rb, "-t", "1"], {"RBQN_GPU": "off"}, off_reps),
                                 ("off", [rb], {"RBQN_GPU": "off"}, off_reps),
                                 ("force", [rb], {"RBQN_GPU": "force"}, 3), ("def", [rb], {}, 3)]:
        t, o = gmed(cmd, w, arg, env, reps)
        cells.append(f"{t:.3f} s" + (" (1 run)" if name in ("off", "t1") and off_reps == 1 else "") if t is not None else f"error: {o}")
        if name != "cb": vals.append(o)
    ran, model = gdebug(w, arg)
    print(f"| {label} | " + " | ".join(cells) + f" | {ran} | {model} | {'yes' if len(set(vals)) == 1 else 'NO'} |")
print()
print("Cells: median in-process seconds of the operation alone, 3 runs unless marked. Model column: the cost model's predicted GPU / CPU milliseconds from the default run's `RBQN_GPU_DEBUG` line (`-`: no model decision, the operation is below the array-size floor).")
PY
echo
kill $sampler 2>/dev/null; wait $sampler 2>/dev/null || true
echo "max foreign CPU% during run: $(cat "$samp")"
rm -f "$samp" "$samp.n"
echo "uptime (end): $(uptime)"
