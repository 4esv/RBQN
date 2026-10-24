//! #101: `[a, b]` with mismatched element shapes errors like `>`.

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

#[test]
fn mismatched_shapes_error() {
    for expr in ["[↕4, ↕5]", "[1‿2, 3]"] {
        let (_, err, code) = run(expr);
        assert_eq!(code, 1, "{expr} should error");
        assert!(err.contains("Incompatible element shapes"), "{expr}: {err}");
    }
}

#[test]
fn matching_shapes_still_merge() {
    let (out, err, code) = run("≢ [1‿2, 3‿4]");
    assert_eq!((code, out.as_str()), (0, "⟨ 2 2 ⟩"), "{err}");
}
