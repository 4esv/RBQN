use std::sync::Arc;

use rbqn_core::{B, FUN_TAG, MD1_TAG, MD2_TAG, tagu64};

use crate::block::Block;
use crate::scope::Scope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivedKind {
    Fork,
    Atop,
    Md1D,
    Md2D,
    Md2PartialL,  // MD2L: has m2 (g) and left operand (f), needs right operand
    Md2PartialR,  // MD2R: has m2 (g) and right operand (h), needs left operand
    FunBlock,
    Md1Block,
    Md2Block,
    NativeFn { prim_idx: usize },
    NativeMd1 { prim_idx: usize },
    NativeMd2 { prim_idx: usize },
    SysFn { sys_idx: u32 },
}

#[derive(Debug)]
pub struct Derived {
    pub kind: DerivedKind,
    pub f: B,
    pub g: B,
    pub h: B,
    pub bl: Option<Arc<Block>>,
    pub sc: Option<Arc<Scope>>,
}

static DERIVED_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_derived_id() -> u64 {
    DERIVED_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

pub fn store_derived(d: Derived) -> u64 {
    let id = next_derived_id();
    DERIVED_STORE.lock().unwrap().insert(id, Arc::new(d));
    id
}

pub fn get_derived(id: u64) -> Arc<Derived> {
    DERIVED_STORE.lock().unwrap().get(&id).cloned()
        .unwrap_or_else(|| rbqn_core::error::throw("Invalid derived object reference"))
}

use std::collections::HashMap;
use std::sync::Mutex;

static DERIVED_STORE: std::sync::LazyLock<Mutex<HashMap<u64, Arc<Derived>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn m_fork(f: B, g: B, h: B) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::Fork,
        f, g, h,
        bl: None, sc: None,
    });
    tagu64(id << 3, FUN_TAG)
}

pub fn m_atop(g: B, h: B) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::Atop,
        f: B::SENTINEL, g, h,
        bl: None, sc: None,
    });
    tagu64(id << 3, FUN_TAG)
}

pub fn m_md1d(m1: B, f: B) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::Md1D,
        f, g: m1, h: B::SENTINEL,
        bl: None, sc: None,
    });
    tagu64(id << 3, FUN_TAG)
}

pub fn m_md2d(m2: B, f: B, g: B) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::Md2D,
        f, g: m2, h: g,
        bl: None, sc: None,
    });
    tagu64(id << 3, FUN_TAG)
}

/// MD2L: partial 2-modifier application with left operand.
/// m2 is stored in g, left operand f is stored in f. When given right operand g via m1_d,
/// produces m2_d(m2, f, g).
pub fn m_md2_partial_l(m2: B, f: B) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::Md2PartialL,
        f, g: m2, h: B::SENTINEL,
        bl: None, sc: None,
    });
    tagu64(id << 3, MD1_TAG)
}

/// MD2R: partial 2-modifier application with right operand.
/// m2 is stored in g, right operand g is stored in h. When given left operand f via m1_d,
/// produces m2_d(m2, f, g).
pub fn m_md2_partial_r(m2: B, g: B) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::Md2PartialR,
        f: B::SENTINEL, g: m2, h: g,
        bl: None, sc: None,
    });
    tagu64(id << 3, MD1_TAG)
}

pub fn m_fun_block(bl: Arc<Block>, psc: Arc<Scope>) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::FunBlock,
        f: B::SENTINEL, g: B::SENTINEL, h: B::SENTINEL,
        bl: Some(bl), sc: Some(psc),
    });
    tagu64(id << 3, FUN_TAG)
}

pub fn m_md1_block_val(bl: Arc<Block>, psc: Arc<Scope>) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::Md1Block,
        f: B::SENTINEL, g: B::SENTINEL, h: B::SENTINEL,
        bl: Some(bl), sc: Some(psc),
    });
    tagu64(id << 3, MD1_TAG)
}

pub fn m_md2_block_val(bl: Arc<Block>, psc: Arc<Scope>) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::Md2Block,
        f: B::SENTINEL, g: B::SENTINEL, h: B::SENTINEL,
        bl: Some(bl), sc: Some(psc),
    });
    tagu64(id << 3, MD2_TAG)
}

pub fn m_native_fn(idx: usize) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::NativeFn { prim_idx: idx },
        f: B::SENTINEL, g: B::SENTINEL, h: B::SENTINEL,
        bl: None, sc: None,
    });
    tagu64(id << 3, FUN_TAG)
}

pub fn m_native_md1(idx: usize) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::NativeMd1 { prim_idx: idx },
        f: B::SENTINEL, g: B::SENTINEL, h: B::SENTINEL,
        bl: None, sc: None,
    });
    tagu64(id << 3, MD1_TAG)
}

pub fn m_native_md2(idx: usize) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::NativeMd2 { prim_idx: idx },
        f: B::SENTINEL, g: B::SENTINEL, h: B::SENTINEL,
        bl: None, sc: None,
    });
    tagu64(id << 3, MD2_TAG)
}

pub fn m_sys_fn(idx: u32) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::SysFn { sys_idx: idx },
        f: B::SENTINEL, g: B::SENTINEL, h: B::SENTINEL,
        bl: None, sc: None,
    });
    tagu64(id << 3, FUN_TAG)
}

pub fn m1_d(m: B, f: B) -> B {
    if m.is_md1() {
        m_md1d(m, f)
    } else {
        rbqn_core::error::throw("Interpreting non-1-modifier as 1-modifier");
    }
}

pub fn m2_d(m: B, f: B, g: B) -> B {
    if m.is_md2() {
        m_md2d(m, f, g)
    } else {
        rbqn_core::error::throw("Interpreting non-2-modifier as 2-modifier");
    }
}

pub fn c1(f: B, x: B) -> B {
    if f.is_fun() {
        let id = (f.0 & 0xFFFFFFFFFFFF) >> 3;
        let d = get_derived(id);
        match d.kind {
            DerivedKind::Fork => {
                let hx = c1(d.h, x);
                let fx = c1(d.f, x);
                c2(d.g, fx, hx)
            }
            DerivedKind::Atop => {
                let hx = c1(d.h, x);
                c1(d.g, hx)
            }
            DerivedKind::FunBlock => {
                crate::vm::vm_trace_push(format!("c1 FunBlock id={} x={:#x} x_is_arr={} nblocks={}", id, x.0, x.is_arr(), d.bl.as_ref().map_or(0, |b| b.blocks.len())));
                let bl = d.bl.as_ref().unwrap().clone();
                let psc = d.sc.as_ref().unwrap().clone();
                let body = bl.bodies[0].clone();
                crate::vm::exec_block_with_args(&bl, body, psc.clone(), &[f, x, B::SENTINEL])
            }
            DerivedKind::Md1D => {
                let modifier = d.g;
                let operand = d.f;
                if modifier.is_md1() {
                    let mid = (modifier.0 & 0xFFFFFFFFFFFF) >> 3;
                    let md = get_derived(mid);
                    if md.kind == DerivedKind::Md1Block {
                        crate::vm::vm_trace_push(format!("c1 Md1Block mid={} x={:#x}", mid, x.0));
                        let bl = md.bl.as_ref().unwrap().clone();
                        let psc = md.sc.as_ref().unwrap().clone();
                        let body = bl.bodies[0].clone();
                        return crate::vm::exec_block_with_args(
                            &bl, body, psc.clone(),
                            &[tagu64(id << 3, FUN_TAG), x, B::SENTINEL, modifier, operand],
                        );
                    }
                    if let DerivedKind::NativeMd1 { prim_idx } = &md.kind {
                        let x_ia = if x.is_arr() { crate::vm::get_arr(x).map_or(-1i64, |a| a.ia() as i64) } else { -2 };
                        crate::vm::vm_trace_push(format!("c1 NativeMd1 prim={} x={:#x} x_ia={}", prim_idx, x.0, x_ia));
                        return dispatch_native_md1_c1(*prim_idx, operand, f, x);
                    }
                    if md.kind == DerivedKind::Md2PartialL {
                        let derived_fn = m2_d(md.g, md.f, operand);
                        return c1(derived_fn, x);
                    }
                    if md.kind == DerivedKind::Md2PartialR {
                        let derived_fn = m2_d(md.g, operand, md.h);
                        return c1(derived_fn, x);
                    }
                }
                rbqn_core::error::throw("c1: unhandled md1d dispatch");
            }
            DerivedKind::Md2D => {
                let modifier = d.g;
                let operand_f = d.f;
                let operand_g = d.h;
                if modifier.is_md2() {
                    let mid = (modifier.0 & 0xFFFFFFFFFFFF) >> 3;
                    let md = get_derived(mid);
                    if md.kind == DerivedKind::Md2Block {
                        let bl = md.bl.as_ref().unwrap().clone();
                        let psc = md.sc.as_ref().unwrap().clone();
                        let body = bl.bodies[0].clone();
                        return crate::vm::exec_block_with_args(
                            &bl, body, psc.clone(),
                            &[tagu64(id << 3, FUN_TAG), x, B::SENTINEL, modifier, operand_f, operand_g],
                        );
                    }
                    if let DerivedKind::NativeMd2 { prim_idx } = &md.kind {
                        return dispatch_native_md2_c1(*prim_idx, operand_f, operand_g, f, x);
                    }
                }
                rbqn_core::error::throw("c1: unhandled md2d dispatch");
            }
            DerivedKind::NativeFn { prim_idx } => {
                let prims = rbqn_prim::get_runtime();
                let prim = &prims[prim_idx];
                let c1_fn = prim.c1.unwrap_or_else(|| {
                    rbqn_core::error::throw(format!("primitive '{}' has no monadic form", prim.glyph))
                });
                let x_arr = crate::vm::get_arr(x);
                crate::vm::vm_trace_push(format!(
                    "c1 prim={} x_tag={:#06x} x_ia={}",
                    prim.glyph,
                    (x.0 >> 48) as u16,
                    x_arr.as_ref().map_or(-1i64, |a| a.ia() as i64),
                ));
                let result = match c1_fn(x, x_arr.as_ref()) {
                    Ok(r) => r,
                    Err(e) => rbqn_core::error::throw(e.to_string()),
                };
                prim_result_to_b(result)
            }
            DerivedKind::SysFn { sys_idx } => {
                dispatch_sys_c1(sys_idx, x)
            }
            _ => rbqn_core::error::throw("c1: unhandled derived kind"),
        }
    } else if f.is_md() {
        rbqn_core::error::throw("Calling a modifier");
    } else {
        f
    }
}

pub fn c2(f: B, w: B, x: B) -> B {
    if f.is_fun() {
        let id = (f.0 & 0xFFFFFFFFFFFF) >> 3;
        let d = get_derived(id);
        match d.kind {
            DerivedKind::Fork => {
                let hx = c2(d.h, w, x);
                let fx = c2(d.f, w, x);
                c2(d.g, fx, hx)
            }
            DerivedKind::Atop => {
                let hx = c2(d.h, w, x);
                c1(d.g, hx)
            }
            DerivedKind::FunBlock => {
                let bl = d.bl.as_ref().unwrap().clone();
                let psc = d.sc.as_ref().unwrap().clone();
                let body = bl.dy_body.clone().unwrap_or_else(|| bl.bodies[0].clone());
                crate::vm::exec_block_with_args(&bl, body, psc.clone(), &[f, x, w])
            }
            DerivedKind::Md1D => {
                let modifier = d.g;
                let operand = d.f;
                if modifier.is_md1() {
                    let mid = (modifier.0 & 0xFFFFFFFFFFFF) >> 3;
                    let md = get_derived(mid);
                    if md.kind == DerivedKind::Md1Block {
                        let bl = md.bl.as_ref().unwrap().clone();
                        let psc = md.sc.as_ref().unwrap().clone();
                        let body = bl.dy_body.clone().unwrap_or_else(|| bl.bodies[0].clone());
                        return crate::vm::exec_block_with_args(
                            &bl, body, psc.clone(),
                            &[tagu64(id << 3, FUN_TAG), x, w, modifier, operand],
                        );
                    }
                    if let DerivedKind::NativeMd1 { prim_idx } = &md.kind {
                        return dispatch_native_md1_c2(*prim_idx, operand, f, w, x);
                    }
                    if md.kind == DerivedKind::Md2PartialL {
                        let derived_fn = m2_d(md.g, md.f, operand);
                        return c2(derived_fn, w, x);
                    }
                    if md.kind == DerivedKind::Md2PartialR {
                        let derived_fn = m2_d(md.g, operand, md.h);
                        return c2(derived_fn, w, x);
                    }
                }
                rbqn_core::error::throw("c2: unhandled md1d dispatch");
            }
            DerivedKind::Md2D => {
                let modifier = d.g;
                let operand_f = d.f;
                let operand_g = d.h;
                if modifier.is_md2() {
                    let mid = (modifier.0 & 0xFFFFFFFFFFFF) >> 3;
                    let md = get_derived(mid);
                    if md.kind == DerivedKind::Md2Block {
                        let bl = md.bl.as_ref().unwrap().clone();
                        let psc = md.sc.as_ref().unwrap().clone();
                        let body = bl.dy_body.clone().unwrap_or_else(|| bl.bodies[0].clone());
                        return crate::vm::exec_block_with_args(
                            &bl, body, psc.clone(),
                            &[tagu64(id << 3, FUN_TAG), x, w, modifier, operand_f, operand_g],
                        );
                    }
                    if let DerivedKind::NativeMd2 { prim_idx } = &md.kind {
                        return dispatch_native_md2_c2(*prim_idx, operand_f, operand_g, f, w, x);
                    }
                }
                rbqn_core::error::throw("c2: unhandled md2d dispatch");
            }
            DerivedKind::NativeFn { prim_idx } => {
                let prims = rbqn_prim::get_runtime();
                let prim = &prims[prim_idx];
                let c2_fn = prim.c2.unwrap_or_else(|| {
                    rbqn_core::error::throw(format!("primitive '{}' has no dyadic form", prim.glyph))
                });
                let w_arr = crate::vm::get_arr(w);
                let x_arr = crate::vm::get_arr(x);
                crate::vm::vm_trace_push(format!(
                    "c2 prim={} w_tag={:#06x} x_tag={:#06x} x_ia={}",
                    prim.glyph,
                    (w.0 >> 48) as u16,
                    (x.0 >> 48) as u16,
                    x_arr.as_ref().map_or(-1i64, |a| a.ia() as i64),
                ));
                let result = match c2_fn(w, w_arr.as_ref(), x, x_arr.as_ref()) {
                    Ok(r) => r,
                    Err(e) => {
                        let trace = crate::vm::vm_trace_dump();
                        eprintln!("=== VM TRACE (last {} ops) ===", trace.len());
                        for t in &trace { eprintln!("  {}", t); }
                        rbqn_core::error::throw(e.to_string())
                    },
                };
                prim_result_to_b(result)
            }
            DerivedKind::SysFn { sys_idx } => {
                dispatch_sys_c2(sys_idx, w, x)
            }
            _ => rbqn_core::error::throw("c2: unhandled derived kind"),
        }
    } else if f.is_md() {
        rbqn_core::error::throw("Calling a modifier");
    } else {
        f
    }
}

fn prim_result_to_b(r: rbqn_prim::PrimResult) -> B {
    match r {
        rbqn_prim::PrimResult::Scalar(b) => b,
        rbqn_prim::PrimResult::Array(arr) => crate::vm::tag_arr(arr),
    }
}

fn dispatch_native_md1_c1(prim_idx: usize, operand: B, self_val: B, x: B) -> B {
    crate::modifiers::native_md1_c1(prim_idx, operand, self_val, x)
}

fn dispatch_native_md1_c2(prim_idx: usize, operand: B, self_val: B, w: B, x: B) -> B {
    crate::modifiers::native_md1_c2(prim_idx, operand, self_val, w, x)
}

fn dispatch_native_md2_c1(prim_idx: usize, operand_f: B, operand_g: B, self_val: B, x: B) -> B {
    crate::modifiers::native_md2_c1(prim_idx, operand_f, operand_g, self_val, x)
}

fn dispatch_native_md2_c2(prim_idx: usize, operand_f: B, operand_g: B, self_val: B, w: B, x: B) -> B {
    crate::modifiers::native_md2_c2(prim_idx, operand_f, operand_g, self_val, w, x)
}

/// Convert a primitive index to a NaN-boxed B value.
/// Indices 0-43 are functions, 44-52 are 1-modifiers, 53-63 are 2-modifiers.
pub fn prim_to_b(idx: usize) -> B {
    if idx < 44 {
        m_native_fn(idx)
    } else if idx < 53 {
        m_native_md1(idx)
    } else {
        m_native_md2(idx)
    }
}

/// Dispatch system function c1.
/// System value indices: 0=Type, 1=Decompose, 4=Glyph, 5=PrimInd, 7=Fill, 22=GroupLen, 23=GroupOrd
fn dispatch_sys_c1(idx: u32, x: B) -> B {
    let x_arr = crate::vm::get_arr(x);
    match idx {
        0 => { // •Type
            let r = rbqn_prim::sysfn::type_fn(x, x_arr.as_ref());
            match r {
                Ok(pr) => prim_result_to_b(pr),
                Err(e) => rbqn_core::error::throw(e.to_string()),
            }
        }
        1 => { // •Decompose
            dispatch_sys_decompose_c1(x)
        }
        4 => { // •Glyph
            dispatch_sys_glyph_c1(x)
        }
        5 => { // •PrimInd — returns primitive index (0..63) or 64 for non-primitives
            dispatch_sys_primind_c1(x)
        }
        7 => { // •Fill / •FillFn
            let r = rbqn_prim::sysfn::fill_fn(x, x_arr.as_ref());
            match r {
                Ok(pr) => prim_result_to_b(pr),
                Err(e) => rbqn_core::error::throw(e.to_string()),
            }
        }
        22 => { // •_groupLen
            let r = rbqn_prim::group::group_len(x, x_arr.as_ref());
            match r {
                Ok(pr) => prim_result_to_b(pr),
                Err(e) => rbqn_core::error::throw(e.to_string()),
            }
        }
        23 => { // •_groupOrd
            let r = rbqn_prim::group::group_ord(x, x_arr.as_ref());
            match r {
                Ok(pr) => prim_result_to_b(pr),
                Err(e) => rbqn_core::error::throw(e.to_string()),
            }
        }
        _ => rbqn_core::error::throw(format!("system value {idx} not yet implemented (c1)")),
    }
}

/// Dispatch system function c2.
fn dispatch_sys_c2(idx: u32, w: B, x: B) -> B {
    let w_arr = crate::vm::get_arr(w);
    let x_arr = crate::vm::get_arr(x);
    match idx {
        7 => { // •_fillBy (dyadic)
            // w‿x: fill value is w, array is x. Return x with fill set to w.
            // For now: just return x (fill tracking is a future enhancement)
            x
        }
        22 => { // •_groupLen dyadic: w is desired length, x is indices
            dispatch_sys_group_len_c2(w, x, x_arr.as_ref())
        }
        23 => { // •_groupOrd dyadic: w is lengths, x is indices
            dispatch_sys_group_ord_c2(w, w_arr.as_ref(), x, x_arr.as_ref())
        }
        _ => rbqn_core::error::throw(format!("system value {idx} not yet implemented (c2)")),
    }
}

fn dispatch_sys_group_len_c2(w: B, x: B, xa: Option<&rbqn_core::BqnArr>) -> B {
    let arr = xa.unwrap_or_else(|| rbqn_core::error::throw("•GroupLen: 𝕩 must be an array"));
    let indices = arr.i32_iter().unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()));
    let n = w.o2i() as usize;
    let mut counts = vec![0i32; n];
    for &g in &indices {
        if g >= 0 && (g as usize) < n {
            counts[g as usize] += 1;
        }
    }
    prim_result_to_b(rbqn_prim::PrimResult::Array(rbqn_core::BqnArr::new_vec_i32(counts)))
}

fn dispatch_sys_group_ord_c2(w: B, wa: Option<&rbqn_core::BqnArr>, x: B, xa: Option<&rbqn_core::BqnArr>) -> B {
    let warr = wa.unwrap_or_else(|| rbqn_core::error::throw("•GroupOrd: 𝕨 must be an array"));
    let xarr = xa.unwrap_or_else(|| rbqn_core::error::throw("•GroupOrd: 𝕩 must be an array"));
    let lengths = warr.i32_iter().unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()));
    let indices = xarr.i32_iter().unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()));
    let n_groups = lengths.len();
    let mut offsets = Vec::with_capacity(n_groups + 1);
    offsets.push(0usize);
    for &l in &lengths {
        offsets.push(offsets.last().unwrap() + l as usize);
    }
    let total = *offsets.last().unwrap();
    let mut result = vec![0i32; total];
    let mut pos = offsets[..n_groups].to_vec();
    for (i, &g) in indices.iter().enumerate() {
        if g >= 0 && (g as usize) < n_groups {
            let gu = g as usize;
            if pos[gu] < offsets[gu + 1] {
                result[pos[gu]] = i as i32;
                pos[gu] += 1;
            }
        }
    }
    prim_result_to_b(rbqn_prim::PrimResult::Array(rbqn_core::BqnArr::new_vec_i32(result)))
}

/// •Decompose: return ⟨kind, value⟩ or ⟨kind, modifier, operands...⟩
fn dispatch_sys_decompose_c1(x: B) -> B {
    use rbqn_core::array::BqnArr;
    if x.is_f64() || x.is_c32() || x.is_arr() {
        // Atoms: ⟨-1, x⟩
        let arr = BqnArr::from_b_vec(vec![B::m_i32(-1), x]);
        crate::vm::tag_arr(arr)
    } else if x.is_fun() {
        let id = (x.0 & 0xFFFFFFFFFFFF) >> 3;
        let d = get_derived(id);
        match d.kind {
            DerivedKind::NativeFn { .. } | DerivedKind::SysFn { .. } | DerivedKind::FunBlock => {
                // Primitives/blocks: ⟨0, x⟩
                let arr = BqnArr::from_b_vec(vec![B::m_i32(0), x]);
                crate::vm::tag_arr(arr)
            }
            DerivedKind::Md1D => {
                // 1-modifier derived: ⟨4, m, f⟩
                let arr = BqnArr::from_b_vec(vec![B::m_i32(4), d.g, d.f]);
                crate::vm::tag_arr(arr)
            }
            DerivedKind::Md2D => {
                // 2-modifier derived: ⟨5, m, f, g⟩
                let arr = BqnArr::from_b_vec(vec![B::m_i32(5), d.g, d.f, d.h]);
                crate::vm::tag_arr(arr)
            }
            DerivedKind::Atop => {
                // Train atop: ⟨2, g, h⟩
                let arr = BqnArr::from_b_vec(vec![B::m_i32(2), d.g, d.h]);
                crate::vm::tag_arr(arr)
            }
            DerivedKind::Fork => {
                // Train fork: ⟨3, f, g, h⟩
                let arr = BqnArr::from_b_vec(vec![B::m_i32(3), d.f, d.g, d.h]);
                crate::vm::tag_arr(arr)
            }
            _ => {
                let arr = BqnArr::from_b_vec(vec![B::m_i32(-1), x]);
                crate::vm::tag_arr(arr)
            }
        }
    } else if x.is_md1() {
        let arr = BqnArr::from_b_vec(vec![B::m_i32(0), x]);
        crate::vm::tag_arr(arr)
    } else if x.is_md2() {
        let arr = BqnArr::from_b_vec(vec![B::m_i32(0), x]);
        crate::vm::tag_arr(arr)
    } else {
        let arr = BqnArr::from_b_vec(vec![B::m_i32(-1), x]);
        crate::vm::tag_arr(arr)
    }
}

/// •Glyph: return the glyph character for a primitive, or empty string
fn dispatch_sys_glyph_c1(x: B) -> B {
    if x.is_fun() {
        let id = (x.0 & 0xFFFFFFFFFFFF) >> 3;
        let d = get_derived(id);
        if let DerivedKind::NativeFn { prim_idx } = d.kind {
            let prims = rbqn_prim::get_runtime();
            if prim_idx < prims.len() {
                let glyph = prims[prim_idx].glyph;
                let chars: Vec<u32> = glyph.chars().map(|c| c as u32).collect();
                let arr = rbqn_core::array::BqnArr::new_vec_c32(chars);
                return crate::vm::tag_arr(arr);
            }
        }
    } else if x.is_md1() || x.is_md2() {
        let id = (x.0 & 0xFFFFFFFFFFFF) >> 3;
        let d = get_derived(id);
        let prim_idx = match d.kind {
            DerivedKind::NativeMd1 { prim_idx } => Some(prim_idx),
            DerivedKind::NativeMd2 { prim_idx } => Some(prim_idx),
            _ => None,
        };
        if let Some(idx) = prim_idx {
            let prims = rbqn_prim::get_runtime();
            if idx < prims.len() {
                let glyph = prims[idx].glyph;
                let chars: Vec<u32> = glyph.chars().map(|c| c as u32).collect();
                let arr = rbqn_core::array::BqnArr::new_vec_c32(chars);
                return crate::vm::tag_arr(arr);
            }
        }
    }
    // Non-primitive: return empty string
    let arr = rbqn_core::array::BqnArr::new_vec_c32(vec![]);
    crate::vm::tag_arr(arr)
}

/// •PrimInd: return the primitive index (0..63) for a primitive, or 64 for non-primitives.
/// CBQN: primInd_c1 in sysfn.c
fn dispatch_sys_primind_c1(x: B) -> B {
    const RT_LEN: i32 = 64;
    if x.is_fun() {
        let id = (x.0 & 0xFFFFFFFFFFFF) >> 3;
        let d = get_derived(id);
        if let DerivedKind::NativeFn { prim_idx } = d.kind {
            return B::m_i32(prim_idx as i32);
        }
    } else if x.is_md1() || x.is_md2() {
        let id = (x.0 & 0xFFFFFFFFFFFF) >> 3;
        let d = get_derived(id);
        match d.kind {
            DerivedKind::NativeMd1 { prim_idx } => return B::m_i32(prim_idx as i32),
            DerivedKind::NativeMd2 { prim_idx } => return B::m_i32(prim_idx as i32),
            _ => {}
        }
    }
    B::m_i32(RT_LEN)
}
