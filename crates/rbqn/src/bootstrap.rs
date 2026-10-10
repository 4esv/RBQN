
use rbqn_core::array::BqnArr;
use rbqn_core::error::BqnError;
use rbqn_core::B;
use rbqn_prim::Primitive;
use rbqn_vm::block::eval_fun_block;
use rbqn_vm::compiler::compile_all;
use rbqn_vm::derive::{c1, c2, m_native_md2, m_sys_fn, prim_to_b};
use rbqn_vm::scope::Scope;
use rbqn_vm::vm::{get_arr, tag_arr};

use crate::embedded::{self, BlockEntry, ObjectEntry, OwnedBytecode};

const RT_LEN: usize = 64;

#[allow(dead_code)]
pub struct Runtime {
    pub prims: Vec<Primitive>,
    pub fruntime: Vec<B>,
    pub runtime_0: Vec<B>,
    pub runtime: Vec<B>,
    pub compgen: B,
    pub compiler: B,
    pub formatter: Option<(B, B)>,
    pub glyphs: Vec<Vec<u32>>,
}

/// Indices into the provide array, matching CBQN's builtins.h provide order.
/// provide[0..22] = basic provide, provide[23..39] = extended provide.
#[allow(dead_code)]
mod prov {
    // Basic provide (23 entries):
    //  type, fill, log, grLen, grOrd, asrt,
    //  add, sub, mul, div, pow, floor,
    //  eq, le, fne, shape, pick, ud,
    //  tbl, scan, fillBy, val, catch
    pub const TYPE: usize = 0;
    pub const FILL: usize = 1;
    pub const LOG: usize = 2;
    pub const GR_LEN: usize = 3;
    pub const GR_ORD: usize = 4;
    pub const ASRT: usize = 5;
    pub const ADD: usize = 6;
    pub const SUB: usize = 7;
    pub const MUL: usize = 8;
    pub const DIV: usize = 9;
    pub const POW: usize = 10;
    pub const FLOOR: usize = 11;
    pub const EQ: usize = 12;
    pub const LE: usize = 13;
    pub const FNE: usize = 14;
    pub const SHAPE: usize = 15;
    pub const PICK: usize = 16;
    pub const UD: usize = 17;
    pub const TBL: usize = 18;
    pub const SCAN: usize = 19;
    pub const FILL_BY: usize = 20;
    pub const VAL: usize = 21;
    pub const CATCH: usize = 22;
    // Extended provide (17 more):
    pub const ROOT: usize = 23;
    pub const NOT: usize = 24;
    pub const AND: usize = 25;
    pub const OR: usize = 26;
    pub const FEQ: usize = 27;
    pub const COUPLE: usize = 28;
    pub const SHIFTA: usize = 29;
    pub const SHIFTB: usize = 30;
    pub const REVERSE: usize = 31;
    pub const TRANSP: usize = 32;
    pub const GRADE_UP: usize = 33;
    pub const GRADE_DOWN: usize = 34;
    pub const INDEX_OF: usize = 35;
    pub const COUNT: usize = 36;
    pub const MEMBER_OF: usize = 37;
    pub const CELL: usize = 38;
    pub const RANK: usize = 39;
}

/// Map provide index → fruntime index (primitive dispatch table index).
/// System functions (type, fill, log, grLen, grOrd, fillBy) are mapped to
/// system function B values instead of fruntime entries.
pub fn build_provide(fruntime: &[B]) -> Vec<B> {
    let mut provide = Vec::with_capacity(40);
    // NOTE: provide indices don't match fruntime indices.
    // provide maps: name → which primitive or system fn to use.

    // 0: type → •Type (sys fn)
    provide.push(m_sys_fn(0));
    // 1: fill → •Fill (sys fn)
    provide.push(m_sys_fn(7));
    // 2: log → ⋆ (pow, prim index 4) — the runtime uses log as pow's c1
    provide.push(fruntime[4]);
    // 3: grLen → •GroupLen (sys fn)
    provide.push(m_sys_fn(22));
    // 4: grOrd → •GroupOrd (sys fn)
    provide.push(m_sys_fn(23));
    // 5: asrt → ! (prim index 43)
    provide.push(fruntime[43]);
    // 6: add → + (prim index 0)
    provide.push(fruntime[0]);
    // 7: sub → - (prim index 1)
    provide.push(fruntime[1]);
    // 8: mul → × (prim index 2)
    provide.push(fruntime[2]);
    // 9: div → ÷ (prim index 3)
    provide.push(fruntime[3]);
    // 10: pow → ⋆ (prim index 4)
    provide.push(fruntime[4]);
    // 11: floor → ⌊ (prim index 6)
    provide.push(fruntime[6]);
    // 12: eq → = (prim index 15)
    provide.push(fruntime[15]);
    // 13: le → ≤ (prim index 16)
    provide.push(fruntime[16]);
    // 14: fne → ≢ (prim index 19)
    provide.push(fruntime[19]);
    // 15: shape → ≢ (prim index 19) — actually shape is ≢ for provide
    // Wait — CBQN uses bi_shape = ⥊ (prim 22). Let me check...
    // CBQN: bi_shape is ≢ (index 22 in fruntime which is ⥊)
    // Actually in CBQN fruntime: index 22 = ⥊ (shape/reshape)
    // But provide[15] = "shape" which maps to bi_shape
    // bi_shape in CBQN is the ≢ function... no.
    // Looking at CBQN load.c: provide = { bi_type, bi_fill, bi_log, bi_grLen, bi_grOrd, bi_asrt,
    //   bi_add, bi_sub, bi_mul, bi_div, bi_pow, bi_floor,
    //   bi_eq, bi_le, bi_fne, bi_shape, bi_pick, bi_ud,
    //   bi_tbl, bi_scan, bi_fillBy, bi_val, bi_catch, ... }
    // bi_shape = ⥊ which is fruntime index 22
    provide.pop(); // remove the wrong ≢ we pushed
    provide.push(fruntime[22]); // 14: fne → ≢ is prim 19
    // Hmm, let me redo this more carefully.

    // Actually, let me just clear and rebuild properly.
    provide.clear();

    // Map from CBQN load.c provide array:
    //   bi_type, bi_fill, bi_log, bi_grLen, bi_grOrd, bi_asrt,
    //   bi_add, bi_sub, bi_mul, bi_div, bi_pow, bi_floor,
    //   bi_eq, bi_le, bi_fne, bi_shape, bi_pick, bi_ud,
    //   bi_tbl, bi_scan, bi_fillBy, bi_val, bi_catch,
    //   bi_root, bi_not, bi_and, bi_or, bi_feq, bi_couple,
    //   bi_shifta, bi_shiftb, bi_reverse, bi_transp,
    //   bi_gradeUp, bi_gradeDown, bi_indexOf, bi_count,
    //   bi_memberOf, bi_cell, bi_rank

    // Now map CBQN's bi_NAME to our fruntime index:
    // fruntime layout (64 entries, matching CBQN prim order):
    //  0: +    1: -    2: ×    3: ÷    4: ⋆    5: √    6: ⌊    7: ⌈    8: |    9: ¬
    // 10: ∧   11: ∨   12: <   13: >   14: ≠   15: =   16: ≤   17: ≥   18: ≡   19: ≢
    // 20: ⊣   21: ⊢   22: ⥊   23: ∾   24: ≍   25: ⋈   26: ↑   27: ↓   28: ↕   29: «
    // 30: »   31: ⌽   32: ⍉   33: /   34: ⍋   35: ⍒   36: ⊏   37: ⊑   38: ⊐   39: ⊒
    // 40: ∊   41: ⍷   42: ⊔   43: !   44: ˙   45: ˜   46: ˘   47: ¨   48: ⌜   49: ⁼
    // 50: ´   51: ˝   52: `   53: ∘   54: ○   55: ⊸   56: ⟜   57: ⌾   58: ⊘   59: ◶
    // 60: ⎉   61: ⚇   62: ⍟   63: ⎊

    let p = |idx: usize| fruntime[idx];

    // Basic provide (23):
    provide.push(m_sys_fn(0));     //  0: type    → •Type
    provide.push(m_sys_fn(7));     //  1: fill    → •Fill
    provide.push(p(4));            //  2: log     → ⋆ (bi_log = bi_pow in CBQN)
    provide.push(m_sys_fn(22));    //  3: grLen   → •GroupLen
    provide.push(m_sys_fn(23));    //  4: grOrd   → •GroupOrd
    provide.push(p(43));           //  5: asrt    → !
    provide.push(p(0));            //  6: add     → +
    provide.push(p(1));            //  7: sub     → -
    provide.push(p(2));            //  8: mul     → ×
    provide.push(p(3));            //  9: div     → ÷
    provide.push(p(4));            // 10: pow     → ⋆
    provide.push(p(6));            // 11: floor   → ⌊
    provide.push(p(15));           // 12: eq      → =
    provide.push(p(16));           // 13: le      → ≤
    provide.push(p(19));           // 14: fne     → ≢
    provide.push(p(22));           // 15: shape   → ⥊
    provide.push(p(37));           // 16: pick    → ⊑
    provide.push(p(28));           // 17: ud      → ↕
    provide.push(p(48));           // 18: tbl     → ⌜
    provide.push(p(52));           // 19: scan    → `
    provide.push(m_native_md2(rbqn_vm::modifiers::MD2_FILL_BY)); // 20: fillBy → •_fillBy_ (2-modifier)
    provide.push(p(58));           // 21: val     → ⊘
    provide.push(p(63));           // 22: catch   → ⎊

    // Extended provide (17):
    provide.push(p(5));            // 23: root     → √
    provide.push(p(9));            // 24: not      → ¬
    provide.push(p(10));           // 25: and      → ∧
    provide.push(p(11));           // 26: or       → ∨
    provide.push(p(18));           // 27: feq      → ≡
    provide.push(p(24));           // 28: couple   → ≍
    provide.push(p(29));           // 29: shifta   → «
    provide.push(p(30));           // 30: shiftb   → »
    provide.push(p(31));           // 31: reverse  → ⌽
    provide.push(p(32));           // 32: transp   → ⍉
    provide.push(p(34));           // 33: gradeUp  → ⍋
    provide.push(p(35));           // 34: gradeDown→ ⍒
    provide.push(p(38));           // 35: indexOf  → ⊐
    provide.push(p(39));           // 36: count    → ⊒
    provide.push(p(40));           // 37: memberOf → ∊
    provide.push(p(46));           // 38: cell     → ˘
    provide.push(p(60));           // 39: rank     → ⎉

    provide
}

/// Reconstruct the objects array (a0) from embedded data.
/// Objects can be: Provide(n) → provide[n], Float(v) → B::m_f64(v),
/// Char(c) → B::m_c32(c), IArr(n) → tag_arr(iarrs[n]), Str(s) → tag_arr(string),
/// Runtime(n) / RuntimePrev(n) → runtime_ref[n]
fn build_objs(
    emb: &OwnedBytecode,
    provide: &[B],
    runtime_prev: Option<&[B]>,
    runtime_ref: Option<&[B]>,
) -> Vec<B> {
    emb.objs.iter().map(|entry| {
        match entry {
            ObjectEntry::Provide(n) => provide[*n],
            ObjectEntry::Float(v) => B::m_f64(*v),
            ObjectEntry::Char(c) => B::m_c32(*c),
            ObjectEntry::IArr(n) => {
                let data: Vec<i32> = emb.iarrs[*n].to_vec();
                tag_arr(BqnArr::new_vec_i32(data))
            }
            ObjectEntry::Str(s) => {
                // s is &Vec<u32>, auto-derefs to &[u32]
                let chars: Vec<u32> = s.to_vec();
                tag_arr(BqnArr::new_vec_c32(chars))
            }
            ObjectEntry::RuntimePrev(n) => {
                runtime_prev.map_or(B::SENTINEL, |rt| rt[*n])
            }
            ObjectEntry::Runtime(n) => {
                runtime_ref.map_or(B::SENTINEL, |rt| rt[*n])
            }
        }
    }).collect()
}

/// Reconstruct the blocks array (a1) from embedded data.
/// In CBQN format, each block is either:
/// - A simple `[type, imm, bodyIndex]` integer array
/// - A `m_blockinfo(info, monadicBodies, dyadicBodies)` = `⟨type, imm, ⟨monadics, dyadics⟩⟩`
///
/// We convert them all to boxed B arrays for compile_all.
fn build_blocks(emb: &OwnedBytecode) -> Vec<B> {
    let mut result = Vec::with_capacity(emb.blocks.len());
    let mut i = 0;
    while i < emb.blocks.len() {
        match emb.blocks[i] {
            BlockEntry::IArr(n) => {
                // Simple block: [type, imm, bodyIndex_or_list]
                let data: Vec<i32> = emb.iarrs[n].to_vec();
                // Convert to boxed array of B values
                let elems: Vec<B> = data.iter().map(|&v| B::m_f64(v as f64)).collect();
                result.push(tag_arr(BqnArr::from_b_vec(elems)));
                i += 1;
            }
            BlockEntry::Info { typ, iarrs0_idx, data_idx } => {
                // m_blockinfo(info, c, d) → ⟨type, imm, ⟨c, d⟩⟩
                // info: type = info & 3, imm = info >> 2
                let ty = typ & 3;
                let imm = typ >> 2;
                // c = monadic body indices from iarrs[iarrs0_idx]
                // d = dyadic body indices from iarrs[data_idx]
                let c_data: Vec<B> = emb.iarrs[iarrs0_idx].iter().map(|&v| B::m_f64(v as f64)).collect();
                let d_data: Vec<B> = emb.iarrs[data_idx].iter().map(|&v| B::m_f64(v as f64)).collect();
                let c_arr = tag_arr(BqnArr::from_b_vec(c_data));
                let d_arr = tag_arr(BqnArr::from_b_vec(d_data));
                let body_pair = tag_arr(BqnArr::from_b_vec(vec![c_arr, d_arr]));
                let block = BqnArr::from_b_vec(vec![
                    B::m_f64(ty as f64),
                    B::m_f64(imm as f64),
                    body_pair,
                ]);
                result.push(tag_arr(block));
                i += 1;
            }
        }
    }
    result
}

/// Reconstruct the bodies array (a2) from embedded data.
/// Each body is `iarrs[n]` = `[bcOffset, varCount, ...]`
/// Convert to boxed B arrays for compile_all.
fn build_bodies(emb: &OwnedBytecode) -> Vec<B> {
    emb.bodies.iter().map(|&iarrs_idx| {
        let data: Vec<i32> = emb.iarrs[iarrs_idx].to_vec();
        let elems: Vec<B> = data.iter().map(|&v| B::m_f64(v as f64)).collect();
        tag_arr(BqnArr::from_b_vec(elems))
    }).collect()
}

/// Execute an embedded bytecode stage (runtime0, runtime1, or compiler).
/// Returns the result B value from evaluating the top-level block.
fn exec_stage(
    emb: &OwnedBytecode,
    objs: Vec<B>,
    blocks: Vec<B>,
    bodies: Vec<B>,
    _name: &str,
) -> Result<B, BqnError> {
    let block = compile_all(
        &emb.bc,
        objs,
        &blocks,
        &bodies,
        B::SENTINEL,  // indices
        B::SENTINEL,  // token_info
        B::SENTINEL,  // src
        B::SENTINEL,  // fullpath
        None,         // sc
        0,            // ns_result
    );

    // After compile_block swap, bodies[0] is the first monadic body
    let body = block.bodies[0].clone();
    let var_am = body.var_am;
    let root_scope = std::rc::Rc::new(Scope::new(body.clone(), None, var_am, &[]));

    Ok(eval_fun_block(block, root_scope))
}

std::thread_local! {
    /// (provide, runtime_0) from bootstrap, consumed by init_runtime1.
    static RT1_INPUTS: std::cell::RefCell<Option<(Vec<B>, Vec<B>)>> =
        const { std::cell::RefCell::new(None) };
}

/// Execute runtime1 and register what it provides: setPrims/setInv callbacks and the
/// BQN-defined ⌾ and ⚇. Runs on first use via rbqn_vm::derive::ensure_rt1.
fn init_runtime1() {
    let Some((provide, runtime_0)) = RT1_INPUTS.with(|c| c.borrow_mut().take()) else {
        return;
    };
    let mut t = std::time::Instant::now();
    let rt1_bin = embedded::decode_bytecode(embedded::RUNTIME1_BIN);
    // runtime1's objects reference runtime_0 results via RuntimePrev(n)
    let r1_objs = build_objs(&rt1_bin, &provide, Some(&runtime_0), None);
    let r1_blocks = build_blocks(&rt1_bin);
    let r1_bodies = build_bodies(&rt1_bin);

    let r1_result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        exec_stage(&rt1_bin, r1_objs, r1_blocks, r1_bodies, "runtime1")
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => {
            eprintln!("rbqn: warning: runtime1 failed: {e}. Inverses and BQN ⌾/⚇ unavailable.");
            return;
        }
        Err(panic) => {
            let msg = panic.downcast_ref::<String>().map(|s| s.as_str())
                .or_else(|| panic.downcast_ref::<&str>().copied())
                .unwrap_or("unknown");
            eprintln!("rbqn: warning: runtime1 panicked: {msg}. Inverses and BQN ⌾/⚇ unavailable.");
            return;
        }
    };

    // runtime1 returns ⟨runtime_array, setPrims, setInv⟩
    let Some(r1_arr) = get_arr(r1_result) else {
        eprintln!("rbqn: warning: runtime1 did not return an array. Inverses and BQN ⌾/⚇ unavailable.");
        return;
    };
    let rt_obj_raw = match r1_arr.get(0) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("rbqn: warning: runtime1 result missing element 0: {e}. Inverses and BQN ⌾/⚇ unavailable.");
            return;
        }
    };
    let set_prims = r1_arr.get(1).ok();
    let set_inv = r1_arr.get(2).ok();

    // NOTE: Like CBQN (load.c line 522), the runtime array uses native fruntime primitives
    // instead of the BQN-defined wrappers from runtime1. CBQN does this when all
    // builtins are natively implemented (rtComplete[] all true):
    //   B r = nnbi? Get(rtObjRaw, i) : inc(fruntime[i]);
    // The BQN wrappers (e.g. `Indices ⊘ Replicate` for `/`) don't have primitive indices,
    // which breaks the inverse system (⁼) since it uses •PrimInd + •Glyph to look up
    // inverses by glyph character. Native fruntime entries have correct prim_idx values.

    // Invoke setPrims callback — registers •Decompose and •PrimInd with the runtime.
    // CBQN: c1(setPrims, ⟨bi_decp, bi_primInd⟩)
    if let Some(sp) = set_prims {
        let decompose_fn = m_sys_fn(1);  // •Decompose
        let primind_fn = m_sys_fn(5);    // •PrimInd
        let args = tag_arr(BqnArr::from_b_vec(vec![decompose_fn, primind_fn]));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c1(sp, args)));
    }

    // Invoke setInv callback — registers inverse tables for ⁼ and ⌾.
    // CBQN: c2(setInv, bi_setInvSwap, bi_setInvReg) — called dyadically.
    // bi_setInvSwap = sys_idx 9, bi_setInvReg = sys_idx 8
    if let Some(si) = set_inv {
        let bi_set_inv_swap = m_sys_fn(9);
        let bi_set_inv_reg = m_sys_fn(8);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            c2(si, bi_set_inv_swap, bi_set_inv_reg)
        }));
    }

    // Extract BQN runtime's Under (⌾) function from rtObjRaw[57].
    // CBQN (load.c line 506): gc_add(rt_under = Get(rtObjRaw, n_under));
    // This is the BQN-defined Under that handles structural cases correctly.
    // Our native Under is a naive G⁻¹(F(G(x))) which fails for structural G
    // (e.g. mask⊸/). The BQN runtime's Under handles both computational and
    // structural cases, matching CBQN's def_fn_uc1 fallback behavior.
    if let Some(rt_obj_arr) = get_arr(rt_obj_raw) {
        if let Ok(rt_under_fn) = rt_obj_arr.get(57)
            && rt_under_fn.is_md2() {
                rbqn_vm::modifiers::set_rt_under(rt_under_fn);
            }
        // Extract BQN runtime's Depth (⚇) function from rtObjRaw[61]
        if let Ok(rt_depth_fn) = rt_obj_arr.get(61)
            && rt_depth_fn.is_md2() {
                rbqn_vm::modifiers::set_rt_depth(rt_depth_fn);
            }
    }
    crate::timing::lap(&mut t, "runtime1 (deferred)");
}

pub fn bootstrap() -> Result<Runtime, BqnError> {
    let mut t = std::time::Instant::now();
    let prims = rbqn_prim::get_runtime().to_vec();
    assert_eq!(prims.len(), RT_LEN, "primitive registry must have exactly {RT_LEN} entries");

    // Build fruntime: 64 B values, one per primitive, as callable NaN-boxed Derived values.
    let fruntime: Vec<B> = (0..RT_LEN).map(prim_to_b).collect();

    // Build glyphs as char arrays
    let fn_glyphs: Vec<u32> = "+-×÷⋆√⌊⌈|¬∧∨<>≠=≤≥≡≢⊣⊢⥊∾≍⋈↑↓↕«»⌽⍉/⍋⍒⊏⊑⊐⊒∊⍷⊔!".chars().map(|c| c as u32).collect();
    let md1_glyphs: Vec<u32> = "˙˜˘¨⌜⁼´˝`".chars().map(|c| c as u32).collect();
    let md2_glyphs: Vec<u32> = "∘○⊸⟜⌾⊘◶⎉⚇⍟⎊".chars().map(|c| c as u32).collect();
    let glyphs = vec![fn_glyphs, md1_glyphs, md2_glyphs];

    // Build provide array (40 entries) mapping names to callable B values.
    let provide = build_provide(&fruntime);
    crate::timing::lap(&mut t, "prims+build_provide");

    // Decode embedded bytecode from .bin files (committed to repo)
    let cc_bin = embedded::decode_bytecode(embedded::COMPILER_BIN);
    let fmt_bin = embedded::decode_bytecode(embedded::FORMATTER_BIN);
    crate::timing::lap(&mut t, "decode bins");

    if cc_bin.is_empty() {
        return Ok(Runtime {
            prims,
            fruntime: fruntime.clone(),
            runtime_0: vec![],
            runtime: fruntime,
            compgen: B::SENTINEL,
            compiler: B::SENTINEL,
            formatter: None,
            glyphs,
        });
    }

    // --- Stage 1: Build runtime_0 from native primitives ---
    // CBQN's bytecodeSubmodule build does NOT execute runtime0 bytecode — it builds
    // runtime_0 directly from native C functions (bi_floor, bi_ceil, etc.).
    // We do the same: map the 24 runtime_0 slots to our fruntime native primitives.
    //
    // CBQN load.c line 467 (when !ALL_R0):
    //   B runtime_0[] = {bi_floor,bi_ceil,bi_stile,bi_lt,bi_gt,bi_ne,bi_ge,
    //     bi_rtack,bi_ltack,bi_join,bi_pair,bi_take,bi_drop,bi_select,
    //     bi_const,bi_swap,bi_each,bi_fold,bi_atop,bi_over,bi_before,
    //     bi_after,bi_cond,bi_repeat};
    //
    // Our fruntime layout (indices): 0:+  1:-  2:×  3:÷  4:⋆  5:√  6:⌊  7:⌈  8:|  9:¬
    //  10:∧  11:∨  12:<  13:>  14:≠  15:=  16:≤  17:≥  18:≡  19:≢  20:⊣  21:⊢  22:⥊  23:∾
    //  24:≍  25:⋈  26:↑  27:↓  28:↕  29:«  30:»  31:⌽  32:⍉  33:/  34:⍋  35:⍒  36:⊏  37:⊑
    //  38:⊐  39:⊒  40:∊  41:⍷  42:⊔  43:!  44:˙  45:˜  46:˘  47:¨  48:⌜  49:⁼  50:´  51:˝
    //  52:`  53:∘  54:○  55:⊸  56:⟜  57:⌾  58:⊘  59:◶  60:⎉  61:⚇  62:⍟  63:⎊
    let runtime_0: Vec<B> = vec![
        fruntime[6],  //  0: ⌊ (floor/min)
        fruntime[7],  //  1: ⌈ (ceil/max)
        fruntime[8],  //  2: | (absolute value / modulus)
        fruntime[12], //  3: < (less than / box)
        fruntime[13], //  4: > (greater than / unbox)
        fruntime[14], //  5: ≠ (length / not-equal)
        fruntime[17], //  6: ≥ (greater or equal / sort up)
        fruntime[21], //  7: ⊢ (identity right / right)
        fruntime[20], //  8: ⊣ (identity left / left)
        fruntime[23], //  9: ∾ (join)
        fruntime[25], // 10: ⋈ (pair)
        fruntime[26], // 11: ↑ (take)
        fruntime[27], // 12: ↓ (drop)
        fruntime[36], // 13: ⊏ (select first/cells)
        fruntime[44], // 14: ˙ (constant)
        fruntime[45], // 15: ˜ (swap/self)
        fruntime[47], // 16: ¨ (each)
        fruntime[50], // 17: ´ (fold)
        fruntime[53], // 18: ∘ (atop)
        fruntime[54], // 19: ○ (over)
        fruntime[55], // 20: ⊸ (before/bind)
        fruntime[56], // 21: ⟜ (after/bind)
        fruntime[59], // 22: ◶ (choose)
        fruntime[62], // 23: ⍟ (repeat)
    ];
    // NOTE: runtime0 bytecode (embedded::RUNTIME0_BIN) is not decoded or executed
    // because the bytecodeSubmodule build expects native primitives as runtime_0, not BQN
    // derived functions. Running runtime0 bytecode would produce BQN FunBlocks that fail
    // in runtime1 context (confirmed: causes fork→add on function arrays crash).

    // Register primitive B values so reshape_computed can identify reshape modes.
    // fruntime[6] = ⌊ (floor, mode 1), fruntime[26] = ↑ (take, mode 3 = ceil+pad).
    rbqn_prim::structural::set_floor_prim(fruntime[6]);
    rbqn_prim::structural::set_take_prim(fruntime[26]);

    // --- Stage 2: runtime1, deferred to first use (#20) ---
    // Its only live outputs are the setInv resolvers and BQN ⌾/⚇, so it runs when
    // one of those is first needed. runtime_0 and provide are kept so it sees the
    // same primitive values as the compiler's objects.
    RT1_INPUTS.with(|c| *c.borrow_mut() = Some((provide.clone(), runtime_0.clone())));
    rbqn_vm::derive::register_rt1_init(init_runtime1);
    let runtime: Vec<B> = fruntime.clone();

    // --- Stage 3: Execute compiler (graceful fallback if it panics) ---
    // Swap in bi_casrt for assert during compilation (CBQN does this)
    let c_objs = build_objs(&cc_bin, &provide, Some(&runtime_0), Some(&runtime));
    let c_blocks = build_blocks(&cc_bin);
    let c_bodies = build_bodies(&cc_bin);

    let (compgen, compiler) = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        exec_stage(&cc_bin, c_objs, c_blocks, c_bodies, "compiler")
    })) {
        Ok(Ok(cg)) => {
            // cg is a function: call it with glyphs to get the actual compiler
            let glyphs_b = {
                let fn_arr = tag_arr(BqnArr::new_vec_c32(glyphs[0].clone()));
                let md1_arr = tag_arr(BqnArr::new_vec_c32(glyphs[1].clone()));
                let md2_arr = tag_arr(BqnArr::new_vec_c32(glyphs[2].clone()));
                tag_arr(BqnArr::from_b_vec(vec![fn_arr, md1_arr, md2_arr]))
            };
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                c1(cg, glyphs_b)
            })) {
                Ok(c) => (cg, c),
                Err(p) => {
                    let msg = if let Some(s) = p.downcast_ref::<String>() {
                        s.clone()
                    } else if let Some(s) = p.downcast_ref::<&str>() {
                        s.to_string()
                    } else {
                        "unknown panic".to_string()
                    };
                    eprintln!("rbqn: warning: compiler initialization panicked: {msg}");
                    (cg, B::SENTINEL)
                }
            }
        }
        Ok(Err(e)) => {
            eprintln!("rbqn: warning: compiler failed: {e}. Compiler unavailable.");
            (B::SENTINEL, B::SENTINEL)
        }
        Err(_panic) => {
            eprintln!("rbqn: warning: compiler panicked. Compiler unavailable.");
            (B::SENTINEL, B::SENTINEL)
        }
    };

    // --- Stage 4: Execute formatter (optional) ---
    crate::timing::lap(&mut t, "compiler setup");
    let formatter = if !fmt_bin.is_empty() {
        let f_objs = build_objs(&fmt_bin, &provide, Some(&runtime_0), Some(&runtime));
        let f_blocks = build_blocks(&fmt_bin);
        let f_bodies = build_bodies(&fmt_bin);

        match exec_stage(&fmt_bin, f_objs, f_blocks, f_bodies, "formatter") {
            Ok(fmt_mod) => {
                // Formatter module: call with ⟨•Type, •Decompose, •Glyph, •Repr⟩
                let type_fn = m_sys_fn(0);
                let decompose_fn = m_sys_fn(1);
                let glyph_fn = m_sys_fn(4);
                // NOTE: Pass sys_fn(200) as the native repr function.
                // sys_fn(200) formats values natively (Rust-level) without going through the
                // BQN formatter, avoiding infinite recursion. The BQN formatter's repr function
                // uses this for atomic values (numbers, chars); arrays are formatted recursively.
                let repr_fn = m_sys_fn(200);
                let args = tag_arr(BqnArr::from_b_vec(vec![type_fn, decompose_fn, glyph_fn, repr_fn]));
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c1(fmt_mod, args))) {
                    Ok(fmt_result) => {
                        if let Some(arr) = get_arr(fmt_result) {
                            let fmt = arr.get(0).ok();
                            let repr = arr.get(1).ok();
                            match (fmt, repr) {
                                (Some(f), Some(r)) => Some((f, r)),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    }
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    crate::timing::lap(&mut t, "formatter setup");
    Ok(Runtime {
        prims,
        fruntime,
        runtime_0,
        runtime,
        compgen,
        compiler,
        formatter,
        glyphs,
    })
}
