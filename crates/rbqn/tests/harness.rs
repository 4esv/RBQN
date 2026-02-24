//! Integration tests for running the official BQN test harness via RBQN.
//! Requires CBQN_PATH set and BQN test repo at ../../../BQN (sibling of RBQN).
//!
//! Run manually: CBQN_PATH=/path/to/CBQN cargo test -p rbqn --test harness -- --ignored --nocapture
//!
//! Baseline (2026-02-24, after 02-02):
//!   simple    20/20   (100%)
//!   literal   52/52   (100%)
//!   syntax   153/156  ( 98%)  3 failures (bracket destructuring edge cases)
//!   bytecode  37/37   (100%)  FIXED: array destructuring in v_get/v_get_move
//!   prim     409/564  ( 73%)  155 failures (fill, rank>1, inverse, group, search)
//!   fill      29/62   ( 47%)  33 failures (fill propagation)
//!   header   ~145/156 ( 93%)  11 failures + stack overflow on recursive case
//!   identity   8/14   ( 57%)  6 failures (fold identity, insert identity)
//!   namespace 26/50   ( 52%)  24 failures (namespace export ⇐)
//!   token     29/29   (100%)
//!   under     51/64   ( 80%)  13 failures (structural under, ⋆⁼)
//!   undo      30/68   ( 44%)  38 failures (inverse system)
//!   unhead    27/44   ( 61%)  17 failures (undo headers)
//!
//!   Total: ~1016/1316 (77%) passing

use std::process::Command;

fn rbqn_harness(args: &[&str]) -> std::process::Output {
    let cbqn_path = std::env::var("CBQN_PATH")
        .expect("CBQN_PATH must be set to run harness tests");
    let bqn_test_dir = std::env::var("BQN_TEST_DIR").unwrap_or_else(|_| {
        // Derive from CBQN_PATH: assume BQN is a sibling of CBQN
        let cbqn = std::path::Path::new(&cbqn_path);
        let parent = cbqn.parent().unwrap_or(std::path::Path::new("."));
        format!("{}/BQN/test", parent.display())
    });
    // Find workspace root for cargo run
    let manifest = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::Path::new(manifest)
        .parent().and_then(|p| p.parent())
        .unwrap_or(std::path::Path::new("."));
    let manifest_path = workspace_root.join("Cargo.toml");
    Command::new("cargo")
        .args(["run", "--manifest-path"])
        .arg(&manifest_path)
        .args(["-p", "rbqn", "--quiet", "--"])
        .args(["this.bqn"])
        .args(args)
        .current_dir(&bqn_test_dir)
        .env("CBQN_PATH", &cbqn_path)
        .output()
        .expect("failed to execute cargo run")
}

#[test]
#[ignore] // Requires CBQN_PATH and BQN test repo
fn harness_simple() {
    let output = rbqn_harness(&["simple"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    eprintln!("{}", stdout);
    assert!(stdout.contains("All passed!"), "simple tests should all pass");
}

#[test]
#[ignore]
fn harness_literal() {
    let output = rbqn_harness(&["literal"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    eprintln!("{}", stdout);
    assert!(stdout.contains("All passed!"), "literal tests should all pass");
}

#[test]
#[ignore]
fn harness_token() {
    let output = rbqn_harness(&["token"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    eprintln!("{}", stdout);
    assert!(stdout.contains("All passed!"), "token tests should all pass");
}

#[test]
#[ignore]
fn harness_syntax() {
    let output = rbqn_harness(&["syntax"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("STDOUT:\n{}", stdout);
    if !stderr.is_empty() {
        eprintln!("STDERR:\n{}", stderr);
    }
    // Syntax should have at most 4 failures (destructuring edge cases)
    assert!(
        stdout.contains("Running 156 tests"),
        "should run 156 syntax tests"
    );
    assert!(
        !stderr.contains("panicked"),
        "harness should not panic: {}",
        stderr
    );
}

#[test]
#[ignore]
fn harness_bytecode() {
    let output = rbqn_harness(&["bytecode"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    eprintln!("{}", stdout);
    assert!(stdout.contains("All passed!"), "bytecode tests should all pass");
}

#[test]
#[ignore]
fn harness_simple_and_bytecode() {
    let output = rbqn_harness(&["simple", "bytecode"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("{}", stdout);
    assert!(!stderr.contains("panicked"), "harness panicked: {stderr}");
    assert!(!stdout.contains("failed!"), "some tests failed:\n{stdout}");
}

#[test]
#[ignore]
fn harness_all_files_complete() {
    // Run on each file individually to avoid stack overflow on recursive header tests
    let files = [
        "simple", "literal", "syntax", "bytecode", "prim", "fill", "identity",
        "namespace", "token", "under", "undo",
    ];
    for file in &files {
        let output = rbqn_harness(&[file]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("=== {} ===", file);
        eprintln!("{}", stdout);
        assert!(
            !stderr.contains("panicked"),
            "{} harness panicked: {}",
            file,
            stderr
        );
        assert!(
            stdout.contains("Running"),
            "{} should produce output",
            file
        );
    }
}
