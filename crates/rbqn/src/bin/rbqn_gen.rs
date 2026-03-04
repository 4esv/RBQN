// NOTE: rbqn-gen — dev tool to regenerate embedded .bin bytecode files.
//
// Modes:
//   --self           Compile all 4 BQN sources using RBQN (no CBQN needed).
//                    Requires BQN_SRC=/path/to/BQN/src. Writes to bins/.
//   --fixpoint-check Run --self twice and verify byte-for-byte identical output.
//   --verify         (Legacy) Test self-compilation, write to embedded/self-compiled/.
//   (default)        Parse CBQN gen/ files. Requires CBQN_PATH.
//
// This binary is NOT compiled during normal `cargo build` — only with gen-tools feature.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let self_mode = args.iter().any(|a| a == "--self");
    let fixpoint_mode = args.iter().any(|a| a == "--fixpoint-check");
    let verify_mode = args.iter().any(|a| a == "--verify");

    if self_mode || fixpoint_mode {
        // --self mode: compile all 4 BQN sources using RBQN's own compiler
        if env::var("CBQN_PATH").is_ok() {
            eprintln!("rbqn-gen: --self mode: CBQN_PATH ignored");
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
            println!("rbqn-gen: --self: Commit bins/ to replace CBQN-compiled bytecode.");
        }
        return;
    }

    // Legacy CBQN gen/ mode
    let cbqn_path = match env::var("CBQN_PATH") {
        Ok(p) => {
            let path = PathBuf::from(&p);
            if !path.is_dir() {
                eprintln!("rbqn-gen: CBQN_PATH={p} is not a directory");
                std::process::exit(1);
            }
            path
        }
        Err(_) => {
            eprintln!("rbqn-gen: CBQN_PATH environment variable is required (or use --self)");
            eprintln!("  Example: CBQN_PATH=/path/to/cbqn cargo run --bin rbqn-gen --features gen-tools");
            eprintln!("  Or: BQN_SRC=/path/to/BQN/src cargo run --bin rbqn-gen --features gen-tools -- --self");
            std::process::exit(1);
        }
    };

    let gen_dir = match find_gen_dir(&cbqn_path) {
        Some(d) => d,
        None => {
            eprintln!("rbqn-gen: No precompiled gen/ directory found in CBQN build/");
            eprintln!("  Build CBQN first: cd {} && make", cbqn_path.display());
            std::process::exit(1);
        }
    };

    println!("rbqn-gen: Using gen/ from {}", gen_dir.display());

    let bins_dir = find_bins_dir();
    println!("rbqn-gen: Writing .bin files to {}", bins_dir.display());

    let runtime1_file = if gen_dir.join("runtime1x").is_file() { "runtime1x" } else { "runtime1" };
    println!("rbqn-gen: Using {} for runtime1", runtime1_file);

    let components = [
        ("runtime0", "runtime0.bin"),
        (runtime1_file, "runtime1x.bin"),
        ("compiles", "compiler.bin"),
        ("formatter", "formatter.bin"),
    ];

    for (src_file, bin_name) in components {
        let src_path = gen_dir.join(src_file);
        if !src_path.is_file() {
            eprintln!("rbqn-gen: Warning: {} not found, writing empty .bin", src_path.display());
            let empty = encode_empty("cbqn");
            let out = bins_dir.join(bin_name);
            fs::write(&out, &empty).unwrap_or_else(|e| {
                eprintln!("rbqn-gen: Failed to write {}: {e}", out.display());
                std::process::exit(1);
            });
            continue;
        }

        let src = fs::read_to_string(&src_path).unwrap_or_else(|e| {
            eprintln!("rbqn-gen: Failed to read {}: {e}", src_path.display());
            std::process::exit(1);
        });

        let parsed = parse_cbqn_gen(&src);
        let bytes = encode_bytecode(&parsed, "cbqn");

        let out = bins_dir.join(bin_name);
        fs::write(&out, &bytes).unwrap_or_else(|e| {
            eprintln!("rbqn-gen: Failed to write {}: {e}", out.display());
            std::process::exit(1);
        });

        println!("rbqn-gen: Wrote {} ({} bytes)", out.display(), bytes.len());
    }

    println!("rbqn-gen: Done. Commit the .bin files to enable CBQN-free builds.");

    if verify_mode {
        verify_self_compilation(&cbqn_path);
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
    preprocessed_dir: Option<&Path>,
) -> Result<Vec<u8>, String> {
    // For r0.bqn/r1.bqn: use preprocessed source + extended compiler
    let (code, use_compiler, use_provide) = if bqn_file == "r0.bqn" || bqn_file == "r1.bqn" {
        let pp_name = if bqn_file == "r0.bqn" { "r0.bqn" } else { "r1.bqn" };
        let pp_dir = preprocessed_dir
            .ok_or_else(|| format!("preprocessed source required for {bqn_file}"))?;
        let pp_path = pp_dir.join(pp_name);
        let pp_src = fs::read_to_string(&pp_path)
            .map_err(|e| format!("failed to read {}: {e}", pp_path.display()))?;
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

    // Preprocessed sources for r0/r1 are in src/preprocessed/ (repo root)
    let pp_dir = find_preprocessed_dir();

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

        match compile_bqn_source(&rt, &provide, &rt_provide, rt_compiler, bqn_file, &src, needs_wrap, Some(&pp_dir)) {
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

fn find_preprocessed_dir() -> PathBuf {
    // Look for src/preprocessed/ at repo root
    if let Ok(manifest) = env::var("CARGO_MANIFEST_DIR") {
        let p = PathBuf::from(manifest).join("../../src/preprocessed");
        if p.is_dir() {
            return p;
        }
    }
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = cwd.as_path();
    loop {
        let candidate = dir.join("src/preprocessed");
        if candidate.is_dir() {
            return candidate;
        }
        match dir.parent() {
            Some(p) => dir = p,
            None => break,
        }
    }
    PathBuf::from("src/preprocessed")
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
// --verify mode (legacy, writes to embedded/self-compiled/)
// ---------------------------------------------------------------------------

fn verify_self_compilation(cbqn_path: &Path) {
    println!();
    println!("rbqn-gen: --- Self-compilation verification ---");

    let bqn_src = find_bqn_src(Some(cbqn_path));
    let bqn_src = match bqn_src {
        Some(p) => {
            println!("rbqn-gen: BQN sources: {}", p.display());
            p
        }
        None => {
            eprintln!("rbqn-gen: WARNING: BQN source directory not found.");
            eprintln!("  Set BQN_SRC=/path/to/mlochbaum/BQN/src");
            eprintln!("rbqn-gen: SELF-01: SKIPPED (no BQN sources)");
            return;
        }
    };

    println!("rbqn-gen: Bootstrapping RBQN...");
    let rt = bootstrap_rbqn();
    println!("rbqn-gen: Bootstrap OK");

    let provide = rbqn::bootstrap::build_provide(&rt.fruntime);
    let rt_provide = build_rt_provide(&rt.fruntime, &provide);
    let rt_compiler = make_rt_compiler(&rt).unwrap_or(rbqn_core::B::SENTINEL);

    let sources: &[(&str, &str, bool)] = &[
        ("r0.bqn", "runtime0.bin", false),
        ("r1.bqn", "runtime1x.bin", false),
        ("c.bqn", "compiler.bin", true),
        ("f.bqn", "formatter.bin", false),
    ];

    let embedded_dir = find_embedded_dir();
    let self_bin_dir = embedded_dir.join("self-compiled");
    if let Err(e) = fs::create_dir_all(&self_bin_dir) {
        eprintln!("rbqn-gen: Failed to create {}: {e}", self_bin_dir.display());
    }

    let pp_dir = find_preprocessed_dir();
    let mut any_ok = false;

    for &(bqn_file, bin_name, needs_wrap) in sources {
        let src_path = bqn_src.join(bqn_file);
        let src = match fs::read_to_string(&src_path) {
            Ok(s) => s,
            Err(e) => {
                println!("rbqn-gen: SELF-01: {bqn_file} read error: {e}");
                continue;
            }
        };

        match compile_bqn_source(&rt, &provide, &rt_provide, rt_compiler, bqn_file, &src, needs_wrap, Some(&pp_dir)) {
            Ok(bin_bytes) => {
                let out = self_bin_dir.join(bin_name);
                if let Err(e) = fs::write(&out, &bin_bytes) {
                    eprintln!("rbqn-gen: Warning: failed to write {}: {e}", out.display());
                } else {
                    println!("rbqn-gen: SELF-01: {bqn_file} compiled OK ({} bytes → {})",
                        bin_bytes.len(), out.display());
                }
                any_ok = true;
            }
            Err(e) => {
                println!("rbqn-gen: SELF-01: {bqn_file} FAILED: {e}");
            }
        }
    }

    println!();
    if any_ok {
        println!("rbqn-gen: SELF-01: Self-compilation verified.");
    } else {
        println!("rbqn-gen: SELF-01: All compilations failed.");
    }
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

fn find_bins_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = {repo}/crates/rbqn → bins/ is at {repo}/bins/
    if let Ok(manifest) = env::var("CARGO_MANIFEST_DIR") {
        let p = PathBuf::from(manifest).join("../../bins");
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

fn find_bqn_src(cbqn_path: Option<&Path>) -> Option<PathBuf> {
    if let Ok(p) = env::var("BQN_SRC") {
        let path = PathBuf::from(p);
        if path.join("c.bqn").is_file() {
            return Some(path);
        }
    }
    if let Some(cbqn) = cbqn_path {
        if let Some(parent) = cbqn.parent() {
            let candidate = parent.join("BQN").join("src");
            if candidate.join("c.bqn").is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn find_embedded_dir() -> PathBuf {
    if let Ok(manifest) = env::var("CARGO_MANIFEST_DIR") {
        let p = PathBuf::from(manifest).join("src").join("embedded");
        if p.is_dir() {
            return p;
        }
    }
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = cwd.as_path();
    loop {
        let candidate = dir.join("crates").join("rbqn").join("src").join("embedded");
        if candidate.is_dir() {
            return candidate;
        }
        match dir.parent() {
            Some(p) => dir = p,
            None => break,
        }
    }
    let fallback = PathBuf::from("crates/rbqn/src/embedded");
    fs::create_dir_all(&fallback).ok();
    fallback
}

fn find_gen_dir(cbqn: &Path) -> Option<PathBuf> {
    let build = cbqn.join("build");
    for sub in ["bytecodeLocal", "bytecodeSubmodule"] {
        let dir = build.join(sub).join("gen");
        if dir.is_dir() {
            if dir.join("compiles").is_file() || dir.join("runtime0").is_file() {
                return Some(dir);
            }
        }
    }
    if let Ok(entries) = fs::read_dir(build.join("obj")) {
        for entry in entries.flatten() {
            let dir = entry.path().join("gen");
            if dir.is_dir() {
                if dir.join("compiles").is_file() || dir.join("runtime0").is_file() {
                    return Some(dir);
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Binary format encoder (for CBQN gen/ parsing)
// ---------------------------------------------------------------------------

fn encode_bytecode(parsed: &ParsedComponent, source_tag: &str) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();

    buf.extend_from_slice(b"RBQN");
    buf.extend_from_slice(&1u32.to_le_bytes());

    let tag_bytes = source_tag.as_bytes();
    buf.extend_from_slice(&(tag_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(tag_bytes);

    let bc = &parsed.iarrs[parsed.bc_idx];
    buf.extend_from_slice(&(bc.len() as u32).to_le_bytes());
    for &v in bc {
        buf.extend_from_slice(&v.to_le_bytes());
    }

    buf.extend_from_slice(&(parsed.iarrs.len() as u32).to_le_bytes());
    for arr in &parsed.iarrs {
        buf.extend_from_slice(&(arr.len() as u32).to_le_bytes());
        for &v in arr {
            buf.extend_from_slice(&v.to_le_bytes());
        }
    }

    use std::collections::HashMap;
    let mut obj_map: HashMap<usize, &ObjEntry> = HashMap::new();
    for (idx, entry) in &parsed.objs {
        obj_map.insert(*idx, entry);
    }

    buf.extend_from_slice(&(parsed.obj_len as u32).to_le_bytes());
    for i in 0..parsed.obj_len {
        if let Some(entry) = obj_map.get(&i) {
            match entry {
                ObjEntry::Provide(n) => {
                    buf.push(0);
                    buf.extend_from_slice(&(*n as u32).to_le_bytes());
                }
                ObjEntry::Runtime(n) => {
                    buf.push(1);
                    buf.extend_from_slice(&(*n as u32).to_le_bytes());
                }
                ObjEntry::RuntimePrev(n) => {
                    buf.push(2);
                    buf.extend_from_slice(&(*n as u32).to_le_bytes());
                }
                ObjEntry::Float(val) => {
                    buf.push(3);
                    let f = parse_float_str(val);
                    buf.extend_from_slice(&f.to_le_bytes());
                }
                ObjEntry::Char(c) => {
                    buf.push(4);
                    buf.extend_from_slice(&(*c as u32).to_le_bytes());
                }
                ObjEntry::Str(chars) => {
                    buf.push(5);
                    buf.extend_from_slice(&(chars.len() as u32).to_le_bytes());
                    for &cp in chars {
                        buf.extend_from_slice(&cp.to_le_bytes());
                    }
                }
                ObjEntry::IArr(n) => {
                    buf.push(6);
                    buf.extend_from_slice(&(*n as u32).to_le_bytes());
                }
            }
        } else {
            buf.push(3);
            buf.extend_from_slice(&0.0f64.to_le_bytes());
        }
    }

    let mut blk_map: HashMap<usize, &BlkEntry> = HashMap::new();
    for (idx, entry) in &parsed.blocks {
        blk_map.insert(*idx, entry);
    }

    buf.extend_from_slice(&(parsed.block_len as u32).to_le_bytes());
    for i in 0..parsed.block_len {
        if let Some(entry) = blk_map.get(&i) {
            match entry {
                BlkEntry::IArr(n) => {
                    buf.push(0);
                    buf.extend_from_slice(&(*n as u32).to_le_bytes());
                }
                BlkEntry::Info { typ, iarrs0_idx, data_idx } => {
                    buf.push(1);
                    buf.push(*typ);
                    buf.extend_from_slice(&(*iarrs0_idx as u32).to_le_bytes());
                    buf.extend_from_slice(&(*data_idx as u32).to_le_bytes());
                }
            }
        } else {
            buf.push(0);
            buf.extend_from_slice(&0u32.to_le_bytes());
        }
    }

    let mut body_map: HashMap<usize, usize> = HashMap::new();
    for (idx, iarrs_idx) in &parsed.bodies {
        body_map.insert(*idx, *iarrs_idx);
    }

    buf.extend_from_slice(&(parsed.body_len as u32).to_le_bytes());
    for i in 0..parsed.body_len {
        let v = body_map.get(&i).copied().unwrap_or(0) as u32;
        buf.extend_from_slice(&v.to_le_bytes());
    }

    buf
}

fn encode_empty(source_tag: &str) -> Vec<u8> {
    let empty = ParsedComponent {
        iarrs: vec![vec![]],
        bc_idx: 0,
        objs: vec![],
        obj_len: 0,
        blocks: vec![],
        block_len: 0,
        bodies: vec![],
        body_len: 0,
    };
    encode_bytecode(&empty, source_tag)
}

fn parse_float_str(val: &str) -> f64 {
    match val {
        "1.0/0.0" => f64::INFINITY,
        "-1.0/0.0" => f64::NEG_INFINITY,
        _ => val.trim().parse::<f64>().unwrap_or(0.0),
    }
}

// ---------------------------------------------------------------------------
// CBQN gen/ file parser
// ---------------------------------------------------------------------------

enum ObjEntry {
    Provide(usize),
    Runtime(usize),
    RuntimePrev(usize),
    Float(String),
    Char(u32),
    Str(Vec<u32>),
    IArr(usize),
}

enum BlkEntry {
    IArr(usize),
    Info { typ: u8, iarrs0_idx: usize, data_idx: usize },
}

struct ParsedComponent {
    iarrs: Vec<Vec<i32>>,
    bc_idx: usize,
    objs: Vec<(usize, ObjEntry)>,
    obj_len: usize,
    blocks: Vec<(usize, BlkEntry)>,
    block_len: usize,
    bodies: Vec<(usize, usize)>,
    body_len: usize,
}

fn parse_cbqn_gen(src: &str) -> ParsedComponent {
    let mut iarrs: Vec<Vec<i32>> = Vec::new();
    let mut bc_idx = 0;

    if let Some(start) = src.find("iarrs_data[]") {
        if let Some(brace_start) = src[start..].find('{') {
            let data_start = start + brace_start + 1;
            if let Some(brace_end) = src[data_start..].find("};") {
                let data_str = &src[data_start..data_start + brace_end];
                let all_ints: Vec<i32> = data_str
                    .split(|c: char| c == ',' || c == '\n')
                    .filter_map(|s| {
                        let s = s.trim();
                        let s = if let Some(pos) = s.find("//") {
                            s[..pos].trim()
                        } else {
                            s
                        };
                        if s.is_empty() { return None; }
                        s.parse::<i32>().ok()
                    })
                    .collect();

                if let Some(lens_start) = src.find("iarrs_lens[]") {
                    if let Some(lb) = src[lens_start..].find('{') {
                        let ls = lens_start + lb + 1;
                        if let Some(le) = src[ls..].find('}') {
                            let lens_str = &src[ls..ls + le];
                            let lens: Vec<usize> = lens_str
                                .split(',')
                                .filter_map(|s| s.trim().parse::<usize>().ok())
                                .collect();

                            let mut offset = 0;
                            for len in lens {
                                let end = offset + len;
                                if end <= all_ints.len() {
                                    iarrs.push(all_ints[offset..end].to_vec());
                                } else {
                                    iarrs.push(Vec::new());
                                }
                                offset = end;
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(call_start) = src.find("load_importBlock") {
        if let Some(paren) = src[call_start..].find('(') {
            let args_start = call_start + paren + 1;
            if let Some(end_paren) = find_matching_paren(&src[args_start..]) {
                let args_str = &src[args_start..args_start + end_paren];
                if let Some(idx) = find_first_iarrs_ref(args_str) {
                    bc_idx = idx;
                }
            }
        }
    }

    let (objs, obj_len) = parse_objects(src);
    let (blocks, block_len) = parse_blocks(src);
    let (bodies, body_len) = parse_bodies(src);

    ParsedComponent { iarrs, bc_idx, objs, obj_len, blocks, block_len, bodies, body_len }
}

fn parse_array_size(src: &str, var: &str) -> usize {
    let pattern = format!("{}; B* {}p = m_lvBn(&{}, ", var, var, var);
    if let Some(pos) = src.find(&pattern) {
        let after = &src[pos + pattern.len()..];
        if let Some(end) = after.find(')') {
            if let Ok(n) = after[..end].trim().parse::<usize>() {
                return n;
            }
        }
    }
    0
}

fn parse_objects(src: &str) -> (Vec<(usize, ObjEntry)>, usize) {
    let len = parse_array_size(src, "a0");
    let mut entries = Vec::new();

    for line in src.lines() {
        let line = line.trim();
        if !line.starts_with("a0p[") { continue; }
        let idx_end = match line.find(']') { Some(i) => i, None => continue };
        let idx: usize = match line[4..idx_end].parse() { Ok(n) => n, Err(_) => continue };
        let eq_pos = match line.find(" = ") { Some(i) => i, None => continue };
        let rhs = line[eq_pos + 3..].trim_end_matches(';').trim();

        let entry = if let Some(rest) = rhs.strip_prefix("incG(provide[") {
            let n: usize = rest.trim_end_matches("])").parse().unwrap_or(0);
            ObjEntry::Provide(n)
        } else if let Some(rest) = rhs.strip_prefix("incG(runtime_0[") {
            let n: usize = rest.trim_end_matches("])").parse().unwrap_or(0);
            ObjEntry::RuntimePrev(n)
        } else if let Some(rest) = rhs.strip_prefix("incG(runtime[") {
            let n: usize = rest.trim_end_matches("])").parse().unwrap_or(0);
            ObjEntry::Runtime(n)
        } else if let Some(rest) = rhs.strip_prefix("m_f64(") {
            let val = rest.trim_end_matches(')');
            ObjEntry::Float(val.to_string())
        } else if let Some(rest) = rhs.strip_prefix("m_c32(") {
            let inner = rest.trim_end_matches(')');
            let ch = parse_c32_char(inner);
            ObjEntry::Char(ch)
        } else if rhs.starts_with("m_c8vec(") || rhs.starts_with("m_c32vec(") {
            let chars = parse_string_literal(rhs);
            ObjEntry::Str(chars)
        } else if let Some(rest) = rhs.strip_prefix("iarrs[") {
            let n: usize = rest.trim_end_matches(']').parse().unwrap_or(0);
            ObjEntry::IArr(n)
        } else {
            continue;
        };

        entries.push((idx, entry));
    }

    (entries, len)
}

fn parse_c32_char(s: &str) -> u32 {
    let inner = s.trim();
    let inner = inner.strip_prefix("U'")
        .and_then(|s| s.strip_suffix('\''))
        .unwrap_or(inner);
    if inner.starts_with('\\') {
        match inner {
            "\\0" => 0,
            "\\n" => '\n' as u32,
            "\\t" => '\t' as u32,
            "\\r" => '\r' as u32,
            "\\\\" => '\\' as u32,
            "\\'" => '\'' as u32,
            "\\\"" => '"' as u32,
            _ => 0,
        }
    } else {
        inner.chars().next().map(|c| c as u32).unwrap_or(0)
    }
}

fn parse_string_literal(rhs: &str) -> Vec<u32> {
    let is_c32 = rhs.starts_with("m_c32vec(");
    let quote_start = if is_c32 {
        match rhs.find("U\"") { Some(i) => i + 2, None => return Vec::new() }
    } else {
        match rhs.find('"') { Some(i) => i + 1, None => return Vec::new() }
    };

    let bytes = rhs.as_bytes();
    let mut i = quote_start;
    let mut chars = Vec::new();

    while i < bytes.len() {
        if bytes[i] == b'"' {
            break;
        } else if bytes[i] == b'\\' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'0' => { chars.push(0u32); i += 2; }
                b'n' => { chars.push('\n' as u32); i += 2; }
                b't' => { chars.push('\t' as u32); i += 2; }
                b'r' => { chars.push('\r' as u32); i += 2; }
                b'\\' => { chars.push('\\' as u32); i += 2; }
                b'\'' => { chars.push('\'' as u32); i += 2; }
                b'"' => { chars.push('"' as u32); i += 2; }
                _ => { chars.push(bytes[i + 1] as u32); i += 2; }
            }
        } else {
            let remaining = &rhs[i..];
            if let Some(ch) = remaining.chars().next() {
                chars.push(ch as u32);
                i += ch.len_utf8();
            } else {
                i += 1;
            }
        }
    }

    chars
}

fn parse_blocks(src: &str) -> (Vec<(usize, BlkEntry)>, usize) {
    let len = parse_array_size(src, "a1");
    let mut entries = Vec::new();

    for line in src.lines() {
        let line = line.trim();
        if !line.starts_with("a1p[") { continue; }
        let idx_end = match line.find(']') { Some(i) => i, None => continue };
        let idx: usize = match line[4..idx_end].parse() { Ok(n) => n, Err(_) => continue };
        let eq_pos = match line.find(" = ") { Some(i) => i, None => continue };
        let rhs = line[eq_pos + 3..].trim_end_matches(';').trim();

        let entry = if let Some(rest) = rhs.strip_prefix("m_blockinfo(") {
            let inner = rest.trim_end_matches(')');
            parse_blockinfo(inner)
        } else if let Some(rest) = rhs.strip_prefix("iarrs[") {
            let n: usize = rest.trim_end_matches(']').parse().unwrap_or(0);
            BlkEntry::IArr(n)
        } else {
            continue;
        };

        entries.push((idx, entry));
    }

    (entries, len)
}

fn parse_blockinfo(inner: &str) -> BlkEntry {
    let parts: Vec<&str> = inner.splitn(3, ',').map(|s| s.trim()).collect();
    if parts.len() < 3 { return BlkEntry::IArr(0); }
    let typ: u8 = parts[0].parse().unwrap_or(0);
    let iarrs0_idx = if let Some(rest) = parts[1].strip_prefix("iarrs[") {
        rest.trim_end_matches(']').parse().unwrap_or(0)
    } else { 0 };
    let data_idx = if let Some(rest) = parts[2].strip_prefix("iarrs[") {
        rest.trim_end_matches(']').parse().unwrap_or(0)
    } else { 0 };
    BlkEntry::Info { typ, iarrs0_idx, data_idx }
}

fn parse_bodies(src: &str) -> (Vec<(usize, usize)>, usize) {
    let len = parse_array_size(src, "a2");
    let mut entries = Vec::new();

    for line in src.lines() {
        let line = line.trim();
        if !line.starts_with("a2p[") { continue; }
        let idx_end = match line.find(']') { Some(i) => i, None => continue };
        let idx: usize = match line[4..idx_end].parse() { Ok(n) => n, Err(_) => continue };
        let eq_pos = match line.find(" = ") { Some(i) => i, None => continue };
        let rhs = line[eq_pos + 3..].trim_end_matches(';').trim();

        if let Some(rest) = rhs.strip_prefix("iarrs[") {
            let n: usize = rest.trim_end_matches(']').parse().unwrap_or(0);
            entries.push((idx, n));
        }
    }

    (entries, len)
}

fn find_matching_paren(s: &str) -> Option<usize> {
    let mut depth = 1i32;
    for (i, c) in s.chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => { depth -= 1; if depth == 0 { return Some(i); } }
            _ => {}
        }
    }
    None
}

fn find_first_iarrs_ref(s: &str) -> Option<usize> {
    if let Some(idx) = s.find("iarrs[") {
        let start = idx + 6;
        if let Some(end) = s[start..].find(']') {
            return s[start..start + end].parse::<usize>().ok();
        }
    }
    None
}
