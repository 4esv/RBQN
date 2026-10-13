//! •gpu namespace (issue #17): explicit f32 kernels. With the gpu feature the
//! results match the exact •math path within f32 rounding; a CPU-only build
//! (`--no-default-features`) errors clearly and reports •gpu.Available 0.

use std::process::Command;

/// Run `rbqn -p expr` from the binary built with this test's features.
fn run(expr: &str) -> (String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_rbqn"))
        .args(["-p", expr])
        .output()
        .expect("failed to run rbqn");
    (
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
        String::from_utf8_lossy(&output.stderr).trim().to_string(),
    )
}

#[cfg(feature = "gpu")]
fn num(expr: &str) -> Option<f64> {
    if run("•gpu.Available @").0 != "1" {
        eprintln!("no GPU device, skipping {expr}");
        return None;
    }
    let (out, err) = run(expr);
    Some(out.replace('¯', "-").parse().unwrap_or_else(|_| panic!("{expr}: {out} {err}")))
}

#[cfg(feature = "gpu")]
#[test]
fn matmul_matches_cpu_on_floats() {
    // Positive inputs, so no cancellation inflates the relative error.
    let e = "a←64‿70⥊0.1+0.37×↕13 ⋄ b←70‿50⥊÷1+↕11 ⋄ c←a •math.MatMul b ⋄ ⌈´⥊(|c-a •gpu.MatMul b)÷|c";
    if let Some(r) = num(e) {
        assert!(r < 1e-5, "max relative error {r}");
    }
}

#[cfg(feature = "gpu")]
#[test]
fn matmul_is_f32() {
    // The exact path gives 0.6000000000000001; f32 rounds 0.1 first.
    if let Some(r) = num("⊑⥊(2‿2⥊0.1) •gpu.MatMul 2‿2⥊3") {
        assert_eq!(r, (0.1f32 * 3.0 + 0.1f32 * 3.0) as f64);
    }
}

#[cfg(feature = "gpu")]
#[test]
fn softmax_rows_sum_to_one() {
    // 300 columns takes the multi-pass kernel, 7 the single-workgroup one.
    for e in [
        "⌈´|1-+˝˘ •gpu.Softmax 8‿300⥊÷1+↕17",
        "⌈´|1-+˝˘ •gpu.Softmax 5‿7⥊-↕9",
        "|1-+´ •gpu.Softmax ↕100",
    ] {
        if let Some(r) = num(e) {
            assert!(r < 1e-5, "{e}: {r}");
        }
    }
}

#[cfg(feature = "gpu")]
#[test]
fn empty_arguments() {
    if run("•gpu.Available @").0 != "1" {
        return;
    }
    assert_eq!(run("≢(0‿3⥊0) •gpu.MatMul 3‿2⥊1").0, "⟨ 0 2 ⟩");
    assert_eq!(run("•gpu.Softmax ⟨⟩").0, "⟨⟩");
}

#[cfg(not(feature = "gpu"))]
#[test]
fn cpu_only_build_errors() {
    assert_eq!(run("•gpu.Available @").0, "0");
    for (e, name) in [
        ("(2‿2⥊0.1) •gpu.MatMul 2‿2⥊3", "•gpu.MatMul"),
        ("•gpu.Softmax 1‿2‿3", "•gpu.Softmax"),
    ] {
        let (out, err) = run(e);
        let msg = format!("{out}{err}");
        assert!(msg.contains(&format!("{name}: no GPU available")), "{e}: {msg}");
    }
}
