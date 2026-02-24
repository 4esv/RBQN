//! Phase 1 end-to-end verification tests for RBQN.
//!
//! These tests validate all Phase 1 success criteria: working compiler pipeline,
//! system functions, string handling, REPL persistence, and modifiers.
//!
//! All tests require CBQN_PATH to be set and are marked #[ignore].
//! Run with: CBQN_PATH=/path/to/CBQN cargo test -p rbqn --test phase1_verification -- --test-threads=1 --ignored

use std::process::Command;

fn run_bqn(expr: &str) -> String {
    let output = Command::new("cargo")
        .args(["run", "-p", "rbqn", "-q", "--", "-p", expr])
        .env(
            "CBQN_PATH",
            std::env::var("CBQN_PATH").unwrap_or_default(),
        )
        .output()
        .expect("failed to run rbqn");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn run_bqn_eval(expr: &str) -> String {
    let output = Command::new("cargo")
        .args(["run", "-p", "rbqn", "-q", "--", "-e", expr])
        .env(
            "CBQN_PATH",
            std::env::var("CBQN_PATH").unwrap_or_default(),
        )
        .output()
        .expect("failed to run rbqn");
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        panic!("rbqn -e '{expr}' failed: {stderr}");
    }
    stdout
}

fn run_bqn_piped(input: &str) -> String {
    let output = Command::new("cargo")
        .args(["run", "-p", "rbqn", "-q", "--"])
        .env(
            "CBQN_PATH",
            std::env::var("CBQN_PATH").unwrap_or_default(),
        )
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to start rbqn");
    use std::io::Write;
    let mut child = output;
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let output = child.wait_with_output().expect("failed to wait for rbqn");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

// === SC1: Basic arithmetic through compiler ===

#[test]
#[ignore = "requires CBQN_PATH"]
fn arithmetic_add() {
    assert_eq!(run_bqn("1+1"), "2");
}

#[test]
#[ignore = "requires CBQN_PATH"]
fn arithmetic_multiply() {
    assert_eq!(run_bqn("3\u{00d7}4"), "12");
}

#[test]
#[ignore = "requires CBQN_PATH"]
fn arithmetic_subtract() {
    assert_eq!(run_bqn("10-3"), "7");
}

#[test]
#[ignore = "requires CBQN_PATH"]
fn arithmetic_power() {
    assert_eq!(run_bqn("2\u{22c6}3"), "8");
}

#[test]
#[ignore = "requires CBQN_PATH"]
fn arithmetic_floor() {
    assert_eq!(run_bqn("\u{230a}3.7"), "3");
}

// === SC1: String literals ===

#[test]
#[ignore = "requires CBQN_PATH"]
fn string_literal() {
    assert_eq!(run_bqn("\"hello\""), "\"hello\"");
}

#[test]
#[ignore = "requires CBQN_PATH"]
fn string_match() {
    assert_eq!(run_bqn("\"abc\"\u{2261}\"abc\""), "1");
}

#[test]
#[ignore = "requires CBQN_PATH"]
fn string_length() {
    // ≠"hello" = 5
    assert_eq!(run_bqn("\u{2260}\"hello\""), "5");
}

// === SC1: Modifiers through compiler ===

#[test]
#[ignore = "requires CBQN_PATH"]
fn fold_add() {
    assert_eq!(run_bqn("+\u{00b4}1\u{203f}2\u{203f}3"), "6");
}

#[test]
#[ignore = "requires CBQN_PATH"]
fn fold_multiply() {
    assert_eq!(run_bqn("\u{00d7}\u{00b4}2\u{203f}3\u{203f}4"), "24");
}

// === SC2: PrimInd bypass verification ===

#[test]
#[ignore = "requires CBQN_PATH"]
fn primind_exists() {
    // •PrimInd "+" should return a number (the index of + in the primitive table).
    // NOTE: The exact value depends on the provide array ordering. CBQN returns 0.
    // Our implementation may return a different index; the key is it does not error.
    let result = run_bqn("\u{2022}PrimInd \"+\"");
    let n: f64 = result.parse().expect("PrimInd should return a number");
    assert!(n >= 0.0, "PrimInd should return a non-negative index");
}

// === SC4: System functions ===

#[test]
#[ignore = "requires CBQN_PATH"]
fn sys_fmt() {
    assert_eq!(run_bqn("\u{2022}Fmt 42"), "\"42\"");
}

#[test]
#[ignore = "requires CBQN_PATH"]
fn sys_bqn() {
    assert_eq!(run_bqn("\u{2022}BQN \"1+1\""), "2");
}

#[test]
#[ignore = "requires CBQN_PATH"]
fn sys_repr() {
    assert_eq!(run_bqn("\u{2022}Repr 42"), "\"42\"");
}

// === SC5: Variable sequencing ===

#[test]
#[ignore = "requires CBQN_PATH"]
fn variable_sequence_single() {
    // a←5 ⋄ a should give 5
    assert_eq!(run_bqn("a\u{2190}5 \u{22c4} a"), "5");
}

#[test]
#[ignore = "requires CBQN_PATH"]
fn variable_sequence_two() {
    // a←3 ⋄ b←4 ⋄ a+b should give 7
    assert_eq!(run_bqn("a\u{2190}3 \u{22c4} b\u{2190}4 \u{22c4} a+b"), "7");
}

// === SC3: REPL variable persistence ===

#[test]
#[ignore = "requires CBQN_PATH"]
fn repl_variable_persist() {
    // a←5 on first line, a+1 on second line should output 6
    let output = run_bqn_piped("a\u{2190}5\na+1\n");
    let lines: Vec<&str> = output.lines().collect();
    assert!(lines.len() >= 2, "Expected at least 2 lines of output, got: {:?}", lines);
    assert_eq!(lines[0], "5", "First line should output 5");
    assert_eq!(lines[1], "6", "Second line should output 6");
}

#[test]
#[ignore = "requires CBQN_PATH"]
fn repl_function_persist() {
    // F←{𝕩×2} on first line, F 10 on second should output 20
    let output = run_bqn_piped("F\u{2190}{𝕩\u{00d7}2}\nF 10\n");
    let lines: Vec<&str> = output.lines().collect();
    assert!(lines.len() >= 2, "Expected at least 2 lines of output, got: {:?}", lines);
    assert_eq!(lines[1], "20", "F 10 should output 20");
}

#[test]
#[ignore = "requires CBQN_PATH"]
fn repl_variable_update() {
    // a←5, a←10, a+1 should give 11
    let output = run_bqn_piped("a\u{2190}5\na\u{2190}10\na+1\n");
    let lines: Vec<&str> = output.lines().collect();
    assert!(lines.len() >= 3, "Expected at least 3 lines of output, got: {:?}", lines);
    assert_eq!(lines[2], "11", "a+1 after reassignment should output 11");
}

// === Regression checks ===

#[test]
#[ignore = "requires CBQN_PATH"]
fn regression_no_regression_after_repl() {
    // Ensure -p mode still works correctly after REPL changes
    assert_eq!(run_bqn("1+1"), "2");
    assert_eq!(run_bqn("+\u{00b4}1\u{203f}2\u{203f}3"), "6");
    assert_eq!(run_bqn("\"hello\""), "\"hello\"");
}

// === Deep equality fix verification ===

#[test]
#[ignore = "requires CBQN_PATH"]
fn search_index_of_strings() {
    // ⟨"a"⟩⊐⟨"a"⟩ should give ⟨0⟩
    assert_eq!(run_bqn("\u{27e8}\"a\"\u{27e9}\u{2290}\u{27e8}\"a\"\u{27e9}"), "\u{27e8} 0 \u{27e9}");
}

#[test]
#[ignore = "requires CBQN_PATH"]
fn search_member_of_strings() {
    // ⟨"a"⟩∊⟨"a"⟩ should give ⟨1⟩
    assert_eq!(run_bqn("\u{27e8}\"a\"\u{27e9}\u{220a}\u{27e8}\"a\"\u{27e9}"), "\u{27e8} 1 \u{27e9}");
}
