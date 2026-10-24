//! Unknown fills (#103): overtake, nudge and join on arrays without a fill element must error.
//! Expected output is CBQN b62079a's, captured with `bqn -p`.

use std::process::Command;

fn run(expr: &str) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn"))
        .args(["-p", expr])
        .output()
        .expect("failed to execute rbqn");
    let s = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

fn errs(expr: &str, msg: &str) {
    let (ok, s) = run(expr);
    assert!(!ok && s.contains(msg), "{expr}: expected error {msg:?}, got {s:?}");
}

fn gives(expr: &str, want: &str) {
    let (ok, s) = run(expr);
    assert!(ok && s == want, "{expr}: expected {want:?}, got {s:?}");
}

const TAKE: &str = "𝕨↑𝕩: Fill element of 𝕩 needed for overtaking";
const NUDGE_L: &str = "«𝕩: Fill element of 𝕩 not known";
const NUDGE_R: &str = "»𝕩: Fill element of 𝕩 not known";
const JOIN: &str = "∾𝕩: Empty array 𝕩 cannot have an atom fill element";

#[test]
fn overtake_empty_list() {
    for e in ["1 ↑ ⟨⟩", "¯2 ↑ ⟨⟩", "3↑⟨⟩", "1‿2‿3 ↑ ⟨⟩", "<5 ↑ ⟨⟩"] {
        errs(e, TAKE);
    }
}

#[test]
fn overtake_mixed_list() {
    for e in ["1‿2‿3 ↑ ⟨1,\"a\",⟨2⟩⟩", "<5 ↑ ⟨1,\"a\",⟨2⟩⟩", "5↑⟨⟨1⟩,⟨2⟩⟩"] {
        errs(e, TAKE);
    }
}

#[test]
fn nudge_mixed_list() {
    errs("« ⟨1,\"a\",⟨2⟩⟩", NUDGE_L);
    errs("» ⟨1,\"a\",⟨2⟩⟩", NUDGE_R);
}

#[test]
fn join_atom_fill_empty() {
    for e in ["∾ ↕0", "∾∘⌽ ↕0", "∾○⌽ ↕0", "∾⊸⌽ ↕0", "∾⌾⌽ ↕0", "∾˘ 1‿0⥊0"] {
        errs(e, JOIN);
    }
}

#[test]
fn char_minus_invalid_intermediate() {
    errs("@ ¬ 0‿1‿1", "𝕨-𝕩: Invalid character");
    errs("@ ¬ 1", "𝕨-𝕩: Invalid character");
}

#[test]
fn still_fine() {
    gives("1↑0⥊0", "⟨ 0 ⟩\n");
    gives("1↑\"\"", "\" \"\n");
    gives("»⟨⟩", "⟨⟩\n");
    gives("¯7 ↑ 4⥊3", "⟨ 0 0 0 3 3 3 3 ⟩\n");
    gives("1↑↕0", "⟨ 0 ⟩\n");
    gives("2↑⟨⟨1⟩,⟨2⟩⟩", "⟨ ⟨ 1 ⟩ ⟨ 2 ⟩ ⟩\n");
}

// BQN test suite `fill` lines: Enclose, Pair and ⍉ on atoms carry fills.
#[test]
fn suite_fill_enclose_pair_transpose() {
    gives("∧´ (< ≡○(2⊸↑) ⋈)¨ ⟨'y', ↕0, ≍˘⟨2‿1,↕4‿2⟩⟩", "1\n");
    gives("((2⥊<) ≡○(3⊸↑) ⋈˜) ⟨\"ab\"‿⟨↕4⟩,2⟩", "1\n");
    gives("\"c  \" ≡ 3↑⍉'c'", "1\n");
}
