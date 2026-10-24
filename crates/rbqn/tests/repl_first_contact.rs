//! REPL first contact (#115): header on a terminal, `)help`, Ctrl-D status, silent pipes.
use std::io::Write;
use std::process::{Command, Stdio};

/// Drive rbqn on a pty: send `input`, return (output, exit status).
const PTY_DRIVER: &str = r#"
import os, pty, select, sys, time
exe, inp = sys.argv[1], sys.argv[2].encode().decode("unicode_escape").encode("latin-1")
pid, fd = pty.fork()
if pid == 0:
    os.environ["HOME"] = sys.argv[3]
    os.execv(exe, [exe])
out = b""
def drain(t):
    global out
    end = time.time() + t
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.1)
        if r:
            try:
                d = os.read(fd, 65536)
            except OSError:
                return
            if not d:
                return
            out += d
drain(3)
# One chunk per line: a terminal delivers keystrokes separately.
for chunk in inp.replace(b"\n", b"\n\0").split(b"\0"):
    if chunk:
        os.write(fd, chunk)
        drain(0.7)
drain(1)
for _ in range(50):
    done, st = os.waitpid(pid, os.WNOHANG)
    if done:
        break
    time.sleep(0.1)
else:
    os.kill(pid, 9)
    _, st = os.waitpid(pid, 0)
    st = 255 << 8
sys.stdout.buffer.write(out)
sys.stdout.write("\nSTATUS=%d\n" % os.waitstatus_to_exitcode(st))
"#;

fn pty(input: &str) -> (String, i32) {
    let home = std::env::temp_dir().join(format!("rbqn-fc-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    let o = Command::new("python3")
        .args(["-I", "-c", PTY_DRIVER, env!("CARGO_BIN_EXE_rbqn"), input, home.to_str().unwrap()])
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&o.stdout).replace("\r\n", "\n");
    let (body, st) = s.rsplit_once("STATUS=").expect("driver output");
    (body.to_string(), st.trim().parse().unwrap())
}

#[test]
fn header_once_then_help_then_ctrl_d() {
    let (out, status) = pty(")help\\n1+1\\n\\x04");
    assert_eq!(status, 0, "{out}");
    assert_eq!(out.matches("rbqn 0.1.0 · ").count(), 1, "{out}");
    let header = out.lines().find(|l| l.starts_with("rbqn 0.1.0")).unwrap();
    assert!(header.contains(" threads"), "{header}");
    assert!(header.ends_with(" · )help"), "{header}");
    for c in [")help", ")off", ")exit"] {
        assert!(out.contains(c), "missing {c} in {out}");
    }
    assert!(!out.contains("Unknown REPL command"), "{out}");
    assert!(out.contains("\n2\n"), "{out}");
    assert!(out.ends_with("\n"), "trailing newline after Ctrl-D: {out:?}");
}

#[test]
fn off_exits_zero() {
    let (_, status) = pty(")off\\n");
    assert_eq!(status, 0);
}

#[test]
fn piped_empty_stdin_prints_nothing() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rbqn"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdin.take().unwrap().flush());
    let out = child.wait_with_output().unwrap();
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
}
