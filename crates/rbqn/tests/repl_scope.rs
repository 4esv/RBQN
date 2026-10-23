//! REPL lines and the closures they create share one persistent scope (#67).
use std::io::Write;
use std::process::{Command, Stdio};

fn repl(input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rbqn"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

#[test]
fn closure_sees_later_reassignment() {
    let out = repl("factor ← 3\nMul ← { factor × 𝕩 }\nMul 6\nfactor ↩ 5\nMul 6\n");
    assert!(out.trim_end().ends_with("18\n5\n30"), "{out}");
}

#[test]
fn variables_survive_a_failed_line() {
    let out = repl("a ← 1\na ↩ 2 ⋄ 1+\"x\"\na\nb ← a+1\nb\n");
    assert!(out.trim_end().ends_with("2\n3\n3"), "{out}");
}
