use std::sync::Arc;

use rbqn_core::B;

use crate::block::{Block, Body, Comp, CompKind, arg_count};
use crate::bytecode::Op;
use crate::scope::Scope;
use crate::vm::get_arr;

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
        name_list: B::SENTINEL,
        objs,
        kind: CompKind::Unknown,
        block_am: 0,
    });

    let bc: Vec<u32> = bc_arr.iter().map(|&x| x as u32).collect();
    let bc_ia = bc.len();

    let mut b_done = vec![false; all_blocks.len()];

    compile_block(
        all_blocks[0], &comp, &mut b_done, &bc, bc_ia,
        all_blocks, all_bodies, sc, 0, 0, ns_result,
    )
}

/// Advance past one source bytecode instruction.
fn src_bc_next(bc: &[u32], pos: usize) -> usize {
    let op = Op::from_u32(bc[pos]);
    match op {
        Some(op) => pos + crate::bytecode::bc_len(op) as usize,
        None => pos + 1,
    }
}

/// Extract a Vec<i32> of body indices from a B value that is an array of f64s.
fn b_to_i32_vec(b: B) -> Vec<i32> {
    if let Some(arr) = get_arr(b) {
        (0..arr.ia())
            .map(|j| arr.get(j).map(|v| v.o2f() as i32).unwrap_or(0))
            .collect()
    } else {
        vec![]
    }
}

struct FixupReq {
    off: usize,        // offset into new_bc where the u64(s) should be written
    pos1: usize,       // index into body_ps for next monadic body
    pos2: Option<usize>, // index into body_ps for next dyadic body (None if immediate)
}

fn compile_block(
    block_info: B,
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
    // --- Parse block metadata ---
    let block_arr = get_arr(block_info).expect("VM compiler: Bad block info");
    let ty = block_arr.get(0).map(|v| v.o2f() as u8).unwrap_or(0);
    let imm = block_arr.get(1).map(|v| v.o2f() != 0.0).unwrap_or(false);
    let body_obj = block_arr.get(2).unwrap_or(B::SENTINEL);
    let arg_am = arg_count(ty, imm);

    let mut new_bc: Vec<i32> = Vec::new();
    let mut map_bc: Vec<i32> = Vec::new();
    let mut used_blocks: Vec<Arc<Block>> = Vec::new();
    let mut bodies: Vec<Arc<Body>> = Vec::new();

    // Fail body at index 0
    new_bc.push(Op::FAIL as i32);
    map_bc.push(my_pos);
    let fail_body = Arc::new(Body::fail_body());
    bodies.push(fail_body.clone());

    let mut start_bodies: [Arc<Body>; 6] = [
        fail_body.clone(), fail_body.clone(), fail_body.clone(),
        fail_body.clone(), fail_body.clone(), fail_body.clone(),
    ];

    let bo_arr = body_obj.is_arr();
    let bo_count = if bo_arr {
        get_arr(body_obj).map(|a| a.ia()).unwrap_or(1)
    } else {
        1
    };

    let mut first_m_pos: usize = 0; // index in bodies vec of first monadic body (0 = fail)
    let mut all_fixup_offsets: Vec<usize> = Vec::new(); // bytecode offsets for swap remapping

    // --- Iterate over body pairs: (0,1), (2,3), (4) ---
    let mut pair_i: usize = 0;
    while pair_i < 6 {
        if pair_i >= bo_count { break; }

        // Parse monadic and dyadic body index lists for this pair
        let (mono_indices, dy_indices) = if bo_arr {
            let bo_a = get_arr(body_obj).unwrap();
            if pair_i == 4 {
                // Special: b1 = empty, b2 = bodyObj[4] (inverse w bodies, always dyadic)
                let b2 = bo_a.get(4).unwrap_or(B::SENTINEL);
                (vec![], b_to_i32_vec(b2))
            } else {
                let b1 = bo_a.get(pair_i).unwrap_or(B::SENTINEL);
                let b2 = if pair_i + 1 < bo_count {
                    bo_a.get(pair_i + 1).unwrap_or(B::SENTINEL)
                } else {
                    B::SENTINEL
                };
                let b1_list = b_to_i32_vec(b1);
                let b2_list = if b2.is_arr() { b_to_i32_vec(b2) } else { vec![] };
                (b1_list, b2_list)
            }
        } else {
            // Simple integer body index: same for mono and dyadic
            let idx = body_obj.o2f() as i32;
            (vec![idx], vec![idx])
        };

        let m_count = mono_indices.len();
        let d_count = dy_indices.len();

        // Build merged body_ps: [mono..., I32_MAX, dyadic..., I32_MAX]
        let mut body_ps: Vec<i32> = Vec::with_capacity(m_count + d_count + 2);
        body_ps.extend_from_slice(&mono_indices);
        body_ps.push(i32::MAX);
        body_ps.extend_from_slice(&dy_indices);
        body_ps.push(i32::MAX);

        let map_len = m_count + d_count;
        let mut body_map: Vec<Option<usize>> = vec![None; map_len + 2];
        let mut body_reqs: Vec<FixupReq> = Vec::new();

        let mut pos1: usize = 0;
        let mut pos2: usize = m_count + 1;
        let mut first_m = true;
        let mut first_d = true;

        loop {
            let curr1 = body_ps[pos1];
            let curr2 = body_ps[pos2];
            let curr_body = curr1.min(curr2);
            if curr_body == i32::MAX { break; }

            let is1 = curr1 == curr_body;
            let is2 = curr2 == curr_body;
            if is1 { pos1 += 1; }
            if is2 { pos2 += 1; }

            // Read body info from all_bodies[curr_body]
            let body_repr = all_bodies[curr_body as usize];
            let body_arr = get_arr(body_repr).expect("VM compiler: Body array contained non-array");
            let bo_ia = body_arr.ia();
            let bc_offset = body_arr.get(0).map(|v| v.o2f() as usize).unwrap_or(0);
            let vam = body_arr.get(1).map(|v| v.o2f() as u16).unwrap_or(0);

            if bc_offset >= bc_ia {
                rbqn_core::error::throw("VM compiler: Bytecode index out of bounds");
            }

            let mut h: i32 = 0;
            let mut h_max: i32 = 0;
            let mut mpsc: i32 = 0;

            // Scope extension for REPL (depth==0 && sc present && vam > sc.var_am)
            if depth == 0 {
                if let Some(sc) = sc {
                    if vam > sc.var_am && bo_ia >= 4 {
                        // TODO: implement scope extension for REPL
                    }
                }
            }

            let bc_start = new_bc.len();

            // Pre-scan for PRED
            let mut remap_args = false;
            {
                let mut c = bc_offset;
                while bc[c] != Op::RETN as u32 && bc[c] != Op::RETD as u32 {
                    if bc[c] == Op::PRED as u32 { remap_args = true; break; }
                    c = src_bc_next(bc, c);
                    if c >= bc_ia {
                        rbqn_core::error::throw("VM compiler: No RETN/RETD found before end of bytecode");
                    }
                }
            }

            // Arg remapping preamble when PRED found
            if remap_args {
                if sc.is_some() && depth == 0 {
                    rbqn_core::error::throw("Predicates cannot be used directly in a REPL");
                }
                let mut arg_used = [false; 6];
                let mut c = bc_offset;
                while bc[c] != Op::RETN as u32 && bc[c] != Op::RETD as u32 {
                    let op_val = bc[c];
                    if (op_val == Op::VARO as u32 || op_val == Op::VARM as u32 || op_val == Op::VARU as u32)
                        && bc[c + 1] == 0
                        && (bc[c + 2] as i32) < arg_am
                    {
                        arg_used[bc[c + 2] as usize] = true;
                    }
                    c = src_bc_next(bc, c);
                    if c >= bc_ia { break; }
                }
                for i in 0..6 {
                    if arg_used[i] {
                        // VARO 0 i, VARM 0 vam+i, SETN, POPS
                        new_bc.extend_from_slice(&[
                            Op::VARO as i32, 0, i as i32,
                            Op::VARM as i32, 0, vam as i32 + i as i32,
                            Op::SETN as i32,
                            Op::POPS as i32,
                        ]);
                        map_bc.extend([0i32; 8]);
                    }
                }
            }

            // --- Main bytecode compilation loop ---
            let mut c = bc_offset;
            loop {
                let op_val = bc[c];
                let op = Op::from_u32(op_val);
                let n = src_bc_next(bc, c);
                let old_c = c;
                let mut ret = false;

                match op {
                    Some(Op::PUSH) => {
                        let obj_idx = bc[c + 1] as usize;
                        let obj = comp.objs[obj_idx];
                        new_bc.push(if obj.is_val() { Op::ADDI as i32 } else { Op::ADDU as i32 });
                        new_bc.push(obj.0 as i32);
                        new_bc.push((obj.0 >> 32) as i32);
                    }
                    Some(Op::NOTM) => {
                        new_bc.push(Op::ADDU as i32);
                        new_bc.push(B::SENTINEL.0 as i32);
                        new_bc.push((B::SENTINEL.0 >> 32) as i32);
                    }
                    Some(Op::RETN) => {
                        new_bc.push(Op::RETN as i32);
                        ret = true;
                    }
                    Some(Op::RETD) => {
                        if ns_result != 0 && depth == 0 {
                            if ns_result == -1 {
                                rbqn_core::error::throw("Cannot construct a namespace for a REPL result");
                            }
                            if h == 0 {
                                rbqn_core::error::throw("No value for REPL expression to return");
                            }
                            new_bc.push(Op::RETN as i32);
                        } else {
                            if h == 1 {
                                new_bc.push(Op::POPS as i32);
                                map_bc.push(old_c as i32);
                            }
                            new_bc.push(Op::RETD as i32);
                        }
                        ret = true;
                    }
                    Some(Op::DFND) => {
                        let block_id = bc[c + 1] as usize;
                        if block_id < b_done.len() && !b_done[block_id] {
                            b_done[block_id] = true;
                            let child = compile_block(
                                all_blocks[block_id], comp, b_done, bc, bc_ia,
                                all_blocks, all_bodies, sc, depth + 1, c as i32, 0,
                            );
                            let dfnd_op = match child.ty {
                                0 => Op::DFND0,
                                1 => Op::DFND1,
                                _ => Op::DFND2,
                            };
                            new_bc.push(dfnd_op as i32);
                            let bl_idx = used_blocks.len();
                            used_blocks.push(child);
                            new_bc.push(bl_idx as i32);
                            new_bc.push(0);
                        }
                    }
                    Some(Op::VARO) | Some(Op::VARM) | Some(Op::VARU) => {
                        let mut ins = op_val;
                        let cdepth = bc[c + 1] as i32;
                        let mut cpos = bc[c + 2] as i32;
                        if cdepth + 1 > mpsc { mpsc = cdepth + 1; }

                        // EXT promotion for REPL scope extension
                        if let Some(sc) = sc {
                            if cdepth >= depth {
                                let mut csc = sc;
                                for _ in depth..cdepth {
                                    match &csc.psc {
                                        Some(p) => csc = unsafe { &*Arc::as_ptr(p) },
                                        None => rbqn_core::error::throw("VM compiler: VAR_ has an out-of-bounds depth"),
                                    }
                                }
                                if cpos >= csc.var_am as i32 {
                                    cpos -= csc.var_am as i32;
                                    ins = match ins {
                                        x if x == Op::VARO as u32 => Op::EXTO as u32,
                                        x if x == Op::VARM as u32 => Op::EXTM as u32,
                                        _ => Op::EXTO as u32, // VARU → EXTO (CBQN does this)
                                    };
                                }
                            }
                        }

                        if remap_args && cpos < arg_am && cdepth == 0 {
                            cpos += vam as i32;
                        }

                        new_bc.push(ins as i32);
                        new_bc.push(cdepth);
                        new_bc.push(cpos);
                    }
                    Some(Op::SETH) | Some(Op::PRED) => {
                        if mpsc < 1 { mpsc = 1; }
                        let is_seth = op_val == Op::SETH as u32;
                        let new_op = if is_seth {
                            if imm { Op::SETH1 } else { Op::SETH2 }
                        } else {
                            if imm { Op::PRED1 } else { Op::PRED2 }
                        };
                        new_bc.push(new_op as i32);

                        let fixup_off = new_bc.len();
                        body_reqs.push(FixupReq {
                            off: fixup_off,
                            pos1,
                            pos2: if imm { None } else { Some(pos2) },
                        });

                        // Placeholder u64(s)
                        new_bc.extend_from_slice(&[0, 0]);
                        if !imm {
                            new_bc.extend_from_slice(&[0, 0]);
                        }
                    }
                    Some(Op::FLDO) => {
                        // TODO: FLDG with str2gid when nameList available
                        new_bc.push(Op::FLDO as i32);
                        new_bc.push(bc[c + 1] as i32);
                    }
                    Some(op_e) => {
                        // Default: copy verbatim
                        for i in c..n {
                            new_bc.push(bc[i] as i32);
                        }
                        let sc_val = crate::bytecode::stack_diff(op_e);
                        h += sc_val;
                        if matches!(op_e, Op::LSTO | Op::LSTM | Op::ARMO | Op::ARMM) {
                            h += 1 - bc[c + 1] as i32;
                        }
                    }
                    None => {
                        new_bc.push(op_val as i32);
                    }
                }

                // Map bytecode
                let nlen = new_bc.len() - map_bc.len();
                for _ in 0..nlen {
                    map_bc.push(old_c as i32);
                }

                // Stack tracking (for explicitly handled ops)
                if let Some(op_e) = op {
                    match op_e {
                        Op::PUSH | Op::NOTM | Op::DFND => { h += 1; }
                        Op::RETN => {}
                        Op::RETD => {}
                        Op::VARO | Op::VARM | Op::VARU => { h += 1; }
                        Op::SETH | Op::PRED => {
                            h += crate::bytecode::stack_diff(op_e);
                        }
                        Op::FLDO => {
                            h += crate::bytecode::stack_diff(op_e);
                        }
                        _ => {} // handled in the default arm above
                    }
                }
                if h > h_max { h_max = h; }

                if ret { break; }
                c = n;
            }

            // Create body with correct var_am from all_bodies
            let final_vam = vam as i32 + if remap_args { arg_am } else { 0 };
            let body = Arc::new(Body::new(final_vam as u16, bc_start, h_max as u32, mpsc as u16));

            let body_idx = bodies.len();
            if is1 {
                body_map[pos1 - 1] = Some(body_idx);
                if first_m {
                    first_m = false;
                    start_bodies[pair_i] = body.clone();
                    if pair_i == 0 { first_m_pos = body_idx; }
                }
            }
            if is2 {
                body_map[pos2 - 1] = Some(body_idx);
                if first_d {
                    first_d = false;
                    start_bodies[pair_i + 1] = body.clone();
                }
            }
            bodies.push(body);
        }

        // Fixup SETH/PRED body references for this pair
        for req in &body_reqs {
            let idx1 = body_map[req.pos1].unwrap_or(0) as u64;
            new_bc[req.off] = idx1 as i32;
            new_bc[req.off + 1] = (idx1 >> 32) as i32;
            all_fixup_offsets.push(req.off);

            if let Some(p2) = req.pos2 {
                let idx2 = body_map[p2].unwrap_or(0) as u64;
                new_bc[req.off + 2] = idx2 as i32;
                new_bc[req.off + 3] = (idx2 >> 32) as i32;
                all_fixup_offsets.push(req.off + 2);
            }
        }

        pair_i += 2;
    }

    // --- Swap bodies[0] (fail body) with bodies[first_m_pos] (first monadic body) ---
    if first_m_pos != 0 {
        bodies.swap(0, first_m_pos);
        // Remap any bytecode body references affected by the swap
        for &off in &all_fixup_offsets {
            let idx = (new_bc[off] as u32 as u64) | ((new_bc[off + 1] as u32 as u64) << 32);
            let new_idx = if idx == 0 {
                first_m_pos as u64
            } else if idx == first_m_pos as u64 {
                0
            } else {
                idx
            };
            new_bc[off] = new_idx as i32;
            new_bc[off + 1] = (new_idx >> 32) as i32;
        }
    }

    // --- Assemble Block ---
    let block = Block {
        comp: comp.clone(),
        ty,
        imm,
        bc: new_bc,
        map: map_bc,
        blocks: used_blocks,
        body_count: bodies.len(),
        bodies: bodies.clone(),
        dy_body: if start_bodies[1].exists { Some(start_bodies[1].clone()) } else { None },
        inv_m_body: if start_bodies[2].exists { Some(start_bodies[2].clone()) } else { None },
        inv_x_body: if start_bodies[3].exists { Some(start_bodies[3].clone()) } else { None },
        inv_w_body: if start_bodies[5].exists { Some(start_bodies[5].clone()) } else { None },
    };

    Arc::new(block)
}
