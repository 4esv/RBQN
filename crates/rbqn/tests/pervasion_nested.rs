//! #62: comparison and ¬ pervade into nested lists; char arithmetic takes integers only.
//! Expected values are CBQN's output.

use std::process::Command;

fn run(expr: &str) -> (String, String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn"))
        .args(["-p", expr])
        .output()
        .expect("failed to execute rbqn");
    (
        String::from_utf8_lossy(&out.stdout).trim_end().to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.code().unwrap_or(-1),
    )
}

fn ok(expr: &str, want: &str) {
    let (out, err, code) = run(expr);
    assert_eq!((code, out.as_str()), (0, want), "{expr}: {err}");
}

fn fails(expr: &str, msg: &str) {
    let (_, err, code) = run(expr);
    assert_eq!(code, 1, "{expr} should error");
    assert!(err.contains(msg), "{expr}: {err}");
}

#[test]
fn comparison_pervades_nested() {
    ok("'a' = ⟨1,\"a\",⟨2⟩⟩", "⟨ 0 ⟨ 1 ⟩ ⟨ 0 ⟩ ⟩");
    ok("'a' ≠ ⟨1,\"a\",⟨2⟩⟩", "⟨ 1 ⟨ 0 ⟩ ⟨ 1 ⟩ ⟩");
    ok("0 ≤ ⟨1,\"a\",⟨2⟩⟩", "⟨ 1 ⟨ 1 ⟩ ⟨ 1 ⟩ ⟩");
    ok("0 < ⟨1,\"a\",⟨2⟩⟩", "⟨ 1 ⟨ 1 ⟩ ⟨ 1 ⟩ ⟩");
    ok("0 > ⟨1,\"a\",⟨2⟩⟩", "⟨ 0 ⟨ 0 ⟩ ⟨ 0 ⟩ ⟩");
    ok("0 ≥ ⟨1,\"a\",⟨2⟩⟩", "⟨ 0 ⟨ 0 ⟩ ⟨ 0 ⟩ ⟩");
}

#[test]
fn not_pervades_nested() {
    ok("'a' ¬ ⟨1,\"a\",⟨2⟩⟩", "⟨ 'a' ⟨ 1 ⟩ \"`\" ⟩");
}

#[test]
fn merge_rejects_mixed_depth() {
    fails("> ⟨1,\"a\",⟨2⟩⟩", "Incompatible element shapes");
}

#[test]
fn char_arithmetic_needs_integers() {
    fails("2.5 + 'a'", "Expected integer, got 2.5");
    fails("'a' - 2.5", "Expected integer, got 2.5");
    fails("'a' ¬ 2.5", "Expected integer, got 2.5");
}

#[test]
fn empty_minus_char() {
    ok("⟨⟩ - 'a'", "⟨⟩");
}
