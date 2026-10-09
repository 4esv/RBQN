mod cli;
mod gpu_runtime;
mod repl;

use rbqn::bootstrap;
use rbqn::exec::exec_string;
use rbqn_core::array::BqnArr;
use rbqn_core::error::BqnError;
use rbqn_core::B;
use rbqn_vm::compiler::compile_all;
use rbqn_vm::derive::{c1, c2};
use rbqn_vm::scope::Scope;
use rbqn_vm::vm::{get_arr, tag_arr};


/// Stack for the interpreter thread. Reserved virtual memory: pages are only
/// committed when touched, so a deep recursion costs memory, a shallow one not.
const INTERP_STACK_BYTES: usize = 512 << 20;

fn main() {
    let t0 = std::time::Instant::now();
    // NOTE: All interpreter work (bootstrap, evaluation, REPL, •Exit) runs on this
    // one thread: the value stores are thread-local, and the default 8 MB main
    // stack overflowed at a few thousand BQN calls. std::process::exit from the
    // interpreter thread ends the whole process as before.
    let handle = std::thread::Builder::new()
        .name("rbqn".into())
        .stack_size(INTERP_STACK_BYTES)
        .spawn(move || interp_main(t0))
        .unwrap_or_else(|e| {
            eprintln!("rbqn: could not start interpreter thread: {e}");
            std::process::exit(1);
        });
    if handle.join().is_err() {
        std::process::exit(1);
    }
}

fn interp_main(mut t: std::time::Instant) {
    rbqn::timing::lap(&mut t, "thread spawn");
    let _gpu_summary = gpu_runtime::SummaryGuard;
    // Suppress default panic output: BQN errors use panic-based throw()
    // and we catch them with catch_unwind for clean error messages.
    // GPU dispatch panics are still reported under RBQN_GPU_DEBUG=1.
    std::panic::set_hook(Box::new(gpu_runtime::report_panic));

    let args = cli::parse_args();

    gpu_runtime::init(args.no_gpu);

    // NOTE: Register GPU dispatch hooks into rbqn-prim after GPU runtime is initialized.
    // Function pointer pattern avoids circular dependency (rbqn-prim cannot depend on rbqn).
    rbqn_prim::arith_dyad::register_gpu_arith(gpu_runtime::gpu_arith_binary);

    // NOTE: Register GPU fused arithmetic hook for explicit multi-op dispatch.
    // FusionBuilder wired here; true expression-level auto-fusion is future work (requires
    // VM-level lookahead — the BQN evaluator calls c2 one call at a time with no lookahead).
    rbqn_prim::arith_dyad::register_gpu_fused(gpu_runtime::gpu_fused_arith);

    // NOTE: Register GPU matmul and softmax hooks into rbqn-vm::derive.
    // These enable •math.MatMul and •math.Softmax to dispatch to GPU when available.
    rbqn_vm::derive::register_gpu_matmul(gpu_runtime::gpu_matmul);
    rbqn_vm::derive::register_gpu_softmax(gpu_runtime::gpu_softmax);

    // NOTE: Register GPU fold/scan hooks into rbqn-vm modifiers.
    // These enable +´ and +` to dispatch to GPU for large arrays.
    rbqn_vm::modifiers::register_gpu_fold(gpu_runtime::gpu_fold);
    rbqn_vm::modifiers::register_gpu_scan(gpu_runtime::gpu_scan);

    // NOTE: Register GPU grade and sort hooks into rbqn-prim.
    // These enable ⍋/⍒ and ∧/∨ to dispatch to GPU for large arrays.
    rbqn_prim::sort::register_gpu_grade(gpu_runtime::gpu_grade);
    rbqn_prim::sort::register_gpu_sort(gpu_runtime::gpu_sort);

    rbqn::timing::lap(&mut t, "cli+gpu hooks");
    let rt = match bootstrap::bootstrap() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("rbqn: bootstrap failed: {e}");
            std::process::exit(1);
        }
    };

    rbqn::timing::lap(&mut t, "bootstrap (total)");
    // NOTE: Register the global runtime state for •BQN re-evaluation after bootstrap.
    rbqn_vm::derive::set_sys_runtime(rt.compiler, rt.runtime.clone(), rt.formatter);

    // NOTE: Register derived function structural equality with rbqn-core (for ≡ and = on functions).
    rbqn_vm::derive::register_derived_equality();
    rbqn::timing::lap(&mut t, "set_sys_runtime+eq");

    // Set •args to empty for -e/-p mode (file args will override when executing a file)
    rbqn_vm::derive::set_sys_args(&[]);
    rbqn::timing::lap(&mut t, "set_sys_args");
    // Set •path and •name to empty for -e/-p mode
    rbqn_vm::derive::set_sys_path("");

    let _ = args.heap_max; // TODO: enforce heap limit
    rbqn::timing::lap(&mut t, "post-bootstrap setup");

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
            gpu_runtime::print_summary();
            std::process::exit(1);
        }
        rbqn::timing::lap(&mut t, "action eval");
    }

    // REPL if requested or no actions given
    if args.repl {
        repl::run_repl(&rt, args.silent);
    }
}

/// REPL state: accumulated variable names and values across REPL lines.
pub(crate) struct ReplState {
    pub var_names: Vec<String>,
    pub var_values: Vec<B>,
}

impl ReplState {
    pub fn new() -> Self {
        ReplState {
            var_names: Vec::new(),
            var_values: Vec::new(),
        }
    }
}

/// Execute a REPL line with accumulated variable state.
/// Returns the result value and updates the ReplState with any new/modified variables.
pub(crate) fn exec_repl_line(
    rt: &bootstrap::Runtime,
    code: &str,
    state: &mut ReplState,
) -> rbqn_core::Result<B> {
    if rt.compiler.q_n() || rt.compiler.0 == B::SENTINEL.0 {
        return Err(BqnError::Nyi(
            "compiler not available (bootstrap failed?)".into(),
        ));
    }

    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        exec_repl_line_inner(rt, code, state)
    })) {
        Ok(result) => result,
        Err(panic) => Err(rbqn::exec::panic_to_bqn_error(panic)),
    }
}

/// Build a BQN varNames value (list of strings) from Rust strings.
fn build_var_names_b(names: &[String]) -> B {
    if names.is_empty() {
        return tag_arr(BqnArr::empty_harr());
    }
    let name_arrs: Vec<B> = names.iter().map(|name| {
        let chars: Vec<u32> = name.chars().map(|c| c as u32).collect();
        tag_arr(BqnArr::new_vec_c32(chars))
    }).collect();
    tag_arr(BqnArr::from_b_vec(name_arrs))
}

/// Extract the nameList from compiler tokenInfo: tokenInfo[2][0].
fn extract_name_list(token_info_b: B) -> Option<Vec<String>> {
    let ti_arr = get_arr(token_info_b)?;
    let ti2 = ti_arr.get(2).ok()?;
    let ti2_arr = get_arr(ti2)?;
    let name_list_b = ti2_arr.get(0).ok()?;
    let nl_arr = get_arr(name_list_b)?;

    let mut names = Vec::with_capacity(nl_arr.ia());
    for i in 0..nl_arr.ia() {
        let name_b = nl_arr.get(i).ok()?;
        if let Some(name_arr) = get_arr(name_b) {
            let s: String = (0..name_arr.ia())
                .filter_map(|j| {
                    name_arr.get(j).ok().and_then(|b| {
                        if b.is_c32() {
                            b.o2c().ok().and_then(char::from_u32)
                        } else {
                            None
                        }
                    })
                })
                .collect();
            names.push(s);
        } else {
            names.push(String::new());
        }
    }
    Some(names)
}

/// Extract varIDs from body[0]'s index 2.
fn extract_var_ids(bodies_b: B) -> Option<Vec<usize>> {
    let bodies_arr = get_arr(bodies_b)?;
    let body0 = bodies_arr.get(0).ok()?;
    let body0_arr = get_arr(body0)?;
    if body0_arr.ia() < 3 { return None; }
    let var_ids_b = body0_arr.get(2).ok()?;
    let var_ids_arr = get_arr(var_ids_b)?;
    let mut ids = Vec::with_capacity(var_ids_arr.ia());
    for i in 0..var_ids_arr.ia() {
        let v = var_ids_arr.get(i).ok()?;
        ids.push(v.o2f() as usize);
    }
    Some(ids)
}

fn exec_repl_line_inner(
    rt: &bootstrap::Runtime,
    code: &str,
    state: &mut ReplState,
) -> rbqn_core::Result<B> {
    // Build compiler arguments with accumulated variable names
    let rt_arr = tag_arr(BqnArr::from_b_vec(rt.runtime.clone()));
    let sys_fn = rbqn_vm::derive::m_sys_fn(100);
    let var_names_b = build_var_names_b(&state.var_names);
    // NOTE: CBQN uses depth -1 for REPL variables (loose mode).
    // The BQN compiler recognizes depth -1 as "existing REPL scope" variables.
    let var_depths: Vec<f64> = vec![-1.0; state.var_names.len()];
    let var_depths_b = tag_arr(BqnArr::new_vec_f64(var_depths));
    let comp_args = tag_arr(BqnArr::from_b_vec(vec![rt_arr, sys_fn, var_names_b, var_depths_b]));

    let src_chars: Vec<u32> = code.chars().map(|c| c as u32).collect();
    let src_b = tag_arr(BqnArr::new_vec_c32(src_chars));

    // Call compiler: compiler(args, source) -> ⟨bc, objs, blocks, bodies, indices, tokenInfo⟩
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

    // Extract nameList and varIDs before compile_all
    let name_list = extract_name_list(token_info_b);
    let var_ids = extract_var_ids(bodies_b);
    let prev_var_count = state.var_names.len();

    let src_chars2: Vec<u32> = code.chars().map(|c| c as u32).collect();
    let src_b2 = tag_arr(BqnArr::new_vec_c32(src_chars2));

    // Compile without passing a scope — the compiler already knows about
    // existing variables via varNames. All variables (old + new) are assigned
    // positions at depth 0 in the body's scope.
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

    // Pre-populate scope with accumulated variable values.
    // Positions 0..prev_var_count-1 get old values (compiler preserves positions
    // for names it received in varNames). New variables get NO_VAR.
    let mut init_vars = Vec::with_capacity(var_am as usize);
    for i in 0..var_am as usize {
        if i < state.var_values.len() {
            init_vars.push(state.var_values[i]);
        } else {
            init_vars.push(B::NO_VAR);
        }
    }

    // Execute directly with eval_bc so we retain scope access afterwards.
    // The root block is ty=0, imm=true. exec_block would create a child scope
    // and we'd lose the variable values. Instead we call eval_bc with our scope.
    let exec_scope = std::rc::Rc::new(Scope::new(body.clone(), None, var_am, &init_vars));
    let result = rbqn_vm::vm::eval_bc(&body, exec_scope.clone(), &block);

    // Read back variable values from exec_scope
    let mut new_values = Vec::with_capacity(var_am as usize);
    {
        let vars = exec_scope.vars.borrow_mut();
        for i in 0..var_am as usize {
            if i < vars.len() {
                new_values.push(vars[i]);
            }
        }
    }

    // Build updated variable names list.
    // Positions 0..prev_var_count-1 keep their names.
    // New positions get names from nameList + varIDs.
    let mut new_names = state.var_names.clone();
    if let (Some(nl), Some(ids)) = (&name_list, &var_ids) {
        for i in prev_var_count..var_am as usize {
            if i < ids.len() && ids[i] < nl.len() {
                new_names.push(nl[ids[i]].clone());
            } else {
                new_names.push(format!("_v{}", i));
            }
        }
    } else {
        // Fallback: generate placeholder names for new variables
        for i in prev_var_count..var_am as usize {
            new_names.push(format!("_v{}", i));
        }
    }

    state.var_names = new_names;
    state.var_values = new_values;

    Ok(result)
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
    if let Some((ref fmt_fn, _)) = rt.formatter
        && let Ok(result) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            c1(*fmt_fn, *val)
        }))
            && let Some(arr) = get_arr(result) {
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
