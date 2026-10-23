#!/usr/bin/env python3
"""Run every 8-space-indented example in $BQN_SRC/doc/*.md line by line through
rbqn and $CBQN_BIN, separated by •Out, and diff per line.
Env: BQN_SRC (BQN repo checkout), CBQN_BIN, RBQN_BIN (default target/release/rbqn)."""
import os, pathlib, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import env_bins, run

SEP = "@@SEP@@"
src = os.environ.get("BQN_SRC")
if not src:
    sys.exit("BQN_SRC is not set (path to a checkout of mlochbaum/BQN)")
root = pathlib.Path(src) / "doc"
if not root.is_dir():
    sys.exit(f"no doc directory: {root}")
RB, CB = env_bins()

n = 0
bad = []
for md in sorted(root.glob("*.md")):
    lines = [l[8:] for l in md.read_text(encoding="utf-8", errors="replace").splitlines()
             if l.startswith("        ") and l.strip()]
    if not lines:
        continue
    prog = "".join(f'{l}\n•Out "{SEP}"\n' for l in lines)
    ro, re_ = run(RB, prog, 60, root)
    co, ce = run(CB, prog, 60, root)
    if ro is None or co is None:
        bad.append((md.name, "<TIMEOUT>", "timeout" if co is None else "", "timeout" if ro is None else ""))
        continue
    rs = ro.split(SEP + "\n")
    cs = co.split(SEP + "\n")
    for i, l in enumerate(lines):
        if i >= len(rs) or i >= len(cs):
            bad.append((md.name, l, "<truncated>" if i >= len(cs) else "", "<truncated: rbqn stopped>" if i >= len(rs) else ""))
            break
        r, c = rs[i].rstrip(), cs[i].rstrip()
        n += 1
        if r != c:
            bad.append((md.name, l, c, r))
    if "panicked" in re_:
        bad.append((md.name, "<PANIC>", "", re_[:300]))

print(f"compared {n} expressions; {len(bad)} differ")
for name, l, c, r in bad:
    print(f"--- {name}: {l}\n    cbqn: {c[:200]!r}\n    rbqn: {r[:200]!r}")
