//! Under with negative take/drop (#100): the result goes back where 𝕨⊸↑ / 𝕨⊸↓ took it from.

use std::process::Command;

fn val(expr: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-p", expr]).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{expr}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim_end().to_string()
}

#[test]
fn negative_take_modifies_suffix() {
    assert_eq!(val("⌽⌾(¯4⊸↑) ↕7"), "⟨ 0 1 2 6 5 4 3 ⟩");
    assert_eq!(val("-⌾(¯2⊸↑) 1‿2‿3‿4‿5"), "⟨ 1 2 3 ¯4 ¯5 ⟩");
}

#[test]
fn negative_drop_modifies_prefix() {
    assert_eq!(val("⌽⌾(¯3⊸↓) ↕7"), "⟨ 3 2 1 0 4 5 6 ⟩");
}

#[test]
fn positive_counts_unchanged() {
    assert_eq!(val("⌽⌾(4⊸↑) ↕7"), "⟨ 3 2 1 0 4 5 6 ⟩");
    assert_eq!(val("⌽⌾(2⊸↓) ↕7"), "⟨ 0 1 6 5 4 3 2 ⟩");
}

#[test]
fn negative_count_on_rank_2() {
    assert_eq!(val("(⌽⌾(¯2⊸↑) 4‿3⥊↕12) ≡ 4‿3⥊0‿1‿2‿3‿4‿5‿9‿10‿11‿6‿7‿8"), "1");
    assert_eq!(val("(⌽⌾(¯1⊸↓) 4‿3⥊↕12) ≡ 4‿3⥊6‿7‿8‿3‿4‿5‿0‿1‿2‿9‿10‿11"), "1");
}

#[test]
fn negative_multi_axis() {
    assert_eq!(
        val("(-⌾(¯2‿¯3⊸↑) 4‿5⥊↕20) ≡ 4‿5⥊0‿1‿2‿3‿4‿5‿6‿7‿8‿9‿10‿11‿¯12‿¯13‿¯14‿15‿16‿¯17‿¯18‿¯19"),
        "1"
    );
    assert_eq!(
        val("(-⌾(¯2‿3⊸↓) 4‿5⥊↕20) ≡ 4‿5⥊0‿1‿2‿¯3‿¯4‿5‿6‿7‿¯8‿¯9‿10‿11‿12‿13‿14‿15‿16‿17‿18‿19"),
        "1"
    );
}

#[test]
fn rotate_under_negative_count() {
    assert_eq!(val("-⌾(¯2⊸⌽) ↕6"), "⟨ 0 ¯1 ¯2 ¯3 ¯4 ¯5 ⟩");
}
