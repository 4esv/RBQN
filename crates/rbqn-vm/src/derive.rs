use std::sync::Arc;

use rbqn_core::{B, FUN_TAG, MD1_TAG, MD2_TAG, tagu64};

use crate::block::Block;
use crate::scope::Scope;

// --- Global state for system functions ---

/// Global BQN runtime state for •BQN re-evaluation.
/// Set after bootstrap completes.
pub static SYS_RUNTIME: std::sync::LazyLock<Mutex<Option<SysRuntime>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// Snapshot of what •BQN needs to evaluate code.
pub struct SysRuntime {
    pub compiler: B,
    pub runtime: Vec<B>,
    pub formatter: Option<(B, B)>,
}

// NOTE: B contains a u64 which is Send-safe; all B values are NaN-boxed pointers or scalars.
// The Arc<Derived> data structure is immutable after creation.
unsafe impl Send for SysRuntime {}

/// Set the global runtime state for •BQN (called after bootstrap).
pub fn set_sys_runtime(compiler: B, runtime: Vec<B>, formatter: Option<(B, B)>) {
    *SYS_RUNTIME.lock().unwrap_or_else(|e| e.into_inner()) = Some(SysRuntime {
        compiler,
        runtime,
        formatter,
    });
}

/// Global •args value — set before executing any user code.
pub static SYS_ARGS: std::sync::LazyLock<Mutex<Option<B>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// Global •path value — set from the file path being executed, or "" for -e/-p.
pub static SYS_PATH: std::sync::LazyLock<Mutex<Option<B>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// Global •name value — basename of current file, or "" for -e/-p.
pub static SYS_NAME: std::sync::LazyLock<Mutex<Option<B>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// Set •args from a slice of strings.
pub fn set_sys_args(args: &[String]) {
    let arr = rbqn_core::array::BqnArr::from_b_vec(
        args.iter().map(|s| {
            let chars: Vec<u32> = s.chars().map(|c| c as u32).collect();
            crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
        }).collect()
    );
    *SYS_ARGS.lock().unwrap_or_else(|e| e.into_inner()) = Some(crate::vm::tag_arr(arr));
}

/// Set •path and •name from the executing file path.
pub fn set_sys_path(path: &str) {
    let path_chars: Vec<u32> = path.chars().map(|c| c as u32).collect();
    let path_b = crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(path_chars));
    *SYS_PATH.lock().unwrap_or_else(|e| e.into_inner()) = Some(path_b);

    // Compute name as basename
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let name_chars: Vec<u32> = name.chars().map(|c| c as u32).collect();
    let name_b = crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(name_chars));
    *SYS_NAME.lock().unwrap_or_else(|e| e.into_inner()) = Some(name_b);
}

// NOTE: BQN primitive glyphs in fruntime order (0-63).
// Used for human-readable trace output when RBQN_PRIM_TRACE is set.
const PRIM_GLYPHS: &str = "+-×÷⋆√⌊⌈|¬∧∨<>≠=≤≥≡≢⊣⊢⥊∾≍⋈↑↓↕«»⌽⍉/⍋⍒⊏⊑⊐⊒∊⍷⊔!˙˜˘¨⌜⁼´˝`∘○⊸⟜⌾⊘◶⎉⚇⍟⎊";

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
    LazyInvReg,   // Lazy inverse-reg wrapper: f = original function
    LazyInvSwap,  // Lazy inverse-swap wrapper: f = original function
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
    // NOTE: Use unwrap_or_else to recover from poisoned mutex (caused by catch_unwind)
    DERIVED_STORE.lock().unwrap_or_else(|e| e.into_inner()).insert(id, Arc::new(d));
    id
}

pub fn get_derived(id: u64) -> Arc<Derived> {
    DERIVED_STORE.lock().unwrap_or_else(|e| e.into_inner()).get(&id).cloned()
        .unwrap_or_else(|| rbqn_core::error::throw("Invalid derived object reference"))
}

use std::collections::HashMap;
use std::sync::Mutex;

pub static DERIVED_STORE: std::sync::LazyLock<Mutex<HashMap<u64, Arc<Derived>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

// Global inverse lookup functions, set by setInv callback during bootstrap.
// INV_REG_FN: called as c1(inv_reg_fn, func) to get the regular inverse of func
// INV_SWAP_FN: called as c1(inv_swap_fn, func) to get the swap inverse of func
static INV_REG_FN: std::sync::LazyLock<Mutex<Option<B>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));
static INV_SWAP_FN: std::sync::LazyLock<Mutex<Option<B>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// Store the BQN inverse lookup function (called from setInvReg system fn).
pub fn set_inv_reg_fn(f: B) {
    *INV_REG_FN.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
}

/// Store the BQN inverse swap function (called from setInvSwap system fn).
pub fn set_inv_swap_fn(f: B) {
    *INV_SWAP_FN.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
}

/// Look up the regular inverse of a function using the BQN runtime's inverse tables.
pub fn inv_reg(func: B) -> B {
    if func.is_fun() {
        let id = (func.0 & 0xFFFFFFFFFFFF) >> 3;
        let d = get_derived(id);

        // Fast path: native primitive inverse
        if let DerivedKind::NativeFn { prim_idx } = d.kind {
            if let Some(inv) = native_inverse_reg(prim_idx) {
                return inv;
            }
        }

        // NOTE: Md2D inverses (e.g., val⊸⊏⁼) are handled by the BQN runtime's
        // inverse resolver (INV_REG_FN), not natively. The native md2d_inverse_reg
        // was computing wrong results (e.g., val⊸(⊏⁼) instead of val⊏˜⁼).
        // CBQN uses fn_ix dispatch tables which we don't have, so we fall through.
    }

    // Fall through to BQN runtime resolver
    let reg_fn = INV_REG_FN.lock().unwrap_or_else(|e| e.into_inner()).unwrap_or_else(||
        rbqn_core::error::throw("⁼: inverse system not initialized (setInv not called)")
    );
    c1(reg_fn, func)
}

/// Native inverse for Md2D (2-modifier derived) values.
/// Matches CBQN's before_im and after_im in md2.c.
fn md2d_inverse_reg(d: &Derived) -> Option<B> {
    // d.g = modifier, d.f = left operand, d.h = right operand
    let modifier = d.g;
    if !modifier.is_md2() { return None; }

    let mid = (modifier.0 & 0xFFFFFFFFFFFF) >> 3;
    let md = get_derived(mid);

    match md.kind {
        DerivedKind::NativeMd2 { prim_idx: 55 } => {
            // ⊸ (before): (F⊸G)⁻¹ x = F G⁻¹ₓ x (dyadic inverse of G with F as w)
            // Only when F is a value (not callable) and G is a function.
            // CBQN: before_im(d,x) = isFun(d->g) && !isCallable(d->f)
            //       ? TI(d->g, fn_ix)(d->g, d->f, x) : def_m2_im(d, x)
            let f_operand = d.f;  // left operand of ⊸
            let g_operand = d.h;  // right operand of ⊸
            if g_operand.is_fun() && !f_operand.is_fun() && !f_operand.is_md() {
                // (val⊸G)⁻¹ = val⊸(inv_reg(G))
                // When called as c1(result, x), ⊸ dispatcher computes:
                //   c1(val, x) = val (constant), then c2(inv_reg(G), val, x)
                let g_inv = inv_reg(g_operand);
                let before_md2 = m_native_md2(55); // ⊸
                return Some(m_md2d(before_md2, f_operand, g_inv));
            }
            None
        }
        DerivedKind::NativeMd2 { prim_idx: 56 } => {
            // ⟜ (after): (F⟜G)⁻¹ x → only when G is a value
            // CBQN: after_im(d,x) = isFun(d->f) && !isCallable(d->g)
            //       ? TI(d->f, fn_iw)(d->f, d->g, x) : def_m2_im(d, x)
            let f_operand = d.f;
            let g_operand = d.h;
            if f_operand.is_fun() && !g_operand.is_fun() && !g_operand.is_md() {
                // (F⟜val)⁻¹ = (inv_swap(F))⟜val
                // When called as c1(result, x), ⟜ dispatcher computes:
                //   c1(val, x) = val, then c2(inv_swap(F), x, val) = swap-inverse
                let f_inv = inv_swap(f_operand);
                let after_md2 = m_native_md2(56); // ⟜
                return Some(m_md2d(after_md2, f_inv, g_operand));
            }
            None
        }
        _ => None,
    }
}

/// Look up the swap inverse of a function using the BQN runtime's inverse tables.
pub fn inv_swap(func: B) -> B {
    // Check for known native swap inverses first
    if func.is_fun() {
        let id = (func.0 & 0xFFFFFFFFFFFF) >> 3;
        let d = get_derived(id);
        if let DerivedKind::NativeFn { prim_idx } = d.kind {
            if let Some(inv) = native_inverse_swap(prim_idx) {
                return inv;
            }
        }
    }
    let swap_fn = INV_SWAP_FN.lock().unwrap_or_else(|e| e.into_inner()).unwrap_or_else(||
        rbqn_core::error::throw("⁼: inverse system not initialized (setInv not called)")
    );
    c1(swap_fn, func)
}

/// Known inverses for native primitives (regular inverse: F⁼).
/// Returns the inverse function as a B value, or None if not known.
/// fruntime layout: 0:+ 1:- 2:× 3:÷ 4:⋆ 5:√ 6:⌊ 7:⌈ 8:| 9:¬
///   10:∧ 11:∨ 12:< 13:> 20:⊣ 21:⊢ 22:⥊ 31:⌽ 32:⍉
fn native_inverse_reg(prim_idx: usize) -> Option<B> {
    // NOTE: These are monadic inverses (F⁼ x = inverse of F applied to x)
    // For dyadic F: w F⁼ x means "find y such that w F y = x"
    match prim_idx {
        0 => Some(m_native_fn(0)),   // +⁼ = + (identity for monadic)
        1 => Some(m_native_fn(1)),   // -⁼ = - (negate is its own inverse)
        2 => Some(m_native_fn(3)),   // ×⁼ = ÷ (monadic: sign → reciprocal... approximate)
        3 => Some(m_native_fn(3)),   // ÷⁼ = ÷ (reciprocal is its own inverse)
        4 => Some(m_native_fn(4)),   // ⋆⁼ = log (use ⋆ with inverse semantics)
        5 => Some(m_native_fn(4)),   // √⁼ = ⋆ (square is inverse of sqrt)
        9 => Some(m_native_fn(9)),   // ¬⁼ = ¬ (not is its own inverse)
        12 => Some(m_native_fn(13)), // <⁼ = > (unbox)
        13 => Some(m_native_fn(12)), // >⁼ = < (box)
        20 => Some(m_native_fn(20)), // ⊣⁼ = ⊣
        21 => Some(m_native_fn(21)), // ⊢⁼ = ⊢
        31 => Some(m_native_fn(31)), // ⌽⁼ = ⌽ (reverse is its own inverse)
        32 => Some(m_native_fn(32)), // ⍉⁼ = ⍉ (transpose is its own inverse for rank≤2)
        33 => Some(m_sys_fn(201)),    // /⁼ = inverse of indices (counts from sorted indices)
        37 => Some(m_native_fn(24)), // ⊑⁼ = ≍ (solo: first inverse wraps in 1-element array)
        _ => None,
    }
}

/// Known swap inverses for native primitives (w F˜⁼ x or similar).
fn native_inverse_swap(prim_idx: usize) -> Option<B> {
    match prim_idx {
        0 => Some(m_native_fn(1)),   // w+˜⁼x = x-w → subtract
        1 => Some(m_native_fn(1)),   // w-˜⁼x = x-w → subtract (same)
        2 => Some(m_native_fn(3)),   // w×˜⁼x = x÷w → divide
        3 => Some(m_native_fn(2)),   // w÷˜⁼x = x×w → multiply
        4 => Some(m_native_fn(5)),   // w⋆˜⁼x = w√x → root
        _ => None,
    }
}

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

/// Create a lazy inverse-reg wrapper. When called (c1 or c2), resolves the inverse
/// of `original` using the BQN runtime's inverse tables, then calls it.
pub fn m_lazy_inv_reg(original: B) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::LazyInvReg,
        f: original, g: B::SENTINEL, h: B::SENTINEL,
        bl: None, sc: None,
    });
    tagu64(id << 3, FUN_TAG)
}

/// Create a lazy inverse-swap wrapper.
pub fn m_lazy_inv_swap(original: B) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::LazyInvSwap,
        f: original, g: B::SENTINEL, h: B::SENTINEL,
        bl: None, sc: None,
    });
    tagu64(id << 3, FUN_TAG)
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
        // NOTE: For immediate 1-modifier blocks (imm=true), execute the block
        // immediately with args [𝕣, 𝕗]. This matches CBQN's md1Bl_d which calls
        // execBlock(bl, bl->bodies[0], sc, 2, {m, f}) for immediate modifiers.
        let mid = (m.0 & 0xFFFFFFFFFFFF) >> 3;
        let md = get_derived(mid);
        if md.kind == DerivedKind::Md1Block {
            let bl = md.bl.as_ref().unwrap().clone();
            if bl.imm {
                let psc = md.sc.as_ref().unwrap().clone();
                let body = bl.bodies[0].clone();
                return crate::vm::exec_block_with_args(&bl, body, psc.clone(), &[m, f]);
            }
        }
        m_md1d(m, f)
    } else {
        rbqn_core::error::throw("Interpreting non-1-modifier as 1-modifier");
    }
}

pub fn m2_d(m: B, f: B, g: B) -> B {
    if m.is_md2() {
        // NOTE: For immediate 2-modifier blocks (imm=true), execute the block
        // immediately with args [𝕣, 𝕗, 𝕘]. This matches CBQN's md2Bl_d which calls
        // execBlock(bl, bl->bodies[0], sc, 3, {m, f, g}) for immediate modifiers.
        let mid = (m.0 & 0xFFFFFFFFFFFF) >> 3;
        let md = get_derived(mid);
        if md.kind == DerivedKind::Md2Block {
            let bl = md.bl.as_ref().unwrap().clone();
            if bl.imm {
                let psc = md.sc.as_ref().unwrap().clone();
                let body = bl.bodies[0].clone();
                return crate::vm::exec_block_with_args(&bl, body, psc.clone(), &[m, f, g]);
            }
        }
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
                if crate::vm::prim_trace_enabled() {
                    eprintln!("[BLOCK c1] id={} x={}", id, crate::vm::fmt_b_short(x));
                }
                let bl = d.bl.as_ref().unwrap().clone();
                let psc = d.sc.as_ref().unwrap().clone();
                let body = bl.bodies[0].clone();
                let result = crate::vm::exec_block_with_args(&bl, body, psc.clone(), &[f, x, B::SENTINEL]);
                if crate::vm::prim_trace_enabled() {
                    eprintln!("[BLOCK c1] id={} -> {}", id, crate::vm::fmt_b_short(result));
                }
                result
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
                if crate::vm::prim_trace_enabled() {
                    let glyph = PRIM_GLYPHS.chars().nth(prim_idx).map(|c| c.to_string())
                        .unwrap_or_else(|| prim.glyph.to_string());
                    eprintln!("[PRIM c1] {} x={}", glyph, crate::vm::fmt_b_short(x));
                }
                let result = match c1_fn(x, x_arr.as_ref()) {
                    Ok(r) => r,
                    Err(e) => rbqn_core::error::throw(e.to_string()),
                };
                let result_b = prim_result_to_b(result);
                if crate::vm::prim_trace_enabled() {
                    let glyph = PRIM_GLYPHS.chars().nth(prim_idx).map(|c| c.to_string())
                        .unwrap_or_else(|| prim.glyph.to_string());
                    eprintln!("[PRIM c1] {} -> {}", glyph, crate::vm::fmt_b_detail(result_b));
                }
                result_b
            }
            DerivedKind::SysFn { sys_idx } => {
                if crate::vm::prim_trace_enabled() {
                    eprintln!("[SYS c1] sys={} x={}", sys_idx, crate::vm::fmt_b_short(x));
                }
                let result = dispatch_sys_c1(sys_idx, x);
                if crate::vm::prim_trace_enabled() {
                    eprintln!("[SYS c1] sys={} -> {}", sys_idx, crate::vm::fmt_b_detail(result));
                }
                result
            }
            DerivedKind::LazyInvReg => {
                // Lazy inverse-reg: d.f is the original function, find its inverse and apply
                let inv_fn = inv_reg(d.f);
                c1(inv_fn, x)
            }
            DerivedKind::LazyInvSwap => {
                // Lazy inverse-swap: d.f is the original function, find its swap inverse and apply
                let inv_fn = inv_swap(d.f);
                c1(inv_fn, x)
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
                if crate::vm::prim_trace_enabled() {
                    eprintln!("[BLOCK c2] id={} w={} x={}", id, crate::vm::fmt_b_short(w), crate::vm::fmt_b_short(x));
                }
                let bl = d.bl.as_ref().unwrap().clone();
                let psc = d.sc.as_ref().unwrap().clone();
                let body = bl.dy_body.clone().unwrap_or_else(|| bl.bodies[0].clone());
                let result = crate::vm::exec_block_with_args(&bl, body, psc.clone(), &[f, x, w]);
                if crate::vm::prim_trace_enabled() {
                    eprintln!("[BLOCK c2] id={} -> {}", id, crate::vm::fmt_b_short(result));
                }
                result
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
                if crate::vm::prim_trace_enabled() {
                    let glyph = PRIM_GLYPHS.chars().nth(prim_idx).map(|c| c.to_string())
                        .unwrap_or_else(|| prim.glyph.to_string());
                    eprintln!("[PRIM c2] w={} {} x={} (prim_idx={})",
                        crate::vm::fmt_b_short(w), glyph, crate::vm::fmt_b_short(x), prim_idx);
                }
                let result = match c2_fn(w, w_arr.as_ref(), x, x_arr.as_ref()) {
                    Ok(r) => r,
                    Err(e) => {
                        if crate::vm::prim_trace_enabled() {
                            let trace = crate::vm::vm_trace_dump();
                            eprintln!("=== VM TRACE (last {} ops) ===", trace.len());
                            for t in &trace { eprintln!("  {}", t); }
                        }
                        rbqn_core::error::throw(e.to_string())
                    },
                };
                let result_b = prim_result_to_b(result);
                if crate::vm::prim_trace_enabled() {
                    let glyph = PRIM_GLYPHS.chars().nth(prim_idx).map(|c| c.to_string())
                        .unwrap_or_else(|| prim.glyph.to_string());
                    eprintln!("[PRIM c2] {} -> {}", glyph, crate::vm::fmt_b_detail(result_b));
                }
                result_b
            }
            DerivedKind::SysFn { sys_idx } => {
                if crate::vm::prim_trace_enabled() {
                    eprintln!("[SYS c2] sys={} w={} x={}", sys_idx, crate::vm::fmt_b_short(w), crate::vm::fmt_b_short(x));
                }
                let result = dispatch_sys_c2(sys_idx, w, x);
                if crate::vm::prim_trace_enabled() {
                    eprintln!("[SYS c2] sys={} -> {}", sys_idx, crate::vm::fmt_b_detail(result));
                }
                result
            }
            DerivedKind::LazyInvReg => {
                let inv_fn = inv_reg(d.f);
                c2(inv_fn, w, x)
            }
            DerivedKind::LazyInvSwap => {
                let inv_fn = inv_swap(d.f);
                c2(inv_fn, w, x)
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
/// System value indices: 0=Type, 1=Decompose, 4=Glyph, 5=PrimInd, 7=Fill,
///   8=setInvReg, 9=setInvSwap, 10=nativeInvReg, 11=nativeInvSwap,
///   22=GroupLen, 23=GroupOrd
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
        8 => { // setInvReg: stores x (a BQN function) as the inverse-reg resolver,
               // returns nativeInvReg (sys_idx=10)
            set_inv_reg_fn(x);
            m_sys_fn(10)
        }
        9 => { // setInvSwap: stores x as the inverse-swap resolver,
               // returns nativeInvSwap (sys_idx=11)
            set_inv_swap_fn(x);
            m_sys_fn(11)
        }
        10 => { // nativeInvReg: wraps x so that calling the result computes x's inverse.
            // In CBQN: wraps ALL functions with a lazy inverse resolver.
            // For native primitives with known inverses, return the inverse directly.
            // For everything else, return a wrapper that calls the BQN inverse lookup lazily.
            if x.is_fun() {
                let id = (x.0 & 0xFFFFFFFFFFFF) >> 3;
                let d = get_derived(id);
                if let DerivedKind::NativeFn { prim_idx } = d.kind {
                    if let Some(inv) = native_inverse_reg(prim_idx) {
                        return inv;
                    }
                }
            }
            // Create a lazy inverse wrapper: a Derived that stores the original function
            // and computes the inverse when called.
            m_lazy_inv_reg(x)
        }
        11 => { // nativeInvSwap: wraps x for swap inverse lookup
            if x.is_fun() {
                let id = (x.0 & 0xFFFFFFFFFFFF) >> 3;
                let d = get_derived(id);
                if let DerivedKind::NativeFn { prim_idx } = d.kind {
                    if let Some(inv) = native_inverse_swap(prim_idx) {
                        return inv;
                    }
                }
            }
            m_lazy_inv_swap(x)
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
        // NOTE: •ReBQN (alias for •BQN for now)
        31 => {
            let src = b_to_string(x);
            dispatch_sys_bqn_eval(&src)
        }
        // NOTE: •Out
        32 => {
            // c1: print x (must be a string — char array) to stdout with newline, return x.
            let s = b_to_string(x);
            println!("{}", s);
            x
        }
        // NOTE: •Show
        33 => {
            // c1: format x and print to stderr with newline, return x.
            let s = format_b_for_show(x);
            eprintln!("{}", s);
            x
        }
        // NOTE: •BQN
        30 => {
            // c1: evaluate BQN source string x, return result.
            let src = b_to_string(x);
            dispatch_sys_bqn_eval(&src)
        }
        // NOTE: •Fmt
        34 => {
            // c1: format x as a BQN value string, return as char array.
            let s = format_b_for_show(x);
            let chars: Vec<u32> = s.chars().map(|c| c as u32).collect();
            crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
        }
        // NOTE: •Repr
        35 => {
            // c1: return the BQN source representation of x (quoted string for strings, etc.)
            let s = format_b_repr(x);
            let chars: Vec<u32> = s.chars().map(|c| c as u32).collect();
            crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
        }
        // NOTE: •Exit
        36 => {
            // c1: terminate the process with exit code x (must be a number)
            let code = if x.is_f64() { x.o2f() as i32 } else { 0 };
            std::process::exit(code);
        }
        // NOTE: •args
        37 => {
            // Return command-line arguments as a list of strings
            SYS_ARGS.lock().unwrap_or_else(|e| e.into_inner())
                .unwrap_or_else(|| crate::vm::tag_arr(rbqn_core::array::BqnArr::empty_harr()))
        }
        // NOTE: •path
        38 => {
            // Return the path of the current file, or "" for -e/-p
            SYS_PATH.lock().unwrap_or_else(|e| e.into_inner())
                .unwrap_or_else(|| {
                    crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(vec![]))
                })
        }
        // NOTE: •name
        39 => {
            // Return the name (basename) of the current file, or "" for -e/-p
            SYS_NAME.lock().unwrap_or_else(|e| e.into_inner())
                .unwrap_or_else(|| {
                    crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(vec![]))
                })
        }
        // NOTE: •wdpath
        40 => {
            // Return the current working directory as a string
            let wd = std::env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let chars: Vec<u32> = wd.chars().map(|c| c as u32).collect();
            crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
        }
        // NOTE: •state (placeholder namespace)
        41 => {
            // Return a placeholder — empty namespace-like value
            // For now return SENTINEL (nothing) since full namespace support is deferred
            B::SENTINEL
        }
        100 => { // System value name resolver
            // The compiler calls System(names) to resolve system value names.
            // x is either:
            //   - An empty array: return it unchanged (no system values needed)
            //   - An array of name strings: return an array of B values (functions or values)
            //     each corresponding to the named system value.
            if !x.is_arr() {
                return B::SENTINEL;
            }
            let arr = match crate::vm::get_arr(x) {
                Some(a) => a,
                None => return x,
            };
            if arr.ia() == 0 {
                return x;
            }
            // Each element is a char array (system value name, lowercase)
            let mut results = Vec::with_capacity(arr.ia());
            for i in 0..arr.ia() {
                let name_b = arr.get(i).unwrap_or(B::SENTINEL);
                let name = b_to_string(name_b);
                let val = sys_name_to_b(&name);
                results.push(val);
            }
            crate::vm::b_vec_to_arr(results)
        }
        201 => { // Internal: /⁼ (inverse of indices)
            let r = rbqn_prim::slash::indices_inverse_c1(x, x_arr.as_ref());
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
        5 => { // •PrimInd dyadic: w is max value (unused), just return primind of x
            dispatch_sys_primind_c1(x)
        }
        7 => { // •_fillBy (dyadic)
            // w‿x: fill value is w, array is x. Return x with fill set to w.
            // For now: just return x (fill tracking is a future enhancement)
            x
        }
        10 => { // nativeInvReg dyadic: w F⁻¹ x → look up inverse-reg of w, call with w,x
            let inv_fn = inv_reg(w);
            c2(inv_fn, w, x)
        }
        11 => { // nativeInvSwap dyadic: similar for swap inverse
            let inv_fn = inv_swap(w);
            c2(inv_fn, w, x)
        }
        22 => { // •_groupLen dyadic: w is desired length, x is indices
            dispatch_sys_group_len_c2(w, x, x_arr.as_ref())
        }
        23 => { // •_groupOrd dyadic: w is lengths, x is indices
            dispatch_sys_group_ord_c2(w, w_arr.as_ref(), x, x_arr.as_ref())
        }
        100 => { // •BQN placeholder — just return SENTINEL for now
            B::SENTINEL
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
            DerivedKind::NativeFn { .. } | DerivedKind::SysFn { .. } => {
                // Primitives/system fns: ⟨0, x⟩
                let arr = BqnArr::from_b_vec(vec![B::m_i32(0), x]);
                crate::vm::tag_arr(arr)
            }
            DerivedKind::FunBlock => {
                // Block functions: ⟨1, x⟩ (CBQN block_decompose returns 1)
                let arr = BqnArr::from_b_vec(vec![B::m_i32(1), x]);
                crate::vm::tag_arr(arr)
            }
            DerivedKind::Md1D => {
                // 1-modifier derived: ⟨4, f_operand, modifier⟩
                // NOTE: CBQN order is ⟨type, f, m1⟩ — operand first, modifier second
                let arr = BqnArr::from_b_vec(vec![B::m_i32(4), d.f, d.g]);
                crate::vm::tag_arr(arr)
            }
            DerivedKind::Md2D => {
                // 2-modifier derived: ⟨5, f_operand, modifier, g_operand⟩
                // NOTE: CBQN order is ⟨type, f, m2, g⟩
                let arr = BqnArr::from_b_vec(vec![B::m_i32(5), d.f, d.g, d.h]);
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

// --- System value name resolver ---

/// Map a system value name (lowercase) to its B value.
/// Returns SENTINEL for unknown names (signals to BQN runtime: value unavailable).
fn sys_name_to_b(name: &str) -> B {
    match name {
        // Basic system values (indices matching CBQN sysfn numbering)
        "type"      => m_sys_fn(0),
        "decompose" => m_sys_fn(1),
        "glyph"     => m_sys_fn(4),
        "primind"   => m_sys_fn(5),
        "fill"      => m_sys_fn(7),
        // Runtime I/O and evaluation (callable functions)
        "bqn"       => m_sys_fn(30),
        "rebqn"     => m_sys_fn(31),  // NOTE: alias for •BQN for now
        "out"       => m_sys_fn(32),
        "show"      => m_sys_fn(33),
        "fmt"       => m_sys_fn(34),
        "repr"      => m_sys_fn(35),
        "exit"      => m_sys_fn(36),
        // Environment values (returned as immediate values, not functions)
        "args"      => dispatch_sys_env(37),
        "path"      => dispatch_sys_env(38),
        "name"      => dispatch_sys_env(39),
        "wdpath"    => dispatch_sys_env(40),
        "state"     => dispatch_sys_env(41),
        _ => B::SENTINEL,
    }
}

/// Return environment value for the given sys_idx.
/// Used both from sys_name_to_b and from sysv_lookup for immediate env values.
pub fn dispatch_sys_env(idx: u32) -> B {
    match idx {
        37 => { // •args
            SYS_ARGS.lock().unwrap_or_else(|e| e.into_inner())
                .unwrap_or_else(|| crate::vm::tag_arr(rbqn_core::array::BqnArr::empty_harr()))
        }
        38 => { // •path
            SYS_PATH.lock().unwrap_or_else(|e| e.into_inner())
                .unwrap_or_else(|| crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(vec![])))
        }
        39 => { // •name
            SYS_NAME.lock().unwrap_or_else(|e| e.into_inner())
                .unwrap_or_else(|| crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(vec![])))
        }
        40 => { // •wdpath
            let wd = std::env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let chars: Vec<u32> = wd.chars().map(|c| c as u32).collect();
            crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
        }
        41 => B::SENTINEL, // •state placeholder
        _ => B::SENTINEL,
    }
}

/// Convert a B value (character array) to a Rust String.
/// Returns empty string if x is not a character array.
fn b_to_string(x: B) -> String {
    if x.is_arr() {
        if let Some(arr) = crate::vm::get_arr(x) {
            let chars: String = (0..arr.ia())
                .filter_map(|i| arr.get(i).ok())
                .filter_map(|b| {
                    if b.is_c32() { char::from_u32(b.0 as u32) } else { None }
                })
                .collect();
            return chars;
        }
    }
    String::new()
}

/// Format a B value for •Show/•Fmt — tries the bootstrap formatter first,
/// falls back to the basic debug format.
fn format_b_for_show(x: B) -> String {
    // Try to use the bootstrap formatter (rt.formatter.0) if available
    let fmt_fn = {
        let guard = SYS_RUNTIME.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().and_then(|rt| rt.formatter.map(|(f, _)| f))
    };
    if let Some(fmt) = fmt_fn {
        if let Ok(result) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            c1(fmt, x)
        })) {
            let s = b_to_string(result);
            if !s.is_empty() {
                return s;
            }
        }
    }
    // Fallback: basic value description
    crate::vm::fmt_b_detail(x)
}

/// Format a B value for •Repr — quoted strings for strings, numbers as-is, etc.
fn format_b_repr(x: B) -> String {
    // Try to use the bootstrap repr function (rt.formatter.1) if available
    let repr_fn = {
        let guard = SYS_RUNTIME.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().and_then(|rt| rt.formatter.map(|(_, r)| r))
    };
    if let Some(repr) = repr_fn {
        if let Ok(result) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            c1(repr, x)
        })) {
            let s = b_to_string(result);
            if !s.is_empty() {
                return s;
            }
        }
    }
    // Fallback
    format_b_for_show(x)
}

/// Execute a BQN source string using the stored global runtime.
/// This is the implementation of •BQN.
fn dispatch_sys_bqn_eval(src: &str) -> B {
    // Retrieve the global runtime state
    let (compiler, runtime, formatter) = {
        let guard = SYS_RUNTIME.lock().unwrap_or_else(|e| e.into_inner());
        match guard.as_ref() {
            Some(rt) => (rt.compiler, rt.runtime.clone(), rt.formatter),
            None => rbqn_core::error::throw("•BQN: runtime not initialized"),
        }
    };

    if compiler.q_n() || compiler.0 == B::SENTINEL.0 {
        rbqn_core::error::throw("•BQN: compiler not available");
    }

    // Build compiler arguments (same as exec_string_inner in main.rs)
    let rt_arr = crate::vm::tag_arr(rbqn_core::array::BqnArr::from_b_vec(runtime));
    let sys_fn = m_sys_fn(100);
    let var_names = crate::vm::tag_arr(rbqn_core::array::BqnArr::empty_harr());
    let var_depths = crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_i32(vec![]));
    let comp_args = crate::vm::tag_arr(rbqn_core::array::BqnArr::from_b_vec(
        vec![rt_arr, sys_fn, var_names, var_depths]
    ));

    let src_chars: Vec<u32> = src.chars().map(|c| c as u32).collect();
    let src_b = crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(src_chars));

    // Call compiler: compiler(comp_args, src_b) → ⟨bc, objs, blocks, bodies, ...⟩
    let comp_result = c2(compiler, comp_args, src_b);

    let comp_arr = match crate::vm::get_arr(comp_result) {
        Some(a) => a,
        None => rbqn_core::error::throw("•BQN: compiler did not return an array"),
    };

    let bc_b = comp_arr.get(0).unwrap_or_else(|_| rbqn_core::error::throw("•BQN: missing bc"));
    let objs_b = comp_arr.get(1).unwrap_or_else(|_| rbqn_core::error::throw("•BQN: missing objs"));
    let blocks_b = comp_arr.get(2).unwrap_or_else(|_| rbqn_core::error::throw("•BQN: missing blocks"));
    let bodies_b = comp_arr.get(3).unwrap_or_else(|_| rbqn_core::error::throw("•BQN: missing bodies"));

    let indices_b = comp_arr.get(4).unwrap_or(B::SENTINEL);
    let token_info_b = comp_arr.get(5).unwrap_or(B::SENTINEL);

    let bc_arr = match crate::vm::get_arr(bc_b) {
        Some(a) => a,
        None => rbqn_core::error::throw("•BQN: bc is not an array"),
    };
    let bc: Vec<i32> = bc_arr.i32_iter().unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()));

    let objs: Vec<B> = if let Some(objs_arr) = crate::vm::get_arr(objs_b) {
        (0..objs_arr.ia()).map(|i| objs_arr.get(i).unwrap_or(B::SENTINEL)).collect()
    } else { vec![] };

    let blocks: Vec<B> = if let Some(blocks_arr) = crate::vm::get_arr(blocks_b) {
        (0..blocks_arr.ia()).map(|i| blocks_arr.get(i).unwrap_or(B::SENTINEL)).collect()
    } else { vec![] };

    let bodies: Vec<B> = if let Some(bodies_arr) = crate::vm::get_arr(bodies_b) {
        (0..bodies_arr.ia()).map(|i| bodies_arr.get(i).unwrap_or(B::SENTINEL)).collect()
    } else { vec![] };

    let src_chars2: Vec<u32> = src.chars().map(|c| c as u32).collect();
    let src_b2 = crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(src_chars2));

    let block = rbqn_vm_compile_all(
        &bc, objs, &blocks, &bodies,
        indices_b, token_info_b, src_b2, B::SENTINEL,
    );

    let body = block.bodies[0].clone();
    let var_am = body.var_am;
    let root_scope = Arc::new(crate::scope::Scope::new(body.clone(), None, var_am, &[]));
    crate::block::eval_fun_block(block, root_scope)
}

/// Wrapper to call rbqn_vm::compiler::compile_all from within derive.rs
/// (avoids needing to import it directly in the module).
fn rbqn_vm_compile_all(
    bc: &[i32],
    objs: Vec<B>,
    blocks: &[B],
    bodies: &[B],
    indices: B,
    token_info: B,
    src: B,
    fullpath: B,
) -> Arc<crate::block::Block> {
    crate::compiler::compile_all(bc, objs, blocks, bodies, indices, token_info, src, fullpath, None, 0)
}
