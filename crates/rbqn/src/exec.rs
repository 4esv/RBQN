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

pub fn exec_string_inner(
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
