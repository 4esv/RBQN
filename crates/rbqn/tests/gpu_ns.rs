//! •gpu namespace (issue #17): explicit f32 kernels. With the gpu feature the
//! results match the exact •math path within f32 rounding; a CPU-only build
//! (`--no-default-features`) errors clearly and reports •gpu.Available 0.
//! Resident arrays (issue #14): •gpu.Tensor / •gpu.Fetch, transfers counted
//! from the RBQN_GPU_DEBUG log.
//! Resident fusion (issue #15): arithmetic on a resident value stays lazy
//! and runs as one kernel (`[gpu] fused …` in the log) when consumed.

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

/// `run` with RBQN_GPU_DEBUG=1: (stdout, stderr log).
#[cfg(feature = "gpu")]
fn run_debug(expr: &str) -> (String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_rbqn"))
        .args(["-p", expr])
        .env("RBQN_GPU_DEBUG", "1")
        .output()
        .expect("failed to run rbqn");
    (
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

/// Log lines starting with `[gpu] <what>`.
#[cfg(feature = "gpu")]
fn count(log: &str, what: &str) -> usize {
    log.lines().filter(|l| l.starts_with(&format!("[gpu] {what}"))).count()
}

#[cfg(feature = "gpu")]
fn gpu_ok() -> bool {
    let ok = run("•gpu.Available @").0 == "1";
    if !ok {
        eprintln!("no GPU device, skipping");
    }
    ok
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

#[cfg(feature = "gpu")]
#[test]
fn resident_matmul_chain_downloads_once() {
    if !gpu_ok() {
        return;
    }
    let (out, log) = run_debug("a←•gpu.Tensor 64‿64⥊0.5 ⋄ +´⥊ •gpu.Fetch a •gpu.MatMul a •gpu.MatMul a");
    assert_eq!(out, "2097152", "{log}");
    assert_eq!(count(&log, "matmul 64x64x64 (f32, resident)"), 2, "{log}");
    assert_eq!(count(&log, "upload"), 1, "{log}");
    assert_eq!(count(&log, "download"), 1, "{log}");
}

#[cfg(feature = "gpu")]
#[test]
fn tensor_fetch_round_trips() {
    if !gpu_ok() {
        return;
    }
    for x in [
        "↕10",
        "¯5+3‿4⥊↕12",
        "1+2⋆30‿25‿3",
        "(¯1-2⋆30)‿7",
        "0.25×↕1000",
        "2‿3‿4⥊÷2⋆↕24",
    ] {
        let e = format!("x←{x} ⋄ x ≡ •gpu.Fetch •gpu.Tensor x");
        assert_eq!(run(&e), ("1".into(), String::new()), "{e}");
    }
}

#[cfg(feature = "gpu")]
#[test]
fn transparent_download_is_cached() {
    if !gpu_ok() {
        return;
    }
    let (out, log) = run_debug("t←•gpu.Tensor ↕1000 ⋄ ⟨+´t, +´t, ≢t⟩");
    assert_eq!(out, "⟨ 499500 499500 ⟨ 1000 ⟩ ⟩", "{log}");
    assert_eq!(count(&log, "download"), 1, "{log}");
    // Shape alone needs no download.
    let (out, log) = run_debug("≢ (•gpu.Tensor 3‿4⥊0.5) •gpu.MatMul •gpu.Tensor 4‿5⥊1");
    assert_eq!(out, "⟨ 3 5 ⟩", "{log}");
    assert_eq!(count(&log, "download"), 0, "{log}");
}

#[cfg(feature = "gpu")]
#[test]
fn show_prints_downloaded_array() {
    if !gpu_ok() {
        return;
    }
    assert_eq!(run("•Show •gpu.Tensor 2‿3⥊↕6").0, run("•Show 2‿3⥊↕6").0);
    assert_eq!(run("•Fmt •gpu.Tensor 2‿3⥊↕6").0, run("•Fmt 2‿3⥊↕6").0);
}

/// Max relative error of `vals` (a BQN list) against `host`, from one run.
#[cfg(feature = "gpu")]
fn rel_err(setup: &str, gpu: &str, host: &str) -> f64 {
    let e = format!("{setup} ⋄ h←{host} ⋄ ⌈´⥊(|h-{gpu})÷1⌈|h");
    let (out, err) = run(&e);
    out.replace('¯', "-").parse().unwrap_or_else(|_| panic!("{e}: {out} {err}"))
}

#[cfg(feature = "gpu")]
#[test]
fn fused_reduce_is_one_kernel() {
    if !gpu_ok() {
        return;
    }
    // 10^6 elements: ~1000 workgroup partials, combined on the host.
    let (out, log) = run_debug("x←↕1000000 ⋄ +´ 1+2×•gpu.Tensor x");
    assert_eq!(count(&log, "fused reduce_add: add(S0,mul(S1,L0)) 1000000 elements (f32)"), 1, "{log}");
    assert_eq!(count(&log, "fused"), 1, "{log}");
    assert_eq!(count(&log, "upload"), 1, "{log}");
    assert_eq!(count(&log, "download"), 0, "{log}");
    let v: f64 = out.parse().unwrap_or_else(|_| panic!("{out} {log}"));
    let exact = 1e12;
    assert!(((v - exact) / exact).abs() < 1e-5, "{v}");
}

#[cfg(feature = "gpu")]
#[test]
fn fused_fetch_is_one_kernel() {
    if !gpu_ok() {
        return;
    }
    let (_, log) = run_debug("t←•gpu.Tensor 0.5×↕1000 ⋄ •gpu.Fetch (2×t)+1");
    assert_eq!(count(&log, "fused map: add(mul(S0,L0),S1) 1000 elements (f32)"), 1, "{log}");
    assert_eq!(count(&log, "fused"), 1, "{log}");
    assert_eq!(count(&log, "upload"), 1, "{log}");
    assert_eq!(count(&log, "download"), 1, "{log}");
    let r = rel_err("x←0.5×↕1000 ⋄ t←•gpu.Tensor x", "•gpu.Fetch (2×t)+1", "(2×x)+1");
    assert!(r < 1e-6, "{r}");
}

#[cfg(feature = "gpu")]
#[test]
fn fused_ops_match_host() {
    if !gpu_ok() {
        return;
    }
    // Values are exact in f32 and never 0, so ÷ stays finite.
    let setup = "x←0.125+¯50+0.25×↕400 ⋄ y←⌽x ⋄ t←•gpu.Tensor x ⋄ u←•gpu.Tensor y";
    for f in [
        "{(𝕩+3)-𝕨×2}", "{𝕨÷𝕩}", "{3÷𝕩×𝕨}", "{𝕩⌊𝕨}", "{𝕩⌈2}", "{1.5|𝕩}", "{𝕨|𝕩}",
        "{-𝕩}", "{÷𝕩}", "{⌊𝕩×0.5}", "{⌈𝕩}", "{|𝕩-𝕨}", "{(|𝕩)⌈-𝕨÷4}",
    ] {
        let (_, log) = run_debug(&format!("{setup} ⋄ •gpu.Fetch u {f} t"));
        assert_eq!(count(&log, "fused map"), 1, "{f}: {log}");
        assert_eq!(count(&log, "download"), 1, "{f}: {log}");
        let r = rel_err(setup, &format!("•gpu.Fetch u {f} t"), &format!("y {f} x"));
        assert!(r < 1e-6, "{f}: {r}");
    }
    for f in ["+´", "×´", "⌈´", "⌊´"] {
        let (_, log) = run_debug(&format!("{setup} ⋄ {f} 1.01⌊÷|t"));
        assert_eq!(count(&log, "fused reduce"), 1, "{f}: {log}");
        assert_eq!(count(&log, "download"), 0, "{f}: {log}");
        let r = rel_err(setup, &format!("{f} 1.01⌊÷|t"), &format!("{f} 1.01⌊÷|x"));
        assert!(r < 1e-5, "{f}: {r}");
    }
}

#[cfg(feature = "gpu")]
#[test]
fn fused_chain_feeds_matmul() {
    if !gpu_ok() {
        return;
    }
    let (out, log) = run_debug("m←•gpu.Tensor 4‿4⥊1 ⋄ +´⥊ •gpu.Fetch m •gpu.MatMul 1+m");
    assert_eq!(out, "128", "{log}");
    assert_eq!(count(&log, "fused map"), 1, "{log}");
    assert_eq!(count(&log, "upload"), 1, "{log}");
    assert_eq!(count(&log, "download"), 1, "{log}");
}

#[cfg(feature = "gpu")]
#[test]
fn other_touches_download() {
    if !gpu_ok() {
        return;
    }
    // A host array operand, a rank-2 fold and an evaluated chain download as before.
    let (out, log) = run_debug("x←↕10 ⋄ t←•gpu.Tensor x ⋄ (+´t+x)≡+´2×x");
    assert_eq!(out, "1", "{log}");
    assert_eq!((count(&log, "fused"), count(&log, "download")), (0, 1), "{log}");
    let (_, log) = run_debug("+´ 2×•gpu.Tensor 2‿3⥊1");
    assert!(log.contains("must be a list"), "{log}");
    let (out, log) = run_debug("a←2×•gpu.Tensor ↕4 ⋄ ⟨•gpu.Fetch a, +´a⟩");
    assert_eq!(out, "⟨ ⟨ 0 2 4 6 ⟩ 12 ⟩", "{log}");
    assert_eq!((count(&log, "fused"), count(&log, "download")), (1, 1), "{log}");
}

#[cfg(not(feature = "gpu"))]
#[test]
fn cpu_only_build_errors() {
    assert_eq!(run("•gpu.Available @").0, "0");
    for (e, name) in [
        ("(2‿2⥊0.1) •gpu.MatMul 2‿2⥊3", "•gpu.MatMul"),
        ("•gpu.Softmax 1‿2‿3", "•gpu.Softmax"),
        ("•gpu.Tensor 1‿2‿3", "•gpu.Tensor"),
        ("+´ 1+2×•gpu.Tensor 1‿2‿3", "•gpu.Tensor"),
    ] {
        let (out, err) = run(e);
        let msg = format!("{out}{err}");
        assert!(msg.contains(&format!("{name}: no GPU available")), "{e}: {msg}");
    }
    // No resident value can exist, so Fetch is the identity.
    assert_eq!(run("•gpu.Fetch 1‿2‿3").0, "⟨ 1 2 3 ⟩");
}
