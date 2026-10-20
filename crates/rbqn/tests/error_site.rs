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

#[test]
fn compile_error_has_site() {
    let (err, code) = run(&["-p", "foo+1"]);
    assert_eq!(code, 1);
    assert_eq!(err, "Error: Undefined identifier\n(-p):1:\n  foo+1\n  ^^^");
}

#[test]
fn compile_error_in_file_has_site() {
    let dir = std::env::temp_dir().join(format!("rbqn-errsite-comp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("t.bqn");
    std::fs::write(&f, "a←1\nb←a+\n").unwrap();
    let (err, code) = run(&["-f", f.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert_eq!(
        err,
        format!("Error: Role of the two sides in assignment must match\n{}:2:\n  b←a+\n  ^", f.display())
    );
}

#[test]
fn sys_bqn_compile_error_names_its_source() {
    let (err, code) = run(&["-p", "•BQN \"1+\""]);
    assert_eq!(code, 1);
    assert_eq!(err, "Error: Second-level parts of a train must be functions\n(•BQN):1:\n  1+\n  ^");
}

#[test]
fn caught_compile_error_does_not_leak_its_site() {
    // The compile error is caught by ⎊; the runtime error that follows must show its own site.
    let (err, code) = run(&["-p", "{•BQN 𝕩}⎊{𝕩⋄ 1+⟨1,2⟩+1‿2‿3} \"1+\""]);
    assert_eq!(code, 1);
    assert_eq!(err, format!("{MSG}\n(-p):1:\n  {{•BQN 𝕩}}⎊{{𝕩⋄ 1+⟨1,2⟩+1‿2‿3}} \"1+\"\n                      ^"));
}

#[test]
fn unknown_long_option_is_named() {
    let (err, code) = run(&["--bogus"]);
    assert_eq!(code, 1);
    assert!(err.ends_with("unknown option: --bogus"), "{err}");
}

#[test]
fn piped_stdin_has_no_trailing_blank_line() {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_rbqn"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"1+1\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "2\n");
}

#[test]
fn import_compile_error_shows_innermost_site() {
    let dir = std::env::temp_dir().join(format!("rbqn-errsite-impc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let lib = dir.join("lib.bqn");
    std::fs::write(&lib, "x←1\ny←x+\n").unwrap();
    let main = dir.join("main.bqn");
    std::fs::write(&main, format!("z←•Import \"{}\"\n", lib.display())).unwrap();
    let (err, code) = run(&["-f", main.to_str().unwrap()]);
    assert_eq!(code, 1);
    let lib = lib.display().to_string();
    assert_eq!(err, format!("Error: •Import: error loading {0}\n{0}:2:\n  y←x+\n  ^", lib));
}
