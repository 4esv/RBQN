#!/usr/bin/env python3
"""Fit the GPU/CPU cost-model constants in crates/rbqn-gpu/src/dispatch.rs.

Usage: bench/thresholds/fit.py RBQN_BIN [out.json]
       HOST_BIN=<build whose decide() always picks the host> adds the h_ rows
       (host evaluator costs, default mode).

Each cost is isolated as the difference of two rows measured with hyperfine
at n in SIZES, then fitted as ms = a + b*n by least squares; b (ns/elem or
ns/byte) is what goes into dispatch.rs. GPU rows run under RBQN_GPU=force with
a warm-up prefix (W) that pays device init and the first compile before the
measured work, so init lands in the intercept and not the slope. CPU rows run
under RBQN_GPU=off. Sort rows (force vs off) give the crossover table.
"""
import json, subprocess, sys, tempfile, os

BIN = sys.argv[1]
OUT = sys.argv[2] if len(sys.argv) > 2 else None
HF = os.environ.get("HYPERFINE", "/opt/homebrew/bin/hyperfine")
SIZES = [100_000, 300_000, 1_000_000, 3_000_000, 10_000_000, 30_000_000]
SORT_SIZES = [100_000, 300_000, 1_000_000, 3_000_000, 10_000_000]
W = "w←+´1+↕2 ⋄ "

# name -> (mode, expression with {n})
ROWS = {
    # GPU (force)
    "g_base": ("force", W + "≠ 1+↕{n}"),             # lazy node only, no kernel
    "g_fold1": ("force", W + "+´ 1+↕{n}"),           # fused map+reduce, 1 op
    "g_fold4": ("force", W + "+´ 1+2×3+4×↕{n}"),     # fused map+reduce, 4 ops
    "g_map_dl": ("force", W + "¯1⊑ 1+↕{n}"),         # fused map + download 4n B
    "g_resh": ("force", "a←{n}⥊1‿2 ⋄ " + W + "≠ a"),
    "g_up": ("force", "a←{n}⥊1‿2 ⋄ " + W + "+´ a+1"),  # + upload 4n B + fold kernel
    "g_maxf": ("force", W + "⌈´ 1⌊↕{n}"),            # fused map+reduce
    "g_scan": ("force", W + "⌈´ +` 1⌊↕{n}"),         # + i64 scan + i64 reduce (no download)
    # CPU (off)
    "c_base": ("off", "a←↕{n} ⋄ ≠ a"),
    "c_add": ("off", "a←↕{n} ⋄ ≠ 1+a"),
    "c_add2": ("off", "a←↕{n} ⋄ ≠ a+a"),
    "c_mul": ("off", "a←↕{n} ⋄ ≠ a×a"),
    "c_min": ("off", "a←↕{n} ⋄ ≠ a⌊3"),
    "c_fold": ("off", "a←↕{n} ⋄ +´ a"),
    "c_scan": ("off", "a←↕{n} ⋄ ≠ +` a"),
}
# Host evaluator (HOST_BIN, default mode: every decision goes to the host).
HOST = {
    "h_base": ("default", "a←↕{n} ⋄ ≠ a"),
    "h_fold0": ("default", "+´ ↕{n}"),                  # leaf read + fold
    "h_fold4": ("default", "+´ 1+2×3+4×↕{n}"),          # + 4 ops
    "h_mat1": ("default", "a←↕{n} ⋄ ¯1⊑ 1+a"),          # leaf + 1 op + write + squeeze
    "h_mat4": ("default", "a←↕{n} ⋄ ¯1⊑ 1+2×3+4×a"),
}
HOST_BIN = os.environ.get("HOST_BIN")
SORT = {"s_force": ("force", "a←{n}⥊↕1000 ⋄ ≠ ∧a"), "s_off": ("off", "a←{n}⥊↕1000 ⋄ ≠ ∧a")}


def bench(mode, expr, bin=BIN):
    with tempfile.NamedTemporaryFile(suffix=".json") as f:
        env = dict(os.environ, RBQN_GPU=mode)
        env.pop("RBQN_GPU_DEBUG", None)
        subprocess.run([HF, "-N", "-w", "2", "-r", "10", "--export-json", f.name, "--", f"{bin} -p '{expr}'"],
                       env=env, check=True, capture_output=True)
        return json.load(open(f.name))["results"][0]["median"] * 1000


def fit(xs, ys):
    n = len(xs); mx = sum(xs) / n; my = sum(ys) / n
    b = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sum((x - mx) ** 2 for x in xs)
    return my - b * mx, b


def main():
    raw = {}
    bench("force", W + "+´ 1+↕100")  # prime the Metal shader cache
    rows = {**ROWS, **SORT, **(HOST if HOST_BIN else {})}
    for name, (mode, e) in rows.items():
        sizes = SORT_SIZES if name.startswith("s_") else SIZES
        b = HOST_BIN if name.startswith("h_") else BIN
        raw[name] = {n: bench(mode, e.format(n=n), b) for n in sizes}
        print(name, " ".join(f"{raw[name][n]:.2f}" for n in sizes), flush=True)

    def diff(a, b):
        return [raw[a][n] - raw[b][n] for n in SIZES]

    ns = SIZES
    res = {}
    # (constant, rows, divisor to get per-unit slope, unit)
    spec = {
        "gpu_fold1_ns_per_elem": (diff("g_fold1", "g_base"), 1),
        "gpu_fold4_ns_per_elem": (diff("g_fold4", "g_base"), 1),
        "gpu_map_dl_ns_per_elem": (diff("g_map_dl", "g_base"), 1),
        "gpu_upload_ns_per_byte": ([u - (f - b) for u, f, b in zip(diff("g_up", "g_resh"), raw["g_fold1"].values(), raw["g_base"].values())], 4),
        "gpu_scan_ns_per_elem": (diff("g_scan", "g_maxf"), 1),
        "cpu_add_scalar_ns_per_elem": (diff("c_add", "c_base"), 1),
        "cpu_add_arr_ns_per_elem": (diff("c_add2", "c_base"), 1),
        "cpu_mul_arr_ns_per_elem": (diff("c_mul", "c_base"), 1),
        "cpu_min_scalar_ns_per_elem": (diff("c_min", "c_base"), 1),
        "cpu_fold_ns_per_elem": (diff("c_fold", "c_base"), 1),
        "cpu_scan_ns_per_elem": (diff("c_scan", "c_base"), 1),
    }
    for k, (ys, div) in spec.items():
        a, b = fit(ns, ys)
        res[k] = {"intercept_ms": round(a, 3), "slope": round(b * 1e6 / div, 4)}  # ms/elem -> ns
        print(f"{k:28s} slope={b * 1e6 / div:.4f}  intercept={a:.3f} ms")
    # Derived: download = map+download minus the map kernel (≈ fold1 kernel), per byte.
    a1, b1 = fit(ns, diff("g_map_dl", "g_fold1"))
    res["gpu_download_ns_per_byte"] = {"intercept_ms": round(a1, 3), "slope": round(b1 * 1e6 / 4, 4)}
    print(f"{'gpu_download_ns_per_byte':28s} slope={b1 * 1e6 / 4:.4f}  intercept={a1:.3f} ms")
    # Per extra fused op: (fold4 - fold1) / 3.
    a2, b2 = fit(ns, diff("g_fold4", "g_fold1"))
    res["gpu_per_op_ns_per_elem"] = {"intercept_ms": round(a2 / 3, 3), "slope": round(b2 * 1e6 / 3, 4)}
    print(f"{'gpu_per_op_ns_per_elem':28s} slope={b2 * 1e6 / 3:.4f}")
    if HOST_BIN:
        for k, (a, b, div) in {
            "host_leaf_fold_ns_per_elem": ("h_fold0", "h_base", 1),
            "host_op_ns_per_elem": ("h_fold4", "h_fold0", 4),
            "host_op_ns_per_elem_mat": ("h_mat4", "h_mat1", 3),
            "host_mat1_ns_per_elem": ("h_mat1", "h_base", 1),
        }.items():
            ai, bi = fit(ns, diff(a, b))
            res[k] = {"intercept_ms": round(ai, 3), "slope": round(bi * 1e6 / div, 4)}
            print(f"{k:28s} slope={bi * 1e6 / div:.4f}  intercept={ai:.3f} ms")
    print("\nsort crossover (ms, force / off):")
    for n in SORT_SIZES:
        f, o = raw["s_force"][n], raw["s_off"][n]
        print(f"  n={n:>9}  {f:7.1f} / {o:7.1f}  {'gpu' if f < o else 'cpu'}")
    if OUT:
        json.dump({"raw": raw, "fit": res}, open(OUT, "w"), indent=1)


main()
