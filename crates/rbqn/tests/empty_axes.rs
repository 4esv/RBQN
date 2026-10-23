//! Empty arrays (#63): fills and 0-length axes. Expected output is CBQN's, captured with `bqn -p`.

use std::process::Command;

fn p(expr: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn"))
        .args(["-p", expr])
        .output()
        .expect("failed to execute rbqn");
    assert!(out.status.success(), "{expr}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn case_00() {
    assert_eq!(p("3↑⟨⟩"), "⟨ 0 0 0 ⟩\n");
}

#[test]
fn case_01() {
    assert_eq!(p("1↑⟨⟩"), "⟨ 0 ⟩\n");
}

#[test]
fn case_02() {
    assert_eq!(p("¯2↑⟨⟩"), "⟨ 0 0 ⟩\n");
}

#[test]
fn case_03() {
    assert_eq!(p("≢ ∾ 2‿0⥊<3‿4‿1⥊0"), "⟨ 6 0 1 ⟩\n");
}

#[test]
fn case_04() {
    assert_eq!(p("≢ ⌽˘ ↕0‿4‿3"), "⟨ 0 4 3 ⟩\n");
}

#[test]
fn case_05() {
    assert_eq!(p("↑↕3‿0"), "┌─             \n· ┌┐ ┌┐ ┌┐ ┌┐  \n  └┘ ╵  ╵  ╵   \n      ┘        \n         ┘     \n            ┘  \n              ┘\n");
}

#[test]
fn case_06() {
    assert_eq!(p("↑ 3‿0⥊0"), "┌─             \n· ┌┐ ┌┐ ┌┐ ┌┐  \n  └┘ ╵  ╵  ╵   \n      ┘        \n         ┘     \n            ┘  \n              ┘\n");
}

#[test]
fn case_07() {
    assert_eq!(p("↑ 1‿0⥊0"), "┌─       \n· ┌┐ ┌┐  \n  └┘ ╵   \n      ┘  \n        ┘\n");
}

#[test]
fn case_08() {
    assert_eq!(p("↓ 1‿0⥊0"), "┌─       \n· ┌┐ ┌┐  \n  ╵  └┘  \n   ┘     \n        ┘\n");
}

#[test]
fn case_09() {
    assert_eq!(p("1 ↑ 1‿0⥊0"), "┌┐\n╵ \n ┘\n");
}

#[test]
fn case_10() {
    assert_eq!(p("0 ↓ 1‿0⥊0"), "┌┐\n╵ \n ┘\n");
}

#[test]
fn case_11() {
    assert_eq!(p("⌽ 1‿0⥊0"), "┌┐\n╵ \n ┘\n");
}

#[test]
fn case_12() {
    assert_eq!(p("1 / 1‿0⥊0"), "┌┐\n╵ \n ┘\n");
}

#[test]
fn case_13() {
    assert_eq!(p("1‿2‿3 ⊏ 1‿0⥊0"), "┌┐\n╵ \n  \n  \n ┘\n");
}

#[test]
fn case_14() {
    assert_eq!(p("⟨⟩ « 1‿0⥊0"), "┌┐\n╵ \n ┘\n");
}

#[test]
fn case_15() {
    assert_eq!(p("⟨⟩ » 1‿0⥊0"), "┌┐\n╵ \n ┘\n");
}

#[test]
fn case_16() {
    assert_eq!(p("∾⟜⌽ 1‿0⥊0"), "┌┐\n╵ \n  \n ┘\n");
}

#[test]
fn case_17() {
    assert_eq!(p("↕0 ⥊ 'a'"), "┌·    \n· ⟨⟩  \n     ┘\n");
}

#[test]
fn case_18() {
    assert_eq!(p("∾˘ \"abc\""), "\"abc\"\n");
}

#[test]
fn case_19() {
    assert_eq!(p("∾˘ 1‿2‿3"), "⟨ 1 2 3 ⟩\n");
}

#[test]
fn case_20() {
    assert_eq!(p("1 ∾˘ ⟨⟩"), "↕0‿2\n");
}

#[test]
fn case_21() {
    assert_eq!(p("⟨⟩ +´ \"abc\""), "⟨⟩\n");
}

#[test]
fn case_22() {
    assert_eq!(p("⟨⟩ +˝ \"abc\""), "⟨⟩\n");
}

#[test]
fn case_23() {
    assert_eq!(p("⟨⟩ -˘ 'a'"), "⟨⟩\n");
}

#[test]
fn case_24() {
    assert_eq!(p("⟨⟩ -⁼ 'a'"), "⟨⟩\n");
}

#[test]
fn case_25() {
    assert_eq!(p("2‿3⥊↕6 ↓ 0"), "┌─          \n╵ ⟨⟩ ⟨⟩ ⟨⟩  \n  ⟨⟩ ⟨⟩ ⟨⟩  \n           ┘\n");
}
