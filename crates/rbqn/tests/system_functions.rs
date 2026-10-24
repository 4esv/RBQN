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

// spec/system.md line 75: "`•path` simply gives the path of the file in which it
// appears. It includes a trailing slash but not the name of the file itself."
// -p has no file, so RBQN uses the working directory plus a trailing slash (so
// `•path∾"data.txt"` works). CBQN returns the cwd without a slash; deliberate.
#[test]
fn sys_path_is_wdpath_with_slash_in_p_mode() {
    let (path, _, code) = run_print("•path");
    assert_eq!(code, 0, "•path should succeed");
    let (wd, _, _) = run_print("•wdpath∾\"/\"");
    assert_eq!(path, wd, "-p •path must be •wdpath plus slash");
    assert!(path.ends_with("/\""), "got: {path}");
}

/// Fresh canonical temp dir (macOS /var is a symlink to /private/var).
fn path_test_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("rbqn-151-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::canonicalize(&d).unwrap()
}

// spec/system.md#•path: the file's directory with a trailing slash, no file name.
// (CBQN omits the slash; deliberate divergence, spec is the target.)
#[test]
fn sys_path_is_directory_with_trailing_slash_for_script() {
    let d = path_test_dir("script");
    let f = d.join("p.bqn");
    std::fs::write(&f, "•Out •path\n•Out •name\n").unwrap();
    let out = rbqn_cmd().arg(&f).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let got = String::from_utf8_lossy(&out.stdout).to_string();
    assert_eq!(got, format!("{}/\np.bqn\n", d.display()));
}

#[test]
fn sys_path_imported_file_sees_its_own_directory() {
    let d = path_test_dir("import");
    std::fs::create_dir_all(d.join("sub")).unwrap();
    std::fs::write(d.join("sub/q.bqn"), "•Out •path\n•Out •name\n").unwrap();
    std::fs::write(d.join("p.bqn"), "•Import \"sub/q.bqn\"\n•Out •path\n").unwrap();
    let out = rbqn_cmd().arg(d.join("p.bqn")).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let got = String::from_utf8_lossy(&out.stdout).to_string();
    let d = d.display();
    assert_eq!(got, format!("{d}/sub/\nq.bqn\n{d}/\n"));
}

#[test]
fn sys_path_repl_is_wdpath_with_trailing_slash() {
    use std::io::Write;
    use std::process::Stdio;
    let d = path_test_dir("repl");
    let mut child = rbqn_cmd()
        .current_dir(&d)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all("•Out •path\n".as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let got = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(got.contains(&format!("{}/", d.display())), "REPL •path, got: {got}");
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

// -- •ns --

#[test]
fn sys_ns_get_has_keys_order() {
    for (expr, want) in [
        (r#"{a⇐1} •ns.Get "a""#, "1"),
        (r#"{a⇐1} •ns.Has "a""#, "1"),
        (r#"{a⇐1} •ns.Has "b""#, "0"),
        ("•ns.Keys {a⇐1,b⇐2}", r#"⟨ "a" "b" ⟩"#),
    ] {
        let (out, err, code) = run_print(expr);
        assert_eq!((out.as_str(), code), (want, 0), "{expr}: {err}");
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

#[test]
fn sys_ns_errors_match_cbqn() {
    for (expr, want) in [
        ("•ns.Get {a⇐1}", "This function can't be called monadically"),
        ("•ns.Has {a⇐1}", "This function can't be called monadically"),
        (r#"{a⇐1} •ns.Get "b""#, r#"Field named "b" not found"#),
    ] {
        let (out, err, code) = run_print(expr);
        assert_ne!(code, 0, "{expr}");
        assert!(format!("{out}{err}").contains(want), "{expr}: {out}{err}");
    }
}

// =============================================================================
// •MakeRand (#141)
// =============================================================================

fn print_expr(expr: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-p", expr]).output().unwrap();
    assert!(out.status.success(), "{expr}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn makerand_same_seed_same_sequence_across_processes() {
    let expr = "(•MakeRand 42).Range⟜1000000¨ ↕5";
    let first = print_expr(expr);
    for _ in 0..2 {
        assert_eq!(print_expr(expr), first);
    }
}

#[test]
fn makerand_seeds_differ_and_generators_are_independent() {
    assert_eq!(print_expr("((•MakeRand 1).Range 1000000) ≡ (•MakeRand 1).Range 1000000"), "1\n");
    assert_eq!(print_expr("((•MakeRand 1).Range 1000000) ≡ (•MakeRand 2).Range 1000000"), "0\n");
    // Drawing from one generator must not advance another.
    assert_eq!(
        print_expr("a←•MakeRand 3 ⋄ b←•MakeRand 3 ⋄ c←a.Range 1000000 ⋄ b.Range 1000000"),
        print_expr("(•MakeRand 3).Range 1000000"),
    );
}

#[test]
fn makerand_namespace_has_rand_fields() {
    assert_eq!(print_expr("g←•MakeRand 5 ⋄ ≢ g.Deal 6"), "⟨ 6 ⟩\n");
    assert_eq!(print_expr("g←•MakeRand 5 ⋄ ∧ g.Deal 6"), "⟨ 0 1 2 3 4 5 ⟩\n");
    assert_eq!(print_expr("g←•MakeRand 5 ⋄ ≠ 3 g.Subset 9"), "3\n");
}

#[test]
fn rand_stays_nondeterministic_across_processes() {
    let draws: std::collections::HashSet<String> =
        (0..4).map(|_| print_expr("•rand.Range 1000000000")).collect();
    assert!(draws.len() > 1, "•rand repeated: {draws:?}");
}

// ============================================================
// •math hyperbolics/ErfC/LogFact/Sum, •file metadata, •_timed (#133):
// expected values are CBQN's own output for the same expression
// ============================================================

/// First line of `-p` output, stdout and stderr combined.
fn print_first(expr: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-p", expr]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    text.lines().next().unwrap_or("").to_string()
}

#[test]
fn test_math_new_functions_match_cbqn() {
    let cases: &[(&str, &str)] = &[
        ("⟨•math.Sinh 1, •math.Cosh 1, •math.Tanh 1, •math.Asinh 1, •math.Acosh 2, •math.Atanh 0.5⟩",
         "⟨ 1.1752011936438014 1.5430806348152437 0.7615941559557649 0.881373587019543 1.3169578969248166 0.5493061443340549 ⟩"),
        ("•math.Sinh 0‿1‿2", "⟨ 0 1.1752011936438014 3.6268604078470186 ⟩"),
        ("•math.Sinh ⟨1,⟨2,3⟩⟩", "⟨ 1.1752011936438014 ⟨ 3.6268604078470186 10.017874927409903 ⟩ ⟩"),
        ("⟨•math.Acosh 0.5, •math.Atanh 2, •math.Atanh 1, •math.Atanh ¯1⟩", "⟨ NaN NaN ∞ ¯∞ ⟩"),
        ("⟨•math.ErfC 0.5, •math.ErfC ¯1, •math.ErfC 3⟩", "⟨ 0.4795001221869535 1.8427007929497148 2.2090496998585438e¯5 ⟩"),
        ("⟨•math.LogFact 5, •math.LogFact 0, •math.LogFact 0.5, •math.LogFact ¯1⟩", "⟨ 4.787491742782045 0 ¯0.12078223763524526 ∞ ⟩"),
        ("⟨•math.Sum 1e6⥊0.1, •math.Sum ⟨⟩, •math.Sum 1‿2‿3, •math.Sum 100⥊0.1⟩", "⟨ 100000.00000022355 0 6 9.999999999999998 ⟩"),
        ("•math.Sum 1e5⥊0.1", "9999.999999999669"),
        ("•math.Sum (↕50)÷3", "408.33333333333337"),
        // #152: expected values are CBQN's own output
        ("⟨2 •math.Atan2 ¯3, ¯1 •math.Atan2 ¯1, 0 •math.Atan2 0, 1 •math.Atan2 2⟩", "⟨ ¯0.982793723247329 ¯2.356194490192345 0 1.1071487177940904 ⟩"),
        ("⟨•math.Expm1 1e¯10, •math.Expm1 0.5, •math.Log1p 1e¯10, •math.Log1p 0.5, •math.Log1p ¯1⟩",
         "⟨ 1.00000000005e¯10 0.6487212707001282 9.999999999500001e¯11 0.4054651081081644 ¯∞ ⟩"),
        ("⟨•math.Log10 1000, •math.Log10 2, •math.Log2 8, •math.Log2 3, •math.Log2 ¯1, •math.Log10 0⟩",
         "⟨ 3 0.3010299956639812 3 1.584962500721156 NaN ¯∞ ⟩"),
        ("•math.Log10 1‿10‿⟨100,1000⟩", "⟨ 0 1 ⟨ 2 3 ⟩ ⟩"),
        ("•math.Log2 \"a\"", "Error: •math.Log2 𝕩: 𝕩 contained non-number"),
        ("•math.Atan2 1", "Error: This function can't be called monadically"),
        ("1 •math.Expm1 2", "Error: This function can't be called dyadically"),
        ("•math.Sinh \"a\"", "Error: •math.Sinh 𝕩: 𝕩 contained non-number"),
        ("•math.Sum 2‿2⥊1", "Error: •math.Sum 𝕩: 𝕩 must be a list (2‿2 ≡ ≢𝕩)"),
        ("•math.Sum 3", "Error: •math.Sum 𝕩: 𝕩 must be a list (⟨⟩ ≡ ≢𝕩)"),
        ("•math.Sum ⟨1,\"a\"⟩", "Error: •math.Sum 𝕩: 𝕩 elements must be numbers"),
        ("1 •math.Sinh 2", "Error: This function can't be called dyadically"),
    ];
    // NOTE: libm results differ by an ulp across platforms (macOS vs glibc), so numbers
    // are compared to 1e-14 relative; everything else must match exactly.
    let close = |got: &str, want: &str| {
        let (g, w): (Vec<&str>, Vec<&str>) = (got.split(' ').collect(), want.split(' ').collect());
        g.len() == w.len()
            && g.iter().zip(&w).all(|(a, b)| match (a.replace('¯', "-").parse::<f64>(), b.replace('¯', "-").parse::<f64>()) {
                (Ok(x), Ok(y)) => x == y || (x.is_nan() && y.is_nan()) || (x - y).abs() <= 1e-14 * y.abs().max(1e-300),
                _ => a == b,
            })
    };
    for (expr, want) in cases {
        let got = print_first(expr);
        // Sum is pure f64 arithmetic in a fixed order: bit-exact everywhere.
        let ok = if expr.contains("Sum") { got == *want } else { close(&got, want) };
        assert!(ok, "{expr}: got {got}, want {want}");
    }
}

#[test]
fn test_file_metadata_matches_cbqn() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let meta = std::fs::metadata("Cargo.toml").unwrap();
    assert_eq!(print_first("•file.Size \"Cargo.toml\""), meta.len().to_string());
    let secs = |t: std::time::SystemTime| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
    for (name, t) in [("Modified", meta.modified().unwrap()), ("Accessed", meta.accessed().unwrap())] {
        let got: f64 = print_first(&format!("•file.{name} \"Cargo.toml\"")).parse().unwrap();
        assert!((got - secs(t)).abs() < 5.0, "{name}: {got}");
    }
    let created: f64 = print_first("•file.Created \"Cargo.toml\"").parse().unwrap();
    assert!(created > 1.0e9);
    assert_eq!(print_first("•file.RealPath \"Cargo.toml\""), format!("\"{dir}/Cargo.toml\""));
    assert_eq!(print_first("•file.RealPath \"src/..\""), format!("\"{dir}\""));
    assert_eq!(print_first("(•file.MapBytes \"Cargo.toml\") ≡ •file.Bytes \"Cargo.toml\""), "1");
    assert_eq!(print_first("≠ •file.MapBytes \"Cargo.toml\""), meta.len().to_string());
    assert_eq!(
        print_first("•file.Size \"nonexist\""),
        format!("Error: Failed to access file \"{dir}/nonexist\": No such file or directory")
    );
    assert_eq!(
        print_first("•file.RealPath \"nonexist\""),
        format!("Error: Failed to resolve \"{dir}/nonexist\": No such file or directory")
    );
    assert_eq!(print_first("•file.MapBytes \"nonexist\""), "Error: Failed to open file: No such file or directory");
    assert_eq!(print_first("•file.MapBytes \"src\""), "Error: failed to mmap file");
    assert_eq!(print_first("•file.Size 3"), "Error: •file.Size: Path must be a list of characters");
    assert_eq!(print_first("•file.RealPath 3"), "Error: •file.RealPath: Path must be a list of characters");
    assert_eq!(print_first("1 •file.Size \"Cargo.toml\""), "Error: This function can't be called dyadically");
    assert_eq!(print_first("•Type •file.path"), "0");
}

#[test]
fn test_timed_matches_cbqn() {
    for expr in ["{𝕩} •_timed 100", "+´∘↕ •_timed 5", "3 {𝕨+𝕩} •_timed 2", "2 {1+𝕩} •_timed 3"] {
        let v: f64 = print_first(expr).replace('¯', "-").parse().unwrap_or_else(|_| panic!("{expr}"));
        assert!((0.0..1.0).contains(&v), "{expr}: {v}");
    }
    assert_eq!(print_first("0 {𝕩} •_timed 2"), "Error: 𝕨 𝔽•_timed 𝕩: 𝕨 must be an integer greater than 0");
    assert_eq!(print_first("1.5 {𝕩} •_timed 2"), "Error: Expected integer, got 1.5");
    assert_eq!(print_first("\"a\" {𝕩} •_timed 2"), "Error: Expected integer, got array");
    // 𝕨 is the repeat count: the operand runs 𝕨 times
    assert_eq!(print_first("n←0 ⋄ t←3 {n+↩1 ⋄ 𝕩} •_timed 0 ⋄ n"), "3");
    assert_eq!(print_first("•Type¨ ⟨•_timed⟩"), "⟨ 4 ⟩");
}
