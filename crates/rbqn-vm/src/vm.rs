use std::sync::Arc;

use rbqn_core::value::{B, bi_N, bi_optOut, tagu64};
use rbqn_core::value::{VAR_TAG, EXT_TAG};
use rbqn_core::array::BqnArr;

use crate::block::{Block, Body, eval_fun_block, m_md1_block, m_md2_block};
use crate::bytecode::Op;
use crate::derive::{c1, c2, m_fork, m_atop, m1_d, m2_d};
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
            Some(Op::LSTO) | Some(Op::LSTM) => {
                let sz = read_u32!() as usize;
                if sz == 0 {
                    let arr = BqnArr::empty_harr();
                    push!(tag_arr(arr));
                } else {
                    let mut elems = vec![bi_N; sz];
                    for i in 0..sz {
                        elems[sz - i - 1] = pop!();
                    }
                    let arr = BqnArr::from_b_vec(elems);
                    push!(tag_arr(arr));
                }
            }
            Some(Op::ARMO) => {
                let sz = read_u32!() as usize;
                let mut elems = vec![bi_N; sz];
                for i in 0..sz {
                    elems[sz - i - 1] = pop!();
                }
                let arr = BqnArr::from_b_vec(elems);
                push!(tag_arr(arr));
            }
            Some(Op::ARMM) => {
                let sz = read_u32!() as usize;
                let mut elems = vec![bi_N; sz];
                for i in 0..sz {
                    elems[sz - i - 1] = pop!();
                }
                let arr = BqnArr::from_b_vec(elems);
                push!(tag_arr(arr));
            }
            Some(Op::DFND0) => {
                let bl_data = read_u64!();
                let bl_idx = bl_data as usize;
                if bl_idx < bl.blocks.len() {
                    let child_bl = bl.blocks[bl_idx].clone();
                    // Reconstruct psc from current scope
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
                push!(tagu64((d as u64) << 32 | p as u64, VAR_TAG));
            }
            Some(Op::VARU) => {
                let d = read_u32!();
                let p = read_u32!();
                let val = pscs[d as usize].vars[p as usize];
                push!(val);
                // Replace with bi_optOut
                let sc_mut = Arc::make_mut(&mut pscs[d as usize]);
                sc_mut.vars[p as usize] = bi_optOut;
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
                push!(tagu64((d as u64) << 32 | p as u64, EXT_TAG));
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
                // Replace with bi_optOut in ext
                let sc_mut = Arc::make_mut(&mut pscs[d as usize]);
                if let Some(ref mut ext) = sc_mut.ext {
                    ext.vars[p as usize] = bi_optOut;
                }
            }
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
            Some(Op::SETH1) => {
                let s = pop!();
                let x = pop!();
                let _v1 = read_u64!();
                let ok = v_seth(&mut pscs, s, x);
                if !ok {
                    // goto next body - for now, just error
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
            Some(Op::FLDG) => {
                let ns = pop!();
                let _gid = read_u32!();
                if !ns.is_nsp() {
                    rbqn_core::error::throw("Trying to read a field from non-namespace");
                }
                // TODO: namespace field access
                rbqn_core::error::throw("FLDG: namespace field access not yet implemented");
            }
            Some(Op::ALIM) => {
                let _o = pop!();
                let _gid = read_u32!();
                // TODO: field alias
                rbqn_core::error::throw("ALIM: not yet implemented");
            }
            Some(Op::CHKV) => {
                if peek!(1).q_n() {
                    rbqn_core::error::throw("Unexpected Nothing (\u{00B7})");
                }
            }
            Some(Op::VFYM) => {
                let _o = pop!();
                // TODO: push verify-mutable wrapper
                rbqn_core::error::throw("VFYM: not yet implemented");
            }
            Some(Op::FAIL) => {
                rbqn_core::error::throw("This block cannot be called with these arguments");
            }
            Some(Op::SYSV) => {
                let _n = read_u32!();
                // System function - will be implemented with the primitive dispatch
                push!(bi_N); // placeholder
            }
            Some(Op::RETD) => {
                // Return namespace
                // TODO: proper namespace construction
                if stack.is_empty() {
                    return bi_N;
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

use std::sync::atomic::AtomicU64;

static ARR_COUNTER: AtomicU64 = AtomicU64::new(1);
use std::collections::HashMap;
use std::sync::Mutex;

static ARR_STORE: std::sync::LazyLock<Mutex<HashMap<u64, BqnArr>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

fn tag_arr(arr: BqnArr) -> B {
    let id = ARR_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    ARR_STORE.lock().unwrap().insert(id, arr);
    tagu64(id << 3, rbqn_core::value::ARR_TAG)
}

pub fn get_arr(b: B) -> Option<BqnArr> {
    if !b.is_arr() {
        return None;
    }
    let id = (b.u & 0xFFFFFFFFFFFF) >> 3;
    ARR_STORE.lock().unwrap().get(&id).cloned()
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
