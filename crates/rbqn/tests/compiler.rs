//! Integration tests for the RBQN compiler pipeline.
//!
//! These tests shell out to `cargo run -p rbqn -- -e 'expr'` for end-to-end
//! validation. Tests that fail due to known bootstrap issues (runtime1 not yet
//! working) are marked `#[ignore]`.

use std::process::Command;

fn eval(expr: &str) -> Result<String, String> {
    let output = Command::new("cargo")
        .args(["run", "-p", "rbqn", "-q", "--", "-e", expr])
        .env(
            "CBQN_PATH",
            std::env::var("CBQN_PATH").unwrap_or_default(),
        )
        .output()
        .expect("Failed to execute cargo run");

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn assert_eval(expr: &str, expected: &str) {
    match eval(expr) {
        Ok(result) => assert_eq!(result, expected, "Expression: {}", expr),
        Err(err) => panic!("Expression '{}' failed: {}", expr, err),
    }
}

// ── Arithmetic ──────────────────────────────────────────────────────────────

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_basic_arithmetic() {
    assert_eval("1+1", "2");
    assert_eval("3×4", "12");
    assert_eval("10-7", "3");
    assert_eval("15÷3", "5");
}

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_negative_numbers() {
    assert_eval("¯3+5", "2");
    assert_eval("¯1×¯1", "1");
}

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_math_functions() {
    assert_eval("⌊3.7", "3");
    assert_eval("⌈3.2", "4");
}

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_float_arithmetic() {
    assert_eval("1.5+2.5", "4");
}

// ── Arrays ──────────────────────────────────────────────────────────────────

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_array_literal() {
    assert_eval("⟨1,2,3⟩", "⟨ 1 2 3 ⟩");
}

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_string_literal() {
    assert_eval("\"hello\"", "\"hello\"");
}

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_strand() {
    assert_eval("1‿2‿3", "⟨ 1 2 3 ⟩");
}

// ── Blocks / Functions ──────────────────────────────────────────────────────

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_function_block() {
    assert_eval("{𝕩+1} 5", "6");
}

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_dyadic_block() {
    assert_eval("3 {𝕨+𝕩} 4", "7");
}

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_recursive() {
    assert_eval("{𝕩≤1 ? 1 ; 𝕩×𝕊 𝕩-1} 5", "120");
}

// ── Trains ──────────────────────────────────────────────────────────────────

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_fork() {
    assert_eval("(⊢+⊢) 3", "6");
}

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_mean() {
    assert_eval("(+´÷≠) ⟨2,4,6⟩", "4");
}

// ── Modifiers ───────────────────────────────────────────────────────────────

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_each() {
    assert_eval("√¨ ⟨4,9,16⟩", "⟨ 2 3 4 ⟩");
}

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_fold() {
    assert_eval("+´ ⟨1,2,3,4⟩", "10");
}

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_scan() {
    assert_eval("+` ⟨1,2,3,4⟩", "⟨ 1 3 6 10 ⟩");
}

// ── Assignment ──────────────────────────────────────────────────────────────

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_assignment() {
    assert_eval("a←5 ⋄ a+3", "8");
}

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_modify() {
    assert_eval("a←5 ⋄ a+↩3 ⋄ a", "8");
}

// ── Namespace ───────────────────────────────────────────────────────────────

#[test]
#[ignore] // NOTE: blocked on runtime1 bootstrap — compiler not available
fn eval_namespace() {
    assert_eval("n←{a⇐1 ⋄ b⇐2} ⋄ n.a+n.b", "3");
}
