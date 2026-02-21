mod bootstrap;
mod cli;
mod embedded;
mod repl;

use rbqn_core::array::BqnArr;
use rbqn_core::error::BqnError;
use rbqn_core::B;
use rbqn_vm::compiler::compile_all;
use rbqn_vm::block::eval_fun_block;
use rbqn_vm::derive::{c1, c2};
use rbqn_vm::scope::Scope;
use rbqn_vm::vm::{get_arr, tag_arr};

use std::sync::Arc;

fn main() {
    let args = cli::parse_args();

    if let cli::Mode::Help = args.mode {
        cli::print_help();
        return;
    }

    let rt = match bootstrap::bootstrap() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("rbqn: bootstrap failed: {e}");
            std::process::exit(1);
        }
    };

    let _ = args.heap_max; // TODO: enforce heap limit

    let result = match args.mode {
        cli::Mode::Help => unreachable!(),
        cli::Mode::Repl => {
            repl::run_repl(&rt);
            Ok(())
        }
        cli::Mode::Eval(code) => {
            exec_string(&rt, &code).map(|r| {
                // NOTE: BQN -e prints the last expression result for REPL-like behavior
                println!("{}", format_result(&rt, &r));
            })
        }
        cli::Mode::Print(code) => {
            exec_string(&rt, &code).map(|r| {
                println!("{}", format_result(&rt, &r));
            })
        }
        cli::Mode::Output(code) => {
            exec_string(&rt, &code).map(|r| {
                println!("{}", format_result(&rt, &r));
            })
        }
        cli::Mode::File(path, _file_args) => {
            if path == "-" {
                let code = std::io::read_to_string(std::io::stdin()).unwrap_or_default();
                exec_string(&rt, &code).map(|_| ())
            } else {
                match std::fs::read_to_string(&path) {
                    Ok(code) => exec_string(&rt, &code).map(|_| ()),
                    Err(e) => Err(BqnError::Domain(format!("cannot read {path}: {e}"))),
                }
            }
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn exec_string(
    rt: &bootstrap::Runtime,
    code: &str,
) -> rbqn_core::Result<B> {
    if rt.compiler.q_n() || rt.compiler.0 == B::SENTINEL.0 {
        return Err(BqnError::Nyi(
            "compiler not available (bootstrap failed?)".into(),
        ));
    }

    // Build compiler arguments: ⟨runtime, •BQN_SYS, varNames, varDepths⟩
    // For top-level execution: no outer scope vars
    let rt_arr = tag_arr(BqnArr::from_b_vec(rt.runtime.clone()));
    let sys_fn = rbqn_vm::derive::m_sys_fn(100); // placeholder for •BQN system
    let var_names = tag_arr(BqnArr::empty_harr());
    let var_depths = tag_arr(BqnArr::new_vec_i32(vec![]));
    let comp_args = tag_arr(BqnArr::from_b_vec(vec![rt_arr, sys_fn, var_names, var_depths]));

    // Encode source as char array
    let src_chars: Vec<u32> = code.chars().map(|c| c as u32).collect();
    let src_b = tag_arr(BqnArr::new_vec_c32(src_chars));

    // Call compiler: compiler(args, source) → ⟨bc, objs, blocks, bodies, indices?, tokenInfo?⟩
    let comp_result = c2(rt.compiler, comp_args, src_b);

    let comp_arr = get_arr(comp_result)
        .ok_or_else(|| BqnError::Domain("compiler did not return an array".into()))?;

    let bc_b = comp_arr.get(0).map_err(|e| BqnError::Domain(e.to_string()))?;
    let objs_b = comp_arr.get(1).map_err(|e| BqnError::Domain(e.to_string()))?;
    let blocks_b = comp_arr.get(2).map_err(|e| BqnError::Domain(e.to_string()))?;
    let bodies_b = comp_arr.get(3).map_err(|e| BqnError::Domain(e.to_string()))?;
    let indices_b = comp_arr.get(4).unwrap_or(B::SENTINEL);
    let token_info_b = comp_arr.get(5).unwrap_or(B::SENTINEL);

    // Extract bytecode as i32 slice
    let bc_arr = get_arr(bc_b)
        .ok_or_else(|| BqnError::Domain("compiler bc is not an array".into()))?;
    let bc: Vec<i32> = bc_arr.i32_iter().map_err(|e| BqnError::Domain(e.to_string()))?;

    // Extract objects as Vec<B>
    let objs: Vec<B> = if let Some(objs_arr) = get_arr(objs_b) {
        (0..objs_arr.ia()).map(|i| objs_arr.get(i).unwrap_or(B::SENTINEL)).collect()
    } else {
        vec![]
    };

    // Blocks and bodies stay as B arrays for compile_all
    let blocks: Vec<B> = if let Some(blocks_arr) = get_arr(blocks_b) {
        (0..blocks_arr.ia()).map(|i| blocks_arr.get(i).unwrap_or(B::SENTINEL)).collect()
    } else {
        vec![]
    };

    let bodies: Vec<B> = if let Some(bodies_arr) = get_arr(bodies_b) {
        (0..bodies_arr.ia()).map(|i| bodies_arr.get(i).unwrap_or(B::SENTINEL)).collect()
    } else {
        vec![]
    };

    // Compile the user code
    let src_chars2: Vec<u32> = code.chars().map(|c| c as u32).collect();
    let src_b2 = tag_arr(BqnArr::new_vec_c32(src_chars2));
    let block = compile_all(
        &bc,
        objs,
        &blocks,
        &bodies,
        indices_b,
        token_info_b,
        src_b2,
        B::SENTINEL, // fullpath
        None,         // sc
        0,            // ns_result
    );

    // After compile_block swap, bodies[0] is the first monadic body
    let body = block.bodies[0].clone();
    let var_am = body.var_am;
    let root_scope = Arc::new(Scope::new(body.clone(), None, var_am, &[]));
    let result = eval_fun_block(block, root_scope);

    Ok(result)
}

fn format_result(rt: &bootstrap::Runtime, val: &B) -> String {
    // Try using the formatter if available
    if let Some((ref fmt_fn, _)) = rt.formatter {
        if let Ok(result) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            c1(*fmt_fn, *val)
        })) {
            if let Some(arr) = get_arr(result) {
                // Formatter returns a string (char array)
                let chars: String = (0..arr.ia())
                    .filter_map(|i| arr.get(i).ok())
                    .filter_map(|b| {
                        if b.is_c32() {
                            b.o2c().ok().and_then(char::from_u32)
                        } else {
                            None
                        }
                    })
                    .collect();
                if !chars.is_empty() {
                    return chars;
                }
            }
        }
    }

    // Fallback: basic formatting
    format_b(*val)
}

/// Basic B value formatter for when the self-hosted formatter isn't available.
fn format_b(val: B) -> String {
    if val.is_f64() {
        format_number(val.o2f())
    } else if val.is_c32() {
        let c = val.o2c().ok().and_then(char::from_u32).unwrap_or('?');
        // BQN displays characters as @+codepoint or just the char for printable ASCII
        format!("'{}'", c)
    } else if val.is_arr() {
        if let Some(arr) = get_arr(val) {
            format_arr(&arr)
        } else {
            "⟨?⟩".to_string()
        }
    } else if val.q_n() {
        "·".to_string()
    } else if val.is_fun() || val.is_md1() || val.is_md2() {
        "(function)".to_string()
    } else {
        format!("(B:{:#x})", val.0)
    }
}

/// Format a number using BQN conventions: ¯ prefix for negatives, no trailing zeros.
fn format_number(f: f64) -> String {
    if f.is_nan() {
        return "NaN".to_string();
    }
    if f.is_infinite() {
        return if f > 0.0 { "∞".to_string() } else { "¯∞".to_string() };
    }
    let neg = f < 0.0;
    let abs = f.abs();
    let formatted = if abs == abs.floor() && abs < 1e15 {
        format!("{}", abs as i64)
    } else {
        // Use Rust's default float formatting, then trim trailing zeros after decimal
        let s = format!("{}", abs);
        s
    };
    if neg {
        format!("¯{}", formatted)
    } else {
        formatted
    }
}

fn format_arr(arr: &BqnArr) -> String {
    // Check if it's a string (rank 1, all characters)
    if arr.rank() == 1 {
        if let rbqn_core::array::ArrData::C8(ref data) = arr.data {
            let s: String = data.iter().map(|&c| c as char).collect();
            return format!("\"{}\"", s);
        }
        if let rbqn_core::array::ArrData::C16(ref data) = arr.data {
            let s: String = data.iter().filter_map(|&c| char::from_u32(c as u32)).collect();
            return format!("\"{}\"", s);
        }
        if let rbqn_core::array::ArrData::C32(ref data) = arr.data {
            let s: String = data.iter().filter_map(|&c| char::from_u32(c)).collect();
            return format!("\"{}\"", s);
        }
    }

    // List display
    if arr.rank() <= 1 {
        let elems: Vec<String> = (0..arr.ia())
            .map(|i| {
                match arr.get(i) {
                    Ok(b) => format_b(b),
                    Err(_) => "?".to_string(),
                }
            })
            .collect();
        if arr.ia() == 0 {
            "⟨⟩".to_string()
        } else {
            // NOTE: BQN display uses space-separated elements for lists, not ‿ (strand)
            format!("⟨ {} ⟩", elems.join(" "))
        }
    } else {
        // Multi-rank: show shape
        let shape_str: Vec<String> = arr.shape.iter().map(|s| s.to_string()).collect();
        format!("[{}×…]", shape_str.join("‿"))
    }
}
