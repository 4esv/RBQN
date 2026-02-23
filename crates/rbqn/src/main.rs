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
    // Suppress default panic output — BQN errors use panic-based throw()
    // and we catch them with catch_unwind for clean error messages.
    std::panic::set_hook(Box::new(|_| {}));

    let args = cli::parse_args();

    let rt = match bootstrap::bootstrap() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("rbqn: bootstrap failed: {e}");
            std::process::exit(1);
        }
    };

    // NOTE: Register the global runtime state for •BQN re-evaluation after bootstrap.
    rbqn_vm::derive::set_sys_runtime(rt.compiler, rt.runtime.clone(), rt.formatter);

    // Set •args to empty for -e/-p mode (file args will override when executing a file)
    rbqn_vm::derive::set_sys_args(&[]);
    // Set •path and •name to empty for -e/-p mode
    rbqn_vm::derive::set_sys_path("");

    let _ = args.heap_max; // TODO: enforce heap limit

    // Execute pre-REPL arguments
    for action in &args.actions {
        let result = match action {
            cli::Action::Eval(code) => exec_string(&rt, code).map(|_| ()),
            cli::Action::Print(code) => exec_string(&rt, code).map(|r| {
                println!("{}", format_result(&rt, &r));
            }),
            cli::Action::Output(code) => exec_string(&rt, code).and_then(|r| {
                output_raw(&r)
            }),
            cli::Action::File(path, file_args) => {
                if path == "-" {
                    // stdin: set path/name to empty
                    rbqn_vm::derive::set_sys_path("");
                    rbqn_vm::derive::set_sys_args(file_args);
                    let code = std::io::read_to_string(std::io::stdin()).unwrap_or_default();
                    exec_string(&rt, &code).map(|_| ())
                } else {
                    // Set •path to absolute path, •name to basename, •args to file args
                    let abs_path = std::fs::canonicalize(path)
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| path.to_string());
                    rbqn_vm::derive::set_sys_path(&abs_path);
                    rbqn_vm::derive::set_sys_args(file_args);
                    match std::fs::read_to_string(path) {
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

    // REPL if requested or no actions given
    if args.repl {
        repl::run_repl(&rt, args.silent);
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

    // Wrap in catch_unwind: BQN errors use panic-based throw()
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        exec_string_inner(rt, code)
    })) {
        Ok(result) => result,
        Err(panic) => {
            let msg = if let Some(s) = panic.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "unknown error".into()
            };
            // Strip "Domain error: " prefix since BqnError::Domain adds it
            let msg = msg.strip_prefix("Domain error: ")
                .or_else(|| msg.strip_prefix("Not yet implemented: "))
                .unwrap_or(&msg);
            Err(BqnError::Domain(msg.to_string()))
        }
    }
}

fn exec_string_inner(
    rt: &bootstrap::Runtime,
    code: &str,
) -> rbqn_core::Result<B> {
    // Build compiler arguments: ⟨runtime, •BQN_SYS, varNames, varDepths⟩
    let rt_arr = tag_arr(BqnArr::from_b_vec(rt.runtime.clone()));

    // Verify runtime array integrity
    if std::env::var("RBQN_COMP_TRACE").is_ok() {
        if let Some(rta) = get_arr(rt_arr) {
            eprintln!("[COMP] runtime array: {} elems, data_kind={}", rta.ia(), match &rta.data {
                rbqn_core::array::ArrData::Boxed(_) => "Boxed",
                rbqn_core::array::ArrData::F64(_) => "F64",
                _ => "other",
            });
            for i in [0, 1, 2, 3, 37] {
                let b = rta.get(i).unwrap();
                if b.is_fun() {
                    let fid = (b.0 & 0xFFFFFFFFFFFF) >> 3;
                    let fd = rbqn_vm::derive::get_derived(fid);
                    eprintln!("[COMP]   rt[{}] = fun({:?}) raw={:#018x}", i, fd.kind, b.0);
                } else {
                    eprintln!("[COMP]   rt[{}] = tag={:#06x} raw={:#018x}", i, (b.0 >> 48) as u16, b.0);
                }
            }
        }
    }

    let sys_fn = rbqn_vm::derive::m_sys_fn(100);
    let var_names = tag_arr(BqnArr::empty_harr());
    let var_depths = tag_arr(BqnArr::new_vec_i32(vec![]));
    let comp_args = tag_arr(BqnArr::from_b_vec(vec![rt_arr, sys_fn, var_names, var_depths]));

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

    let bc_arr = get_arr(bc_b)
        .ok_or_else(|| BqnError::Domain("compiler bc is not an array".into()))?;
    let bc: Vec<i32> = bc_arr.i32_iter().map_err(|e| BqnError::Domain(e.to_string()))?;

    // Compiler trace: gated behind RBQN_COMP_TRACE=1
    if std::env::var("RBQN_COMP_TRACE").is_ok() {
        eprintln!("[COMP] bc={:?}", bc);

        // Format objects
        if let Some(objs_arr) = get_arr(objs_b) {
            let n = objs_arr.ia();
            let mut descs = Vec::with_capacity(n);
            for i in 0..n {
                let b = objs_arr.get(i).unwrap_or(B::SENTINEL);
                let desc = if b.is_f64() {
                    format!("f64({})", b.o2f())
                } else if b.is_c32() {
                    let ch = char::from_u32(b.0 as u32).unwrap_or('?');
                    format!("c32('{}')", ch)
                } else if b.is_fun() {
                    let fid = (b.0 & 0xFFFFFFFFFFFF) >> 3;
                    let fd = rbqn_vm::derive::get_derived(fid);
                    format!("fun({:?})", fd.kind)
                } else if b.is_md1() {
                    "md1".to_string()
                } else if b.is_md2() {
                    "md2".to_string()
                } else if b.is_arr() {
                    if let Some(a) = get_arr(b) {
                        format!("arr(len={})", a.ia())
                    } else {
                        "arr(?)".to_string()
                    }
                } else if b.q_n() {
                    "nothing".to_string()
                } else {
                    format!("{:#018x}", b.0)
                };
                descs.push(desc);
            }
            eprintln!("[COMP] objs: {} items [{}]", n, descs.join(", "));
        } else {
            eprintln!("[COMP] objs: not an array");
        }

        // Format blocks
        if let Some(blocks_arr) = get_arr(blocks_b) {
            eprintln!("[COMP] blocks: {} items", blocks_arr.ia());
            for i in 0..blocks_arr.ia() {
                if let Ok(blk) = blocks_arr.get(i) {
                    if let Some(ba) = get_arr(blk) {
                        let elems: Vec<String> = (0..ba.ia())
                            .map(|j| {
                                let e = ba.get(j).unwrap_or(B::SENTINEL);
                                if e.is_f64() { format!("{}", e.o2f() as i32) }
                                else { format!("{:#018x}", e.0) }
                            })
                            .collect();
                        eprintln!("[COMP]   block[{}]: [{}]", i, elems.join(", "));
                    }
                }
            }
        } else {
            eprintln!("[COMP] blocks: not an array");
        }

        // Format bodies
        if let Some(bodies_arr) = get_arr(bodies_b) {
            eprintln!("[COMP] bodies: {} items", bodies_arr.ia());
            for i in 0..bodies_arr.ia() {
                if let Ok(bod) = bodies_arr.get(i) {
                    if let Some(ba) = get_arr(bod) {
                        let elems: Vec<String> = (0..ba.ia())
                            .map(|j| {
                                let e = ba.get(j).unwrap_or(B::SENTINEL);
                                if e.is_f64() { format!("{}", e.o2f() as i32) }
                                else if e.is_arr() {
                                    if let Some(inner) = get_arr(e) {
                                        let vals: Vec<String> = (0..inner.ia())
                                            .map(|k| {
                                                let v = inner.get(k).unwrap_or(B::SENTINEL);
                                                if v.is_f64() { format!("{}", v.o2f() as i32) }
                                                else { format!("{:#018x}", v.0) }
                                            })
                                            .collect();
                                        format!("[{}]", vals.join(","))
                                    } else { "arr(?)".to_string() }
                                }
                                else { format!("{:#018x}", e.0) }
                            })
                            .collect();
                        eprintln!("[COMP]   body[{}]: [{}]", i, elems.join(", "));
                    }
                }
            }
        } else {
            eprintln!("[COMP] bodies: not an array");
        }
    }

    let objs: Vec<B> = if let Some(objs_arr) = get_arr(objs_b) {
        (0..objs_arr.ia()).map(|i| objs_arr.get(i).unwrap_or(B::SENTINEL)).collect()
    } else {
        vec![]
    };

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
        B::SENTINEL,
        None,
        0,
    );

    let body = block.bodies[0].clone();
    let var_am = body.var_am;
    let root_scope = Arc::new(Scope::new(body.clone(), None, var_am, &[]));
    Ok(eval_fun_block(block, root_scope))
}

/// Raw output for -o: write characters directly, error on non-characters
fn output_raw(val: &B) -> rbqn_core::Result<()> {
    use std::io::Write;
    if val.is_arr() {
        if let Some(arr) = get_arr(*val) {
            let mut out = std::io::stdout().lock();
            for i in 0..arr.ia() {
                let b = arr.get(i).map_err(|e| BqnError::Domain(e.to_string()))?;
                if b.is_c32() {
                    let c = b.o2c().map_err(|_| BqnError::Domain("Trying to output non-character".into()))?;
                    if let Some(ch) = char::from_u32(c) {
                        let mut buf = [0u8; 4];
                        let s = ch.encode_utf8(&mut buf);
                        out.write_all(s.as_bytes()).map_err(|e| BqnError::Domain(e.to_string()))?;
                    }
                } else {
                    return Err(BqnError::Domain("Trying to output non-character".into()));
                }
            }
            out.flush().map_err(|e| BqnError::Domain(e.to_string()))?;
            Ok(())
        } else {
            Err(BqnError::Domain("Trying to output non-character".into()))
        }
    } else {
        Err(BqnError::Domain("Trying to output non-character".into()))
    }
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
