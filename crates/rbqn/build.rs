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

// Parsed representation of object array entries
enum ObjEntry {
    Provide(usize),
    Runtime(usize),
    RuntimePrev(usize),
    Float(String), // keep as string to preserve exact representation (e.g. "1.0/0.0")
    Char(u32),
    Str(Vec<u32>),
    IArr(usize),
}

// Parsed representation of block array entries
enum BlkEntry {
    IArr(usize),
    Info { typ: u8, iarrs0_idx: usize, data_idx: usize },
}

struct ParsedComponent {
    iarrs: Vec<Vec<i32>>,
    bc_idx: usize,
    objs: Vec<(usize, ObjEntry)>,   // (index, entry) — sparse
    obj_len: usize,
    blocks: Vec<(usize, BlkEntry)>,  // (index, entry) — sparse
    block_len: usize,
    bodies: Vec<(usize, usize)>,     // (index, iarrs_idx) — sparse
    body_len: usize,
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

    // Parse objects (a0), blocks (a1), bodies (a2)
    let (objs, obj_len) = parse_objects(src);
    let (blocks, block_len) = parse_blocks(src);
    let (bodies, body_len) = parse_bodies(src);

    ParsedComponent { iarrs, bc_idx, objs, obj_len, blocks, block_len, bodies, body_len }
}

/// Extract the array size from `m_lvBn(&aX, SIZE)`
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
        if !line.starts_with("a0p[") {
            continue;
        }
        // Extract index: a0p[N] = ...
        let idx_end = match line.find(']') {
            Some(i) => i,
            None => continue,
        };
        let idx: usize = match line[4..idx_end].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        // Get the RHS after " = "
        let eq_pos = match line.find(" = ") {
            Some(i) => i,
            None => continue,
        };
        let rhs = line[eq_pos + 3..].trim_end_matches(';').trim();

        let entry = if let Some(rest) = rhs.strip_prefix("incG(provide[") {
            // incG(provide[N])
            let n: usize = rest.trim_end_matches("])").parse().unwrap_or(0);
            ObjEntry::Provide(n)
        } else if let Some(rest) = rhs.strip_prefix("incG(runtime_0[") {
            // incG(runtime_0[N])
            let n: usize = rest.trim_end_matches("])").parse().unwrap_or(0);
            ObjEntry::RuntimePrev(n)
        } else if let Some(rest) = rhs.strip_prefix("incG(runtime[") {
            // incG(runtime[N])
            let n: usize = rest.trim_end_matches("])").parse().unwrap_or(0);
            ObjEntry::Runtime(n)
        } else if let Some(rest) = rhs.strip_prefix("m_f64(") {
            // m_f64(VALUE)
            let val = rest.trim_end_matches(')');
            ObjEntry::Float(val.to_string())
        } else if let Some(rest) = rhs.strip_prefix("m_c32(") {
            // m_c32(U'X') or m_c32(U'\0')
            let inner = rest.trim_end_matches(')');
            let ch = parse_c32_char(inner);
            ObjEntry::Char(ch)
        } else if rhs.starts_with("m_c8vec(") || rhs.starts_with("m_c32vec(") {
            // m_c8vec("...",N) or m_c32vec(U"...",N)
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

/// Parse a C char literal from m_c32(U'X'), handling escape sequences
fn parse_c32_char(s: &str) -> u32 {
    // Format: U'X' or U'\0' or U'\n' etc.
    let inner = s.trim().trim_start_matches("U'").trim_end_matches('\'');
    if inner.starts_with('\\') {
        match inner {
            "\\0" => 0,
            "\\n" => '\n' as u32,
            "\\t" => '\t' as u32,
            "\\r" => '\r' as u32,
            "\\\\" => '\\' as u32,
            "\\'" => '\'' as u32,
            _ => 0,
        }
    } else {
        inner.chars().next().map(|c| c as u32).unwrap_or(0)
    }
}

/// Parse m_c8vec("...",N) or m_c32vec(U"...",N) into Vec<u32>
fn parse_string_literal(rhs: &str) -> Vec<u32> {
    // Find the string content between quotes
    let is_c32 = rhs.starts_with("m_c32vec(");

    // Find opening quote
    let quote_start = if is_c32 {
        // m_c32vec(U"...",N)  — find U" after (
        match rhs.find("U\"") {
            Some(i) => i + 2,
            None => return Vec::new(),
        }
    } else {
        // m_c8vec("...",N)  — find first " after (
        match rhs.find('"') {
            Some(i) => i + 1,
            None => return Vec::new(),
        }
    };

    // Find closing quote — scan from quote_start, handling escapes
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
            // Decode UTF-8 character
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
        if !line.starts_with("a1p[") {
            continue;
        }
        let idx_end = match line.find(']') {
            Some(i) => i,
            None => continue,
        };
        let idx: usize = match line[4..idx_end].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let eq_pos = match line.find(" = ") {
            Some(i) => i,
            None => continue,
        };
        let rhs = line[eq_pos + 3..].trim_end_matches(';').trim();

        let entry = if let Some(rest) = rhs.strip_prefix("m_blockinfo(") {
            // m_blockinfo(TYPE, iarrs0, iarrs[N])
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

/// Parse m_blockinfo(TYPE, iarrs0, iarrs[N])
fn parse_blockinfo(inner: &str) -> BlkEntry {
    // Split by commas, but be careful of nested brackets
    let parts: Vec<&str> = inner.splitn(3, ',').map(|s| s.trim()).collect();
    if parts.len() < 3 {
        return BlkEntry::IArr(0);
    }
    let typ: u8 = parts[0].parse().unwrap_or(0);
    // parts[1] is "iarrs0" — extract the iarrs0 index (always refers to iarrs[0])
    let iarrs0_idx = 0; // iarrs0 is always iarrs[0] per the gen files
    // parts[2] is "iarrs[N]"
    let data_idx = if let Some(rest) = parts[2].strip_prefix("iarrs[") {
        rest.trim_end_matches(']').parse().unwrap_or(0)
    } else {
        0
    };
    BlkEntry::Info { typ, iarrs0_idx, data_idx }
}

fn parse_bodies(src: &str) -> (Vec<(usize, usize)>, usize) {
    let len = parse_array_size(src, "a2");
    let mut entries = Vec::new();

    for line in src.lines() {
        let line = line.trim();
        if !line.starts_with("a2p[") {
            continue;
        }
        let idx_end = match line.find(']') {
            Some(i) => i,
            None => continue,
        };
        let idx: usize = match line[4..idx_end].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let eq_pos = match line.find(" = ") {
            Some(i) => i,
            None => continue,
        };
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

    // Write string data for object string entries
    let mut str_count = 0usize;
    for (_, entry) in &parsed.objs {
        if let ObjEntry::Str(chars) = entry {
            let _ = write!(code, "static {name}_STR_{str_count}: &[u32] = &[");
            for (j, ch) in chars.iter().enumerate() {
                if j > 0 {
                    code.push_str(", ");
                }
                let _ = write!(code, "{ch}");
            }
            code.push_str("];\n");
            str_count += 1;
        }
    }

    // Write objects array
    write_objects(code, name, parsed);

    // Write blocks array
    write_blocks(code, name, parsed);

    // Write bodies array
    write_bodies(code, name, parsed);

    // Write the EmbeddedBytecode struct
    let bc_idx = parsed.bc_idx;
    let _ = writeln!(
        code,
        "pub static {name}: crate::embedded::EmbeddedBytecode = crate::embedded::EmbeddedBytecode {{\
         bc: {name}_IARR_{bc_idx}, iarrs: {name}_ALL_IARRS, objs: &{name}_OBJS, blocks: &{name}_BLOCKS, bodies: &{name}_BODIES }};"
    );
    code.push('\n');
}

fn write_objects(code: &mut String, name: &str, parsed: &ParsedComponent) {
    use std::collections::HashMap;
    let obj_len = parsed.obj_len;

    // Build a map from index to entry
    let mut map: HashMap<usize, &ObjEntry> = HashMap::new();
    for (idx, entry) in &parsed.objs {
        map.insert(*idx, entry);
    }

    // Track which string we're on
    let mut str_idx = 0usize;
    // Pre-compute string indices for each entry
    let mut str_indices: HashMap<usize, usize> = HashMap::new();
    for (idx, entry) in &parsed.objs {
        if matches!(entry, ObjEntry::Str(_)) {
            str_indices.insert(*idx, str_idx);
            str_idx += 1;
        }
    }

    let _ = write!(code, "static {name}_OBJS: [crate::embedded::ObjectEntry; {obj_len}] = [\n");

    for i in 0..obj_len {
        if i > 0 {
            code.push_str(",\n");
        }
        if let Some(entry) = map.get(&i) {
            match entry {
                ObjEntry::Provide(n) => {
                    let _ = write!(code, "  crate::embedded::ObjectEntry::Provide({n})");
                }
                ObjEntry::Runtime(n) => {
                    let _ = write!(code, "  crate::embedded::ObjectEntry::Runtime({n})");
                }
                ObjEntry::RuntimePrev(n) => {
                    let _ = write!(code, "  crate::embedded::ObjectEntry::RuntimePrev({n})");
                }
                ObjEntry::Float(val) => {
                    let rust_val = c_float_to_rust(val);
                    let _ = write!(code, "  crate::embedded::ObjectEntry::Float({rust_val})");
                }
                ObjEntry::Char(ch) => {
                    let _ = write!(code, "  crate::embedded::ObjectEntry::Char({ch})");
                }
                ObjEntry::Str(_) => {
                    let si = str_indices[&i];
                    let _ = write!(code, "  crate::embedded::ObjectEntry::Str({name}_STR_{si})");
                }
                ObjEntry::IArr(n) => {
                    let _ = write!(code, "  crate::embedded::ObjectEntry::IArr({n})");
                }
            }
        } else {
            // Unset slot — default to Provide(0) as placeholder
            let _ = write!(code, "  crate::embedded::ObjectEntry::Float(0.0)");
        }
    }

    code.push_str("\n];\n");
}

fn write_blocks(code: &mut String, name: &str, parsed: &ParsedComponent) {
    use std::collections::HashMap;
    let block_len = parsed.block_len;

    let mut map: HashMap<usize, &BlkEntry> = HashMap::new();
    for (idx, entry) in &parsed.blocks {
        map.insert(*idx, entry);
    }

    let _ = write!(code, "static {name}_BLOCKS: [crate::embedded::BlockEntry; {block_len}] = [\n");

    for i in 0..block_len {
        if i > 0 {
            code.push_str(",\n");
        }
        if let Some(entry) = map.get(&i) {
            match entry {
                BlkEntry::IArr(n) => {
                    let _ = write!(code, "  crate::embedded::BlockEntry::IArr({n})");
                }
                BlkEntry::Info { typ, iarrs0_idx, data_idx } => {
                    let _ = write!(
                        code,
                        "  crate::embedded::BlockEntry::Info {{ typ: {typ}, iarrs0_idx: {iarrs0_idx}, data_idx: {data_idx} }}"
                    );
                }
            }
        } else {
            // Unset slot — placeholder
            let _ = write!(code, "  crate::embedded::BlockEntry::IArr(0)");
        }
    }

    code.push_str("\n];\n");
}

fn write_bodies(code: &mut String, name: &str, parsed: &ParsedComponent) {
    use std::collections::HashMap;
    let body_len = parsed.body_len;

    let mut map: HashMap<usize, usize> = HashMap::new();
    for (idx, iarrs_idx) in &parsed.bodies {
        map.insert(*idx, *iarrs_idx);
    }

    let _ = write!(code, "static {name}_BODIES: [usize; {body_len}] = [\n");

    for i in 0..body_len {
        if i > 0 {
            code.push_str(",\n");
        }
        let val = map.get(&i).copied().unwrap_or(0);
        let _ = write!(code, "  {val}");
    }

    code.push_str("\n];\n");
}

/// Convert C float expressions to Rust
fn c_float_to_rust(val: &str) -> String {
    match val {
        "1.0/0.0" => "f64::INFINITY".to_string(),
        "-1.0/0.0" => "f64::NEG_INFINITY".to_string(),
        _ => {
            // Ensure it has a decimal point or is a valid float literal
            let v = val.trim();
            if v.contains('.') || v.contains('e') || v.contains('E') {
                format!("{v}_f64")
            } else {
                format!("{v}.0_f64")
            }
        }
    }
}

fn write_empty_component(code: &mut String, name: &str) {
    // Empty arrays for all fields
    let _ = writeln!(code, "static {name}_OBJS: [crate::embedded::ObjectEntry; 0] = [];");
    let _ = writeln!(code, "static {name}_BLOCKS: [crate::embedded::BlockEntry; 0] = [];");
    let _ = writeln!(code, "static {name}_BODIES: [usize; 0] = [];");
    let _ = writeln!(
        code,
        "pub static {name}: crate::embedded::EmbeddedBytecode = crate::embedded::EmbeddedBytecode {{\
         bc: &[], iarrs: &[], objs: &{name}_OBJS, blocks: &{name}_BLOCKS, bodies: &{name}_BODIES }};"
    );
}

fn write_empty_bytecode(code: &mut String) {
    for name in ["RUNTIME0", "RUNTIME1", "COMPILER", "FORMATTER"] {
        write_empty_component(code, name);
    }
}
