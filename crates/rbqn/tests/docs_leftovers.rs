//! Docs corpus lines that differed from CBQN after the stranger batch (#90).
use std::io::Write;
use std::process::{Command, Stdio};

fn run(expr: &str) -> (String, String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-p", expr]).output().unwrap();
    (
        String::from_utf8_lossy(&out.stdout).trim_end().to_string(),
        String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("").to_string(),
        out.status.code().unwrap_or(-1),
    )
}

fn prints(expr: &str, want: &str) {
    let (out, err, code) = run(expr);
    assert_eq!((code, out.as_str()), (0, want), "{expr}: {err}");
}

fn errors(expr: &str, want: &str) {
    let (out, err, code) = run(expr);
    assert_eq!(code, 1, "{expr} returned a value: {out}");
    assert!(err.starts_with(want), "{expr}: {err}");
}

#[test]
fn join_items_may_omit_axes() {
    // join.md: a bordered multiplication table.
    prints(
        "n ← 2‿4‿6 ×{⟨𝕗,𝕩⟩≍⟨𝕨,𝕨𝔽⌜𝕩⟩} 5‿6‿7‿8 ⋄ ∾ n",
        "┌─               \n╵ ×  5  6  7  8  \n  2 10 12 14 16  \n  4 20 24 28 32  \n  6 30 36 42 48  \n                ┘",
    );
    prints("∾ 2‿2⥊⟨0, \"ab\", ↕3, 3‿2⥊↕6⟩", "┌─           \n╵ 0 'a' 'b'  \n  0 0   1    \n  1 2   3    \n  2 4   5    \n            ┘");
    prints("∾ (3‿1≍⌜4‿2‿5) ⥊¨ 2‿3⥊↕6", "┌─                       \n╵ 0 0 0 0 1 1 2 2 2 2 2  \n  0 0 0 0 1 1 2 2 2 2 2  \n  0 0 0 0 1 1 2 2 2 2 2  \n  3 3 3 3 4 4 5 5 5 5 5  \n                        ┘");
    errors("∾ 1‿2⥊⟨1,2⟩", "Error: ∾𝕩: Ranks of argument items too small");
    errors("∾ 2‿2⥊⟨0, 2‿3⥊0, ↕3, 3‿2⥊↕6⟩", "Error: ∾𝕩: Item ranks along an axis can differ by at most one");
    errors("∾ 2‿2⥊⟨0, \"abc\", ↕3, 3‿2⥊↕6⟩", "Error: ∾𝕩: Incompatible item shapes");
    errors("∾ 2‿2⥊⟨2‿2⥊0, 2‿3⥊0, 3‿2⥊0, 3‿2⥊0⟩", "Error: ∾𝕩: Incompatible item shapes");
}

#[test]
fn prefixes_of_suffixes_do_not_panic() {
    // prefixes.md; the panic was multi-axis ↑/↓ over a zero-length trailing axis.
    let line = "Prefs ← (↕1+≢)↑¨< ⋄ Suffs ← (↕1+≢)↓¨< ⋄ Prefs¨Suffs 3‿2⥊\"abcdef\"";
    let (out, err, code) = run(line);
    assert_eq!(code, 0, "{err}");
    assert!(out.starts_with("┌─"), "{out}");
    prints(
        &line.replace("Prefs¨Suffs", "≢¨ Prefs¨Suffs"),
        "┌─                         \n╵ ⟨ 4 3 ⟩ ⟨ 4 2 ⟩ ⟨ 4 1 ⟩  \n  ⟨ 3 3 ⟩ ⟨ 3 2 ⟩ ⟨ 3 1 ⟩  \n  ⟨ 2 3 ⟩ ⟨ 2 2 ⟩ ⟨ 2 1 ⟩  \n  ⟨ 1 3 ⟩ ⟨ 1 2 ⟩ ⟨ 1 1 ⟩  \n                          ┘",
    );
    prints("≢ ⟨1,0⟩↑2‿0⥊0", "⟨ 1 0 ⟩");
    prints("≢ ⟨1,0⟩↓2‿0⥊0", "⟨ 1 0 ⟩");
}

#[test]
fn reverse_unit_wording() {
    prints("⌽⎊•CurrentError 2", "\"⌽𝕩: 𝕩 cannot be a unit\"");
    prints("⌽⎊•CurrentError <2", "\"⌽𝕩: 𝕩 cannot be a unit\"");
}

#[test]
fn top_level_export_shows_value() {
    prints("diff←¯32 ⋄ Lower ⇐ -⟜diff", "-⟜¯32");
    prints("a⇐5 ⋄ 3", "3");
    prints("•Type •BQN \"a⇐5\"", "6");
}

#[test]
fn rebqn_result_displays_as_repl() {
    prints("•ReBQN {repl⇐\"strict\"}", "(REPL)");
}

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
fn repl_export_shows_value() {
    // quick.md
    let out = repl("diff←¯32\nLower ⇐ -⟜diff\nUpper ⇐ Lower⁼\nUpper 5\n");
    assert!(out.trim_end().ends_with("¯32\n-⟜¯32\n-⟜¯32⁼\n¯27"), "{out}");
}

#[test]
fn repl_last_read_keeps_variable() {
    // depth.md: a line's last read of a name must not clear it for later lines.
    let out = repl("F←+ ⋄ ≡f\n≡ ⟨'c',f,2⟩\na←1 ⋄ a\na\n");
    assert!(out.trim_end().ends_with("0\n1\n1\n1"), "{out}");
    assert!(!out.contains("Error"), "{out}");
}
