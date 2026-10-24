//! Parallel typed arithmetic and folds (#104): every op in scope on 1<<20
//! arrays (above PAR_MIN) must equal the serial `-t 1` run, and the
//! per-element `¨` oracle, which never takes the typed path.
use std::process::Command;

fn run(threads: &str, code: &str) -> String {
    // NOTE: the parallel run lowers every threshold to 1<<16 so the 1<<20 arrays take the parallel paths.
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn"))
        .args(["-t", threads, "-p", code])
        .env("RBQN_PAR_MIN", "65536")
        .env("RBQN_FOLD_MIN", "65536")
        .output()
        .unwrap();
    assert!(out.status.success(), "-t {threads} {code}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim_end().to_string()
}

/// Run at -t 1 and -t 4 and require identical output; returns it.
fn same(code: &str) -> String {
    let s = run("1", code);
    let p = run("4", code);
    assert_eq!(s, p, "serial and parallel differ for {code}");
    s
}

const PRELUDE: &str = "n←2⋆20 ⋄ i←¯3+7|↕n ⋄ j←(1+5|↕n)×1-2×2|↕n ⋄ f←0.25×i ⋄ g←0.5+j ⋄ b←2|↕n ⋄ s←(1+↕n)|¯7+↕n ⋄ ps←⟨i‿j, f‿g, i‿g, b‿j, s‿i, b‿b, j‿b⟩ ⋄ Eq←{(≢𝕨)≡≢𝕩}∧{∧´(𝕨=𝕩)∨(𝕨≠𝕨)∧𝕩≠𝕩} ⋄ _ck←{w‿x←𝕩 ⋄ (w 𝔽 x) Eq w 𝔽¨ x}";

#[test]
fn dyadic_matches_serial_and_oracle() {
    // i, j: small ints; f, g: F64; b: Bit; s: wider ints. `¨` never takes the typed path.
    for op in "+ - × ÷ ⋆ ⌊ ⌈ | ∧ ∨ < > ≤ ≥ = ≠ ¬ √".split(' ') {
        let out = same(&format!("{PRELUDE} ⋄ ∧´ ({op}_ck)¨ ps"));
        assert_eq!(out, "1", "oracle mismatch for {op}");
    }
}

#[test]
fn monadic_matches_serial_and_oracle() {
    for op in "- × ÷ ⋆ √ ⌊ ⌈ | ¬ +".split(' ') {
        for v in ["f", "j", "i", "s", "g"] {
            let out = same(&format!("{PRELUDE} ⋄ ({op} {v}) Eq {op}¨ {v}"));
            assert_eq!(out, "1", "oracle mismatch for {op} {v}");
        }
    }
}

#[test]
fn integer_folds_exact() {
    let out = same(&format!(
        "{PRELUDE} ⋄ ⟨+´i, +´j, +´s, +´b, ⌈´i, ⌊´j, ⌈´s, ⌊´b, ×´1+b, 3+´i, 3⌈´j, ×´2⌊1+i⟩"
    ));
    assert!(out.starts_with("⟨ "), "{out}");
    // Exact values, independent of thread count.
    assert_eq!(same("n←2⋆20 ⋄ +´(n)⥊3"), "3145728");
    assert_eq!(same("n←2⋆20 ⋄ ⌈´(n)⥊¯5‿9‿2"), "9");
    assert_eq!(same("n←2⋆20 ⋄ ⌊´(n)⥊¯5‿9‿2"), "¯5");
    assert_eq!(same("n←2⋆20 ⋄ ×´(n)⥊1"), "1");
}

#[test]
fn integer_sum_magnitude_guard() {
    // 2^23 × (2^31-1) exceeds 2^53: the guard sends it down the serial path; same answer.
    assert_eq!(same("a←(2⋆23)⥊2147483647 ⋄ (+´a) = (2⋆23)×2147483647"), "1");
    // 2^20 × (2^31-1) < 2^53: the parallel path, exact.
    assert_eq!(same("a←(2⋆20)⥊2147483647 ⋄ (+´a) = (2⋆20)×2147483647"), "1");
    assert_eq!(same("a←(2⋆20)⥊¯2147483648 ⋄ (+´a) = (2⋆20)×¯2147483648"), "1");
}

#[test]
fn product_overflow_falls_back() {
    // 2^60 does not stay exact in the integer path: the f64 chain decides, parallel must agree.
    same("×´ ∾⟨(2⋆19)⥊1, 60⥊2, (2⋆19)⥊1⟩");
    same("×´ ∾⟨(2⋆19)⥊1, 40⥊2, (2⋆19)⥊1⟩");
    same("×´ (2⋆20)⥊¯2‿1‿1‿1‿0");
    same("×´ (2⋆20)⥊¯1‿1");
}

#[test]
fn float_minmax_and_nan_guard() {
    // F64 ⌈´/⌊´ parallel; NaN anywhere gives NaN; ±0 results stay serial.
    same("a←(2⋆20)÷˜1+↕2⋆20 ⋄ ⟨⌈´a, ⌊´a, 0.5⌈´a, 1e9⌊´a⟩");
    assert_eq!(same("a←∾⟨(2⋆19)⥊1.5, ⟨0÷0⟩, (2⋆19)⥊9.5⟩ ⋄ ⟨(⌈´a)≠⌈´a, (⌊´a)≠⌊´a⟩"), "⟨ 1 1 ⟩");
    same("a←(2⋆20)⥊0‿¯0 ⋄ ⟨1÷⌈´a, 1÷⌊´a⟩");
    same("a←(2⋆20)⥊0.5‿¯0 ⋄ ⟨1÷⌈´a, 1÷⌊´a⟩");
}

#[test]
fn float_sum_stays_serial() {
    // +´ on F64 must keep BQN's right-to-left rounding order.
    same("a←0.1×1+↕2⋆20 ⋄ •Repr +´a");
}

#[test]
fn threads_flag_and_version() {
    let v = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-t", "3", "--version"]).output().unwrap();
    assert!(String::from_utf8_lossy(&v.stdout).lines().any(|l| l == "threads: 3"));
    let v = Command::new(env!("CARGO_BIN_EXE_rbqn")).env("RBQN_THREADS", "2").arg("--version").output().unwrap();
    assert!(String::from_utf8_lossy(&v.stdout).lines().any(|l| l == "threads: 2"));
    let v = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-t", "0", "-p", "1"]).output().unwrap();
    assert!(!v.status.success());
}

#[test]
fn version_has_gpu_line() {
    let gpu = |envs: &[(&str, &str)], args: &[&str]| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_rbqn"));
        c.args(args).arg("--version");
        for (k, v) in envs {
            c.env(k, v);
        }
        let o = c.output().unwrap();
        String::from_utf8_lossy(&o.stdout).lines().find(|l| l.starts_with("gpu: ")).map(String::from)
    };
    // Disabled: exactly `gpu: off`, whatever hardware the test runs on.
    assert_eq!(gpu(&[("RBQN_GPU", "off")], &[]).as_deref(), Some("gpu: off"));
    assert_eq!(gpu(&[], &["--no-gpu"]).as_deref(), Some("gpu: off"));
    // Default: an adapter name or `none` (CI has no GPU), never empty.
    let d = gpu(&[], &[]).expect("gpu line");
    assert!(d.len() > "gpu: ".len(), "{d}");
}
