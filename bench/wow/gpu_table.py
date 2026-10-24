#!/usr/bin/env python3
"""Rewrite the README GPU table from a bench/wow results file (#146).

Usage: python3 -I bench/wow/gpu_table.py bench/wow/results/<file>.md
Copies the rows of the results file's last table (sort, matmul, row sums) between the
<!-- gpu-table --> markers of README.md, dropping the model and agreement columns.
"""
import re, sys
res = open(sys.argv[1], encoding="utf-8").read().splitlines()
start = max(i for i, l in enumerate(res) if l.startswith("| workload | CBQN | rbqn CPU"))
rows = []
for l in res[start:]:
    if not l.startswith("|"):
        break
    rows.append([c.strip() for c in l.strip("|").split("|")])
assert all(r[-1] in ("yes", "rbqn modes agree", "---") for r in rows), "a row does not agree"
out = ["| " + " | ".join(r[:7]) + " |" for r in rows]
out[1] = "|---|---|---|---|---|---|---|"
text = open("README.md", encoding="utf-8").read()
text = re.sub(r"(<!-- gpu-table -->\n).*?(\n<!-- /gpu-table -->)", lambda m: m.group(1) + "\n".join(out) + m.group(2), text, flags=re.S)
open("README.md", "w", encoding="utf-8").write(text)
