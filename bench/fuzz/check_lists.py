#!/usr/bin/env python3
"""Fail if the primitive/argument lists in fuzz/src/lib.rs drift from bench/stranger/grid.py."""
import re, sys, os
root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
grid = open(os.path.join(root, "bench/stranger/grid.py")).read()
lib = open(os.path.join(root, "fuzz/src/lib.rs")).read()

def py(name):
    return eval(re.search(rf"^{name} = (.*)$", grid, re.M).group(1).replace("list(", "(").rstrip(), {})

fns = re.search(r'^fns = list\("(.*)"\)', grid, re.M).group(1)
args = eval(re.search(r"^args = (\[.*?\])\nexprs", grid, re.M | re.S).group(1))
mods = eval(re.search(r"^mods = (\[.*\])$", grid, re.M).group(1))
mod_fns = re.search(r'^for f in "(.*)":\n    for m in mods', grid, re.M).group(1)
mods2 = eval(re.search(r'for m2 in (\[.*?\]):', grid).group(1))
pairs = eval(re.search(r"for f, g in (\[.*?\]):", grid).group(1))

def rs_str(name):
    return re.search(rf'pub const {name}: &str = "(.*)";', lib).group(1)
def rs_list(name):
    body = re.search(rf'pub const {name}: &\[.*?\] = &\[(.*?)\];', lib, re.S).group(1)
    return body
def lit(s):
    return s.replace("\\", "\\\\").replace('"', '\\"')

bad = []
if rs_str("FNS") != fns: bad.append("FNS")
if rs_str("MOD_FNS") != mod_fns: bad.append("MOD_FNS")
a = rs_list("ARGS")
for x in args:
    if f'"{lit(x)}"' not in a: bad.append(f"ARGS {x}")
if len(re.findall(r'"(?:[^"\\]|\\.)*"', a)) != len(args): bad.append("ARGS count")
for name, xs in (("MODS", mods), ("MODS2", mods2)):
    if re.findall(r'"(.*?)"', rs_list(name)) != xs: bad.append(name)
if re.findall(r'\("(.*?)", "(.*?)"\)', rs_list("PAIRS")) != pairs: bad.append("PAIRS")
if bad:
    sys.exit("fuzz/src/lib.rs differs from bench/stranger/grid.py: " + ", ".join(bad))
print("lists match")
