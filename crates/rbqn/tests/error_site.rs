//! Error output carries the source position: `<name>:<line>:`, the source line, a caret line.

use std::process::Command;

fn run(args: &[&str]) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn"))
        .args(args)
        .output()
        .expect("failed to execute rbqn");
    (
        String::from_utf8_lossy(&out.stderr).trim_end().to_string(),
        out.status.code().unwrap_or(-1),
    )
}

const MSG: &str = "Error: Mapping: Expected equal shape prefix (⟨2⟩ ≡ ≢𝕨, ⟨3⟩ ≡ ≢𝕩)";

#[test]
fn expression_error_has_site() {
    let (err, code) = run(&["-p", "1+⟨1,2⟩+1‿2‿3"]);
    assert_eq!(code, 1);
    assert_eq!(err, format!("{MSG}\n(-p):1:\n  1+⟨1,2⟩+1‿2‿3\n         ^"));
}

#[test]
fn file_error_has_site() {
    let dir = std::env::temp_dir().join(format!("rbqn-errsite-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("t.bqn");
    std::fs::write(&f, "a←1\nb←2\nc←a+⟨1,2⟩+1‿2‿3\n").unwrap();
    let (err, code) = run(&["-f", f.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert_eq!(err, format!("{MSG}\n{}:3:\n  c←a+⟨1,2⟩+1‿2‿3\n           ^", f.display()));
}

#[test]
fn import_error_shows_innermost_site() {
    let dir = std::env::temp_dir().join(format!("rbqn-errsite-imp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let lib = dir.join("lib.bqn");
    std::fs::write(&lib, "x←1\ny←x+⟨1,2⟩+1‿2‿3\n").unwrap();
    let main = dir.join("main.bqn");
    std::fs::write(&main, format!("z←•Import \"{}\"\n", lib.display())).unwrap();
    let (err, code) = run(&["-f", main.to_str().unwrap()]);
    assert_eq!(code, 1);
    let lib = lib.display().to_string();
    assert_eq!(
        err,
        format!(
            "Error: •Import: error loading {0}\n{0}:2:\n  y←x+⟨1,2⟩+1‿2‿3\n           ^",
            lib
        )
    );
}

#[test]
fn no_user_source_prints_message_only() {
    let (err, code) = run(&["-p", "•Import \"/nonexistent/x.bqn\""]);
    assert_eq!(code, 1);
    assert!(err.starts_with("Error: •Import: cannot read"), "{err}");
}
