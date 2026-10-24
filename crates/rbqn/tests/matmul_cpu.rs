//! CPU matmul kernel (#166): `•math.MatMul` and the `+˝∘×⎉1‿∞` idiom against a naive BQN reference,
//! at several thread counts.
use std::process::Command;

fn run(threads: &str, expr: &str) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn"))
        .env("RBQN_THREADS", threads)
        .env("RBQN_GPU", "off")
        .args(["-p", expr])
        .output()
        .unwrap();
    (String::from_utf8_lossy(&out.stdout).trim_end().to_string() + &String::from_utf8_lossy(&out.stderr), out.status.code().unwrap_or(-1))
}

fn all_threads(expr: &str, want: &str) {
    for t in ["1", "2", "12"] {
        let (out, code) = run(t, expr);
        assert_eq!((code, out.as_str()), (0, want), "RBQN_THREADS={t}: {expr}");
    }
}

#[test]
fn integers_exact_and_equal_to_generic_path_at_every_thread_count() {
    // 200×150 by 150×90 integers (including negatives): MatMul, the idiom, and the generic block all agree.
    let e = "r←•MakeRand 5 ⋄ a←(r.Range 2001)-1000 ⋄ w←200‿150⥊a ⋄ x←150‿90⥊(r.Range 301)-150 ⋄ G←{+˝𝕨×𝕩}⎉1‿∞ ⋄ (w •math.MatMul x)≡w G x";
    all_threads(e, "1");
    let e = "r←•MakeRand 6 ⋄ w←200‿150⥊(r.Range 2001)-1000 ⋄ x←150‿90⥊(r.Range 301)-150 ⋄ G←{+˝𝕨×𝕩}⎉1‿∞ ⋄ (w +˝∘×⎉1‿∞ x)≡w G x";
    all_threads(e, "1");
}

#[test]
fn f64_matches_generic_path_within_one_ulp_64() {
    // 64×64 random f64: elementwise relative difference vs the generic rank path is at most 1 ulp (2⋆¯52).
    let e = "r←•MakeRand 9 ⋄ w←64‿64⥊r.Range 0 ⋄ x←64‿64⥊r.Range 0 ⋄ G←{+˝𝕨×𝕩}⎉1‿∞ ⋄ a←w •math.MatMul x ⋄ b←w G x ⋄ ⌈´⥊(|a-b)÷(2⋆¯52)×1⌈|b";
    for t in ["1", "2", "12"] {
        let (out, code) = run(t, e);
        assert_eq!(code, 0, "{out}");
        let v: f64 = out.replace('¯', "-").parse().unwrap();
        assert!(v <= 1.0, "RBQN_THREADS={t}: {v} ulp");
    }
}

#[test]
fn three_thread_settings_identical_f64_checksum() {
    let e = "r←•MakeRand 2 ⋄ w←130‿257⥊r.Range 0 ⋄ x←257‿71⥊r.Range 0 ⋄ •Repr +´⥊ w •math.MatMul x";
    let first = run("1", e);
    assert_eq!(first.1, 0, "{}", first.0);
    for t in ["2", "12"] {
        assert_eq!(run(t, e), first, "RBQN_THREADS={t}");
    }
}

#[test]
fn idiom_edge_cases() {
    all_threads("(1‿1⥊¯1) +˝∘×⎉1‿∞ 1‿1⥊0", "┌─   \n╵ 0  \n    ┘");
    all_threads("(2‿3⥊1‿2‿3‿4‿5‿6) +˝∘×⎉1‿∞ 3‿2⥊1‿0", "┌─      \n╵  6 0  \n  15 0  \n       ┘");
    // non-matching inner length still errors through the generic path
    let (out, code) = run("1", "(2‿3⥊0) +˝∘×⎉1‿∞ 4‿2⥊0");
    assert_eq!(code, 1, "{out}");
    // empty shapes still work (generic path)
    all_threads("≢ (0‿3⥊0) +˝∘×⎉1‿∞ 3‿2⥊1", "⟨ 0 2 ⟩");
    all_threads("≢ (2‿0⥊0) +˝∘×⎉1‿∞ 0‿3⥊1", "⟨ 2 3 ⟩");
}
