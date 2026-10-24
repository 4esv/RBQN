#!/usr/bin/env python3
"""Gate for the CI parity job: gate.py DOCS_OUT GRID_OUT KNOWN.
The spec is the target and CBQN is the floor: this gate is a regression floor so rbqn never falls below CBQN, and a deliberate divergence beyond CBQN is recorded in KNOWN as `expr | reason | spec citation`, not fixed. The old two-field `expr | reason` form still parses.
Fails (exit 1) if docs_diff.py or grid.py reports a difference not listed in KNOWN, a timeout, or a panic. Prints the offending lines."""
import re, sys


def unlisted(text, allowed):
    """Blocks ("--- key" plus its indented lines) whose key is not in allowed."""
    bad = []
    lines = text.splitlines()
    for i, l in enumerate(lines):
        if l.startswith("--- ") and l[4:] not in allowed:
            bad.append(l)
            bad.extend(x for x in lines[i + 1:i + 3] if x.startswith(" "))
    return bad


def docs_failures(text, allowed=frozenset()):
    if not re.search(r"compared (\d+) expressions; (\d+) differ", text):
        return ["docs_diff output has no summary line"]
    return unlisted(text, allowed)


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
    return bad + unlisted(text, allowed)


def main(docs, grid, kn):
    allowed = known(kn)
    bad = docs_failures(open(docs, encoding="utf-8").read(), allowed)
    bad += grid_failures(open(grid, encoding="utf-8").read(), allowed)
    for l in bad:
        print(l)
    print("PARITY GATE:", "FAIL" if bad else "ok")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:4]))
