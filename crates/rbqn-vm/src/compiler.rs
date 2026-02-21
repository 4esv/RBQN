use std::sync::Arc;

use rbqn_core::value::{B, bi_N};

use crate::block::{Block, Body, Comp, CompKind};
use crate::bytecode::Op;
use crate::scope::Scope;

pub fn compile_all(
    bc_arr: &[i32],
    objs: Vec<B>,
    all_blocks: &[B],
    all_bodies: &[B],
    indices: B,
    _token_info: B,
    src: B,
    fullpath: B,
    sc: Option<&Scope>,
    ns_result: i32,
) -> Arc<Block> {
    let comp = Arc::new(Comp {
        src,
        fullpath,
        indices,
        name_list: bi_N,
        objs,
        kind: CompKind::Unknown,
        block_am: 0,
    });

    let bc: Vec<u32> = bc_arr.iter().map(|&x| x as u32).collect();
    let bc_ia = bc.len();

    let mut b_done = vec![false; all_blocks.len()];

    compile_block(
        all_blocks, 0, &comp, &mut b_done, &bc, bc_ia,
        all_blocks, all_bodies, sc, 0, 0, ns_result,
    )
}

fn compile_block(
    _blocks_src: &[B],
    _block_idx: usize,
    comp: &Arc<Comp>,
    b_done: &mut [bool],
    bc: &[u32],
    bc_ia: usize,
    all_blocks: &[B],
    all_bodies: &[B],
    sc: Option<&Scope>,
    depth: i32,
    my_pos: i32,
    ns_result: i32,
) -> Arc<Block> {
    // Simplified compiler: for now, we pass through bytecode mostly unchanged.
    // The full compiler transforms PUSH->ADDI/ADDU, DFND->DFND0/1/2, etc.
    // This will be expanded as needed.

    let fail_body = Arc::new(Body::fail_body());

    let mut new_bc: Vec<i32> = Vec::new();
    let mut map_bc: Vec<i32> = Vec::new();
    let mut bodies: Vec<Arc<Body>> = Vec::new();
    let mut used_blocks: Vec<Arc<Block>> = Vec::new();

    // Add fail body's bytecode
    new_bc.push(Op::FAIL as i32);
    map_bc.push(my_pos);
    bodies.push(fail_body.clone());

    // For the simple case: single body starting at bytecode offset from the block info
    // In a real implementation, we'd parse the block/body structure from the input arrays.
    // For now, compile the bytecode starting from position 0.
    let bc_start = new_bc.len();
    let mut h: i32 = 0;
    let mut h_max: i32 = 0;
    let mut mpsc: i32 = 0;
    let mut idx = 0;

    while idx < bc_ia {
        let op_val = bc[idx];
        let op = Op::from_u32(op_val);
        let old_idx = idx;

        match op {
            Some(Op::PUSH) => {
                let obj_idx = bc[idx + 1] as usize;
                let obj = comp.objs[obj_idx];
                if obj.is_val() {
                    new_bc.push(Op::ADDI as i32);
                } else {
                    new_bc.push(Op::ADDU as i32);
                }
                new_bc.push(obj.u as i32);
                new_bc.push((obj.u >> 32) as i32);
                map_bc.extend([old_idx as i32; 3]);
                idx += 2;
                h += 1;
            }
            Some(Op::NOTM) => {
                new_bc.push(Op::ADDU as i32);
                new_bc.push(bi_N.u as i32);
                new_bc.push((bi_N.u >> 32) as i32);
                map_bc.extend([old_idx as i32; 3]);
                idx += 1;
                h += 1;
            }
            Some(Op::DFND) => {
                let block_id = bc[idx + 1] as usize;
                if block_id < b_done.len() && !b_done[block_id] {
                    b_done[block_id] = true;
                    let child = compile_block(
                        all_blocks, block_id, comp, b_done, bc, bc_ia,
                        all_blocks, all_bodies, sc, depth + 1, old_idx as i32, 0,
                    );
                    let dfnd_op = match child.ty {
                        0 => Op::DFND0,
                        1 => Op::DFND1,
                        _ => Op::DFND2,
                    };
                    new_bc.push(dfnd_op as i32);
                    // Store block pointer as index (we'll resolve later)
                    let bl_idx = used_blocks.len();
                    used_blocks.push(child);
                    new_bc.push(bl_idx as i32);
                    new_bc.push(0); // padding for u64 alignment
                    map_bc.extend([old_idx as i32; 3]);
                }
                idx += 2;
                h += 1;
            }
            Some(Op::VARO) | Some(Op::VARM) | Some(Op::VARU) => {
                let cdepth = bc[idx + 1] as i32;
                let cpos = bc[idx + 2];
                if cdepth + 1 > mpsc {
                    mpsc = cdepth + 1;
                }
                new_bc.push(op_val as i32);
                new_bc.push(cdepth);
                new_bc.push(cpos as i32);
                map_bc.extend([old_idx as i32; 3]);
                idx += 3;
                h += 1;
            }
            Some(Op::RETN) => {
                new_bc.push(Op::RETN as i32);
                map_bc.push(old_idx as i32);
                idx += 1;
                break;
            }
            Some(Op::RETD) => {
                if ns_result == 1 && depth == 0 {
                    if h == 0 {
                        rbqn_core::error::throw("No value for REPL expression to return");
                    }
                    new_bc.push(Op::RETN as i32);
                } else {
                    if h == 1 {
                        new_bc.push(Op::POPS as i32);
                        map_bc.push(old_idx as i32);
                    }
                    new_bc.push(Op::RETD as i32);
                }
                map_bc.push(old_idx as i32);
                idx += 1;
                break;
            }
            Some(op) => {
                let len = crate::bytecode::bc_len(op) as usize;
                for i in 0..len {
                    new_bc.push(bc[idx + i] as i32);
                    map_bc.push(old_idx as i32);
                }
                let sc_val = crate::bytecode::stack_diff(op);
                h += sc_val;
                if matches!(op, Op::LSTO | Op::LSTM | Op::ARMO | Op::ARMM) {
                    h += 1 - bc[idx + 1] as i32;
                }
                idx += len;
            }
            None => {
                new_bc.push(op_val as i32);
                map_bc.push(old_idx as i32);
                idx += 1;
            }
        }
        if h > h_max {
            h_max = h;
        }
    }

    // Create the main body
    let mut main_body = Body::new(0, bc_start, h_max as u32, mpsc as u16);
    main_body.bc = new_bc.iter().map(|&x| x as u32).collect();
    main_body.bc_offset = bc_start;
    let main_body = Arc::new(main_body);
    bodies.push(main_body.clone());

    let block = Block {
        comp: comp.clone(),
        ty: 0,
        imm: true,
        bc: new_bc,
        map: map_bc,
        blocks: used_blocks,
        body_count: bodies.len(),
        bodies,
        dy_body: None,
        inv_m_body: None,
        inv_x_body: None,
        inv_w_body: None,
    };

    Arc::new(block)
}
