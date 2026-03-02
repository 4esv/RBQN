// NOTE: exec.rs — shared BQN compilation/execution logic.
// Used by both main.rs (REPL/file execution) and rbqn-gen (--verify self-compilation).

use std::sync::Arc;

use rbqn_core::array::BqnArr;
use rbqn_core::error::BqnError;
use rbqn_core::B;
use rbqn_vm::block::eval_fun_block;
use rbqn_vm::compiler::compile_all;
use rbqn_vm::derive::{c2};
use rbqn_vm::scope::Scope;
use rbqn_vm::vm::{get_arr, tag_arr};

use crate::bootstrap;
use crate::embedded::{BlockEntry, ObjectEntry, OwnedBytecode};

/// Convert a catch_unwind panic payload to a BqnError.
/// Prefers typed BqnError (from throw_bqn) over string extraction to preserve
/// the original error variant (Assert, Type, Rank, etc.) without re-wrapping.
pub fn panic_to_bqn_error(panic: Box<dyn std::any::Any + Send>) -> BqnError {
    // First: try typed BqnError (set via throw_bqn / panic_any)
    if let Some(e) = panic.downcast_ref::<BqnError>() {
        return e.clone();
    }
    // Fallback: string payload (legacy or external panic)
    let msg = if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = panic.downcast_ref::<&str>() {
        s.to_string()
    } else {
        "unknown error".into()
    };
    BqnError::Domain(msg)
}

/// Raw output from the BQN compiler (before VM execution).
/// Contains the same arrays that exec_string_inner uses to call compile_all + eval_fun_block,
/// but as raw BQN B values so they can be converted to OwnedBytecode for serialization.
pub struct CompilerOutput {
    pub bc: Vec<i32>,
    pub objs: Vec<B>,
    pub blocks: Vec<B>,
    pub bodies: Vec<B>,
}

/// Compile BQN source code and return the raw compiler output without executing.
/// This is the first half of exec_string_inner: it calls the BQN compiler and
/// extracts bc/objs/blocks/bodies, but does NOT call compile_all or eval_fun_block.
/// Used by rbqn-gen --verify to serialize self-compiled bytecode to .bin format.
pub fn compile_string(
    rt: &bootstrap::Runtime,
    code: &str,
) -> rbqn_core::Result<CompilerOutput> {
    if rt.compiler.q_n() || rt.compiler.0 == B::SENTINEL.0 {
        return Err(BqnError::Nyi(
            "compiler not available (bootstrap failed?)".into(),
        ));
    }

    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        compile_string_inner(rt, code)
    })) {
        Ok(result) => result,
        Err(panic) => Err(panic_to_bqn_error(panic)),
    }
}

fn compile_string_inner(
    rt: &bootstrap::Runtime,
    code: &str,
) -> rbqn_core::Result<CompilerOutput> {
    let rt_arr = tag_arr(BqnArr::from_b_vec(rt.runtime.clone()));
    let sys_fn = rbqn_vm::derive::m_sys_fn(100);
    let var_names = tag_arr(BqnArr::empty_harr());
    let var_depths = tag_arr(BqnArr::new_vec_i32(vec![]));
    let comp_args = tag_arr(BqnArr::from_b_vec(vec![rt_arr, sys_fn, var_names, var_depths]));

    let src_chars: Vec<u32> = code.chars().map(|c| c as u32).collect();
    let src_b = tag_arr(BqnArr::new_vec_c32(src_chars));

    let comp_result = c2(rt.compiler, comp_args, src_b);

    let comp_arr = get_arr(comp_result)
        .ok_or_else(|| BqnError::Domain("compiler did not return an array".into()))?;

    let bc_b = comp_arr.get(0).map_err(|e| BqnError::Domain(e.to_string()))?;
    let objs_b = comp_arr.get(1).map_err(|e| BqnError::Domain(e.to_string()))?;
    let blocks_b = comp_arr.get(2).map_err(|e| BqnError::Domain(e.to_string()))?;
    let bodies_b = comp_arr.get(3).map_err(|e| BqnError::Domain(e.to_string()))?;

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

    Ok(CompilerOutput { bc, objs, blocks, bodies })
}

/// Convert a CompilerOutput (raw BQN B values) into an OwnedBytecode (typed entries).
/// This is the reverse of bootstrap.rs build_objs/build_blocks/build_bodies:
/// instead of B → typed data, we go typed data → B → typed ObjectEntry.
///
/// For function values (provide/runtime refs), we scan the provided arrays to find
/// which index the B value corresponds to (by raw u64 identity).
pub fn compiler_output_to_owned(
    output: &CompilerOutput,
    provide: &[B],
    runtime_prev: Option<&[B]>,
    runtime: Option<&[B]>,
) -> OwnedBytecode {
    let mut iarrs: Vec<Vec<i32>> = Vec::new();

    // NOTE: Do NOT put bc in iarrs[0]. In the CBQN wire format, iarrs[0] is an empty array
    // used as a shared "no-bodies" placeholder for blocks with no monadic body list.
    // The bc is stored separately in the wire format (not in iarrs at all).
    // We pre-populate iarrs[0] = [] to match this convention.
    iarrs.push(vec![]);  // iarrs[0] = empty (placeholder for zero-length body list)
    let _empty_iarrs_idx = 0usize;  // index of empty array, used for blocks with no mono bodies

    // Helper: find or insert an i32 array into iarrs, return its index
    let find_or_insert_iarr = |data: Vec<i32>, iarrs: &mut Vec<Vec<i32>>| -> usize {
        // Check if identical array already exists
        for (i, existing) in iarrs.iter().enumerate() {
            if *existing == data {
                return i;
            }
        }
        let idx = iarrs.len();
        iarrs.push(data);
        idx
    };

    // Convert a B value to an ObjectEntry, looking up provide/runtime arrays for function refs
    let b_to_obj_entry = |b: B, iarrs: &mut Vec<Vec<i32>>| -> ObjectEntry {
        if b.is_f64() {
            ObjectEntry::Float(b.o2f())
        } else if b.is_c32() {
            ObjectEntry::Char(b.0 as u32)
        } else if b.is_arr() {
            // Try as integer array first
            if let Some(arr) = get_arr(b) {
                if arr.is_char_arr()
                    && let Ok(chars) = arr.c32_iter() {
                        return ObjectEntry::Str(chars);
                    }
                if let Ok(ints) = arr.i32_iter() {
                    let idx = {
                        // find or insert
                        let mut found = None;
                        for (i, existing) in iarrs.iter().enumerate() {
                            if *existing == ints {
                                found = Some(i);
                                break;
                            }
                        }
                        if let Some(i) = found {
                            i
                        } else {
                            let i = iarrs.len();
                            iarrs.push(ints);
                            i
                        }
                    };
                    return ObjectEntry::IArr(idx);
                }
            }
            // Fallback
            eprintln!("compiler_output_to_owned: unrecognized array object, using Float(0.0)");
            ObjectEntry::Float(0.0)
        } else if b.is_fun() || b.is_md1() || b.is_md2() {
            // Search provide array
            for (i, &p) in provide.iter().enumerate() {
                if p.0 == b.0 {
                    return ObjectEntry::Provide(i);
                }
            }
            // Search runtime array
            if let Some(rt) = runtime {
                for (i, &r) in rt.iter().enumerate() {
                    if r.0 == b.0 {
                        return ObjectEntry::Runtime(i);
                    }
                }
            }
            // Search runtime_prev
            if let Some(rtp) = runtime_prev {
                for (i, &r) in rtp.iter().enumerate() {
                    if r.0 == b.0 {
                        return ObjectEntry::RuntimePrev(i);
                    }
                }
            }
            eprintln!("compiler_output_to_owned: function not found in provide/runtime arrays (raw={:#018x}), using Float(0.0)", b.0);
            ObjectEntry::Float(0.0)
        } else {
            eprintln!("compiler_output_to_owned: unknown B value tag (raw={:#018x}), using Float(0.0)", b.0);
            ObjectEntry::Float(0.0)
        }
    };

    // Convert objs
    let objs: Vec<ObjectEntry> = output.objs.iter().map(|&b| {
        b_to_obj_entry(b, &mut iarrs)
    }).collect();

    // Convert blocks
    // Each block B is an array [type, imm, bodyArg]
    // If bodyArg is a scalar → simple block → BlockEntry::IArr pointing to the whole [type,imm,bodyArg]
    // If bodyArg is an array pair [monadics, dyadics] → BlockEntry::Info
    let blocks: Vec<BlockEntry> = output.blocks.iter().map(|&b| {
        let arr = match get_arr(b) {
            Some(a) => a,
            None => {
                eprintln!("compiler_output_to_owned: block is not an array");
                return BlockEntry::IArr(0);
            }
        };

        if arr.ia() < 3 {
            eprintln!("compiler_output_to_owned: block array has < 3 elements");
            return BlockEntry::IArr(0);
        }

        let typ_b = arr.get(0).unwrap_or(B::m_f64(0.0));
        let imm_b = arr.get(1).unwrap_or(B::m_f64(0.0));
        let body_arg = arr.get(2).unwrap_or(B::m_f64(0.0));

        let typ = typ_b.o2f() as u8;
        let imm = imm_b.o2f() as u8;

        if body_arg.is_arr() {
            // Compound block: bodyArg is ⟨monadics, dyadics⟩
            let body_pair = get_arr(body_arg).unwrap();
            if body_pair.ia() >= 2 {
                let monadic_b = body_pair.get(0).unwrap_or(B::m_f64(0.0));
                let dyadic_b = body_pair.get(1).unwrap_or(B::m_f64(0.0));

                let monadic_ints: Vec<i32> = if let Some(marr) = get_arr(monadic_b) {
                    marr.i32_iter().unwrap_or_default()
                } else if monadic_b.is_f64() {
                    vec![monadic_b.o2f() as i32]
                } else {
                    vec![]
                };
                let dyadic_ints: Vec<i32> = if let Some(darr) = get_arr(dyadic_b) {
                    darr.i32_iter().unwrap_or_default()
                } else if dyadic_b.is_f64() {
                    vec![dyadic_b.o2f() as i32]
                } else {
                    vec![]
                };

                let iarrs0_idx = find_or_insert_iarr(monadic_ints, &mut iarrs);
                let data_idx = find_or_insert_iarr(dyadic_ints, &mut iarrs);
                // info = type | (imm << 2)
                let info = typ | (imm << 2);
                BlockEntry::Info { typ: info, iarrs0_idx, data_idx }
            } else {
                // Degenerate pair — treat as simple
                let whole: Vec<i32> = vec![typ as i32, imm as i32, 0];
                let idx = find_or_insert_iarr(whole, &mut iarrs);
                BlockEntry::IArr(idx)
            }
        } else {
            // Simple block: [type, imm, bodyIndex] as integer array
            let body_idx = body_arg.o2f() as i32;
            let whole: Vec<i32> = vec![typ as i32, imm as i32, body_idx];
            let idx = find_or_insert_iarr(whole, &mut iarrs);
            BlockEntry::IArr(idx)
        }
    }).collect();

    // Convert bodies: each body B is an array [bcOffset, varCount, ...].
    // The BQN compiler returns bodies as 4-element mixed arrays [bcOffset, varCount, names, depths].
    // Only elements [0] (bcOffset) and [1] (varCount) are used by compile_all and needed for serialization.
    // The compile_all source in compiler.rs reads only body_arr.get(0) and body_arr.get(1).
    let bodies: Vec<usize> = output.bodies.iter().map(|&b| {
        let body_ints: Vec<i32> = if let Some(arr) = get_arr(b) {
            // Extract only bcOffset and varCount (elements 0 and 1)
            let bc_offset = arr.get(0).ok().and_then(|v| if v.is_f64() { Some(v.o2f() as i32) } else { None }).unwrap_or(0);
            let var_count = arr.get(1).ok().and_then(|v| if v.is_f64() { Some(v.o2f() as i32) } else { None }).unwrap_or(0);
            vec![bc_offset, var_count]
        } else if b.is_f64() {
            vec![b.o2f() as i32, 0]
        } else {
            vec![0, 0]
        };
        find_or_insert_iarr(body_ints, &mut iarrs)
    }).collect();

    OwnedBytecode {
        bc: output.bc.clone(),
        iarrs,
        objs,
        blocks,
        bodies,
        source_tag: "rbqn-self".to_string(),
    }
}

pub fn exec_string(
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
        Err(panic) => Err(panic_to_bqn_error(panic)),
    }
}

pub fn exec_string_inner(
    rt: &bootstrap::Runtime,
    code: &str,
) -> rbqn_core::Result<B> {
    // Build compiler arguments: ⟨runtime, •BQN_SYS, varNames, varDepths⟩
    let rt_arr = tag_arr(BqnArr::from_b_vec(rt.runtime.clone()));

    // Verify runtime array integrity
    if std::env::var("RBQN_COMP_TRACE").is_ok()
        && let Some(rta) = get_arr(rt_arr) {
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
