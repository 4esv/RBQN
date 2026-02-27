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

/// Import cache: maps canonical file path to the cached result B value.
/// SENTINEL is used as a "currently loading" sentinel to detect circular imports.
static IMPORT_CACHE: std::sync::LazyLock<Mutex<HashMap<String, B>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

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
    InvBlock,     // Function block inverse body: bl has inv_x_body / inv_w_body
    InvMd1Block,  // 1-modifier block inverse: bl=modifier block, f=operand fn
    InvMd2Block,  // 2-modifier block inverse: bl=modifier block, f=left operand, h=right operand
    ScanInv,      // Scan inverse (F`⁼): f=F (the scan operand), c1/c2 compute scan undone
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

        // Block inverse: check for inverse header bodies (𝕊⁼: or 𝕊⁼𝕨:)
        if d.kind == DerivedKind::FunBlock {
            if let Some(ref bl) = d.bl {
                if crate::vm::prim_trace_enabled() {
                    eprintln!("[INV_REG FunBlock] inv_m={} inv_x={} inv_w={}",
                        bl.inv_m_body.is_some(), bl.inv_x_body.is_some(), bl.inv_w_body.is_some());
                }
                if bl.inv_m_body.is_some() || bl.inv_x_body.is_some() || bl.inv_w_body.is_some() {
                    let psc = d.sc.clone().unwrap();
                    return m_inv_block(bl.clone(), psc);
                }
            }
        }

        // Md1D modifier block inverse: check if the modifier has inverse bodies (𝔽_𝕣⁼𝕩: etc.)
        if d.kind == DerivedKind::Md1D {
            let modifier = d.g;
            if modifier.is_md1() {
                let mid = (modifier.0 & 0xFFFFFFFFFFFF) >> 3;
                let md = get_derived(mid);

                // ˜ (prim 45): inv_reg(F˜) = inv_swap(F).
                // NOTE: Don't intercept natively — native_inverse_swap only handles monadic.
                // For dyadic (w F˜⁼ x), the BQN runtime handles both cases correctly.
                // Only intercept for block functions with explicit inverse bodies.
                if md.kind == (DerivedKind::NativeMd1 { prim_idx: 45 }) {
                    if d.f.is_fun() {
                        let fid = (d.f.0 & 0xFFFFFFFFFFFF) >> 3;
                        let fd = get_derived(fid);
                        if fd.kind == DerivedKind::FunBlock {
                            if let Some(ref bl) = fd.bl {
                                if bl.inv_w_body.is_some() || bl.inv_x_body.is_some() {
                                    return inv_swap(d.f);
                                }
                            }
                        }
                    }
                    // Fall through to BQN runtime for all native F˜⁼
                }

                // ` (prim 52): inv_reg(F`) = ScanInv(F) — native scan inverse
                // This intercepts scan-inverse before the BQN runtime, avoiding issues
                // with the runtime's scan inverse not handling rank>1 arrays correctly.
                if md.kind == (DerivedKind::NativeMd1 { prim_idx: 52 }) {
                    return m_scan_inv(d.f);
                }

                if md.kind == DerivedKind::Md1Block {
                    if let Some(ref bl) = md.bl {
                        if bl.inv_m_body.is_some() || bl.inv_x_body.is_some() {
                            let psc = md.sc.clone().unwrap();
                            return m_inv_md1_block(bl.clone(), psc, d.f);
                        }
                    }
                }
            }
        }

        // Md2D modifier block inverse: check if the modifier has inverse bodies.
        if d.kind == DerivedKind::Md2D {
            let modifier = d.g;
            if modifier.is_md2() {
                let mid = (modifier.0 & 0xFFFFFFFFFFFF) >> 3;
                let md = get_derived(mid);
                if md.kind == DerivedKind::Md2Block {
                    if let Some(ref bl) = md.bl {
                        if bl.inv_m_body.is_some() || bl.inv_x_body.is_some() {
                            let psc = md.sc.clone().unwrap();
                            return m_inv_md2_block(bl.clone(), psc, d.f, d.h);
                        }
                    }
                }
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

        // Block swap inverse: check for inv_w_body (B˜⁼: header) or inv_x_body
        if d.kind == DerivedKind::FunBlock {
            if let Some(ref bl) = d.bl {
                if bl.inv_w_body.is_some() || bl.inv_x_body.is_some() {
                    // Create an InvBlock that for c1 uses inv_x_body and for c2 uses inv_w_body
                    let psc = d.sc.clone().unwrap();
                    return m_inv_block(bl.clone(), psc);
                }
            }
        }

        // Md1D swap inverse: check if the modifier is a Md1Block with inv_w_body (𝔽_𝕣˜⁼: header)
        if d.kind == DerivedKind::Md1D {
            let modifier = d.g;
            if modifier.is_md1() {
                let mid = (modifier.0 & 0xFFFFFFFFFFFF) >> 3;
                let md = get_derived(mid);
                if md.kind == DerivedKind::Md1Block {
                    if let Some(ref bl) = md.bl {
                        if bl.inv_w_body.is_some() || bl.inv_x_body.is_some() {
                            // Create an InvMd1Block using the dyadic inverse body
                            let psc = md.sc.clone().unwrap();
                            return m_inv_md1_block(bl.clone(), psc, d.f);
                        }
                    }
                }
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
        // NOTE: +⁼ falls through to BQN runtime — the runtime returns -˜ as dyadic inverse
        // and + as monadic inverse. Returning + here would break dyadic char arithmetic.
        // 0 => Some(m_native_fn(0)),   // +⁼ = + (identity) — REMOVED: let runtime handle
        1 => Some(m_native_fn(1)),   // -⁼ = - (negate is its own inverse)
        // NOTE: ×⁼ (prim 2): monadic × = signum, not reliably invertible.
        // Remove: return None, let BQN runtime report an error.
        3 => Some(m_native_fn(3)),   // ÷⁼ = ÷ (reciprocal is its own inverse)
        // NOTE: ⋆⁼ (prim 4): ⋆ x = e^x, so ⋆⁼ x = ln(x). Use sys_fn 202 (log).
        4 => Some(m_sys_fn(202)),    // ⋆⁼ = ln (natural log)
        // NOTE: √⁼ (prim 5): √ x = x^0.5, so √⁼ x = x^2. Use sys_fn 203 (square).
        5 => Some(m_sys_fn(203)),    // √⁼ = x^2 (square)
        9 => Some(m_native_fn(9)),   // ¬⁼ = ¬ (not is its own inverse)
        12 => Some(m_sys_fn(206)),    // <⁼ = unbox (extract from rank-0 array)
        13 => Some(m_native_fn(12)), // >⁼ = < (box)
        // NOTE: ⊣ monadic is identity (⊣ x = x), so ⊣⁼ x = x. But dyadic w⊣⁼x has no inverse
        // (⊣ always returns 𝕨, ignoring 𝕩, so there's no unique 𝕩). Let the BQN runtime error.
        // 20 => Some(m_native_fn(20)), // REMOVED: BQN runtime errors on w⊣⁼x (no inverse)
        21 => Some(m_native_fn(21)), // ⊢⁼ = ⊢
        // NOTE: ⌽⁼ monadic = ⌽, but dyadic n⌽⁼ x = (-n)⌽ x (not n⌽ x).
        // Returning ⌽ breaks dyadic. Let BQN runtime handle ⌽⁼ correctly.
        // 31 => Some(m_native_fn(31)), // REMOVED: BQN runtime handles n⌽⁼ x = (-n)⌽ x
        // NOTE: ⍉⁼ uses sys_fn 205: self-inverse for rank≤2, moves first axis to last for rank>2.
        32 => Some(m_sys_fn(205)), // ⍉⁼ = inverse transpose
        33 => Some(m_sys_fn(201)),   // /⁼ = inverse of indices (counts from sorted indices)
        37 => Some(m_native_fn(24)), // ⊑⁼ = ≍ (solo: first inverse wraps in 1-element array)
        _ => None,
    }
}

/// Known swap inverses for native primitives (w F˜⁼ x or similar).
/// Monadic F˜⁼ x: inverse of (F˜ x = x F x).
/// Dyadic w F˜⁼ x: inverse of (w F˜ x = x F w), i.e. find y: x F y = w F˜ x
fn native_inverse_swap(prim_idx: usize) -> Option<B> {
    match prim_idx {
        // NOTE: +˜ x = x+x = 2x, so +˜⁼ x = x÷2. Dyadic: w+˜⁼x = x-w (subtract).
        // The sys_fn 204 handles the monadic case (x÷2). The BQN runtime handles dyadic.
        0 => Some(m_sys_fn(204)),    // +˜⁼ monadic = x÷2; dyadic handled by runtime
        // NOTE: -˜ dyadically: w-˜⁼x = x+w (add). Return +.
        1 => Some(m_native_fn(0)),   // -˜⁼ = + (w-˜⁼x means x+w)
        // NOTE: ×˜⁼: monadic √x, dyadic x÷w. The BQN runtime handles both correctly.
        // Don't intercept — let inv_swap_fn (BQN runtime) handle this.
        // 2 => Some(m_native_fn(5)),  // REMOVED: runtime handles both cases correctly
        // NOTE: ÷˜⁼ dyadically: w÷˜⁼x = x×w (multiply).
        3 => Some(m_native_fn(2)),   // ÷˜⁼ = × (w÷˜⁼x = x×w)
        // NOTE: ⋆˜⁼ dyadically: w⋆˜⁼x = w√x (w-th root of x). Return √.
        4 => Some(m_native_fn(5)),   // ⋆˜⁼ = √ (w⋆˜⁼x = w√x)
        // NOTE: √˜⁼ dyadic: w(√˜)⁼x — find y such that w(√˜)y = x, i.e., y√w = x, i.e., w^(1/y) = x.
        // Solving: 1/y = log_w(x) = ln(x)/ln(w), so y = ln(w)/ln(x).
        // Use sys_fn 207 for the dyadic case: w(√˜⁼)x = ln(w)/ln(x).
        5 => Some(m_sys_fn(207)),   // √˜⁼ dyadic: ln(w)/ln(x)
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

/// Create an inverse-block wrapper. When called (c1 or c2), executes the block's
/// inv_m_body (monadic) or inv_w_body/inv_x_body (dyadic) header body.
pub fn m_inv_block(bl: Arc<Block>, psc: Arc<Scope>) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::InvBlock,
        f: B::SENTINEL, g: B::SENTINEL, h: B::SENTINEL,
        bl: Some(bl), sc: Some(psc),
    });
    tagu64(id << 3, FUN_TAG)
}

/// Create an inverse 1-modifier-block wrapper.
/// bl = the modifier block (with inv_m_body), f = operand function.
/// When called, executes bl.inv_m_body with [self, x, w?, modifier, operand] args.
pub fn m_inv_md1_block(bl: Arc<Block>, psc: Arc<Scope>, operand: B) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::InvMd1Block,
        f: operand, g: B::SENTINEL, h: B::SENTINEL,
        bl: Some(bl), sc: Some(psc),
    });
    tagu64(id << 3, FUN_TAG)
}

/// Create an inverse 2-modifier-block wrapper.
/// bl = the modifier block (with inv_m_body), f = left operand, h = right operand.
pub fn m_inv_md2_block(bl: Arc<Block>, psc: Arc<Scope>, f_operand: B, g_operand: B) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::InvMd2Block,
        f: f_operand, g: B::SENTINEL, h: g_operand,
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

/// Create a scan-inverse wrapper. When called (c1 or c2), undoes the scan using F.
/// c1(ScanInv, x): monadic scan inverse — result[0]=x[0], result[i] = x[i] F⁼ x[i-1]
/// c2(ScanInv, w, x): dyadic scan inverse — result[0]=x[0] F⁼ w, result[i] = x[i] F⁼ x[i-1]
pub fn m_scan_inv(f: B) -> B {
    let id = store_derived(Derived {
        kind: DerivedKind::ScanInv,
        f, g: B::SENTINEL, h: B::SENTINEL,
        bl: None, sc: None,
    });
    tagu64(id << 3, FUN_TAG)
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
            DerivedKind::ScanInv => {
                // Monadic scan inverse (F`⁼ x): undo a scan.
                // result[0] = x[0], result[i] = x[i] F⁼ x[i-1]
                crate::modifiers::scan_inv_c1(d.f, x)
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
            DerivedKind::InvBlock => {
                // Block inverse body: execute inv_m_body (monadic 𝕊⁼:) for monadic call.
                // Fall back to inv_x_body (dyadic-inverse monadic half) if inv_m_body absent.
                // NOTE: 𝕊 in the inverse body refers to the FORWARD function (the block itself).
                let bl = d.bl.as_ref().unwrap().clone();
                let psc = d.sc.as_ref().unwrap().clone();
                let inv_body = bl.inv_m_body.clone().or_else(|| bl.inv_x_body.clone());
                if let Some(body) = inv_body {
                    // Reconstruct the forward block function for 𝕊 binding
                    let forward_fn = m_fun_block(bl.clone(), psc.clone());
                    crate::vm::exec_block_with_args(&bl, body, psc, &[forward_fn, x, B::SENTINEL])
                } else {
                    rbqn_core::error::throw("Block has no monadic inverse header (𝕊⁼:)")
                }
            }
            DerivedKind::InvMd1Block => {
                // 1-modifier block inverse: execute the inverse body of the modifier block.
                // bl = modifier block (with inv_m_body), d.f = operand function.
                // Args: [forward_derived, x, SENTINEL, modifier_val, operand]
                // NOTE: 𝕊 in the inverse body refers to the FORWARD derived (not the inverse).
                // This allows the inverse body to call the forward function recursively.
                let bl = d.bl.as_ref().unwrap().clone();
                let psc = d.sc.as_ref().unwrap().clone();
                let operand = d.f;
                let inv_body = bl.inv_m_body.clone().or_else(|| bl.inv_x_body.clone());
                if let Some(body) = inv_body {
                    // Reconstruct modifier B value for 𝔽 binding
                    let modifier_val = {
                        let tmp_id = store_derived(Derived {
                            kind: DerivedKind::Md1Block,
                            f: B::SENTINEL, g: B::SENTINEL, h: B::SENTINEL,
                            bl: Some(bl.clone()), sc: Some(psc.clone()),
                        });
                        tagu64(tmp_id << 3, MD1_TAG)
                    };
                    // Build the forward derived function (operand modifier) for 𝕊 binding
                    let forward_derived = m_md1d(modifier_val, operand);
                    crate::vm::exec_block_with_args(&bl, body, psc, &[forward_derived, x, B::SENTINEL, modifier_val, operand])
                } else {
                    rbqn_core::error::throw("Modifier block has no inverse header (𝔽_𝕣⁼:)")
                }
            }
            DerivedKind::InvMd2Block => {
                // 2-modifier block inverse: execute the inverse body of the modifier block.
                // bl = modifier block, d.f = left operand, d.h = right operand.
                // NOTE: 𝕊 in the inverse body refers to the FORWARD derived function.
                let bl = d.bl.as_ref().unwrap().clone();
                let psc = d.sc.as_ref().unwrap().clone();
                let f_operand = d.f;
                let g_operand = d.h;
                let inv_body = bl.inv_m_body.clone().or_else(|| bl.inv_x_body.clone());
                if let Some(body) = inv_body {
                    let modifier_val = {
                        let tmp_id = store_derived(Derived {
                            kind: DerivedKind::Md2Block,
                            f: B::SENTINEL, g: B::SENTINEL, h: B::SENTINEL,
                            bl: Some(bl.clone()), sc: Some(psc.clone()),
                        });
                        tagu64(tmp_id << 3, MD2_TAG)
                    };
                    let forward_derived = m_md2d(modifier_val, f_operand, g_operand);
                    crate::vm::exec_block_with_args(&bl, body, psc, &[forward_derived, x, B::SENTINEL, modifier_val, f_operand, g_operand])
                } else {
                    rbqn_core::error::throw("2-modifier block has no inverse header")
                }
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
                        // NOTE: if no dyadic body exists, the derived function cannot be called dyadically
                        let body = if let Some(ref dy) = bl.dy_body {
                            dy.clone()
                        } else {
                            rbqn_core::error::throw("This block cannot be called dyadically");
                        };
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
            DerivedKind::ScanInv => {
                // Dyadic scan inverse (w F`⁼ x): undo a scan with initial value w.
                // result[0] = x[0] F⁼ w, result[i] = x[i] F⁼ x[i-1]
                crate::modifiers::scan_inv_c2(d.f, w, x)
            }
            DerivedKind::LazyInvReg => {
                // For dyadic F⁼, the native monadic inverse table is not correct.
                // Fall through to the BQN runtime's inverse resolver which handles
                // dyadic inverses properly (e.g., w+⁼x = x-w, w-⁼x = x+w, etc.).
                let orig = d.f;
                let reg_fn = INV_REG_FN.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(runtime_inv) = *reg_fn {
                    drop(reg_fn);
                    // Runtime's inv_reg returns a function; call it as c2 wrapper
                    let inv_fn = c1(runtime_inv, orig);
                    c2(inv_fn, w, x)
                } else {
                    drop(reg_fn);
                    // No runtime — fall back to native monadic inverse table (may be wrong for dyadic)
                    let inv_fn = inv_reg(orig);
                    c2(inv_fn, w, x)
                }
            }
            DerivedKind::LazyInvSwap => {
                let inv_fn = inv_swap(d.f);
                c2(inv_fn, w, x)
            }
            DerivedKind::InvBlock => {
                // Block dyadic inverse: prefer inv_w_body (𝕊⁼𝕨:), then inv_x_body.
                // NOTE: inv_m_body is only for monadic calls (𝕊⁼:); don't use it for dyadic.
                // NOTE: 𝕊 refers to the FORWARD function (block itself, not the inverse).
                let bl = d.bl.as_ref().unwrap().clone();
                let psc = d.sc.as_ref().unwrap().clone();
                let inv_body = bl.inv_w_body.clone()
                    .or_else(|| bl.inv_x_body.clone());
                if let Some(body) = inv_body {
                    let forward_fn = m_fun_block(bl.clone(), psc.clone());
                    crate::vm::exec_block_with_args(&bl, body, psc, &[forward_fn, x, w])
                } else {
                    rbqn_core::error::throw("Block has no dyadic inverse header (𝕊⁼𝕨:)")
                }
            }
            DerivedKind::InvMd1Block => {
                // 1-modifier block inverse dyadic: execute inverse body with w given.
                // Prefer inv_w_body (˜⁼ path), then inv_x_body, then inv_m_body.
                // NOTE: 𝕊 refers to the FORWARD derived (not the inverse).
                let bl = d.bl.as_ref().unwrap().clone();
                let psc = d.sc.as_ref().unwrap().clone();
                let operand = d.f;
                let inv_body = bl.inv_w_body.clone()
                    .or_else(|| bl.inv_x_body.clone())
                    .or_else(|| bl.inv_m_body.clone());
                if let Some(body) = inv_body {
                    let modifier_val = {
                        let tmp_id = store_derived(Derived {
                            kind: DerivedKind::Md1Block,
                            f: B::SENTINEL, g: B::SENTINEL, h: B::SENTINEL,
                            bl: Some(bl.clone()), sc: Some(psc.clone()),
                        });
                        tagu64(tmp_id << 3, MD1_TAG)
                    };
                    let forward_derived = m_md1d(modifier_val, operand);
                    crate::vm::exec_block_with_args(&bl, body, psc, &[forward_derived, x, w, modifier_val, operand])
                } else {
                    rbqn_core::error::throw("Modifier block has no dyadic inverse header")
                }
            }
            DerivedKind::InvMd2Block => {
                // 2-modifier block inverse dyadic: execute inverse body with w given.
                // NOTE: 𝕊 refers to the FORWARD derived (not the inverse).
                let bl = d.bl.as_ref().unwrap().clone();
                let psc = d.sc.as_ref().unwrap().clone();
                let f_operand = d.f;
                let g_operand = d.h;
                let inv_body = bl.inv_x_body.clone().or_else(|| bl.inv_m_body.clone());
                if let Some(body) = inv_body {
                    let modifier_val = {
                        let tmp_id = store_derived(Derived {
                            kind: DerivedKind::Md2Block,
                            f: B::SENTINEL, g: B::SENTINEL, h: B::SENTINEL,
                            bl: Some(bl.clone()), sc: Some(psc.clone()),
                        });
                        tagu64(tmp_id << 3, MD2_TAG)
                    };
                    let forward_derived = m_md2d(modifier_val, f_operand, g_operand);
                    crate::vm::exec_block_with_args(&bl, body, psc, &[forward_derived, x, w, modifier_val, f_operand, g_operand])
                } else {
                    rbqn_core::error::throw("2-modifier block has no dyadic inverse header")
                }
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
        // NOTE: •file.Lines / •FLines
        50 => file_lines_c1(x),
        // NOTE: •file.List
        51 => file_list_c1(x),
        // NOTE: •FChars — read file as character array
        52 => file_chars_c1(x),
        // NOTE: •FBytes — read file as byte array
        53 => file_bytes_c1(x),
        // NOTE: •file.At — resolve relative path against base
        54 => file_at_c1(x),
        // NOTE: •file.Name — extract filename (basename) from path
        55 => file_name_c1(x),
        // NOTE: •file.Parent — extract parent directory from path
        56 => file_parent_c1(x),
        // NOTE: •file.Exists — check if file exists (returns 0 or 1)
        57 => file_exists_c1(x),
        // NOTE: •file.Type — return file type: 'f', 'd', 'l'
        58 => file_type_c1(x),
        // NOTE: •file.CreateDir — create directory (like mkdir -p)
        59 => file_createdir_c1(x),
        // NOTE: •file.Remove — remove file
        61 => file_remove_c1(x),
        // NOTE: •file.Open — stub (NYI)
        62 => rbqn_core::error::throw("•file.Open: not yet implemented"),
        // NOTE: •file.Chars — same as •FChars via namespace
        63 => file_chars_c1(x),
        // NOTE: •file.Bytes — same as •FBytes via namespace
        64 => file_bytes_c1(x),
        // NOTE: •file.Lines (namespace alias, same as sys 50)
        65 => file_lines_c1(x),
        // NOTE: •Import — load and cache BQN source file
        70 => sys_import_c1(x),
        // NOTE: •ParseFloat — convert BQN string to number
        75 => sys_parsefloat_c1(x),
        // NOTE: •Hash — deterministic hash of value
        76 => sys_hash_c1(x),
        // NOTE: •FromUTF8 — byte array to character array
        78 => sys_fromutf8_c1(x),
        // NOTE: •ToUTF8 — character array to byte array
        79 => sys_toutf8_c1(x),
        // NOTE: •CurrentError — current error in catch context (stub)
        80 => B::SENTINEL,
        201 => { // Internal: /⁼ (inverse of indices)
            let r = rbqn_prim::slash::indices_inverse_c1(x, x_arr.as_ref());
            match r {
                Ok(pr) => prim_result_to_b(pr),
                Err(e) => rbqn_core::error::throw(e.to_string()),
            }
        }
        // NOTE: Internal inverse functions registered in native_inverse_reg
        202 => { // ⋆⁼ = ln(x) — natural logarithm
            let r = rbqn_prim::arith_monad::log_c1(x, x_arr.as_ref());
            match r {
                Ok(pr) => prim_result_to_b(pr),
                Err(e) => rbqn_core::error::throw(e.to_string()),
            }
        }
        203 => { // √⁼ = x^2 — square
            let r = rbqn_prim::arith_monad::square_c1(x, x_arr.as_ref());
            match r {
                Ok(pr) => prim_result_to_b(pr),
                Err(e) => rbqn_core::error::throw(e.to_string()),
            }
        }
        204 => { // +˜⁼ = x÷2 — halve
            let r = rbqn_prim::arith_monad::halve_c1(x, x_arr.as_ref());
            match r {
                Ok(pr) => prim_result_to_b(pr),
                Err(e) => rbqn_core::error::throw(e.to_string()),
            }
        }
        205 => { // ⍉⁼ = inverse transpose (rank≤2: same as ⍉; rank>2: move first axis to last)
            let r = rbqn_prim::structural::transpose_inv_c1(x, x_arr.as_ref());
            match r {
                Ok(pr) => prim_result_to_b(pr),
                Err(e) => rbqn_core::error::throw(e.to_string()),
            }
        }
        206 => { // <⁼ = unbox: extract content from rank-0 array
            if let Some(ref arr) = x_arr {
                if arr.rank() == 0 {
                    arr.get(0).unwrap_or(x)
                } else {
                    rbqn_core::error::throw("<⁼𝕩: 𝕩 must be a rank-0 array")
                }
            } else {
                rbqn_core::error::throw("<⁼𝕩: 𝕩 must be a rank-0 array")
            }
        }
        // NOTE: math sys functions — unique range 1100-1114 to avoid conflicts with sys 100
        1100 => math_sin_c1(x),
        1101 => math_cos_c1(x),
        1102 => math_tan_c1(x),
        1103 => math_asin_c1(x),
        1104 => math_acos_c1(x),
        1105 => math_atan_c1(x),  // monadic: atan(x)
        1106 => math_log_c1(x),   // monadic: ln(x)
        1107 => math_cbrt_c1(x),
        1108 => math_hypot_c1(x), // monadic: |x| (just as fallback)
        1109 => math_erf_c1(x),
        1110 => math_comb_c1(x),  // monadic: treat x as self-comb? use 0 •math.Comb x
        1111 => math_fact_c1(x),
        1112 => math_gcd_c1(x),   // monadic: not well-defined, return x
        1113 => math_lcm_c1(x),   // monadic: not well-defined, return x
        // NOTE: •rand sys functions — range 1120-1123
        1121 => rand_range_c1(x),
        1122 => rand_deal_c1(x),
        1123 => rand_subset_c1(x),
        // NOTE: •platform.environment — range 1130
        1130 => platform_env_c1(x),
        // NOTE: time functions — sys 140-142
        140 => { // •UnixTime — current epoch seconds
            use std::time::UNIX_EPOCH;
            let secs = std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            B::m_f64(secs)
        }
        141 => { // •MonoTime — monotonic clock in seconds
            static START: std::sync::LazyLock<std::time::Instant> =
                std::sync::LazyLock::new(std::time::Instant::now);
            let secs = START.elapsed().as_secs_f64();
            B::m_f64(secs)
        }
        142 => { // •Delay — sleep for x seconds, return x
            let secs = if x.is_f64() { x.o2f() } else { 0.0 };
            std::thread::sleep(std::time::Duration::from_secs_f64(secs));
            x
        }
        145 => { // •SH — execute shell command, return ⟨exit_code, stdout, stderr⟩
            sh_exec_c1(x)
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
        // NOTE: •file.Lines dyadic — write array of strings to file
        50 => file_lines_c2(w, x),
        // NOTE: •FChars dyadic / •file.Chars dyadic — write string to file
        52 | 63 => file_chars_c2(w, x),
        // NOTE: •FBytes dyadic / •file.Bytes dyadic — write byte array to file
        53 | 64 => file_bytes_c2(w, x),
        // NOTE: •file.At dyadic — resolve name relative to path
        54 => file_at_c2(w, x),
        // NOTE: •file.Rename dyadic — rename file: old •file.Rename new
        60 => file_rename_c2(w, x),
        // NOTE: •Cmp dyadic — total order comparison: returns ¯1, 0, or 1
        77 => sys_cmp_c2(w, x),
        100 => { // •BQN placeholder — just return SENTINEL for now
            B::SENTINEL
        }
        201 => { // Internal: w /⁼ x (dyadic inverse of indices)
            let r = rbqn_prim::slash::indices_inverse_c2(w, w_arr.as_ref(), x, x_arr.as_ref());
            match r {
                Ok(pr) => prim_result_to_b(pr),
                Err(e) => rbqn_core::error::throw(e.to_string()),
            }
        }
        202 => { // Dyadic ⋆⁼: w⋆⁼x = log_w(x) = ln(x)/ln(w) — apply to each element pair
            // For numeric scalar args:
            fn log_base(base: f64, x: f64) -> f64 {
                if base == std::f64::consts::E { x.ln() }
                else { x.log(base) }
            }
            if w.is_f64() && x.is_f64() {
                let w_f = w.o2f();
                let x_f = x.o2f();
                B::m_f64(log_base(w_f, x_f))
            } else if let (Some(wa), Some(xa)) = (w_arr.as_ref(), x_arr.as_ref()) {
                if wa.ia() == xa.ia() {
                    let mut result = Vec::with_capacity(wa.ia());
                    for i in 0..wa.ia() {
                        let wv = wa.get(i).unwrap_or(B::m_f64(1.0)).o2f();
                        let xv = xa.get(i).unwrap_or(B::m_f64(1.0)).o2f();
                        result.push(B::m_f64(log_base(wv, xv)));
                    }
                    let mut out = rbqn_core::array::BqnArr::new_vec_f64(result.iter().map(|b| b.o2f()).collect());
                    out.shape = xa.shape.clone();
                    crate::vm::tag_arr(rbqn_core::array::squeeze_num(out))
                } else if wa.ia() == 1 {
                    // Scalar w, array x
                    let wv = wa.get(0).unwrap_or(B::m_f64(1.0)).o2f();
                    let mut result = Vec::with_capacity(xa.ia());
                    for i in 0..xa.ia() {
                        let xv = xa.get(i).unwrap_or(B::m_f64(1.0)).o2f();
                        result.push(log_base(wv, xv));
                    }
                    let mut out = rbqn_core::array::BqnArr::new_vec_f64(result);
                    out.shape = xa.shape.clone();
                    crate::vm::tag_arr(rbqn_core::array::squeeze_num(out))
                } else {
                    rbqn_core::error::throw("w⋆⁼x: mismatched array lengths")
                }
            } else if w.is_f64() {
                if let Some(xa) = x_arr.as_ref() {
                    let wv = w.o2f();
                    let mut result = Vec::with_capacity(xa.ia());
                    for i in 0..xa.ia() {
                        let xv = xa.get(i).unwrap_or(B::m_f64(1.0)).o2f();
                        result.push(log_base(wv, xv));
                    }
                    let mut out = rbqn_core::array::BqnArr::new_vec_f64(result);
                    out.shape = xa.shape.clone();
                    crate::vm::tag_arr(rbqn_core::array::squeeze_num(out))
                } else {
                    rbqn_core::error::throw("w⋆⁼x: x must be a number")
                }
            } else {
                rbqn_core::error::throw("w⋆⁼x: w and x must be numbers")
            }
        }
        // NOTE: math dyadic functions
        1105 => { // atan2: w •math.Atan x = atan2(w, x) = angle of point (x, w)
            let w_f = w.o2f();
            let x_f = x.o2f();
            B::m_f64(w_f.atan2(x_f))
        }
        1106 => { // log base: w •math.Log x = log_w(x) = ln(x)/ln(w)
            B::m_f64(x.o2f().log(w.o2f()))
        }
        1108 => { // hypot: w •math.Hypot x
            B::m_f64(w.o2f().hypot(x.o2f()))
        }
        1110 => { // comb: w •math.Comb x = C(x, w) = binomial coefficient
            math_comb_c2(w, x)
        }
        1112 => { // gcd: w •math.GCD x
            math_gcd_c2(w, x)
        }
        1113 => { // lcm: w •math.LCM x
            math_lcm_c2(w, x)
        }
        // NOTE: rand dyadic: shape •rand.Range n
        1121 => rand_range_c2(w, x),
        1123 => rand_subset_c2(w, x),
        // NOTE: •SH dyadic: options •SH args
        145 => sh_exec_c2(w, x),
        // NOTE: w(+˜)⁼x = x - w  (dyadic: +˜ swaps: w +˜ y = y+w, inverse = y = x - w)
        204 => {
            let r = rbqn_prim::arith_dyad::sub_c2(x, x_arr.as_ref(), w, w_arr.as_ref());
            match r {
                Ok(pr) => prim_result_to_b(pr),
                Err(e) => rbqn_core::error::throw(e.to_string()),
            }
        }
        // NOTE: w√⁼x = x^w (dyadic sqrt-inverse = power with args swapped)
        // √⁼ monadic is x^2 (sys 203 c1); dyadic is x raised to the power w.
        203 => {
            let r = rbqn_prim::arith_dyad::pow_c2(x, x_arr.as_ref(), w, w_arr.as_ref());
            match r {
                Ok(pr) => prim_result_to_b(pr),
                Err(e) => rbqn_core::error::throw(e.to_string()),
            }
        }
        205 => { // w⍉⁼x = inverse-permutation(w)⍉x
            // For a bijective permutation p, inv_perm[j] = i where p[i] = j.
            // For partial permutations (p has length < rank(x)), fall back to reorder_c2(w, x).
            let rank = if let Some(ref xa) = x_arr { xa.rank() as usize } else { 0 };
            let perm_len = if let Some(ref wa) = w_arr {
                wa.ia()
            } else if w.is_f64() {
                1
            } else {
                0
            };
            // Only compute inverse if p is a full bijection (length == rank, all values distinct and cover 0..rank)
            let is_full_bijection = perm_len == rank && rank > 0 && w_arr.is_some();
            if is_full_bijection {
                if let Some(ref wa) = w_arr {
                    if let Ok(perm) = wa.i32_iter() {
                        let mut inv_perm = vec![-1i32; rank];
                        let mut valid = true;
                        for (i, &p) in perm.iter().enumerate() {
                            if p < 0 || p as usize >= rank {
                                valid = false;
                                break;
                            }
                            if inv_perm[p as usize] != -1 {
                                valid = false; // duplicate
                                break;
                            }
                            inv_perm[p as usize] = i as i32;
                        }
                        if valid && inv_perm.iter().all(|&v| v >= 0) {
                            // Build the inverse permutation array and call reorder_c2
                            let inv_arr = rbqn_core::array::BqnArr::new_vec_i32(inv_perm);
                            let inv_b = crate::vm::tag_arr(inv_arr);
                            let inv_b_arr = crate::vm::get_arr(inv_b);
                            let r = rbqn_prim::structural::reorder_c2(inv_b, inv_b_arr.as_ref(), x, x_arr.as_ref());
                            return match r {
                                Ok(pr) => prim_result_to_b(pr),
                                Err(e) => rbqn_core::error::throw(e.to_string()),
                            };
                        }
                    }
                }
            }
            // Partial permutation (len(w) < rank(x)): extend to full permutation, then invert.
            // CBQN extension rule: uncovered target positions get remaining source axes in order.
            // Example: p=2‿1 rank=4: full_perm = 2‿1‿0‿3
            let n_partial = if let Some(ref wa) = w_arr { wa.ia() } else { 1 };
            let partial: Vec<i32> = if let Some(ref wa) = w_arr {
                wa.i32_iter().unwrap_or_default()
            } else if w.is_f64() {
                vec![w.to_i32().unwrap_or(0) as i32]
            } else {
                vec![]
            };
            // Compute which target positions are covered by the partial permutation
            let mut covered = vec![false; rank];
            for &p in &partial {
                if p >= 0 && (p as usize) < rank {
                    covered[p as usize] = true;
                }
            }
            // Uncovered target positions in order
            let uncovered: Vec<i32> = (0..rank as i32).filter(|&i| !covered[i as usize]).collect();
            // Full permutation: partial for first n_partial axes, then uncovered for remaining
            let mut full_perm = partial.clone();
            for (i, &uc) in uncovered.iter().enumerate() {
                if n_partial + i < rank {
                    full_perm.push(uc);
                }
            }
            if full_perm.len() == rank {
                // Compute inverse of the full permutation
                let mut inv_perm = vec![-1i32; rank];
                let mut valid = true;
                for (i, &p) in full_perm.iter().enumerate() {
                    if p < 0 || p as usize >= rank || inv_perm[p as usize] != -1 {
                        valid = false; break;
                    }
                    inv_perm[p as usize] = i as i32;
                }
                if valid && inv_perm.iter().all(|&v| v >= 0) {
                    let inv_arr = rbqn_core::array::BqnArr::new_vec_i32(inv_perm);
                    let inv_b = crate::vm::tag_arr(inv_arr);
                    let inv_b_arr = crate::vm::get_arr(inv_b);
                    let r = rbqn_prim::structural::reorder_c2(inv_b, inv_b_arr.as_ref(), x, x_arr.as_ref());
                    return match r {
                        Ok(pr) => prim_result_to_b(pr),
                        Err(e) => rbqn_core::error::throw(e.to_string()),
                    };
                }
            }
            rbqn_core::error::throw("⍉⁼: cannot compute inverse for given permutation")
        }
        // NOTE: sys 207 = √˜⁼ dyadic: w(√˜)⁼x = ln(w)/ln(x)
        207 => {
            let wf = w.o2f();
            let xf = x.o2f();
            if xf == 1.0 {
                rbqn_core::error::throw("√˜⁼: x=1 has no finite inverse (log(1)=0)");
            }
            B::m_f64(wf.ln() / xf.ln())
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
        // File I/O top-level functions
        "flines"    => m_sys_fn(50),   // •FLines — read file as array of strings
        "fchars"    => m_sys_fn(52),   // •FChars — read file as char array
        "fbytes"    => m_sys_fn(53),   // •FBytes — read file as byte array
        // Import
        "import"    => m_sys_fn(70),   // •Import — load and cache BQN file
        // Utility functions
        "parsefloat"   => m_sys_fn(75), // •ParseFloat — string to number
        "hash"         => m_sys_fn(76), // •Hash — deterministic hash
        "fromutf8"     => m_sys_fn(78), // •FromUTF8 — bytes to chars
        "toutf8"       => m_sys_fn(79), // •ToUTF8 — chars to bytes
        "currenterror" => m_sys_fn(80), // •CurrentError — current error in catch
        // NOTE: •file namespace object (Lines, List, and expanded file ops)
        "file"      => make_file_namespace(),
        // NOTE: •math namespace (trig, log, special functions, constants)
        "math"      => make_math_namespace(),
        // NOTE: •rand namespace (PRNG: Range, Deal, Subset)
        "rand"      => make_rand_namespace(),
        // NOTE: •MakeRand — creates a seeded PRNG (simplified: returns global •rand)
        "makerand"  => make_rand_namespace(),
        // NOTE: •platform namespace (os, bqn.impl, etc.)
        "platform"  => make_platform_namespace(),
        // NOTE: time functions
        "unixtime"  => m_sys_fn(140),
        "monotime"  => m_sys_fn(141),
        "delay"     => m_sys_fn(142),
        // NOTE: shell execution
        "sh"        => m_sys_fn(145),
        // NOTE: •_while_ 2-modifier (CBQN compiler strips underscores → "while")
        "_while_" | "while" => m_native_md2(crate::modifiers::MD2_WHILE),
        // NOTE: •_fillBy_ 2-modifier (CBQN compiler strips underscores → "fillby")
        "_fillBy_" | "_fillby_" | "fillby" => m_native_md2(crate::modifiers::MD2_FILL_BY),
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

// --- •file namespace ---

/// Cached •file namespace object. Built once on first access.
static FILE_NS: std::sync::LazyLock<Mutex<Option<B>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// Build the •file namespace with all file operation fields.
fn make_file_namespace() -> B {
    let mut guard = FILE_NS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ns_b) = *guard {
        return ns_b;
    }

    use crate::namespace::{str2gid, NSDesc, NS};

    // NOTE: Field order must match the scope values array order exactly.
    // Fields: Lines(50), List(51), Chars(63), Bytes(64), At(54), Name(55),
    //         Parent(56), Exists(57), Type(58), CreateDir(59), Rename(60), Remove(61), Open(62)
    let lines_gid     = str2gid("lines");
    let list_gid      = str2gid("list");
    let chars_gid     = str2gid("chars");
    let bytes_gid     = str2gid("bytes");
    let at_gid        = str2gid("at");
    let name_gid      = str2gid("name");
    let parent_gid    = str2gid("parent");
    let exists_gid    = str2gid("exists");
    let type_gid      = str2gid("type");
    let createdir_gid = str2gid("createdir");
    let rename_gid    = str2gid("rename");
    let remove_gid    = str2gid("remove");
    let open_gid      = str2gid("open");

    let var_am_i32: i32 = 13;
    let var_am_u16: u16 = 13;
    let exp_gids = vec![
        lines_gid, list_gid, chars_gid, bytes_gid, at_gid, name_gid,
        parent_gid, exists_gid, type_gid, createdir_gid, rename_gid, remove_gid, open_gid,
    ];

    let desc = Arc::new(NSDesc { var_am: var_am_i32, exp_gids });

    // Create a minimal Body for the Scope constructor
    let body = Arc::new(crate::block::Body::new(var_am_u16, 0, 0, 0));

    // Build scope with all system function values (order must match exp_gids)
    let sc = Arc::new(crate::scope::Scope::new(
        body,
        None,
        var_am_u16,
        &[
            m_sys_fn(50),  // Lines
            m_sys_fn(51),  // List
            m_sys_fn(63),  // Chars
            m_sys_fn(64),  // Bytes
            m_sys_fn(54),  // At
            m_sys_fn(55),  // Name
            m_sys_fn(56),  // Parent
            m_sys_fn(57),  // Exists
            m_sys_fn(58),  // Type
            m_sys_fn(59),  // CreateDir
            m_sys_fn(60),  // Rename
            m_sys_fn(61),  // Remove
            m_sys_fn(62),  // Open
        ],
    ));

    let ns = NS { desc, sc };
    let ns_b = crate::namespace::store_ns(ns);
    *guard = Some(ns_b);
    ns_b
}

/// •file.Lines: read a file and return an array of BQN strings (one per line).
fn file_lines_c1(x: B) -> B {
    let path = b_to_string(x);
    let content = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        rbqn_core::error::throw(format!("•file.Lines: cannot read {path}: {e}"))
    });
    let lines: Vec<B> = content.lines().map(|line| {
        let chars: Vec<u32> = line.chars().map(|c| c as u32).collect();
        crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
    }).collect();
    crate::vm::b_vec_to_arr(lines)
}

/// •file.Lines dyadic: w •file.Lines x — read file x with encoding/options w.
/// For now just ignores w and reads as UTF-8.
fn file_lines_c2(_w: B, x: B) -> B {
    file_lines_c1(x)
}

/// •file.List: list directory entries and return a sorted array of filename strings.
fn file_list_c1(x: B) -> B {
    let path = b_to_string(x);
    let entries = std::fs::read_dir(&path).unwrap_or_else(|e| {
        rbqn_core::error::throw(format!("•file.List: cannot list {path}: {e}"))
    });
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    let bqn_names: Vec<B> = names.iter().map(|name| {
        let chars: Vec<u32> = name.chars().map(|c| c as u32).collect();
        crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
    }).collect();
    crate::vm::b_vec_to_arr(bqn_names)
}

/// •FChars / •file.Chars: read file as character array.
fn file_chars_c1(x: B) -> B {
    let path = resolve_path(b_to_string(x));
    let content = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        rbqn_core::error::throw(format!("•FChars: cannot read {path}: {e}"))
    });
    let chars: Vec<u32> = content.chars().map(|c| c as u32).collect();
    crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
}

/// •FChars dyadic / •file.Chars dyadic: write string w to file x.
fn file_chars_c2(w: B, x: B) -> B {
    let path = resolve_path(b_to_string(x));
    let content = b_to_string(w);
    std::fs::write(&path, content).unwrap_or_else(|e| {
        rbqn_core::error::throw(format!("•FChars: cannot write {path}: {e}"))
    });
    w
}

/// •FBytes / •file.Bytes: read file as byte array (numeric 0-255).
fn file_bytes_c1(x: B) -> B {
    let path = resolve_path(b_to_string(x));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        rbqn_core::error::throw(format!("•FBytes: cannot read {path}: {e}"))
    });
    let nums: Vec<f64> = bytes.iter().map(|&b| b as f64).collect();
    crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_f64(nums))
}

/// •FBytes dyadic / •file.Bytes dyadic: write byte array w to file x.
fn file_bytes_c2(w: B, x: B) -> B {
    let path = resolve_path(b_to_string(x));
    let bytes = b_to_byte_vec(w);
    std::fs::write(&path, &bytes).unwrap_or_else(|e| {
        rbqn_core::error::throw(format!("•FBytes: cannot write {path}: {e}"))
    });
    w
}

/// •file.At monadic: returns x unchanged (identity for absolute paths).
fn file_at_c1(x: B) -> B {
    // Monadic: just resolve relative to •path
    let path_str = b_to_string(x);
    let resolved = resolve_path(path_str);
    let chars: Vec<u32> = resolved.chars().map(|c| c as u32).collect();
    crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
}

/// •file.At dyadic: resolve name x relative to base path w.
fn file_at_c2(w: B, x: B) -> B {
    let base = b_to_string(w);
    let name = b_to_string(x);
    let result = std::path::Path::new(&base).join(&name);
    let result_str = result.to_string_lossy().into_owned();
    let chars: Vec<u32> = result_str.chars().map(|c| c as u32).collect();
    crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
}

/// •file.Name: extract filename (basename) from path.
fn file_name_c1(x: B) -> B {
    let path_str = b_to_string(x);
    let name = std::path::Path::new(&path_str)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let chars: Vec<u32> = name.chars().map(|c| c as u32).collect();
    crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
}

/// •file.Parent: extract parent directory from path (includes trailing slash).
fn file_parent_c1(x: B) -> B {
    let path_str = b_to_string(x);
    let p = std::path::Path::new(&path_str);
    let parent = p.parent()
        .map(|d| {
            let mut s = d.to_string_lossy().into_owned();
            if !s.is_empty() && !s.ends_with('/') {
                s.push('/');
            }
            s
        })
        .unwrap_or_default();
    let chars: Vec<u32> = parent.chars().map(|c| c as u32).collect();
    crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
}

/// •file.Exists: check if file or directory exists. Returns 0 or 1.
fn file_exists_c1(x: B) -> B {
    let path_str = resolve_path(b_to_string(x));
    let exists = std::path::Path::new(&path_str).exists();
    B::m_f64(if exists { 1.0 } else { 0.0 })
}

/// •file.Type: return file type as character — 'f' (file), 'd' (dir), 'l' (symlink).
fn file_type_c1(x: B) -> B {
    let path_str = resolve_path(b_to_string(x));
    let p = std::path::Path::new(&path_str);
    let meta = std::fs::symlink_metadata(&path_str).unwrap_or_else(|e| {
        rbqn_core::error::throw(format!("•file.Type: cannot stat {path_str}: {e}"))
    });
    let ch = if meta.file_type().is_symlink() {
        'l'
    } else if p.is_dir() {
        'd'
    } else {
        'f'
    };
    B::m_c32(ch as u32)
}

/// •file.CreateDir: create directory including parents (like mkdir -p).
fn file_createdir_c1(x: B) -> B {
    let path_str = resolve_path(b_to_string(x));
    std::fs::create_dir_all(&path_str).unwrap_or_else(|e| {
        rbqn_core::error::throw(format!("•file.CreateDir: cannot create {path_str}: {e}"))
    });
    x
}

/// •file.Rename dyadic: rename file w to x.
fn file_rename_c2(w: B, x: B) -> B {
    let old = resolve_path(b_to_string(w));
    let new = resolve_path(b_to_string(x));
    std::fs::rename(&old, &new).unwrap_or_else(|e| {
        rbqn_core::error::throw(format!("•file.Rename: cannot rename {old} to {new}: {e}"))
    });
    x
}

/// •file.Remove: remove file.
fn file_remove_c1(x: B) -> B {
    let path_str = resolve_path(b_to_string(x));
    std::fs::remove_file(&path_str).unwrap_or_else(|e| {
        rbqn_core::error::throw(format!("•file.Remove: cannot remove {path_str}: {e}"))
    });
    x
}

// --- •Import ---

/// •Import: load and cache a BQN source file.
fn sys_import_c1(x: B) -> B {
    let path_str = b_to_string(x);
    let resolved = resolve_path(path_str);

    // Canonicalize path for cache key
    let canonical = std::fs::canonicalize(&resolved)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| resolved.clone());

    // Check cache first
    {
        let cache = IMPORT_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(&cached) = cache.get(&canonical) {
            if cached.0 == B::SENTINEL.0 {
                rbqn_core::error::throw(format!("•Import: circular import detected: {canonical}"));
            }
            return cached;
        }
    }

    // Insert sentinel to detect circular imports
    {
        let mut cache = IMPORT_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        cache.insert(canonical.clone(), B::SENTINEL);
    }

    // Read file contents
    let contents = std::fs::read_to_string(&resolved).unwrap_or_else(|e| {
        rbqn_core::error::throw(format!("•Import: cannot read {resolved}: {e}"))
    });

    // Set •path and •name for the imported file's context
    let import_dir = std::path::Path::new(&resolved)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let old_path = SYS_PATH.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let old_name = SYS_NAME.lock().unwrap_or_else(|e| e.into_inner()).clone();
    set_sys_path(&resolved);

    // Compile and execute the imported file using •BQN machinery
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        dispatch_sys_bqn_eval_with_path(&contents, &import_dir)
    }));

    // Restore •path and •name
    *SYS_PATH.lock().unwrap_or_else(|e| e.into_inner()) = old_path;
    *SYS_NAME.lock().unwrap_or_else(|e| e.into_inner()) = old_name;

    let result = match result {
        Ok(v) => v,
        Err(_) => {
            // Remove sentinel on error so caller can retry
            IMPORT_CACHE.lock().unwrap_or_else(|e| e.into_inner()).remove(&canonical);
            rbqn_core::error::throw(format!("•Import: error loading {resolved}"))
        }
    };

    // Cache the result
    IMPORT_CACHE.lock().unwrap_or_else(|e| e.into_inner()).insert(canonical, result);
    result
}

/// Execute BQN source with a given working path context.
fn dispatch_sys_bqn_eval_with_path(src: &str, _dir: &str) -> B {
    dispatch_sys_bqn_eval(src)
}

// --- Utility functions ---

/// •ParseFloat: convert BQN string to f64.
/// Handles BQN number format: ¯ (minus), ∞ (infinity), π (pi).
fn sys_parsefloat_c1(x: B) -> B {
    let s = b_to_string(x);
    // Pre-process BQN number format
    let normalized: String = s.chars().map(|c| match c {
        '¯' => '-',
        c => c,
    }).collect();
    // Handle special values
    let val: f64 = if normalized == "∞" || normalized == "inf" {
        f64::INFINITY
    } else if normalized == "-∞" || normalized == "-inf" {
        f64::NEG_INFINITY
    } else if normalized == "π" {
        std::f64::consts::PI
    } else if normalized == "-π" {
        -std::f64::consts::PI
    } else {
        // Replace ∞ and π occurrences in mixed expressions
        let s2 = normalized
            .replace('∞', "inf")
            .replace('π', "3.141592653589793");
        s2.parse::<f64>().unwrap_or_else(|_| {
            rbqn_core::error::throw(format!("•ParseFloat: cannot parse {:?}", s))
        })
    };
    B::m_f64(val)
}

/// •Hash: compute a deterministic hash of a BQN value. Returns a non-negative f64.
fn sys_hash_c1(x: B) -> B {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash_b(b: B) -> u64 {
        let mut h = DefaultHasher::new();
        b.0.hash(&mut h);
        if b.is_arr() {
            if let Some(arr) = crate::vm::get_arr(b) {
                for i in 0..arr.ia() {
                    let elem = arr.get(i).unwrap_or(B::SENTINEL);
                    hash_b(elem).hash(&mut h);
                }
            }
        }
        h.finish()
    }

    // Return as positive f64 (mask off sign bit)
    let hash = hash_b(x);
    let positive = (hash & 0x7FFFFFFFFFFFFFFF) as f64;
    B::m_f64(positive)
}

/// •FromUTF8: convert byte array (numeric values 0-255) to character array.
fn sys_fromutf8_c1(x: B) -> B {
    let bytes = b_to_byte_vec(x);
    let s = String::from_utf8(bytes).unwrap_or_else(|e| {
        rbqn_core::error::throw(format!("•FromUTF8: invalid UTF-8: {e}"))
    });
    let chars: Vec<u32> = s.chars().map(|c| c as u32).collect();
    crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
}

/// •ToUTF8: convert character array to UTF-8 byte array.
fn sys_toutf8_c1(x: B) -> B {
    let s = b_to_string(x);
    let bytes: Vec<f64> = s.bytes().map(|b| b as f64).collect();
    crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_f64(bytes))
}

/// •Cmp dyadic: total order comparison. Returns ¯1 (w < x), 0 (w = x), 1 (w > x).
fn sys_cmp_c2(w: B, x: B) -> B {
    B::m_i32(total_order_cmp(w, x))
}

/// Total order comparison for BQN values.
/// Type order: number(0) < character(1) < array(2) < function(3) < md1(4) < md2(5) < namespace(6)
fn total_order_cmp(a: B, b: B) -> i32 {
    let type_a = bqn_type_ord(a);
    let type_b = bqn_type_ord(b);
    if type_a != type_b {
        return if type_a < type_b { -1 } else { 1 };
    }
    // Same type — compare by value
    match type_a {
        0 => { // numbers
            let fa = a.o2f();
            let fb = b.o2f();
            if fa < fb { -1 } else if fa > fb { 1 } else { 0 }
        }
        1 => { // characters
            let ca = a.0 as u32;
            let cb = b.0 as u32;
            if ca < cb { -1 } else if ca > cb { 1 } else { 0 }
        }
        2 => { // arrays — lexicographic comparison
            let aa = crate::vm::get_arr(a);
            let ab = crate::vm::get_arr(b);
            match (aa, ab) {
                (Some(arr_a), Some(arr_b)) => {
                    let len = arr_a.ia().min(arr_b.ia());
                    for i in 0..len {
                        let ea = arr_a.get(i).unwrap_or(B::SENTINEL);
                        let eb = arr_b.get(i).unwrap_or(B::SENTINEL);
                        let c = total_order_cmp(ea, eb);
                        if c != 0 { return c; }
                    }
                    if arr_a.ia() < arr_b.ia() { -1 }
                    else if arr_a.ia() > arr_b.ia() { 1 }
                    else { 0 }
                }
                _ => 0,
            }
        }
        _ => { // functions/modifiers: compare by raw id
            if a.0 < b.0 { -1 } else if a.0 > b.0 { 1 } else { 0 }
        }
    }
}

/// Map a B value to a type ordinal for total ordering.
fn bqn_type_ord(b: B) -> i32 {
    if b.is_f64() { 0 }
    else if b.is_c32() { 1 }
    else if b.is_arr() { 2 }
    else if b.is_fun() { 3 }
    else if b.is_md1() { 4 }
    else if b.is_md2() { 5 }
    else { 6 } // namespace or other
}

/// Resolve a path: if relative, join with •path directory. Otherwise return as-is.
fn resolve_path(path: String) -> String {
    let p = std::path::Path::new(&path);
    if p.is_absolute() {
        return path;
    }
    // Try to get •path (the script's directory)
    let sys_path = SYS_PATH.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(path_b) = sys_path {
        let path_str = b_to_string(path_b);
        if !path_str.is_empty() {
            let base = std::path::Path::new(&path_str);
            // •path is the directory (not the file), so join directly
            let joined = base.join(&path);
            return joined.to_string_lossy().into_owned();
        }
    }
    path
}

/// Convert a B array of numeric values to a Vec<u8> (byte values 0-255).
fn b_to_byte_vec(x: B) -> Vec<u8> {
    if let Some(arr) = crate::vm::get_arr(x) {
        (0..arr.ia())
            .filter_map(|i| arr.get(i).ok())
            .map(|b| {
                if b.is_f64() {
                    (b.o2f() as u8)
                } else if b.is_c32() {
                    (b.0 as u8)
                } else {
                    0u8
                }
            })
            .collect()
    } else {
        vec![]
    }
}

/// Public wrapper for b_to_string, used by compiler.rs for nameList resolution.
pub fn b_to_string_pub(x: B) -> String {
    b_to_string(x)
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

// =============================================================================
// •math namespace
// =============================================================================

/// Cached •math namespace object.
static MATH_NS: std::sync::LazyLock<Mutex<Option<B>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

fn make_math_namespace() -> B {
    let mut guard = MATH_NS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ns_b) = *guard {
        return ns_b;
    }

    use crate::namespace::{str2gid, NSDesc, NS};

    // Fields: sin, cos, tan, asin, acos, atan, log, cbrt, hypot, erf, comb, fact, gcd, lcm, pi
    let gids = vec![
        str2gid("sin"),   // 0
        str2gid("cos"),   // 1
        str2gid("tan"),   // 2
        str2gid("asin"),  // 3
        str2gid("acos"),  // 4
        str2gid("atan"),  // 5
        str2gid("log"),   // 6
        str2gid("cbrt"),  // 7
        str2gid("hypot"), // 8
        str2gid("erf"),   // 9
        str2gid("comb"),  // 10
        str2gid("fact"),  // 11
        str2gid("gcd"),   // 12
        str2gid("lcm"),   // 13
        str2gid("pi"),    // 14
    ];
    let var_am: i32 = gids.len() as i32;
    let var_am_u16 = var_am as u16;

    let desc = Arc::new(NSDesc { var_am, exp_gids: gids });
    let body = Arc::new(crate::block::Body::new(var_am_u16, 0, 0, 0));
    let sc = Arc::new(crate::scope::Scope::new(
        body,
        None,
        var_am_u16,
        &[
            m_sys_fn(1100), // sin
            m_sys_fn(1101), // cos
            m_sys_fn(1102), // tan
            m_sys_fn(1103), // asin
            m_sys_fn(1104), // acos
            m_sys_fn(1105), // atan (mono+dyadic atan2)
            m_sys_fn(1106), // log (mono ln, dyadic log_w)
            m_sys_fn(1107), // cbrt
            m_sys_fn(1108), // hypot (dyadic only, mono falls back)
            m_sys_fn(1109), // erf
            m_sys_fn(1110), // comb
            m_sys_fn(1111), // fact
            m_sys_fn(1112), // gcd
            m_sys_fn(1113), // lcm
            B::m_f64(std::f64::consts::PI), // pi — immediate constant
        ],
    ));

    let ns = NS { desc, sc };
    let ns_b = crate::namespace::store_ns(ns);
    *guard = Some(ns_b);
    ns_b
}

// NOTE: Math function helpers — all operate on scalar f64
fn math_scalar(x: B) -> f64 { x.o2f() }

fn math_sin_c1(x: B) -> B  { B::m_f64(math_scalar(x).sin()) }
fn math_cos_c1(x: B) -> B  { B::m_f64(math_scalar(x).cos()) }
fn math_tan_c1(x: B) -> B  { B::m_f64(math_scalar(x).tan()) }
fn math_asin_c1(x: B) -> B { B::m_f64(math_scalar(x).asin()) }
fn math_acos_c1(x: B) -> B { B::m_f64(math_scalar(x).acos()) }
fn math_atan_c1(x: B) -> B { B::m_f64(math_scalar(x).atan()) }
fn math_log_c1(x: B) -> B  { B::m_f64(math_scalar(x).ln()) }
fn math_cbrt_c1(x: B) -> B { B::m_f64(math_scalar(x).cbrt()) }

fn math_hypot_c1(x: B) -> B {
    // Monadic hypot: |x|
    B::m_f64(math_scalar(x).abs())
}

/// Erf approximation (Abramowitz & Stegun, max error ~1.5e-7)
fn erf_approx(x: f64) -> f64 {
    let a1 =  0.254829592_f64;
    let a2 = -0.284496736_f64;
    let a3 =  1.421413741_f64;
    let a4 = -1.453152027_f64;
    let a5 =  1.061405429_f64;
    let p  =  0.3275911_f64;
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
    sign * y
}

fn math_erf_c1(x: B) -> B { B::m_f64(erf_approx(math_scalar(x))) }

fn math_fact_c1(x: B) -> B {
    let n = math_scalar(x);
    if n < 0.0 {
        return B::m_f64(f64::NAN);
    }
    if n.fract() == 0.0 && n <= 20.0 {
        // Exact integer factorial
        let mut result = 1u64;
        for i in 1..=(n as u64) {
            result = result.saturating_mul(i);
        }
        B::m_f64(result as f64)
    } else {
        // Use Stirling / lgamma approximation: n! = Γ(n+1) = exp(lgamma(n+1))
        B::m_f64(lgamma_approx(n + 1.0).exp())
    }
}

/// Simple lgamma approximation using Lanczos method (g=7, n=9 coefficients)
fn lgamma_approx(x: f64) -> f64 {
    // NOTE: Use Lanczos approximation. Coefficients for g=7.
    const G: f64 = 7.0;
    const COEFFS: [f64; 9] = [
        0.99999999999980993,
        676.5203681218851,
        -1259.1392167224028,
        771.32342877765313,
        -176.61502916214059,
        12.507343278686905,
        -0.13857109526572012,
        9.9843695780195716e-6,
        1.5056327351493116e-7,
    ];
    if x < 0.5 {
        std::f64::consts::PI.ln() - ((std::f64::consts::PI * x).sin().ln()) - lgamma_approx(1.0 - x)
    } else {
        let x = x - 1.0;
        let mut a = COEFFS[0];
        for (i, &c) in COEFFS[1..].iter().enumerate() {
            a += c / (x + (i + 1) as f64);
        }
        let t = x + G + 0.5;
        0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
    }
}

fn math_comb_c1(x: B) -> B {
    // Monadic: C(x, 0) = 1
    B::m_f64(1.0)
}

fn math_comb_c2(w: B, x: B) -> B {
    // w •math.Comb x = C(x, w) = x! / (w! * (x-w)!)
    let n = x.o2f().round() as i64;
    let k = w.o2f().round() as i64;
    if k < 0 || k > n {
        return B::m_f64(0.0);
    }
    let k = k.min(n - k); // symmetry: C(n,k) = C(n,n-k)
    let mut result = 1.0_f64;
    for i in 0..k {
        result = result * (n - i) as f64 / (i + 1) as f64;
    }
    B::m_f64(result.round())
}

fn math_gcd_c1(x: B) -> B {
    // Monadic: gcd(x, 0) = |x|
    B::m_f64(x.o2f().abs())
}

fn math_gcd_c2(w: B, x: B) -> B {
    let mut a = w.o2f().round() as i64;
    let mut b = x.o2f().round() as i64;
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    B::m_f64(a as f64)
}

fn math_lcm_c1(x: B) -> B {
    // Monadic: lcm(x, 0) = 0
    B::m_f64(0.0)
}

fn math_lcm_c2(w: B, x: B) -> B {
    let a = w.o2f().round() as i64;
    let b = x.o2f().round() as i64;
    if a == 0 || b == 0 {
        return B::m_f64(0.0);
    }
    // gcd
    let mut ga = a.abs();
    let mut gb = b.abs();
    while gb != 0 {
        let t = gb;
        gb = ga % gb;
        ga = t;
    }
    let gcd = ga;
    B::m_f64((a.abs() / gcd * b.abs()) as f64)
}

// =============================================================================
// •rand namespace
// =============================================================================

/// Wyrand PRNG state.
static RAND_STATE: std::sync::LazyLock<Mutex<u64>> = std::sync::LazyLock::new(|| {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    Mutex::new(seed ^ 0xa0761d6478bd642f)
});

fn wyrand(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0xa0761d6478bd642f);
    let a = *state;
    let b = a ^ 0xe7037ed1a0b428db;
    let t = (a as u128).wrapping_mul(b as u128);
    ((t >> 64) ^ t) as u64
}

fn rand_next() -> u64 {
    let mut guard = RAND_STATE.lock().unwrap_or_else(|e| e.into_inner());
    wyrand(&mut guard)
}

/// Cached •rand namespace object.
static RAND_NS: std::sync::LazyLock<Mutex<Option<B>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

fn make_rand_namespace() -> B {
    let mut guard = RAND_NS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ns_b) = *guard {
        return ns_b;
    }

    use crate::namespace::{str2gid, NSDesc, NS};

    let gids = vec![
        str2gid("range"),   // 0
        str2gid("deal"),    // 1
        str2gid("subset"),  // 2
    ];
    let var_am: i32 = gids.len() as i32;
    let var_am_u16 = var_am as u16;

    let desc = Arc::new(NSDesc { var_am, exp_gids: gids });
    let body = Arc::new(crate::block::Body::new(var_am_u16, 0, 0, 0));
    let sc = Arc::new(crate::scope::Scope::new(
        body,
        None,
        var_am_u16,
        &[
            m_sys_fn(1121), // Range
            m_sys_fn(1122), // Deal
            m_sys_fn(1123), // Subset
        ],
    ));

    let ns = NS { desc, sc };
    let ns_b = crate::namespace::store_ns(ns);
    *guard = Some(ns_b);
    ns_b
}

fn rand_range_c1(x: B) -> B {
    // •rand.Range n — random integer in [0, n)
    let n = x.o2f().round() as u64;
    if n == 0 { return B::m_f64(0.0); }
    let r = rand_next() % n;
    B::m_f64(r as f64)
}

fn rand_range_c2(w: B, x: B) -> B {
    // shape •rand.Range n — array of random integers in [0, n)
    let n = x.o2f().round() as u64;
    if !w.is_arr() {
        // w is a scalar: produce array of length w
        let len = w.o2f().round() as usize;
        let vals: Vec<f64> = (0..len).map(|_| (rand_next() % n.max(1)) as f64).collect();
        let arr = rbqn_core::array::BqnArr::new_vec_f64(vals);
        return crate::vm::tag_arr(arr);
    }
    // w is a shape array: produce shaped array
    let shape_arr = match crate::vm::get_arr(w) {
        Some(a) => a,
        None => rbqn_core::error::throw("•rand.Range: 𝕨 must be a shape"),
    };
    let shape: Vec<usize> = (0..shape_arr.ia())
        .map(|i| shape_arr.get(i).unwrap_or(B::SENTINEL).o2f().round() as usize)
        .collect();
    let total: usize = shape.iter().product();
    let vals: Vec<f64> = (0..total).map(|_| (rand_next() % n.max(1)) as f64).collect();
    let mut arr = rbqn_core::array::BqnArr::new_vec_f64(vals);
    arr.shape = shape;
    crate::vm::tag_arr(arr)
}

fn rand_deal_c1(x: B) -> B {
    // •rand.Deal n — random permutation of ↕n (Fisher-Yates shuffle)
    let n = x.o2f().round() as usize;
    let mut perm: Vec<f64> = (0..n).map(|i| i as f64).collect();
    for i in (1..n).rev() {
        let j = (rand_next() % (i + 1) as u64) as usize;
        perm.swap(i, j);
    }
    let arr = rbqn_core::array::BqnArr::new_vec_f64(perm);
    crate::vm::tag_arr(arr)
}

fn rand_subset_c1(x: B) -> B {
    // Monadic: treat as •rand.Deal (return permutation)
    rand_deal_c1(x)
}

fn rand_subset_c2(w: B, x: B) -> B {
    // k •rand.Subset n — k random distinct indices from ↕n
    let k = w.o2f().round() as usize;
    let n = x.o2f().round() as usize;
    if k > n { rbqn_core::error::throw("•rand.Subset: k > n"); }
    // Fisher-Yates partial shuffle
    let mut pool: Vec<usize> = (0..n).collect();
    let mut result = Vec::with_capacity(k);
    for i in 0..k {
        let j = i + (rand_next() % (n - i) as u64) as usize;
        pool.swap(i, j);
        result.push(pool[i] as f64);
    }
    let arr = rbqn_core::array::BqnArr::new_vec_f64(result);
    crate::vm::tag_arr(arr)
}

// =============================================================================
// •platform namespace
// =============================================================================

/// Cached •platform namespace object.
static PLATFORM_NS: std::sync::LazyLock<Mutex<Option<B>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

fn make_platform_namespace() -> B {
    let mut guard = PLATFORM_NS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ns_b) = *guard {
        return ns_b;
    }

    use crate::namespace::{str2gid, NSDesc, NS};

    // Fields: os, environment, arch, bqn.impl (mapped as "impl")
    let gids = vec![
        str2gid("os"),
        str2gid("environment"),
        str2gid("arch"),
        str2gid("impl"),
    ];
    let var_am: i32 = gids.len() as i32;
    let var_am_u16 = var_am as u16;

    // Build os string as immediate value
    let os_str = std::env::consts::OS; // "linux", "macos", "windows", etc.
    let os_chars: Vec<u32> = os_str.chars().map(|c| c as u32).collect();
    let os_b = crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(os_chars));

    // arch string
    let arch_str = std::env::consts::ARCH;
    let arch_chars: Vec<u32> = arch_str.chars().map(|c| c as u32).collect();
    let arch_b = crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(arch_chars));

    // impl string
    let impl_str = "RBQN";
    let impl_chars: Vec<u32> = impl_str.chars().map(|c| c as u32).collect();
    let impl_b = crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(impl_chars));

    let desc = Arc::new(NSDesc { var_am, exp_gids: gids });
    let body = Arc::new(crate::block::Body::new(var_am_u16, 0, 0, 0));
    let sc = Arc::new(crate::scope::Scope::new(
        body,
        None,
        var_am_u16,
        &[
            os_b,              // os — immediate string
            m_sys_fn(1130),    // environment — callable function
            arch_b,            // arch — immediate string
            impl_b,            // impl — immediate string
        ],
    ));

    let ns = NS { desc, sc };
    let ns_b = crate::namespace::store_ns(ns);
    *guard = Some(ns_b);
    ns_b
}

fn platform_env_c1(x: B) -> B {
    // •platform.environment "VAR" — look up environment variable
    let name = b_to_string(x);
    match std::env::var(&name) {
        Ok(val) => {
            let chars: Vec<u32> = val.chars().map(|c| c as u32).collect();
            crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
        }
        Err(_) => {
            // Return empty string if not found
            crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(vec![]))
        }
    }
}

// =============================================================================
// •SH — shell execution
// =============================================================================

fn sh_exec_c1(x: B) -> B {
    let cmd = b_to_string(x);
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()
        .unwrap_or_else(|e| rbqn_core::error::throw(format!("•SH: failed to run command: {e}")));

    let exit_code = output.status.code().unwrap_or(-1) as f64;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    let stdout_chars: Vec<u32> = stdout.chars().map(|c| c as u32).collect();
    let stderr_chars: Vec<u32> = stderr.chars().map(|c| c as u32).collect();
    let stdout_b = crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(stdout_chars));
    let stderr_b = crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(stderr_chars));

    let result = rbqn_core::array::BqnArr::from_b_vec(vec![
        B::m_f64(exit_code),
        stdout_b,
        stderr_b,
    ]);
    crate::vm::tag_arr(result)
}

fn sh_exec_c2(w: B, x: B) -> B {
    // w •SH x — w is options (ignored for now), x is command or array of command parts
    if x.is_arr() {
        if let Some(arr) = crate::vm::get_arr(x) {
            if arr.ia() > 0 {
                let cmd_b = arr.get(0).unwrap_or(B::SENTINEL);
                let cmd = b_to_string(cmd_b);
                let mut proc = std::process::Command::new(&cmd);
                for i in 1..arr.ia() {
                    let arg_b = arr.get(i).unwrap_or(B::SENTINEL);
                    proc.arg(b_to_string(arg_b));
                }
                let output = proc.output()
                    .unwrap_or_else(|e| rbqn_core::error::throw(format!("•SH: failed to run: {e}")));
                let exit_code = output.status.code().unwrap_or(-1) as f64;
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                let stdout_chars: Vec<u32> = stdout.chars().map(|c| c as u32).collect();
                let stderr_chars: Vec<u32> = stderr.chars().map(|c| c as u32).collect();
                let stdout_b = crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(stdout_chars));
                let stderr_b = crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(stderr_chars));
                let result = rbqn_core::array::BqnArr::from_b_vec(vec![
                    B::m_f64(exit_code), stdout_b, stderr_b,
                ]);
                return crate::vm::tag_arr(result);
            }
        }
    }
    // Fall back to monadic form with x as string command
    sh_exec_c1(x)
}

// ============================================================
// Structural equality for derived functions (used by ≡ and =)
// ============================================================

/// Compare two B values structurally, up to a recursion depth limit.
fn b_struct_equal(w: B, x: B, depth: u32) -> bool {
    if depth > 16 { return false; }
    if w.0 == x.0 { return true; }
    if w.is_f64() && x.is_f64() { return w.o2f() == x.o2f(); }
    if w.is_c32() && x.is_c32() { return w.0 as u32 == x.0 as u32; }
    if w.is_arr() && x.is_arr() {
        let wa = crate::vm::get_arr(w);
        let xa = crate::vm::get_arr(x);
        if let (Some(wa), Some(xa)) = (wa, xa) {
            if wa.shape != xa.shape { return false; }
            for i in 0..wa.ia() {
                let wv = wa.get(i).unwrap_or(B::SENTINEL);
                let xv = xa.get(i).unwrap_or(B::SENTINEL);
                if !b_struct_equal(wv, xv, depth + 1) { return false; }
            }
            return true;
        }
        return false;
    }
    if (w.is_fun() && x.is_fun()) || (w.is_md1() && x.is_md1()) || (w.is_md2() && x.is_md2()) {
        return derived_values_equal(w, x, depth + 1);
    }
    false
}

/// Compare two derived function/modifier values structurally.
/// Two derived values are equal if they have the same kind and the same operands.
pub fn derived_values_equal(w: B, x: B, depth: u32) -> bool {
    if depth > 16 { return false; }
    if w.0 == x.0 { return true; }

    let wid = (w.0 & 0xFFFFFFFFFFFF) >> 3;
    let xid = (x.0 & 0xFFFFFFFFFFFF) >> 3;
    let wd = get_derived(wid);
    let xd = get_derived(xid);

    // Kinds must match
    if wd.kind != xd.kind { return false; }

    // Compare operands based on kind
    match wd.kind {
        DerivedKind::NativeFn { prim_idx: wp } => {
            if let DerivedKind::NativeFn { prim_idx: xp } = xd.kind { wp == xp } else { false }
        }
        DerivedKind::NativeMd1 { prim_idx: wp } => {
            if let DerivedKind::NativeMd1 { prim_idx: xp } = xd.kind { wp == xp } else { false }
        }
        DerivedKind::NativeMd2 { prim_idx: wp } => {
            if let DerivedKind::NativeMd2 { prim_idx: xp } = xd.kind { wp == xp } else { false }
        }
        DerivedKind::SysFn { sys_idx: ws } => {
            if let DerivedKind::SysFn { sys_idx: xs } = xd.kind { ws == xs } else { false }
        }
        DerivedKind::Fork | DerivedKind::Atop | DerivedKind::Md1D | DerivedKind::Md2D
        | DerivedKind::Md2PartialL | DerivedKind::Md2PartialR => {
            b_struct_equal(wd.f, xd.f, depth + 1)
                && b_struct_equal(wd.g, xd.g, depth + 1)
                && b_struct_equal(wd.h, xd.h, depth + 1)
        }
        // Block functions/modifiers: equal only if same block (same Arc pointer)
        DerivedKind::FunBlock | DerivedKind::Md1Block | DerivedKind::Md2Block => {
            // Compare block identity by pointer
            let w_bl_ptr = wd.bl.as_ref().map(|bl| Arc::as_ptr(bl) as usize);
            let x_bl_ptr = xd.bl.as_ref().map(|bl| Arc::as_ptr(bl) as usize);
            w_bl_ptr == x_bl_ptr
        }
        _ => false, // Other derived kinds not compared structurally
    }
}

/// Register derived equality with rbqn-core so that ≡ and = work for functions.
pub fn register_derived_equality() {
    rbqn_core::compare::register_derived_equal_fn(|w, x, depth| {
        derived_values_equal(w, x, depth)
    });
}
