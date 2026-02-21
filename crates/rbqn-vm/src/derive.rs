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
                let gx = c1(d.g, hx);
                c2(d.f, B::m_f64(0.0), gx) // TODO: proper fork c1
            }
            DerivedKind::Atop => {
                let hx = c1(d.h, x);
                c1(d.g, hx)
            }
            DerivedKind::FunBlock => {
                let bl = d.bl.as_ref().unwrap().clone();
                let psc = d.sc.as_ref().unwrap().clone();
                let body = bl.bodies[0].clone();
                crate::vm::exec_block_with_args(&bl, body, &psc, &[f, x, B::SENTINEL])
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
                        let body = bl.bodies[0].clone();
                        return crate::vm::exec_block_with_args(
                            &bl, body, &psc,
                            &[tagu64(id << 3, FUN_TAG), x, B::SENTINEL, modifier, operand],
                        );
                    }
                    if let DerivedKind::NativeMd1 { prim_idx } = &md.kind {
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
                            &bl, body, &psc,
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
                let result = match c1_fn(x, x_arr.as_ref()) {
                    Ok(r) => r,
                    Err(e) => rbqn_core::error::throw(e.to_string()),
                };
                prim_result_to_b(result)
            }
            DerivedKind::SysFn { sys_idx } => {
                dispatch_sys_c1(*sys_idx, x)
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
                crate::vm::exec_block_with_args(&bl, body, &psc, &[f, x, w])
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
                            &bl, body, &psc,
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
                            &bl, body, &psc,
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
                let result = match c2_fn(w, w_arr.as_ref(), x, x_arr.as_ref()) {
                    Ok(r) => r,
                    Err(e) => rbqn_core::error::throw(e.to_string()),
                };
                prim_result_to_b(result)
            }
            DerivedKind::SysFn { sys_idx } => {
                dispatch_sys_c2(*sys_idx, w, x)
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
