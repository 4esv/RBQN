#!/usr/bin/env python3
"""Primitive x argument grid through rbqn and $CBQN_BIN, batched with timeouts.
Buckets: wrong (both give values, differ), strict (rbqn errors, cbqn gives a value),
lenient (rbqn gives a value, cbqn errors). Both erroring counts as agreement.
Env: CBQN_BIN, RBQN_BIN (default target/release/rbqn)."""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import env_bins, run, classify

RB, CB = env_bins()
fns = list("+-×÷⋆√⌊⌈∧∨¬|≤<>≥=≠≡≢⊣⊢⥊∾≍⋈↑↓↕«»⌽⍉/⍋⍒⊏⊑⊐⊒∊⍷⊔!")
args = ["0", "1", "¯2", "2.5", "'a'", '"abc"', "⟨⟩", "1‿2‿3", '⟨1,"a",⟨2⟩⟩',
        "2‿3⥊↕6", "<5", "@", "↕0", "0‿1‿1", "1‿0⥊0"]
exprs = []
for f in fns:
    for x in args:
        exprs.append(f"{f} {x}")
    for w in args:
        for x in args:
            exprs.append(f"{w} {f} {x}")
mods = ["´", "˝", "`", "¨", "˘", "⌜", "⁼", "˜"]
for f in "+-×⌊∾⋈≍⊣":
    for m in mods:
        for x in args:
            exprs.append(f"{f}{m} {x}")
        for w in ["1", "1‿2‿3", '"abc"', "2‿3⥊↕6", "⟨⟩"]:
            for x in args:
                exprs.append(f"{w} {f}{m} {x}")
for f, g in [("+", "-"), ("∾", "⌽"), ("⌽", "↕")]:
    for m2 in ["∘", "○", "⊸", "⟜", "⌾"]:
        for x in args:
            exprs.append(f"{f}{m2}{g} {x}")
            exprs.append(f"1‿2‿3 {f}{m2}{g} {x}")

BATCH = 300
TIMEOUT = 90
ERR = "ERR"


def batch_run(binary, batch):
    prog = "".join('•Out "@@"\n{𝕊: •Out •Fmt %s}⎊{𝕊: •Out "%s"} 0\n' % (e, ERR) for e in batch)
    out, err = run(binary, prog, TIMEOUT)
    return (None if out is None else out.split("@@\n")[1:]), err


wrong, strict, lenient = [], [], []
timeouts, panics, early = [], [], []
for s in range(0, len(exprs), BATCH):
    batch = exprs[s:s + BATCH]
    rs, re_ = batch_run(RB, batch)
    cs, ce = batch_run(CB, batch)
    if rs is None:
        timeouts.append(("rbqn", s))
    if cs is None:
        timeouts.append(("cbqn", s))
    if rs is None or cs is None:
        continue
    if "panicked" in re_:
        panics.append((s, re_[:300]))
    if len(rs) < len(batch):
        early.append((batch[len(rs)], re_[-400:]))
    for i, e in enumerate(batch):
        if i >= len(rs) or i >= len(cs):
            break
        r, c = rs[i].rstrip(), cs[i].rstrip()
        k = classify(c, r)
        if k != "same":
            {"strict": strict, "lenient": lenient, "wrong": wrong}[k].append((e, c, r))
    print(f"batch {s}: done", file=sys.stderr)

print(f"exprs={len(exprs)} wrong={len(wrong)} strict={len(strict)} lenient={len(lenient)} "
      f"timeouts={timeouts} panics={len(panics)} rbqn_stopped_early={len(early)}")
for e, t in early:
    print("EARLY STOP at", e, "\n", t)
for s, t in panics:
    print("PANIC batch", s, t)
for name, bucket in (("wrong", wrong), ("strict", strict), ("lenient", lenient)):
    print(f"=== {name} ({len(bucket)})")
    for e, c, r in bucket:
        print(f"--- {e}\n  cbqn: {c[:150]!r}\n  rbqn: {r[:150]!r}")
