//! `↕` rejects what it cannot represent instead of wrapping to a wrong length.

use std::process::Command;

fn run(expr: &str) -> (String, String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-p", expr]).output().unwrap();
    (
        String::from_utf8_lossy(&out.stdout).trim_end().to_string(),
        String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("").to_string(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn range_beyond_i32_is_an_error_not_a_truncated_length() {
    // Before: ≠↕1e10 printed 1410065408 and ↕3e9 printed ⟨⟩.
    for e in ["≠↕1e10", "↕3e9", "↕2147483648"] {
        let (out, err, code) = run(e);
        assert_eq!(code, 1, "{e}: {out}");
        assert_eq!(err, format!("Error: ↕𝕩: 𝕩 must be less than 2147483648, got {}", e.trim_start_matches(['≠', '↕']).replace("1e10", "10000000000").replace("3e9", "3000000000")));
    }
}

#[test]
fn range_rejects_negative_and_fractional() {
    assert_eq!(run("↕¯1").1, "Error: ↕𝕩: Expected non-negative integer, got ¯1");
    assert_eq!(run("↕2.5").1, "Error: ↕𝕩: Expected non-negative integer, got 2.5");
    assert_eq!(run("↕2‿¯1").1, "Error: ↕𝕩: Expected non-negative integer, got ¯1");
}

#[test]
fn range_limit_is_inclusive_below() {
    assert_eq!(run("≠↕1000").0, "1000");
    assert_eq!(run("≢↕2‿0‿3").0, "⟨ 2 0 3 ⟩");
}
