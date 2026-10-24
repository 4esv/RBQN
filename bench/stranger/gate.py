#!/usr/bin/env python3
"""Gate for the CI parity job: gate.py DOCS_OUT GRID_OUT KNOWN.
Fails (exit 1) if docs_diff.py reports any difference, or grid.py reports a
mismatch not listed in KNOWN, a timeout, or a panic. Prints the offending lines."""
import re, sys


def docs_failures(text):
    m = re.search(r"compared (\d+) expressions; (\d+) differ", text)
    if not m:
        return ["docs_diff output has no summary line"]
    if m.group(2) == "0":
        return []
    return [l for l in text.splitlines() if l.startswith("--- ") or l.startswith("    ")]


def known(path):
    out = set()
    for l in open(path, encoding="utf-8"):
        l = l.rstrip("\n")
        if l.strip() and not l.startswith("#"):
            out.add(l.split(" | ")[0].strip())
    return out


def grid_failures(text, allowed):
    bad = []
    head = re.search(r"^exprs=.*$", text, re.M)
    if not head:
        return ["grid output has no summary line"]
    if not re.search(r"timeouts=\[\] panics=0 rbqn_stopped_early=0", head.group(0)):
        bad.append("grid did not complete cleanly: " + head.group(0))
    lines = text.splitlines()
    for i, l in enumerate(lines):
        if l.startswith("--- ") and l[4:] not in allowed:
            bad.extend(lines[i:i + 3])
    return bad


def main(docs, grid, kn):
    bad = docs_failures(open(docs, encoding="utf-8").read())
    bad += grid_failures(open(grid, encoding="utf-8").read(), known(kn))
    for l in bad:
        print(l)
    print("PARITY GATE:", "FAIL" if bad else "ok")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:4]))
