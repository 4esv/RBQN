//! `=` on block functions is identity of the block instance, environment included.

use std::process::Command;

fn eval(src: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn"))
        .args(["-p", src])
        .output()
        .expect("failed to execute rbqn");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

const GEN: &str = "Gen ← { a←𝕩 ⋄ {a×𝕩} }\nt2 ← Gen 2 ⋄ t3 ← Gen 3\n";

#[test]
fn distinct_closures_differ() {
    assert_eq!(eval(&format!("{GEN}t2 = t3")), "0");
}

#[test]
fn same_closure_equals_itself() {
    assert_eq!(eval(&format!("{GEN}t2 = t2")), "1");
}
