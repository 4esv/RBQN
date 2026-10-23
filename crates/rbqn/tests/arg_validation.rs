//! Invalid arguments raise errors, as in CBQN, instead of returning a value (#65).

use std::process::Command;

fn run(expr: &str) -> (String, String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-p", expr]).output().unwrap();
    (
        String::from_utf8_lossy(&out.stdout).trim_end().to_string(),
        String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("").to_string(),
        out.status.code().unwrap_or(-1),
    )
}

fn errors(e: &str) {
    let (out, err, code) = run(e);
    assert_eq!(code, 1, "{e} returned a value: {out}");
    assert!(err.starts_with("Error: "), "{e}: {err}");
}

#[test]
fn couple_needs_equal_shapes() {
    errors("0 ≍ 1‿2‿3");
    errors("1‿2‿3 ≍ 0");
}

#[test]
fn grade_and_bins_validate() {
    errors("5‿6‿2‿4‿1 ⍋ 3");
    errors("1‿2‿3 ⍒ 3");
    errors("⍋ <5");
    errors("⍒ <5");
    errors("5 ⍋ 3");
}

#[test]
fn bins_skip_sortedness_for_empty_x() {
    assert_eq!(run("\"abc\" ⍒ ⟨⟩").0, "⟨⟩");
}

#[test]
fn pick_select_group_replicate_reject_units() {
    errors("0 ⊑ 0");
    errors("0 ⊏ <5");
    errors("0 ⊔ 0");
    errors("0 / 0");
    assert_eq!(run("⟨⟩ ⊑ 5").0, "5");
}

#[test]
fn shifts_validate() {
    errors("« <5");
    errors("» <5");
    errors("0 « 2‿3⥊↕6");
    errors("0 » 2‿3⥊↕6");
}

#[test]
fn rotate_member_transpose_windows_sort_reject_units() {
    errors("0 ⌽ 0");
    errors("1‿2‿3 ∊ <5");
    errors("0 ⍉ <5");
    errors("0 ↕ <5");
    errors("∧ <5");
    errors("∨ <5");
}

#[test]
fn folds_and_scans_validate_shape() {
    errors("1 +´ 2‿3⥊↕6");
    errors("1 +˝ <5");
    errors("+` 5");
    errors("1 +` <5");
    errors("1‿2 +` 2‿3⥊↕6");
}

#[test]
fn cells_validate_frames_and_result_shapes() {
    errors("\"∘∘\" »˘ 3‿2⥊\"abcdef\"");
    assert!(run("\"∘∘\" »˘ 2‿2⥊\"abcd\"").2 == 0);
    errors("∾˘ ⟨↕2, 2‿2⥊↕4⟩");
    errors("1‿2 +˘ 1‿2‿3");
}

#[test]
fn unit_cells_give_unit_results() {
    // Cells of a list and an atom 𝕩 are units, so these are enclosed like in CBQN.
    assert_eq!(run("6 +˝ 1‿2").0, "┌·   \n· 9  \n    ┘");
    assert_eq!(run("6 +˘ 5").0, "┌·    \n· 11  \n     ┘");
    assert_eq!(run("0 ×˝ ↕0").0, "0");
    assert_eq!(run("1‿2‿3 ≍˘ 5").0, "┌─     \n╵ 1 5  \n  2 5  \n  3 5  \n      ┘");
    assert_eq!(run("5 ∾˘ @").0, "⟨ 5 @ ⟩");
    assert_eq!(run("≢0 ≍˘ ⟨⟩").0, "⟨ 0 2 ⟩");
    assert_eq!(run("≢⟨⟩ ≍˘ 5").0, "⟨ 0 2 ⟩");
}

#[test]
fn merge_and_reshape_validate() {
    errors(">⟨1,\"a\"⟩");
    errors("1‿\"a\" ⥊ 5");
    assert_eq!(run(">⟨1,<2⟩").0, "⟨ 1 2 ⟩");
}

#[test]
fn reading_an_undefined_variable_errors() {
    let (_, err, code) = run("{ { a } ⋄ a←4 }");
    assert_eq!(code, 1);
    assert_eq!(err, "Error: Attempting to read variable which is not yet defined");
}
