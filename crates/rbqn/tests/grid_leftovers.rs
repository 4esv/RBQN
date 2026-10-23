//! Grid cells (#89) where rbqn and CBQN disagreed on error versus value.

use std::process::Command;

fn run(expr: &str) -> (String, String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-p", expr]).output().unwrap();
    (
        String::from_utf8_lossy(&out.stdout).trim_end().to_string(),
        String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("").to_string(),
        out.status.code().unwrap_or(-1),
    )
}

fn val(expr: &str) -> String {
    let (out, err, code) = run(expr);
    assert_eq!(code, 0, "{expr}: {err}");
    out
}

fn err(expr: &str) -> String {
    let (out, err, code) = run(expr);
    assert_eq!(code, 1, "{expr} should error, got {out}");
    err
}

#[test]
fn member_and_find_with_unit_or_empty_x() {
    assert_eq!(val("(<0) ≡ \"abc\" ∊ 1‿0⥊0"), "1");
    assert_eq!(val("(<1) ≡ ⟨⟩ ∊ 1‿0⥊0"), "1");
    assert_eq!(val("(<0) ≡ 0 ⍷ <5"), "1");
    assert_eq!(val("(2‿4⥊1) ≡ ⟨⟩ ⍷ 2‿3⥊↕6"), "1");
    assert_eq!(err("0 ∊ <5"), "Error: 𝕨∊𝕩: 𝕩 cannot have rank 0");
}

#[test]
fn multi_axis_take_drop_on_empty() {
    assert_eq!(val("≢1‿2‿3 ↑ 1‿0⥊0"), "⟨ 1 2 3 ⟩");
    assert_eq!(val("≢0‿1‿1 ↓ ⟨⟩"), "⟨ 1 0 0 ⟩");
}

#[test]
fn char_minus_and_not() {
    assert_eq!(val("\"abc\" ¬ <5"), "\"]^_\"");
}

#[test]
fn empty_side_shape_mismatch_errors() {
    assert_eq!(err("⟨⟩ - \"abc\""), "Error: Mapping: Expected equal shape prefix (⟨0⟩ ≡ ≢𝕨, ⟨3⟩ ≡ ≢𝕩)");
    assert!(err("\"abc\" -˜ 1‿0⥊0").starts_with("Error: Mapping: Expected equal shape prefix"));
}

#[test]
fn char_indices_and_empty_joins_error() {
    assert!(err("\"abc\" ⊏ 1‿0⥊0").starts_with("Error: 𝕨⊏𝕩: 𝕨 must be an array of numbers"));
    assert_eq!(err("∾∘⌽ 1‿0⥊0"), "Error: ∾𝕩: Empty array 𝕩 cannot have an atom fill element");
    assert_eq!(err("⌽⊸↕ 1‿0⥊0"), "Error: 𝕨↕𝕩: 𝕨 must have rank at most 1");
}

#[test]
fn scan_with_mismatched_w_errors() {
    err("1‿2‿3 ≍` 2‿3⥊↕6");
}

#[test]
fn negative_scalar_replicate_on_empty_errors() {
    assert_eq!(err("¯2 / ⟨⟩"), "Error: 𝕨/𝕩: 𝕨 cannot be negative");
}
