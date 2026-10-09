#!/usr/bin/env python3
"""Default-mode before/after on README rows, W1-W7, leads rows and extras.

Usage: bench/thresholds/rows.py OLD_BIN NEW_BIN [--mode default|off|force] [--only SUBSTR]
Median of 10 runs (2 warmups) per binary with hyperfine; prints a markdown table
and, with RBQN_GPU_DEBUG, the new binary's dispatch count per row.
"""
import json, os, subprocess, sys, tempfile

HF = os.environ.get("HYPERFINE", "/opt/homebrew/bin/hyperfine")
README = [
    "1", "+´↕10000000", "+´×˜↕10000000", "+´ {𝕩+1}¨ ↕1000000", "{𝕩<2 ? 𝕩 ; (𝕊 𝕩-1)+𝕊 𝕩-2} 25",
    "≠ ⊐ 3000000⥊↕1000", "+´⥊ (↕3000) ×⌜ ↕3000", "+´ (⊢ ⍋⊸⊏ ⊢) 1000000⥊3‿1‿2", "≠ •Fmt ↕100000",
    "+´ +` ↕5000000", "+´ 2 × ↕5000000", "≠ ∾ 100000⥊⟨\"ab\",\"cde\"⟩", "+´ 5000000⥊1", "≠ •Fmt ↕1000000",
    "{𝕩<2 ? 𝕩 ; (𝕊 𝕩-1)+𝕊 𝕩-2} 27", "+´ {𝕩×𝕩}¨ ↕1000000",
]
W = [
    ("W1", "a←↕1e7 ⋄ ≠ a+a"), ("W2", "a←↕1e7 ⋄ +´ (a×a)+(a×a)"), ("W3", "a←↕1e7 ⋄ +´ 1+2×3+4×a"),
    ("W4", "a←↕1e7 ⋄ ⌈´ +` a"), ("W5", "a←1e7⥊↕1000 ⋄ ≠ ∧a"), ("W6", "a←1e6⥊↕1000 ⋄ ≠ ∧a"),
    ("W7", "a←↕1e6 ⋄ +´ (a×a)+(a×a)"),
]
LEADS = ["+´↕1e7", "≠ ⊐ 3000000⥊↕1000", "+´⥊ (↕3000)×⌜↕3000", "+´ +` ↕5e6", "≠ {𝕩+1}¨ ↕1e6",
         "{𝕩<2 ? 𝕩 ; (𝕊 𝕩-1)+𝕊 𝕩-2} 27", "≠ •Fmt ↕1e6"]
EXTRA = [("W2'", "a←↕1e7 ⋄ +´ (a+a)+(a+a)"), ("", "a←↕1e7 ⋄ ≠ 1+a"), ("", "a←↕1e7 ⋄ ¯1⊑ 1+2×a"),
         ("", "a←↕1e5 ⋄ ¯1⊑ 1+2×a"), ("W3@1e6", "a←↕1e6 ⋄ +´ 1+2×3+4×a"),
         ("", "a←10000000⥊1 ⋄ ⌈´ +` a"), ("", "≠ {𝕩+1}¨↕1000000")]
ROWS = [("R", e) for e in README] + W + [("L", e) for e in LEADS] + EXTRA


def bench(b, e, mode):
    with tempfile.NamedTemporaryFile(suffix=".json") as f:
        env = dict(os.environ, RBQN_GPU=mode)
        env.pop("RBQN_GPU_DEBUG", None)
        subprocess.run([HF, "-N", "-w", "2", "-r", "10", "--export-json", f.name, "--", f"{b} -p '{e}'"],
                       env=env, check=True, capture_output=True)
        return json.load(open(f.name))["results"][0]["median"] * 1000


def dispatches(b, e, mode):
    env = dict(os.environ, RBQN_GPU=mode, RBQN_GPU_DEBUG="1")
    err = subprocess.run([b, "-p", e], env=env, capture_output=True, text=True).stderr
    for line in err.splitlines():
        if line.startswith("gpu: dispatches="):
            return line.split()[1].split("=")[1]
    return "-"


def main():
    old, new = sys.argv[1], sys.argv[2]
    mode = sys.argv[sys.argv.index("--mode") + 1] if "--mode" in sys.argv else "default"
    only = sys.argv[sys.argv.index("--only") + 1] if "--only" in sys.argv else None
    print(f"mode={mode}\n\n| id | expression | old ms | new ms | new/old | new dispatches |\n|---|---|---:|---:|---:|---:|")
    for tag, e in ROWS:
        if only and only not in e and only != tag:
            continue
        o, n = bench(old, e, mode), bench(new, e, mode)
        print(f"| {tag} | `{e}` | {o:.1f} | {n:.1f} | {n / o:.2f} | {dispatches(new, e, mode)} |", flush=True)


main()
