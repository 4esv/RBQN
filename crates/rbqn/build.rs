use std::env;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);

    let cbqn_path = find_cbqn_path();
    let bc_dir = cbqn_path.as_ref().and_then(|p| find_gen_dir(p));

    let mut code = String::new();

    if let Some(ref dir) = bc_dir {
        println!("cargo:warning=Using CBQN bytecode from {}", dir.display());
        write_bytecode_from_gen(&mut code, dir);
    } else {
        if cbqn_path.is_some() {
            println!("cargo:warning=CBQN path found but no precompiled bytecode in build/*/gen/");
            println!("cargo:warning=Build CBQN first, or run `make -C <cbqn> bytecodeLocal`");
        } else {
            println!("cargo:warning=No CBQN source found. Set CBQN_PATH or place cbqn-ref alongside rbqn.");
        }
        println!("cargo:warning=Using empty placeholder bytecode. Bootstrap will not work.");
        write_empty_bytecode(&mut code);
    }

    fs::write(out_path.join("embedded_bytecode.rs"), code).unwrap();
}

fn find_cbqn_path() -> Option<PathBuf> {
    if let Ok(p) = env::var("CBQN_PATH") {
        let path = PathBuf::from(p);
        if path.is_dir() {
            return Some(path);
        }
    }
    let candidates = [
        "../cbqn-ref",
        "../cbqn",
        "../../cbqn-ref",
        "../../cbqn",
    ];
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let base = Path::new(&manifest_dir);
    for c in candidates {
        let p = base.join(c);
        if p.join("src/load.c").is_file() {
            return Some(p);
        }
    }
    None
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

fn write_bytecode_from_gen(code: &mut String, dir: &Path) {
    let components = [
        ("RUNTIME0", "runtime0"),
        ("RUNTIME1", "runtime1"),
        ("COMPILER", "compiles"),
        ("FORMATTER", "formatter"),
    ];

    for (name, file) in components {
        let path = dir.join(file);
        if path.is_file() {
            let src = fs::read_to_string(&path).unwrap_or_default();
            let parsed = parse_cbqn_gen(&src);
            write_component(code, name, &parsed);
        } else {
            println!("cargo:warning=Missing bytecode file: {}", path.display());
            write_empty_component(code, name);
        }
    }
}

struct ParsedComponent {
    iarrs: Vec<Vec<i32>>,
    bc_idx: usize,
}

fn parse_cbqn_gen(src: &str) -> ParsedComponent {
    let mut iarrs: Vec<Vec<i32>> = Vec::new();
    let mut bc_idx = 0;

    // Extract all integer arrays from iarrs_data[]
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
                        if s.is_empty() {
                            return None;
                        }
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

    // The bytecode is the first direct iarrs[] reference in load_importBlock call.
    // Format: load_importBlock("name",\n  iarrs[N],\n  a0,\n  a1,\n  a2\n);
    if let Some(call_start) = src.find("load_importBlock") {
        if let Some(paren) = src[call_start..].find('(') {
            let args_start = call_start + paren + 1;
            if let Some(end_paren) = find_matching_paren(&src[args_start..]) {
                let args_str = &src[args_start..args_start + end_paren];
                // Find first iarrs[N] reference — that's the bytecode
                if let Some(idx) = find_first_iarrs_ref(args_str) {
                    bc_idx = idx;
                }
            }
        }
    }

    ParsedComponent { iarrs, bc_idx }
}

fn find_matching_paren(s: &str) -> Option<usize> {
    let mut depth = 1i32;
    for (i, c) in s.chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
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

fn write_component(code: &mut String, name: &str, parsed: &ParsedComponent) {
    let n = parsed.iarrs.len();

    // Write all integer arrays
    for (i, arr) in parsed.iarrs.iter().enumerate() {
        let _ = write!(code, "static {name}_IARR_{i}: &[i32] = &[");
        for (j, v) in arr.iter().enumerate() {
            if j > 0 {
                code.push_str(", ");
            }
            let _ = write!(code, "{v}");
        }
        code.push_str("];\n");
    }

    // Write the collected iarrs table
    let _ = write!(code, "static {name}_ALL_IARRS: &[&[i32]] = &[");
    for i in 0..n {
        if i > 0 {
            code.push_str(", ");
        }
        let _ = write!(code, "{name}_IARR_{i}");
    }
    code.push_str("];\n");

    // Write bc reference
    let bc_idx = parsed.bc_idx;
    let _ = writeln!(
        code,
        "pub static {name}: crate::embedded::EmbeddedBytecode = crate::embedded::EmbeddedBytecode {{\
         bc: {name}_IARR_{bc_idx}, objs: {name}_ALL_IARRS, blocks: &[], bodies: &[] }};"
    );
    code.push('\n');
}

fn write_empty_component(code: &mut String, name: &str) {
    let _ = writeln!(
        code,
        "pub static {name}: crate::embedded::EmbeddedBytecode = crate::embedded::EmbeddedBytecode {{\
         bc: &[], objs: &[], blocks: &[], bodies: &[] }};"
    );
}

fn write_empty_bytecode(code: &mut String) {
    for name in ["RUNTIME0", "RUNTIME1", "COMPILER", "FORMATTER"] {
        write_empty_component(code, name);
    }
}
