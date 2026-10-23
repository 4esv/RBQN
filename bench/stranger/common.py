import os, re, subprocess, sys

ANSI = re.compile(r'\x1b\[[0-9;?]*[A-Za-z]')


def env_bins():
    """Return (rbqn, cbqn); exit 2 with a message if unset or missing."""
    here = os.path.dirname(os.path.abspath(__file__))
    rb = os.environ.get("RBQN_BIN") or os.path.join(here, "../../target/release/rbqn")
    cb = os.environ.get("CBQN_BIN")
    if not cb:
        sys.exit("CBQN_BIN is not set (path to a CBQN executable)")
    for p in (rb, cb):
        if not os.path.isfile(p):
            sys.exit(f"not a file: {p}")
    return rb, cb


def run(binary, prog, timeout, cwd=None):
    """Run prog on stdin. Returns (stdout, stderr) with ANSI stripped, or (None, 'TIMEOUT')."""
    try:
        p = subprocess.run([binary], input=prog.encode(), capture_output=True,
                           timeout=timeout, cwd=cwd)
    except subprocess.TimeoutExpired:
        return None, "TIMEOUT"
    dec = lambda b: ANSI.sub('', b.decode("utf-8", errors="replace"))
    return dec(p.stdout), dec(p.stderr)
