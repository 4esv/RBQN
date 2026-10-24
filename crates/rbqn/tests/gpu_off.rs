//! `RBQN_GPU=off` and `--no-gpu` only switch the device off (#137): results must equal the
//! default mode on the bench pipelines, and the host-fused path must be the one taken.
use std::process::Command;

fn run(env: Option<&str>, flag: Option<&str>, code: &str) -> String {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rbqn"));
    c.env_remove("RBQN_GPU");
    if let Some(m) = env {
        c.env("RBQN_GPU", m);
    }
    if let Some(f) = flag {
        c.arg(f);
    }
    let out = c.args(["-p", code]).output().unwrap();
    assert!(out.status.success(), "{env:?} {flag:?} {code}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim_end().to_string()
}

// The three bench/wow/multicore.bqn workloads and the bench/wow/gpu.bqn pipeline, at 1e6.
const PIPELINES: [&str; 4] = [
    "a ← 1e6 ⥊ 100000‿2‿3 ⋄ +´ a",
    "a ← 1e6 ⥊ 0.5‿1.5‿2.25 ⋄ b ← 1e6 ⥊ 3.5‿0.25 ⋄ c ← 1e6 ⥊ 1.125‿7.5‿2‿0.75 ⋄ ⌈´ (a×b)+c",
    "a ← 1e6 ⥊ 0.5‿1.5‿2.25‿¯7.5 ⋄ ⌈´ a",
    "x ← 1e6 ⥊ 1‿2‿3‿4 ⋄ +´ (x×x) + 2×x",
];

#[test]
fn off_and_default_agree_on_bench_pipelines() {
    for p in PIPELINES {
        let d = run(None, None, p);
        assert_eq!(run(Some("off"), None, p), d, "RBQN_GPU=off differs: {p}");
        assert_eq!(run(None, Some("--no-gpu"), p), d, "--no-gpu differs: {p}");
    }
}

#[test]
fn off_keeps_host_fusion() {
    // The lazy graph is built and folded fused on the host, with no device dispatch.
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn"))
        .env("RBQN_GPU", "off")
        .env("RBQN_GPU_DEBUG", "1")
        .args(["-p", PIPELINES[3]])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("[gpu] lazy mul") && err.contains("host fold_add"), "no host fusion under off: {err}");
}
