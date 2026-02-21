use std::sync::Arc;

use rbqn_core::B;
use rbqn_core::array::BqnArr;

use crate::block::{Block, Body, eval_fun_block, m_md1_block, m_md2_block};
use crate::bytecode::Op;
use crate::derive::{c1, c2, m_fork, m_atop, m1_d, m2_d, m_md2_partial_l, m_md2_partial_r};
use crate::namespace::{self, NS, NSDesc, get_ns, store_ns};
use crate::scope::{Scope, v_get, v_set, v_seth, v_check_bad_read};

pub fn exec_block(bl: &Block, body: Arc<Body>, psc: &Scope) -> B {
    let var_am = body.var_am;
    let sc = Arc::new(Scope::new(body.clone(), Some(Arc::new(psc.clone())), var_am, &[]));
    eval_bc(&body, sc, bl)
}

pub fn exec_block_with_args(bl: &Block, body: Arc<Body>, psc: &Scope, args: &[B]) -> B {
    let var_am = body.var_am.max(args.len() as u16);
    let sc = Arc::new(Scope::new(body.clone(), Some(Arc::new(psc.clone())), var_am, args));
    eval_bc(&body, sc, bl)
}

fn build_pscs(sc: &Arc<Scope>, max_psc: u16) -> Vec<Arc<Scope>> {
    let mut pscs = Vec::with_capacity(max_psc as usize);
    if max_psc > 0 {
        pscs.push(sc.clone());
        let mut current = sc.clone();
        for _ in 1..max_psc {
            match &current.psc {
                Some(p) => {
                    pscs.push(p.clone());
                    current = p.clone();
                }
                None => break,
            }
        }
    }
    pscs
}

/// Helper: unpack an immediate variable reference from a u64.
/// The encoding is: low 32 bits = position, high 32 bits = depth.
fn unpack_var_ref(packed: u64) -> (usize, usize) {
    let pos = packed as u32 as usize;
    let depth = (packed >> 32) as u32 as usize;
    (depth, pos)
}

/// Resolve a SYSV system value by index.
fn sysv_lookup(idx: u32) -> B {
    // System values used by the bootstrap compiler:
    //  0: Type    1: Decompose   4: Glyph   7: Fill/FillFn
    // 22: GroupLen  23: GroupOrd
    match idx {
        0 | 1 | 4 | 7 | 22 | 23 => crate::derive::m_sys_fn(idx),
        _ => B::SENTINEL,
    }
}

pub fn eval_bc(body: &Body, sc: Arc<Scope>, bl: &Block) -> B {
    let bc = &bl.bc;
    let bc_offset = body.bc_offset;
    let mut pc = bc_offset;
    let mut stack: Vec<B> = Vec::with_capacity(body.max_stack as usize);
    let mut pscs = build_pscs(&sc, body.max_psc);

    macro_rules! pop {
        () => {
            stack.pop().unwrap_or_else(|| rbqn_core::error::throw("VM: stack underflow"))
        };
    }
    macro_rules! push {
        ($v:expr) => {
            stack.push($v)
        };
    }
    macro_rules! peek {
        ($n:expr) => {
            stack[stack.len() - $n]
        };
    }
    macro_rules! read_u32 {
        () => {{
            let v = bc[pc] as u32;
            pc += 1;
            v
        }};
    }
    macro_rules! read_u64 {
        () => {{
            let lo = bc[pc] as u32;
            let hi = bc[pc + 1] as u32;
            pc += 2;
            (lo as u64) | ((hi as u64) << 32)
        }};
    }

    loop {
        if pc >= bc.len() {
            rbqn_core::error::throw("VM: bytecode overrun");
        }
        let op_val = bc[pc] as u32;
        pc += 1;

        let op = Op::from_u32(op_val);
        match op {
            Some(Op::POPS) => {
                pop!();
            }
            Some(Op::ADDI) => {
                let v = read_u64!();
                push!(B::from_u64(v));
            }
            Some(Op::ADDU) => {
                let v = read_u64!();
                push!(B::from_u64(v));
            }
            Some(Op::PUSH) => {
                let idx = read_u32!() as usize;
                push!(bl.comp.objs[idx]);
            }

            // --- Function calls ---
            Some(Op::FN1C) => {
                let f = pop!();
                let x = pop!();
                push!(c1(f, x));
            }
            Some(Op::FN1O) => {
                let f = pop!();
                let x = pop!();
                if x.q_n() {
                    push!(x);
                } else {
                    push!(c1(f, x));
                }
            }
            Some(Op::FN2C) => {
                let w = pop!();
                let f = pop!();
                let x = pop!();
                push!(c2(f, w, x));
            }
            Some(Op::FN2O) => {
                let w = pop!();
                let f = pop!();
                let x = pop!();
                if x.q_n() {
                    push!(x);
                } else if w.q_n() {
                    push!(c1(f, x));
                } else {
                    push!(c2(f, w, x));
                }
            }

            // --- Inline function call variants ---
            Some(Op::FN1Ci) => {
                let f = B::from_u64(read_u64!());
                let x = pop!();
                push!(c1(f, x));
            }
            Some(Op::FN1Oi) => {
                let f = B::from_u64(read_u64!());
                let x = pop!();
                if x.q_n() {
                    push!(x);
                } else {
                    push!(c1(f, x));
                }
            }
            Some(Op::FN2Ci) => {
                let f = B::from_u64(read_u64!());
                let w = pop!();
                let x = pop!();
                push!(c2(f, w, x));
            }
            Some(Op::FN2Oi) => {
                let f = B::from_u64(read_u64!());
                let w = pop!();
                let x = pop!();
                if x.q_n() {
                    push!(x);
                } else if w.q_n() {
                    push!(c1(f, x));
                } else {
                    push!(c2(f, w, x));
                }
            }

            // --- List/array construction ---
            Some(Op::LSTO) | Some(Op::LSTM) => {
                let sz = read_u32!() as usize;
                if sz == 0 {
                    let arr = BqnArr::empty_harr();
                    push!(tag_arr(arr));
                } else {
                    let mut elems = vec![B::SENTINEL; sz];
                    for i in 0..sz {
                        elems[sz - i - 1] = pop!();
                    }
                    let arr = BqnArr::from_b_vec(elems);
                    push!(tag_arr(arr));
                }
            }
            Some(Op::ARMO) => {
                let sz = read_u32!() as usize;
                let mut elems = vec![B::SENTINEL; sz];
                for i in 0..sz {
                    elems[sz - i - 1] = pop!();
                }
                let arr = BqnArr::from_b_vec(elems);
                push!(tag_arr(arr));
            }
            Some(Op::ARMM) => {
                let sz = read_u32!() as usize;
                let mut elems = vec![B::SENTINEL; sz];
                for i in 0..sz {
                    elems[sz - i - 1] = pop!();
                }
                let arr = BqnArr::from_b_vec(elems);
                push!(tag_arr(arr));
            }

            // --- Block definitions ---
            Some(Op::DFND0) => {
                let bl_data = read_u64!();
                let bl_idx = bl_data as usize;
                if bl_idx < bl.blocks.len() {
                    let child_bl = bl.blocks[bl_idx].clone();
                    let psc = if !pscs.is_empty() { pscs[0].clone() } else { sc.clone() };
                    push!(eval_fun_block(child_bl, psc));
                } else {
                    rbqn_core::error::throw("DFND0: block index out of bounds");
                }
            }
            Some(Op::DFND1) => {
                let bl_data = read_u64!();
                let bl_idx = bl_data as usize;
                if bl_idx < bl.blocks.len() {
                    let child_bl = bl.blocks[bl_idx].clone();
                    let psc = if !pscs.is_empty() { pscs[0].clone() } else { sc.clone() };
                    push!(m_md1_block(child_bl, psc));
                } else {
                    rbqn_core::error::throw("DFND1: block index out of bounds");
                }
            }
            Some(Op::DFND2) => {
                let bl_data = read_u64!();
                let bl_idx = bl_data as usize;
                if bl_idx < bl.blocks.len() {
                    let child_bl = bl.blocks[bl_idx].clone();
                    let psc = if !pscs.is_empty() { pscs[0].clone() } else { sc.clone() };
                    push!(m_md2_block(child_bl, psc));
                } else {
                    rbqn_core::error::throw("DFND2: block index out of bounds");
                }
            }

            // --- Modifier application ---
            Some(Op::MD1C) => {
                let f = pop!();
                let m = pop!();
                push!(m1_d(m, f));
            }
            Some(Op::MD2C) => {
                let f = pop!();
                let m = pop!();
                let g = pop!();
                push!(m2_d(m, f, g));
            }
            Some(Op::MD2L) => {
                // Partial 2-modifier: has left operand, needs right
                let f = pop!();
                let m2 = pop!();
                push!(m_md2_partial_l(m2, f));
            }
            Some(Op::MD2R) => {
                // Partial 2-modifier: has right operand, needs left
                let g = pop!();
                let m2 = pop!();
                push!(m_md2_partial_r(m2, g));
            }

            // --- Trains ---
            Some(Op::TR2D) => {
                let g = pop!();
                let h = pop!();
                push!(m_atop(g, h));
            }
            Some(Op::TR3D) => {
                let f = pop!();
                let g = pop!();
                let h = pop!();
                push!(m_fork(f, g, h));
            }
            Some(Op::TR3O) => {
                let f = pop!();
                let g = pop!();
                let h = pop!();
                if f.q_n() {
                    push!(m_atop(g, h));
                } else {
                    push!(m_fork(f, g, h));
                }
            }

            // --- Variable access ---
            Some(Op::VARO) => {
                let d = read_u32!();
                let p = read_u32!();
                let val = pscs[d as usize].vars[p as usize];
                if v_check_bad_read(val) {
                    rbqn_core::error::throw("Attempting to read variable which is not yet defined");
                }
                push!(val);
            }
            Some(Op::VARM) => {
                let d = read_u32!();
                let p = read_u32!();
                push!(rbqn_core::tagu64((d as u64) << 32 | p as u64, rbqn_core::VAR_TAG));
            }
            Some(Op::VARU) => {
                let d = read_u32!();
                let p = read_u32!();
                let val = pscs[d as usize].vars[p as usize];
                push!(val);
                let sc_mut = Arc::make_mut(&mut pscs[d as usize]);
                sc_mut.vars[p as usize] = B::OPT_OUT;
            }
            Some(Op::EXTO) => {
                let d = read_u32!();
                let p = read_u32!();
                if let Some(ref ext) = pscs[d as usize].ext {
                    let val = ext.vars[p as usize];
                    if v_check_bad_read(val) {
                        rbqn_core::error::throw("Attempting to read ext variable which is not yet defined");
                    }
                    push!(val);
                } else {
                    rbqn_core::error::throw("EXTO: no scope extension");
                }
            }
            Some(Op::EXTM) => {
                let d = read_u32!();
                let p = read_u32!();
                push!(rbqn_core::tagu64((d as u64) << 32 | p as u64, rbqn_core::EXT_TAG));
            }
            Some(Op::EXTU) => {
                let d = read_u32!();
                let p = read_u32!();
                if let Some(ref ext) = pscs[d as usize].ext {
                    let val = ext.vars[p as usize];
                    push!(val);
                } else {
                    rbqn_core::error::throw("EXTU: no scope extension");
                }
                let sc_mut = Arc::make_mut(&mut pscs[d as usize]);
                if let Some(ref mut ext) = sc_mut.ext {
                    ext.vars[p as usize] = B::OPT_OUT;
                }
            }

            // --- Dynamic variables ---
            Some(Op::DYNO) => {
                let idx = read_u32!();
                push!(sysv_lookup(idx));
            }
            Some(Op::DYNM) => {
                let _idx = read_u32!();
                // Push a mutable reference placeholder — for now treat as read-only
                push!(B::SENTINEL);
            }

            // --- Assignment opcodes ---
            Some(Op::SETN) => {
                let s = pop!();
                let x = pop!();
                v_set(&mut pscs, s, x, false, true);
                push!(x);
            }
            Some(Op::SETU) => {
                let s = pop!();
                let x = pop!();
                v_set(&mut pscs, s, x, true, true);
                push!(x);
            }
            Some(Op::SETM) => {
                let s = pop!();
                let f = pop!();
                let x = pop!();
                let w = v_get(&pscs, s, true);
                let r = c2(f, w, x);
                v_set(&mut pscs, s, r, true, false);
                push!(r);
            }
            Some(Op::SETC) => {
                let s = pop!();
                let f = pop!();
                let x = v_get(&pscs, s, true);
                let r = c1(f, x);
                v_set(&mut pscs, s, r, true, false);
                push!(r);
            }

            // --- Immediate set variants (variable ref encoded as immediate u64) ---
            Some(Op::SETNi) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let x = pop!();
                let sc_mut = Arc::make_mut(&mut pscs[d]);
                sc_mut.vars[p] = x;
                push!(x);
            }
            Some(Op::SETUi) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let x = pop!();
                let sc_mut = Arc::make_mut(&mut pscs[d]);
                let prev = sc_mut.vars[p];
                if crate::scope::v_check_bad_write(prev) {
                    crate::scope::v_tag_error_pub(prev, true);
                }
                sc_mut.vars[p] = x;
                push!(x);
            }
            Some(Op::SETMi) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let f = pop!();
                let x = pop!();
                let w = pscs[d].vars[p];
                let r = c2(f, w, x);
                let sc_mut = Arc::make_mut(&mut pscs[d]);
                sc_mut.vars[p] = r;
                push!(r);
            }
            Some(Op::SETCi) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let f = pop!();
                let x = pscs[d].vars[p];
                let r = c1(f, x);
                let sc_mut = Arc::make_mut(&mut pscs[d]);
                sc_mut.vars[p] = r;
                push!(r);
            }

            // --- Void set variants (like SETNi but don't push result) ---
            Some(Op::SETNv) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let x = pop!();
                let sc_mut = Arc::make_mut(&mut pscs[d]);
                sc_mut.vars[p] = x;
            }
            Some(Op::SETUv) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let x = pop!();
                let sc_mut = Arc::make_mut(&mut pscs[d]);
                let prev = sc_mut.vars[p];
                if crate::scope::v_check_bad_write(prev) {
                    crate::scope::v_tag_error_pub(prev, true);
                }
                sc_mut.vars[p] = x;
            }
            Some(Op::SETMv) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let f = pop!();
                let x = pop!();
                let w = pscs[d].vars[p];
                let r = c2(f, w, x);
                let sc_mut = Arc::make_mut(&mut pscs[d]);
                sc_mut.vars[p] = r;
            }
            Some(Op::SETCv) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let f = pop!();
                let x = pscs[d].vars[p];
                let r = c1(f, x);
                let sc_mut = Arc::make_mut(&mut pscs[d]);
                sc_mut.vars[p] = r;
            }

            // --- Header match ---
            Some(Op::SETH1) => {
                let s = pop!();
                let x = pop!();
                let _v1 = read_u64!();
                let ok = v_seth(&mut pscs, s, x);
                if !ok {
                    rbqn_core::error::throw("SETH1: header match failed");
                }
            }
            Some(Op::SETH2) => {
                let s = pop!();
                let x = pop!();
                let _v1 = read_u64!();
                let _v2 = read_u64!();
                let ok = v_seth(&mut pscs, s, x);
                if !ok {
                    rbqn_core::error::throw("SETH2: header match failed");
                }
            }

            // --- Predicates ---
            Some(Op::PRED1) => {
                let x = pop!();
                let _v1 = read_u64!();
                if !x.o2b() {
                    rbqn_core::error::throw("PRED1: predicate failed");
                }
            }
            Some(Op::PRED2) => {
                let x = pop!();
                let _v1 = read_u64!();
                let _v2 = read_u64!();
                if !x.o2b() {
                    rbqn_core::error::throw("PRED2: predicate failed");
                }
            }

            // --- Namespace field access ---
            Some(Op::FLDG) => {
                let ns_val = pop!();
                let gid = read_u32!() as i32;
                if !ns_val.is_nsp() {
                    rbqn_core::error::throw("Trying to read a field from non-namespace");
                }
                let ns = get_ns(ns_val);
                match ns.get_by_gid(gid) {
                    Some(v) => push!(v),
                    None => rbqn_core::error::throw(
                        format!("Namespace does not have field '{}'", namespace::gid2str(gid))
                    ),
                }
            }
            Some(Op::FLDO) => {
                let ns_val = pop!();
                let gid = read_u32!() as i32;
                if !ns_val.is_nsp() {
                    rbqn_core::error::throw("Trying to read a field from non-namespace");
                }
                let ns = get_ns(ns_val);
                match ns.get_by_gid(gid) {
                    Some(v) => push!(v),
                    None => push!(B::SENTINEL), // optional: Nothing if not found
                }
            }
            Some(Op::FLDM) => {
                let ns_val = pop!();
                let gid = read_u32!() as i32;
                if !ns_val.is_nsp() {
                    rbqn_core::error::throw("Trying to read a field from non-namespace");
                }
                let ns = get_ns(ns_val);
                match ns.get_by_gid(gid) {
                    Some(v) => push!(v),
                    None => rbqn_core::error::throw(
                        format!("Namespace does not have field '{}' for modification", namespace::gid2str(gid))
                    ),
                }
            }

            Some(Op::ALIM) => {
                // Array limit: currently just consume the operand and pass through
                let _o = pop!();
                let _gid = read_u32!();
            }

            Some(Op::CHKV) => {
                if peek!(1).q_n() {
                    rbqn_core::error::throw("Unexpected Nothing (\u{00B7})");
                }
            }

            Some(Op::VFYM) => {
                // Verify mutable: check that top of stack is a valid mutable reference.
                // For now, just pass through (the value stays on the stack).
                // VFYM consumes 1, produces 1 => net 0.
                // It pops and re-pushes, or just peeks. stack_diff=0, consumed=1 suggests pop+push.
                let v = pop!();
                push!(v);
            }

            Some(Op::FAIL) => {
                rbqn_core::error::throw("This block cannot be called with these arguments");
            }

            // --- System values ---
            Some(Op::SYSV) => {
                let idx = read_u32!();
                push!(sysv_lookup(idx));
            }

            // --- Return opcodes ---
            Some(Op::RETD) => {
                // Build namespace from scope exports and return it
                if let Some(ref ns_desc) = body.ns_desc {
                    // Pop the unused stack value if present
                    if !stack.is_empty() {
                        pop!();
                    }
                    return store_ns(NS {
                        desc: ns_desc.clone(),
                        sc: if !pscs.is_empty() { pscs[0].clone() } else { sc.clone() },
                    });
                }
                // No namespace descriptor: just return top of stack or SENTINEL
                if stack.is_empty() {
                    return B::SENTINEL;
                }
                return pop!();
            }
            Some(Op::RETN) => {
                return pop!();
            }

            _ => {
                rbqn_core::error::throw(format!("VM: unhandled opcode 0x{:02x}", op_val));
            }
        }
    }
}

pub fn tag_arr(arr: BqnArr) -> B {
    rbqn_core::tag_arr(arr)
}

pub fn get_arr(b: B) -> Option<BqnArr> {
    rbqn_core::get_arr(b)
}

impl Clone for Scope {
    fn clone(&self) -> Self {
        Scope {
            psc: self.psc.clone(),
            body: self.body.clone(),
            var_am: self.var_am,
            ext: self.ext.as_ref().map(|e| crate::scope::ScopeExt {
                var_am: e.var_am,
                vars: e.vars.clone(),
            }),
            vars: self.vars.clone(),
        }
    }
}
