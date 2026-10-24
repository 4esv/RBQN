//! Integration tests for BQN system functions.
//!
//! Tests shell out to the rbqn binary for end-to-end verification.
//! System functions require the full bootstrap pipeline (CBQN_PATH).
//!
//! Tests that require string literals in the BQN source are marked with
//! an additional ignore reason since the compiler currently cannot handle
//! string literals (pre-existing VM correctness issue).

use std::process::Command;

/// The rbqn binary cargo built for this test run (no nested `cargo run`, which
/// fails with exit 101 when tests run concurrently).
fn rbqn_cmd() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rbqn"));
    if let Ok(path) = std::env::var("CBQN_PATH") {
        cmd.env("CBQN_PATH", path);
    }
    cmd
}

/// Run rbqn with -p (print result) and return stdout.
fn run_print(expr: &str) -> (String, String, i32) {
    let output = rbqn_cmd()
        .args(["-p", expr])
        .output()
        .expect("Failed to execute rbqn");
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

/// Run rbqn with -e (eval, no print) and return stdout, stderr, exit code.
fn run_eval(expr: &str) -> (String, String, i32) {
    let output = rbqn_cmd()
        .args(["-e", expr])
        .output()
        .expect("Failed to execute rbqn");
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

// -- •Fmt --

#[test]
#[ignore = "requires CBQN_PATH"]
fn sys_fmt_number() {
    let (stdout, _, code) = run_print("•Fmt 42");
    assert_eq!(code, 0, "•Fmt 42 should succeed");
    assert_eq!(stdout, "\"42\"", "•Fmt 42 should return the string \"42\"");
}

#[test]
#[ignore = "requires CBQN_PATH"]
fn sys_fmt_zero() {
    let (stdout, _, code) = run_print("•Fmt 0");
    assert_eq!(code, 0);
    assert_eq!(stdout, "\"0\"");
}

// -- •Repr --

#[test]
#[ignore = "requires CBQN_PATH"]
fn sys_repr_number() {
    let (stdout, _, code) = run_print("•Repr 42");
    assert_eq!(code, 0, "•Repr 42 should succeed");
    // •Repr returns a source representation string
    assert_eq!(stdout, "\"42\"");
}

// -- •Show --

#[test]
fn sys_show_number() {
    let (stdout, stderr, code) = run_print("•Show 42");
    assert_eq!(code, 0, "•Show 42 should succeed");
    // •Show prints to stdout (like CBQN, so `rbqn f.bqn > out` keeps it) and returns x,
    // which -p then prints again.
    assert_eq!(stdout, "42\n42");
    assert_eq!(stderr, "");
}

#[test]
fn sys_out_rejects_non_string() {
    let (stdout, stderr, code) = run_eval("•Out 5");
    assert_eq!(code, 1);
    assert_eq!(stdout, "");
    assert!(stderr.starts_with("Error: •Out 𝕩: 𝕩 must be a string"), "{stderr}");
}

// -- •Exit --

#[test]
#[ignore = "requires CBQN_PATH"]
fn sys_exit_zero() {
    let (_, _, code) = run_eval("•Exit 0");
    assert_eq!(code, 0, "•Exit 0 should exit with code 0");
}

#[test]
#[ignore = "requires CBQN_PATH"]
fn sys_exit_one() {
    let (_, _, code) = run_eval("•Exit 1");
    assert_eq!(code, 1, "•Exit 1 should exit with code 1");
}

// -- •args --

#[test]
#[ignore = "requires CBQN_PATH"]
fn sys_args_empty_in_p_mode() {
    let (stdout, _, code) = run_print("•args");
    assert_eq!(code, 0, "•args should succeed");
    // In -p mode with no file args, •args is an empty list
    // The formatter should display it as ⟨⟩
    assert!(
        stdout.contains("⟨⟩") || stdout.contains("⟨ ⟩") || stdout == "⟨⟩",
        "•args should be empty in -p mode, got: {}",
        stdout
    );
}

// -- •wdpath --

#[test]
#[ignore = "requires CBQN_PATH"]
fn sys_wdpath_nonempty() {
    let (stdout, _, code) = run_print("•wdpath");
    assert_eq!(code, 0, "•wdpath should succeed");
    // •wdpath returns the current directory as a string
    // The formatter wraps it in quotes
    assert!(stdout.len() > 2, "•wdpath should return a non-empty path, got: {}", stdout);
    // Should contain a path separator
    assert!(
        stdout.contains('/') || stdout.contains('\\'),
        "•wdpath should contain a path separator, got: {}",
        stdout
    );
}

// -- •path --

#[test]
#[ignore = "requires CBQN_PATH"]
fn sys_path_empty_in_p_mode() {
    let (stdout, _, code) = run_print("•path");
    assert_eq!(code, 0, "•path should succeed");
    // In -p mode, •path is empty string "" or ⟨⟩
    // An empty char array displays as "" or ⟨⟩
    let is_empty = stdout == "\"\"" || stdout == "⟨⟩" || stdout.is_empty();
    assert!(is_empty, "•path should be empty in -p mode, got: {}", stdout);
}

// -- •name --

#[test]
#[ignore = "requires CBQN_PATH"]
fn sys_name_empty_in_p_mode() {
    let (stdout, _, code) = run_print("•name");
    assert_eq!(code, 0, "•name should succeed");
    let is_empty = stdout == "\"\"" || stdout == "⟨⟩" || stdout.is_empty();
    assert!(is_empty, "•name should be empty in -p mode, got: {}", stdout);
}

// -- •Out (requires string literal support) --

#[test]
#[ignore = "requires CBQN_PATH and string literal support (compiler bug)"]
fn sys_out_string() {
    let (stdout, _, code) = run_eval("•Out \"test\"");
    assert_eq!(code, 0, "•Out should succeed");
    assert!(stdout.contains("test"), "•Out should print 'test' to stdout, got: {}", stdout);
}

// -- •BQN (requires string literal support) --

#[test]
#[ignore = "requires CBQN_PATH and string literal support (compiler bug)"]
fn sys_bqn_eval() {
    let (stdout, _, code) = run_print("•BQN \"2+3\"");
    assert_eq!(code, 0, "•BQN should succeed");
    assert_eq!(stdout, "5", "•BQN \"2+3\" should return 5");
}

// -- Combined tests --

#[test]
#[ignore = "requires CBQN_PATH"]
fn sys_fmt_then_show() {
    // •Fmt returns a string, •Show prints and returns its arg
    // This tests that •Fmt 99 returns a formatted value
    let (stdout, _, code) = run_print("•Fmt 99");
    assert_eq!(code, 0);
    assert_eq!(stdout, "\"99\"");
}

#[test]
#[ignore = "requires CBQN_PATH"]
fn sys_exit_after_value() {
    // •Exit should terminate even after computing a value
    let (_, _, code) = run_eval("•Exit 42");
    assert_eq!(code, 42, "•Exit 42 should exit with code 42");
}

// ============================================================
// Error wording matches CBQN for messages scripts match on (#7)
// ============================================================

#[test]
fn test_error_wording_matches_cbqn() {
    let cases = [
        ("{𝕊𝕩}1", "Error: Stack overflow"),
        ("(↕3)+↕4", "Error: Mapping: Expected equal shape prefix (⟨3⟩ ≡ ≢𝕨, ⟨4⟩ ≡ ≢𝕩)"),
        ("(2‿3⥊0)+3‿2⥊0", "Error: Mapping: Expected equal shape prefix (2‿3 ≡ ≢𝕨, 3‿2 ≡ ≢𝕩)"),
        ("!0", "Error: Assertion error"),
        ("\"msg\"!0", "Error: msg"),
        ("1÷'a'", "Error: 𝕨÷𝕩: Unexpected argument types"),
        ("•Show 1 {", "Error: Unmatched bracket"),
    ];
    for (expr, want) in cases {
        let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-e", expr]).output().unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{expr} should fail");
        assert_eq!(stderr.lines().next().unwrap_or(""), want, "wording for {expr}");
    }
}

// ============================================================
// Recursion limits (#8)
// ============================================================

/// First line of output (•Show and errors both go to stderr) and success.
fn eval_bin(expr: &str) -> (String, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-e", expr]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    (text.lines().next().unwrap_or("").to_string(), out.status.success())
}

#[test]
fn test_block_depth_bounded_by_stack() {
    for (expr, want) in [("•Show {𝕊⍟(𝕩<100000) 𝕩+1} 0", "100001"), ("•Show {𝕩<100000 ? 𝕊 𝕩+1 ; 𝕩} 0", "100000")] {
        assert_eq!(eval_bin(expr), (want.into(), true), "{expr}");
    }
}

#[test]
fn test_runaway_recursion_raises() {
    // Must exit with a normal error status (not a signal) after unwinding.
    for expr in ["{𝕊 𝕩+1} 0", "{1+𝕊¨ <𝕩} 0"] {
        let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-e", expr]).output().unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(stderr.lines().next().unwrap_or(""), "Error: Stack overflow", "{expr}");
        assert!(out.status.code().is_some_and(|c| c != 0), "{expr}: {:?}", out.status);
    }
}

#[test]
fn test_native_recursion_guarded() {
    // Nesting deep enough to exhaust the 512 MB interpreter stack in native code
    // (no block on the recursion path): must raise, not abort the process.
    let expr = "a←0 ⋄ {𝕊: a↩⟨a⟩}¨↕5000000 ⋄ b←1 ⋄ {𝕊: b↩⟨b⟩}¨↕5000000 ⋄ •Show a≡b";
    assert_eq!(eval_bin(expr), ("Error: Stack overflow".into(), false));
}

// ============================================================
// runtime1 runs on first use of ⁼/⌾/⚇, not at startup (#20)
// ============================================================

#[test]
fn test_runtime1_deferred() {
    let run = |expr: &str| {
        let out = Command::new(env!("CARGO_BIN_EXE_rbqn"))
            .args(["-p", expr])
            .env("RBQN_TIMING", "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let ran = String::from_utf8_lossy(&out.stderr).contains("runtime1 (deferred)");
        (stdout, ran)
    };
    // The compiler's own ⌾ uses (k⊸/, k⊸⊏, i⊸⊑) stay native.
    assert_eq!(run("{a‿b←𝕩 ⋄ a+b×2}¨⟨1‿2,3‿4⟩"), ("⟨ 5 11 ⟩".to_string(), false));
    assert_eq!(run("⌽⌾(1‿0‿1‿1⊸/) 1‿2‿3‿4"), ("⟨ 4 2 3 1 ⟩".to_string(), false));
    assert_eq!(run("1‿0‿1 +⌾(1‿0‿1⊸/) 10‿20‿30"), ("⟨ 11 20 31 ⟩".to_string(), false));
    assert_eq!(run("-⌾(2⊸⊑) 1‿2‿3"), ("⟨ 1 2 ¯3 ⟩".to_string(), false));
    // Paths that need runtime1 still get it.
    assert_eq!(run("3 +⁼ 10"), ("7".to_string(), true));
    assert_eq!(run("(3⊸↑)⌾⌽ ↕6"), ("⟨ 3 4 5 ⟩".to_string(), false));
    assert_eq!(run("⌽⚇1 ⟨1‿2,3‿4⟩"), ("⟨ ⟨ 2 1 ⟩ ⟨ 4 3 ⟩ ⟩".to_string(), true));
}

// ============================================================
// •ReBQN and •primitives (#69); expected values are CBQN's output
// ============================================================

#[test]
fn rebqn_primitives_and_repl_match_cbqn() {
    let cases: &[(&str, &str)] = &[
        ("3 ↑ •primitives",
         "⟨ ⟨ '+' + ⟩ ⟨ '-' - ⟩ ⟨ '×' × ⟩ ⟩"),
        ("≠ •primitives",
         "64"),
        ("r←•ReBQN {repl⇐\"strict\"} ⋄ (R \"a←5\") ‿ (R \"a+1\")",
         "⟨ 5 6 ⟩"),
        ("r←•ReBQN {repl⇐\"strict\"} ⋄ R \"a←5\" ⋄ R⎊\"err\" \"a←6\"",
         "\"err\""),
        ("r←•ReBQN {repl⇐\"strict\"} ⋄ R⎊\"err\" \"zz\"",
         "\"err\""),
        ("l←•ReBQN {repl⇐\"loose\"} ⋄ L \"b←2\" ⋄ L \"b←3\" ⋄ L \"b\"",
         "3"),
        ("l←•ReBQN {repl⇐\"loose\"} ⋄ L⎊\"err\" \"zz\"",
         "\"err\""),
        ("n←•ReBQN {repl⇐\"none\"} ⋄ N \"c←2\" ⋄ N⎊\"err\" \"c\"",
         "\"err\""),
        ("n←•ReBQN {a⇐1} ⋄ N \"1+2\"",
         "3"),
        ("p←•ReBQN {primitives⇐⟨⟨'+',{𝕨-𝕩}⟩⟩} ⋄ P \"5+3\"",
         "2"),
        ("p←•ReBQN {primitives⇐⟨⟨'+',{𝕨-𝕩}⟩⟩} ⋄ P⎊\"err\" \"5×3\"",
         "\"err\""),
        ("p←•ReBQN {primitives⇐⟨⟨'+',{𝕨-𝕩}⟩⟩∾1↓•primitives} ⋄ (P \"5×3\")‿(P \"5+3\")",
         "⟨ 15 2 ⟩"),
        ("•ReBQN⎊\"err\" 5",
         "\"err\""),
        ("•ReBQN⎊\"err\" {repl⇐\"bad\"}",
         "\"err\""),
        ("•ReBQN⎊\"err\" {primitives⇐⟨⟨\"+\",{𝕨-𝕩}⟩⟩}",
         "\"err\""),
        ("•primitives",
         "⟨ ⟨ '+' + ⟩ ⟨ '-' - ⟩ ⟨ '×' × ⟩ ⟨ '÷' ÷ ⟩ ⟨ '⋆' ⋆ ⟩ ⟨ '√' √ ⟩ ⟨ '⌊' ⌊ ⟩ ⟨ '⌈' ⌈ ⟩ ⟨ '|' | ⟩ ⟨ '¬' ¬ ⟩ ⟨ '∧' ∧ ⟩ ⟨ '∨' ∨ ⟩ ⟨ '<' < ⟩ ⟨ '>' > ⟩ ⟨ '≠' ≠ ⟩ ⟨ '=' = ⟩ ⟨ '≤' ≤ ⟩ ⟨ '≥' ≥ ⟩ ⟨ '≡' ≡ ⟩ ⟨ '≢' ≢ ⟩ ⟨ '⊣' ⊣ ⟩ ⟨ '⊢' ⊢ ⟩ ⟨ '⥊' ⥊ ⟩ ⟨ '∾' ∾ ⟩ ⟨ '≍' ≍ ⟩ ⟨ '⋈' ⋈ ⟩ ⟨ '↑' ↑ ⟩ ⟨ '↓' ↓ ⟩ ⟨ '↕' ↕ ⟩ ⟨ '«' « ⟩ ⟨ '»' » ⟩ ⟨ '⌽' ⌽ ⟩ ⟨ '⍉' ⍉ ⟩ ⟨ '/' / ⟩ ⟨ '⍋' ⍋ ⟩ ⟨ '⍒' ⍒ ⟩ ⟨ '⊏' ⊏ ⟩ ⟨ '⊑' ⊑ ⟩ ⟨ '⊐' ⊐ ⟩ ⟨ '⊒' ⊒ ⟩ ⟨ '∊' ∊ ⟩ ⟨ '⍷' ⍷ ⟩ ⟨ '⊔' ⊔ ⟩ ⟨ '!' ! ⟩ ⟨ '˙' ˙ ⟩ ⟨ '˜' ˜ ⟩ ⟨ '˘' ˘ ⟩ ⟨ '¨' ¨ ⟩ ⟨ '⌜' ⌜ ⟩ ⟨ '⁼' ⁼ ⟩ ⟨ '´' ´ ⟩ ⟨ '˝' ˝ ⟩ ⟨ '`' ` ⟩ ⟨ '∘' ∘ ⟩ ⟨ '○' ○ ⟩ ⟨ '⊸' ⊸ ⟩ ⟨ '⟜' ⟜ ⟩ ⟨ '⌾' ⌾ ⟩ ⟨ '⊘' ⊘ ⟩ ⟨ '◶' ◶ ⟩ ⟨ '⎉' ⎉ ⟩ ⟨ '⚇' ⚇ ⟩ ⟨ '⍟' ⍟ ⟩ ⟨ '⎊' ⎊ ⟩ ⟩"),
    ];
    for (expr, want) in cases {
        let (stdout, stderr, code) = run_print(expr);
        assert_eq!(code, 0, "{expr}: {stderr}");
        assert_eq!(&stdout, want, "{expr}");
    }
}

// ============================================================
// •HashMap (#131): expected values are CBQN's own output for the same expression
// ============================================================

#[test]
fn test_hashmap_matches_cbqn() {
    let cases: &[(&str, &str, bool)] = &[
        ("h ← ⟨\"ab\",⟨1,2⟩⟩ •HashMap 10‿20 ⋄ ⟨h.Get \"ab\", h.Get 1‿2, h.Has ⟨1,2⟩, h.Count @⟩",
         "⟨ 10 20 1 2 ⟩", true),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ h.Count @",
         "6", true),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ h.Keys @",
         "⟨ \"ab\" ⟨ 1 2 ⟩ 3 'a' ⟨⟩ 0 ⟩", true),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ h.Values @",
         "⟨ 10 20 30 40 50 60 ⟩", true),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ ⟨h.Has \"zz\", h.Has ¯0, h.Get ¯0, h.Has 'a', h.Has \"a\"⟩",
         "⟨ 0 1 60 1 0 ⟩", true),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ 99 h.Get \"zz\"",
         "99", true),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ s ← 7‿8 h.Set 9 ⋄ ⟨h.Count @, h.Get 7‿8, s.Count @⟩",
         "⟨ 7 9 7 ⟩", true),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ s ← \"ab\" h.Set 5 ⋄ ⟨h.Get \"ab\", h.Count @⟩",
         "⟨ 5 6 ⟩", true),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ s ← \"ab\" h.Set 5 ⋄ h.Keys @",
         "⟨ \"ab\" ⟨ 1 2 ⟩ 3 'a' ⟨⟩ 0 ⟩", true),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ d ← h.Delete \"ab\" ⋄ d.Keys @",
         "⟨ ⟨ 1 2 ⟩ 3 'a' ⟨⟩ 0 ⟩", true),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ d ← h.Delete \"ab\" ⋄ s ← \"ab\" h.Set 1 ⋄ d.Values @",
         "⟨ 20 30 40 50 60 1 ⟩", true),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ d ← h.Delete ⟨1,2⟩ ⋄ d.Count @",
         "5", true),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ h.Get \"nope\"",
         "Error: (hashmap).Get: key not found", false),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ h.Delete \"nope\"",
         "Error: (hashmap).Delete: key not found", false),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ h.Set 1",
         "Error: This function can't be called monadically", false),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ 1 h.Delete 2",
         "Error: This function can't be called dyadically", false),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ 1 h.Count 2",
         "Error: This function can't be called dyadically", false),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ 1 h.Keys 2",
         "Error: This function can't be called dyadically", false),
        ("h ← ⟨\"ab\",⟨1,2⟩,3,'a',⟨⟩,0⟩ •HashMap 10‿20‿30‿40‿50‿60 ⋄ 1 h.Has 2",
         "Error: This function can't be called dyadically", false),
        ("⟨1,1,2⟩ •HashMap 5‿6‿7",
         "Error: •HashMap: 𝕨 contained duplicate keys", false),
        ("1 •HashMap 2",
         "Error: 𝕨 •HashMap 𝕩: Arguments must be lists (⟨⟩≡≢𝕨, ⟨⟩≡≢𝕩)", false),
        ("1‿2 •HashMap 3‿4‿5",
         "Error: 𝕨 •HashMap 𝕩: 𝕨 and 𝕩 must have the same length (2≡≠𝕨, 3≡≠𝕩)", false),
        ("1‿2 •HashMap 2‿2⥊0",
         "Error: 𝕨 •HashMap 𝕩: Arguments must be lists (⟨2⟩≡≢𝕨, 2‿2≡≢𝕩)", false),
        ("•HashMap 1‿2",
         "Error: This function can't be called monadically", false),
        ("h ← ⟨⟩ •HashMap ⟨⟩ ⋄ ⟨h.Count @, h.Keys @, h.Has 0⟩",
         "⟨ 0 ⟨⟩ 0 ⟩", true),
        ("f ← ⊑⟨+⟩ ⋄ h ← ⟨f,⊑⟨-⟩⟩ •HashMap 1‿2 ⋄ ⟨h.Get f, h.Get ⊑⟨-⟩, h.Has ⊑⟨×⟩⟩",
         "⟨ 1 2 0 ⟩", true),
        ("h ← ⟨⟩ •HashMap ⟨⟩ ⋄ s ← 1‿2 h.Set 3 ⋄ s ↩ (<2‿3) h.Set 4 ⋄ ⟨h.Get 1‿2, h.Get <2‿3, h.Has 1‿2‿3, h.Has (2‿1⥊1)⟩",
         "⟨ 3 4 0 0 ⟩", true),
        ("h ← ⟨0, ⟨0⟩⟩ •HashMap 1‿2 ⋄ n ← 0×¯1 ⋄ ⟨h.Get n, h.Get ⟨n⟩, 1÷n⟩",
         "⟨ 1 2 ∞ ⟩", true),
        ("a ← ⟨⟩ •HashMap ⟨⟩ ⋄ b ← ⟨⟩ •HashMap ⟨⟩ ⋄ s ← 1 a.Set 2 ⋄ ⟨a.Count @, b.Count @⟩",
         "⟨ 1 0 ⟩", true),
        ("h ← ⟨⟩ •HashMap ⟨⟩ ⋄ s ← {𝕩 h.Set 𝕩×2}¨ ↕1000 ⋄ d ← {h.Delete 𝕩}¨ 2×↕500 ⋄ ⟨h.Count @, h.Has 4, h.Has 5, 3↑h.Keys @, h.Get 999⟩",
         "⟨ 500 0 1 ⟨ 1 3 5 ⟩ 1998 ⟩", true),
    ];
    for (expr, want, ok) in cases {
        let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-p", expr]).output().unwrap();
        let (text, first_err) = (
            String::from_utf8_lossy(&out.stdout).trim().to_string(),
            String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("").to_string(),
        );
        assert_eq!(out.status.success(), *ok, "{expr}: {first_err}");
        assert_eq!(if *ok { &text } else { &first_err }, want, "{expr}");
    }
}

#[test]
fn test_hashmap_scale() {
    // 1e5 inserts, lookups and 5e4 deletes by array key must stay linear.
    let expr = "h ← ⟨⟩ •HashMap ⟨⟩ ⋄ s ← {⟨𝕩,\"k\"⟩ h.Set 𝕩}¨ ↕100000 ⋄ \
                a ← +´ {h.Get ⟨𝕩,\"k\"⟩}¨ ↕100000 ⋄ d ← {h.Delete ⟨𝕩,\"k\"⟩}¨ 2×↕50000 ⋄ ⟨a, h.Count @, h.Has ⟨2,\"k\"⟩, h.Has ⟨3,\"k\"⟩⟩";
    let t = std::time::Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-p", expr]).output().unwrap();
    let dt = t.elapsed();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "⟨ 4999950000 50000 0 1 ⟩");
    assert!(dt.as_secs_f64() < 1.0, "1e5 hashmap ops took {dt:?}");
}
