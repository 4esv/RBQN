// NOTE: rbqn-gen: dev tool to regenerate the embedded .bin bytecode files
// using RBQN alone (no CBQN, no external `bqn`).
//
// Regenerate (from the repo root, with mlochbaum/BQN checked out at the
// revision pinned by BQN_REV in .github/workflows/ci.yml):
//
//   BQN_SRC=/path/to/BQN/src cargo run --release -p rbqn \
//       --features gen-tools --bin rbqn-gen -- --self --fixpoint-check
//
// Modes:
//   --self           Compile the 4 BQN sources (r0, r1, c, f) with RBQN and
//                    write crates/rbqn/bins/*.bin. r0/r1 are first run through
//                    BQN's own preprocessor (src/pr.bqn, executed by RBQN via
//                    dyadic •Import), i.e. the text of
//                    `(⟨"0"⟩ •Import "pr.bqn").src` and `⟨"1"⟩` under any BQN.
//   --fixpoint-check Run --self until two consecutive rounds give
//                    byte-identical bins (leaves the fixpoint in bins/).
//                    CI then runs `git diff --exit-code crates/rbqn/bins`.
//
// This binary is NOT compiled during normal `cargo build`, only with gen-tools.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let self_mode = args.iter().any(|a| a == "--self");
    let fixpoint_mode = args.iter().any(|a| a == "--fixpoint-check");

    if !(self_mode || fixpoint_mode) {
        eprintln!("rbqn-gen: usage: BQN_SRC=/path/to/BQN/src rbqn-gen --self [--fixpoint-check]");
        std::process::exit(1);
    }

    let bqn_src = match env::var("BQN_SRC") {
        Ok(p) => {
            let path = PathBuf::from(p);
            if !path.join("c.bqn").is_file() {
                eprintln!("rbqn-gen: BQN_SRC does not contain c.bqn");
                std::process::exit(1);
            }
            path
        }
        Err(_) => {
            eprintln!("rbqn-gen: --self requires BQN_SRC=/path/to/mlochbaum/BQN/src");
            std::process::exit(1);
        }
    };

    let bins_dir = find_bins_dir();

    if fixpoint_mode {
        run_fixpoint_check(&bqn_src, &bins_dir);
    } else {
        self_compile_all(&bqn_src, &bins_dir);
        println!();
        println!("rbqn-gen: --self: All 4 bins compiled and written to bins/");
    }
}

// ---------------------------------------------------------------------------
// --self mode: compile all 4 BQN sources using RBQN
// ---------------------------------------------------------------------------

/// Bootstrap RBQN and return the Runtime.
fn bootstrap_rbqn() -> rbqn::bootstrap::Runtime {
    // Use catch_unwind instead of panic hook suppression so we can see errors
    let rt = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rbqn::bootstrap::bootstrap()
    })) {
        Ok(Ok(rt)) => rt,
        Ok(Err(e)) => {
            eprintln!("rbqn-gen: Bootstrap failed: {e}");
            std::process::exit(1);
        }
        Err(panic) => {
            let msg = panic.downcast_ref::<String>().map(|s| s.as_str())
                .or_else(|| panic.downcast_ref::<&str>().copied())
                .unwrap_or("unknown panic");
            eprintln!("rbqn-gen: Bootstrap panicked: {msg}");
            std::process::exit(1);
        }
    };
    // Suppress BQN runtime panics from here on (compilation errors use panic-based throw())
    std::panic::set_hook(Box::new(|_| {}));
    rbqn_vm::derive::set_sys_runtime(rt.compiler, rt.runtime.clone(), rt.formatter);
    rbqn_vm::derive::set_sys_compgen(rt.compgen);
    rbqn_vm::derive::set_sys_args(&[]);
    rbqn_vm::derive::set_sys_path("");
    rt
}

/// Create a compiler instance with extended glyphs for runtime compilation.
/// The extended set adds aliases: &=Type, ∩=Fill, ⍣=Log, $=GroupLen, %=GroupOrd (fns) + ⍝=_fillBy_ (md2)
fn make_rt_compiler(rt: &rbqn::bootstrap::Runtime) -> Result<rbqn_core::B, String> {
    use rbqn_core::array::BqnArr;
    use rbqn_vm::vm::tag_arr;
    use rbqn_vm::derive::c1;

    if rt.compgen.0 == rbqn_core::B::SENTINEL.0 {
        return Err("compgen not available".to_string());
    }

    // Extended glyph lists: standard glyphs + aliases
    let fn_ext: Vec<u32> = "+-×÷⋆√⌊⌈|¬∧∨<>≠=≤≥≡≢⊣⊢⥊∾≍⋈↑↓↕«»⌽⍉/⍋⍒⊏⊑⊐⊒∊⍷⊔!&∩⍣$%"
        .chars().map(|c| c as u32).collect();
    let md1_ext: Vec<u32> = "˙˜˘¨⌜⁼´˝`"
        .chars().map(|c| c as u32).collect();
    let md2_ext: Vec<u32> = "∘○⊸⟜⌾⊘◶⎉⚇⍟⎊⍝"
        .chars().map(|c| c as u32).collect();

    let glyphs_b = {
        let fn_arr = tag_arr(BqnArr::new_vec_c32(fn_ext));
        let md1_arr = tag_arr(BqnArr::new_vec_c32(md1_ext));
        let md2_arr = tag_arr(BqnArr::new_vec_c32(md2_ext));
        tag_arr(BqnArr::from_b_vec(vec![fn_arr, md1_arr, md2_arr]))
    };

    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        c1(rt.compgen, glyphs_b)
    })) {
        Ok(c) => Ok(c),
        Err(p) => {
            let msg = p.downcast_ref::<String>().map(|s| s.as_str())
                .or_else(|| p.downcast_ref::<&str>().copied())
                .unwrap_or("unknown panic");
            Err(format!("compgen panicked: {msg}"))
        }
    }
}

/// Build the "compiler runtime" array for runtime compilation with extended glyphs.
/// This array has one entry per glyph position (49 fn + 9 md1 + 12 md2 = 70 entries).
/// Standard glyphs map to fruntime[n], aliases REUSE B values from the standard provide
/// so that compiler_output_to_owned can match them by B value identity.
fn build_rt_compiler_runtime(fruntime: &[rbqn_core::B], provide: &[rbqn_core::B]) -> Vec<rbqn_core::B> {
    let mut rt = Vec::with_capacity(70);

    // 49 function glyphs: standard 44 + 5 aliases
    // Standard fn: "+-×÷⋆√⌊⌈|¬∧∨<>≠=≤≥≡≢⊣⊢⥊∾≍⋈↑↓↕«»⌽⍉/⍋⍒⊏⊑⊐⊒∊⍷⊔!" → fruntime[0..43]
    for i in 0..44 {
        rt.push(fruntime[i]);
    }
    // Aliases: reuse B values from the standard provide so identity matching works
    rt.push(provide[0]);      // 44: & = •Type (provide[0])
    rt.push(provide[1]);      // 45: ∩ = Fill (provide[1])
    rt.push(fruntime[4]);     // 46: ⍣ = Log (= ⋆, same fruntime entry)
    rt.push(provide[3]);      // 47: $ = •GroupLen (provide[3])
    rt.push(provide[4]);      // 48: % = •GroupOrd (provide[4])

    // 9 md1 glyphs: "˙˜˘¨⌜⁼´˝`" → fruntime[44..52]
    for i in 44..53 {
        rt.push(fruntime[i]);
    }

    // 12 md2 glyphs: standard 11 + 1 alias
    // "∘○⊸⟜⌾⊘◶⎉⚇⍟⎊" → fruntime[53..63]
    for i in 53..64 {
        rt.push(fruntime[i]);
    }
    // ⍝ = _fillBy_ — reuse from provide[20]
    rt.push(provide[20]); // 69: ⍝

    rt
}

/// Build extended provide array for compiler_output_to_owned identity matching.
/// Same layout as build_rt_compiler_runtime — used to match Runtime(n) back to Provide(n).
fn build_rt_provide(fruntime: &[rbqn_core::B], provide: &[rbqn_core::B]) -> Vec<rbqn_core::B> {
    build_rt_compiler_runtime(fruntime, provide)
}

/// Compile a single BQN source file and return the .bin bytes.
fn compile_bqn_source(
    rt: &rbqn::bootstrap::Runtime,
    provide: &[rbqn_core::B],
    rt_provide: &[rbqn_core::B],
    rt_compiler: rbqn_core::B,
    bqn_file: &str,
    src: &str,
    needs_wrap: bool,
    pp: &[String; 2],
) -> Result<Vec<u8>, String> {
    // For r0.bqn/r1.bqn: use preprocessed source + extended compiler
    let (code, use_compiler, use_provide) = if bqn_file == "r0.bqn" || bqn_file == "r1.bqn" {
        let pp_src = pp[(bqn_file == "r1.bqn") as usize].clone();
        (pp_src, rt_compiler, rt_provide)
    } else if needs_wrap {
        // c.bqn: replace •args with 𝕩, wrap in function block
        let args_pattern = "•args";
        let first_line = src.lines().next().unwrap_or("");
        let body_rest = src.lines().skip(1).collect::<Vec<_>>().join("\n");
        let new_first = if first_line.ends_with(args_pattern) {
            let prefix_len = first_line.len() - args_pattern.len();
            format!("{}𝕩", &first_line[..prefix_len])
        } else {
            "func‿mod1‿mod2 ← 𝕩".to_string()
        };
        (format!("{{\n{}\n{}\n}}", new_first, body_rest), rt.compiler, provide)
    } else {
        (src.to_string(), rt.compiler, provide)
    };

    // For runtime sources, pass the provide array as the compiler's runtime arg
    let comp_runtime: &[rbqn_core::B] = if bqn_file == "r0.bqn" || bqn_file == "r1.bqn" {
        use_provide
    } else {
        &rt.runtime
    };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rbqn::exec::compile_string_with_rt(&code, use_compiler, comp_runtime)
    }));

    match result {
        Ok(Ok(output)) => {
            // For serialization, use the STANDARD 40-entry provide (not the extended
            // 70-entry rt_provide). The bootstrap reconstructs objects using the 40-entry
            // provide, so function references must be mapped to those indices.
            // The extended rt_provide is only needed for the compiler's runtime arg.
            let serial_provide = if bqn_file == "r0.bqn" || bqn_file == "r1.bqn" {
                provide
            } else {
                use_provide
            };

            let (runtime_prev, runtime_ref) = if bqn_file == "r0.bqn" {
                (None, None)
            } else if bqn_file == "r1.bqn" {
                (Some(rt.runtime_0.as_slice()), None)
            } else {
                (None, Some(rt.runtime.as_slice()))
            };

            let owned = rbqn::exec::compiler_output_to_owned(
                &output,
                serial_provide,
                runtime_prev,
                runtime_ref,
            );
            Ok(rbqn::embedded::encode_owned_bytecode(&owned, "rbqn-self"))
        }
        Ok(Err(e)) => Err(format!("compile error: {e}")),
        Err(panic) => {
            let msg = panic.downcast_ref::<String>().map(|s| s.as_str())
                .or_else(|| panic.downcast_ref::<&str>().copied())
                .unwrap_or("unknown panic");
            Err(format!("panic: {msg}"))
        }
    }
}

/// Compile all 4 BQN sources and write to output_dir.
/// Panics (exit 1) on any compilation failure.
fn self_compile_all(bqn_src: &Path, output_dir: &Path) {
    println!("rbqn-gen: Bootstrapping RBQN...");
    let rt = bootstrap_rbqn();
    println!("rbqn-gen: Bootstrap OK");

    let provide = rbqn::bootstrap::build_provide(&rt.fruntime);
    let rt_provide = build_rt_provide(&rt.fruntime, &provide);

    println!("rbqn-gen: Creating runtime compiler (extended glyphs)...");
    let rt_compiler = match make_rt_compiler(&rt) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("rbqn-gen: --self: Failed to create runtime compiler: {e}");
            std::process::exit(1);
        }
    };

    let pp = [
        preprocess_runtime(&rt, bqn_src, "0"),
        preprocess_runtime(&rt, bqn_src, "1"),
    ];

    let sources: &[(&str, &str, bool)] = &[
        ("r0.bqn", "runtime0.bin", false),
        ("r1.bqn", "runtime1x.bin", false),
        ("c.bqn", "compiler.bin", true),
        ("f.bqn", "formatter.bin", false),
    ];

    for &(bqn_file, bin_name, needs_wrap) in sources {
        let src_path = bqn_src.join(bqn_file);
        let src = match fs::read_to_string(&src_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("rbqn-gen: --self: Failed to read {}: {e}", src_path.display());
                std::process::exit(1);
            }
        };

        match compile_bqn_source(&rt, &provide, &rt_provide, rt_compiler, bqn_file, &src, needs_wrap, &pp) {
            Ok(bin_bytes) => {
                let out = output_dir.join(bin_name);
                fs::write(&out, &bin_bytes).unwrap_or_else(|e| {
                    eprintln!("rbqn-gen: --self: Failed to write {}: {e}", out.display());
                    std::process::exit(1);
                });
                println!("rbqn-gen: --self: {} compiled OK ({} bytes)", bqn_file, bin_bytes.len());
            }
            Err(e) => {
                eprintln!("rbqn-gen: --self: {} FAILED: {e}", bqn_file);
                std::process::exit(1);
            }
        }
    }
}

/// Run BQN's own preprocessor (pr.bqn) under RBQN: the text of
/// `(⟨"<which>"⟩ •Import "<bqn_src>/pr.bqn").src`.
fn preprocess_runtime(rt: &rbqn::bootstrap::Runtime, bqn_src: &Path, which: &str) -> String {
    // NOTE: pr.bqn reads r0.bqn/r1.bqn with •FLines, relative to the working
    // directory (as under CBQN), so run it from BQN's src/.
    let src_abs = bqn_src.canonicalize().unwrap_or_else(|e| {
        eprintln!("rbqn-gen: cannot resolve BQN_SRC: {e}");
        std::process::exit(1);
    });
    let old_cwd = env::current_dir().expect("cwd");
    env::set_current_dir(&src_abs).expect("chdir BQN_SRC");
    let code = format!("(⟨\"{which}\"⟩ •Import \"{}\").src", src_abs.join("pr.bqn").display());
    let res = rbqn::exec::exec_string(rt, &code, 1);
    env::set_current_dir(old_cwd).expect("restore cwd");
    match res {
        Ok(v) => rbqn_vm::derive::b_to_string_pub(v),
        Err(e) => {
            eprintln!("rbqn-gen: preprocessing r{which} with pr.bqn failed: {e}");
            std::process::exit(1);
        }
    }
}

// ---------------------------------------------------------------------------
// --fixpoint-check: compile twice, compare output
// ---------------------------------------------------------------------------

fn run_fixpoint_check(bqn_src: &Path, bins_dir: &Path) {
    println!("rbqn-gen: --- Fixpoint swap test ---");

    let bin_names = ["runtime0.bin", "runtime1x.bin", "compiler.bin", "formatter.bin"];
    let mut prev_bins: Option<[Vec<u8>; 4]> = None;

    for round in 1..=5 {
        println!("rbqn-gen: fixpoint round {round}...");
        self_compile_all(bqn_src, bins_dir);

        // Read back the written bins
        let current_bins: [Vec<u8>; 4] = bin_names.map(|name| {
            fs::read(bins_dir.join(name)).unwrap_or_default()
        });

        if let Some(ref prev) = prev_bins {
            if *prev == current_bins {
                println!();
                println!("rbqn-gen: FIXPOINT reached at round {round} — self-hosting is stable.");
                println!("rbqn-gen: Bins are byte-for-byte reproducible.");
                return;
            } else {
                for (i, name) in bin_names.iter().enumerate() {
                    if prev[i] != current_bins[i] {
                        println!("rbqn-gen: round {round}: {name} differs ({} vs {} bytes)",
                            prev[i].len(), current_bins[i].len());
                    }
                }
            }
        }
        prev_bins = Some(current_bins);
    }

    eprintln!("rbqn-gen: WARNING: fixpoint not reached after 5 rounds.");
    std::process::exit(1);
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

fn find_bins_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = {repo}/crates/rbqn → bins/ is at {repo}/crates/rbqn/bins/
    if let Ok(manifest) = env::var("CARGO_MANIFEST_DIR") {
        let p = PathBuf::from(manifest).join("bins");
        if let Ok(canonical) = p.canonicalize() {
            if canonical.is_dir() {
                return canonical;
            }
        }
    }
    // Walk up from CWD
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = cwd.as_path();
    loop {
        let candidate = dir.join("bins");
        if candidate.is_dir() {
            return candidate;
        }
        match dir.parent() {
            Some(p) => dir = p,
            None => break,
        }
    }
    // Fallback: create it
    let fallback = PathBuf::from("bins");
    fs::create_dir_all(&fallback).ok();
    fallback
}
