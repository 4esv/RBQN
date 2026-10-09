// Native modifier dispatch: implements all 1-modifiers (idx 44-52) and 2-modifiers (idx 53-63).
// Modifier logic lives here rather than in rbqn-prim because modifiers need to call
// derive::c1/c2 to apply operand functions, which creates a circular dep if in rbqn-prim.

use std::sync::OnceLock;

use rbqn_core::{B, BqnArr, ArrData, ElType};

use crate::derive::{c1, c2};

// NOTE: GPU fold hook — reduces large numeric arrays on GPU
// Returns Some(B) if GPU handled the fold, None for CPU fallback
type GpuFoldFn = fn(f: B, arr: &BqnArr) -> Option<B>;
static GPU_FOLD_HOOK: OnceLock<GpuFoldFn> = OnceLock::new();

pub fn register_gpu_fold(f: GpuFoldFn) {
    let _ = GPU_FOLD_HOOK.set(f);
}

// NOTE: GPU scan hook — prefix-sums large numeric arrays on GPU
// Returns Some(B) if GPU handled the scan, None for CPU fallback
type GpuScanFn = fn(f: B, arr: &BqnArr) -> Option<B>;
static GPU_SCAN_HOOK: OnceLock<GpuScanFn> = OnceLock::new();

pub fn register_gpu_scan(f: GpuScanFn) {
    let _ = GPU_SCAN_HOOK.set(f);
}

// BQN runtime's Under (⌾) function, set after runtime1 loads.
// Used as fallback when native Under can't handle a case.
static RT_UNDER: std::sync::LazyLock<std::sync::Mutex<Option<B>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// Store the BQN runtime's Under function (called from bootstrap after runtime1).
pub fn set_rt_under(f: B) {
    *RT_UNDER.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
}

/// Get the BQN runtime's Under function, if available.
fn get_rt_under() -> Option<B> {
    *RT_UNDER.lock().unwrap_or_else(|e| e.into_inner())
}

// BQN runtime's Depth (⚇) function, set after runtime1 loads.
// Used as fallback for non-zero depth values.
static RT_DEPTH: std::sync::LazyLock<std::sync::Mutex<Option<B>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// Store the BQN runtime's Depth function (called from bootstrap after runtime1).
pub fn set_rt_depth(f: B) {
    *RT_DEPTH.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
}

/// Get the BQN runtime's Depth function, if available.
fn get_rt_depth() -> Option<B> {
    *RT_DEPTH.lock().unwrap_or_else(|e| e.into_inner())
}

// 1-modifier indices
const MD1_CONST: usize = 44;  // ˙
const MD1_SWAP: usize = 45;   // ˜
const MD1_CELL: usize = 46;   // ˘
const MD1_EACH: usize = 47;   // ¨
const MD1_TBL: usize = 48;    // ⌜
const MD1_UNDO: usize = 49;   // ⁼
const MD1_FOLD: usize = 50;   // ´
const MD1_INSERT: usize = 51; // ˝
const MD1_SCAN: usize = 52;   // `

// 2-modifier indices
const MD2_ATOP: usize = 53;     // ∘
const MD2_OVER: usize = 54;     // ○
const MD2_BEFORE: usize = 55;   // ⊸
const MD2_AFTER: usize = 56;    // ⟜
const MD2_UNDER: usize = 57;    // ⌾
const MD2_VAL: usize = 58;      // ⊘
const MD2_COND: usize = 59;     // ◶
const MD2_RANK: usize = 60;     // ⎉
const MD2_DEPTH: usize = 61;    // ⚇
const MD2_REPEAT: usize = 62;   // ⍟
const MD2_CATCH: usize = 63;    // ⎊
// NOTE: special system 2-modifiers beyond the fruntime table
// •_fillBy_: F •_fillBy_ G applies F and uses G only for fill element computation.
// Since we don't track fill elements, this just forwards to F.
pub const MD2_FILL_BY: usize = 64; // •_fillBy_
// NOTE: •_while_: F _while_ G — apply F while G returns 1
pub const MD2_WHILE: usize = 65; // •_while_

// ============================================================
// Top-level dispatch
// ============================================================

pub fn native_md1_c1(prim_idx: usize, operand: B, _self_val: B, x: B) -> B {
    match prim_idx {
        MD1_CONST => operand,
        MD1_SWAP => c2(operand, x, x),
        MD1_CELL => cells_c1(operand, x),
        MD1_EACH => each_c1(operand, x),
        MD1_TBL => each_c1(operand, x),
        MD1_UNDO => {
            // F⁼ x: look up the inverse of F, then call it monadically on x
            let inv_fn = crate::derive::inv_reg(operand);
            c1(inv_fn, x)
        }
        MD1_FOLD => fold_c1(operand, x),
        MD1_INSERT => insert_c1(operand, x),
        MD1_SCAN => scan_c1(operand, x),
        // •bit namespace 1-modifiers (monadic)
        70 => bit_cast_c1(operand, x),
        71 => bit_not_c1(operand, x),
        72 => bit_neg_c1(operand, x),
        73..=78 => rbqn_core::error::throw("•bit: this operation requires two arguments"),
        _ => rbqn_core::error::throw(format!(
            "native 1-modifier idx {} c1 not implemented", prim_idx
        )),
    }
}

pub fn native_md1_c2(prim_idx: usize, operand: B, _self_val: B, w: B, x: B) -> B {
    match prim_idx {
        MD1_CONST => operand,
        MD1_SWAP => c2(operand, x, w),
        MD1_CELL => cells_c2(operand, w, x),
        MD1_EACH => each_c2(operand, w, x),
        MD1_TBL => table_c2(operand, w, x),
        MD1_UNDO => {
            // w F⁼ x: look up the inverse of F, then call it dyadically
            let inv_fn = crate::derive::inv_reg(operand);
            c2(inv_fn, w, x)
        }
        MD1_FOLD => fold_c2(operand, w, x),
        MD1_INSERT => insert_c2(operand, w, x),
        MD1_SCAN => scan_c2(operand, w, x),
        // •bit namespace 1-modifiers (dyadic)
        70 => bit_cast_c1(operand, x),  // _cast is monadic-only, dyadic = ignore w
        71 => bit_not_c1(operand, x),   // _not is monadic-only
        72 => bit_neg_c1(operand, x),   // _neg is monadic-only
        73 => bit_binop_c2(operand, w, x, |a, b| a & b),  // _and
        74 => bit_binop_c2(operand, w, x, |a, b| a | b),  // _or
        75 => bit_binop_c2(operand, w, x, |a, b| a ^ b),  // _xor
        76 => bit_arith_c2(operand, w, x, |a, b| a.wrapping_add(b)),  // _add
        77 => bit_arith_c2(operand, w, x, |a, b| a.wrapping_sub(b)),  // _sub
        78 => bit_arith_c2(operand, w, x, |a, b| a.wrapping_mul(b)),  // _mul
        _ => rbqn_core::error::throw(format!(
            "native 1-modifier idx {} c2 not implemented", prim_idx
        )),
    }
}

pub fn native_md2_c1(prim_idx: usize, f: B, g: B, _self_val: B, x: B) -> B {
    match prim_idx {
        MD2_ATOP => c1(f, c1(g, x)),
        MD2_OVER => c1(f, c1(g, x)),
        MD2_BEFORE => {
            let fx = c1(f, x);
            c2(g, fx, x)
        }
        MD2_AFTER => {
            let gx = c1(g, x);
            c2(f, x, gx)
        }
        MD2_UNDER => under_c1(f, g, x),
        MD2_VAL => c1(f, x),
        MD2_COND => choose_c1(f, g, x),
        MD2_RANK => rank_c1(f, g, x),
        MD2_DEPTH => depth_c1(f, g, x),
        MD2_REPEAT => repeat_c1(f, g, x),
        MD2_CATCH => catch_c1(f, g, x),
        // NOTE: •_fillBy_: F •_fillBy_ G applies F; G only provides fill element (ignored here)
        MD2_FILL_BY => c1(f, x),
        // NOTE: •_while_: F _while_ G — apply F while G x returns 1
        MD2_WHILE => {
            let mut acc = x;
            loop {
                let cond = c1(g, acc);
                if !cond.is_f64() || cond.o2f() != 1.0 { break; }
                acc = c1(f, acc);
            }
            acc
        }
        _ => rbqn_core::error::throw(format!(
            "native 2-modifier idx {} c1 not implemented", prim_idx
        )),
    }
}

pub fn native_md2_c2(prim_idx: usize, f: B, g: B, _self_val: B, w: B, x: B) -> B {
    match prim_idx {
        MD2_ATOP => c1(f, c2(g, w, x)),
        MD2_OVER => {
            let gw = c1(g, w);
            let gx = c1(g, x);
            c2(f, gw, gx)
        }
        MD2_BEFORE => {
            let fw = c1(f, w);
            c2(g, fw, x)
        }
        MD2_AFTER => {
            let gx = c1(g, x);
            c2(f, w, gx)
        }
        MD2_UNDER => under_c2(f, g, w, x),
        MD2_VAL => c2(g, w, x),
        MD2_COND => choose_c2(f, g, w, x),
        MD2_RANK => rank_c2(f, g, w, x),
        MD2_DEPTH => depth_c2(f, g, w, x),
        MD2_REPEAT => repeat_c2(f, g, w, x),
        MD2_CATCH => catch_c2(f, g, w, x),
        // NOTE: •_fillBy_: F •_fillBy_ G applies F; G only provides fill element (ignored here)
        MD2_FILL_BY => c2(f, w, x),
        // NOTE: •_while_ dyadic: w F _while_ G x — apply F while G w x returns 1
        MD2_WHILE => {
            let mut acc = x;
            loop {
                let cond = c2(g, w, acc);
                if !cond.is_f64() || cond.o2f() != 1.0 { break; }
                acc = c2(f, w, acc);
            }
            acc
        }
        _ => rbqn_core::error::throw(format!(
            "native 2-modifier idx {} c2 not implemented", prim_idx
        )),
    }
}

// ============================================================
// Helpers
// ============================================================

/// Convert a Vec<B> of results into a typed BQN array.
/// Produces a numeric array (with squeeze), character array, or boxed array as appropriate.
fn results_to_arr(results: Vec<B>, shape: Vec<usize>) -> B {
    results_to_arr_fill(results, shape, None)
}

fn results_to_arr_fill(results: Vec<B>, shape: Vec<usize>, fill: Option<B>) -> B {
    let mut out = rbqn_core::array::typed_arr_from_b_vec(results, shape, fill);
    // NOTE: typed_arr_from_b_vec sets default fills (0.0/space) for non-empty arrays.
    // Restore our explicit fill (may be None) so callers retain control over fill values.
    out.fill = fill;
    crate::vm::tag_arr(out)
}

/// Merge cell results into a higher-rank array.
/// Used by ˘ (cells) where results from each cell are combined with
/// shape = leading_shape ∾ cell_result_shape.
/// If all results are scalars, produces a simple array.
/// If all results are arrays of the same shape, flattens and concatenates.
fn merge_cells_result(results: Vec<B>, lead_shape: Vec<usize>) -> B {
    // Empty or all-scalar cases: delegate to results_to_arr which handles typed dispatch.
    if results.is_empty() || results.iter().all(|b| !b.is_arr()) {
        return results_to_arr(results, lead_shape);
    }
    // All arrays with same cell shape → merge into higher-rank
    let arrs: Vec<std::sync::Arc<BqnArr>> = results.iter()
        .filter_map(|b| crate::vm::get_arr(*b))
        .collect();
    if arrs.len() == results.len() && !arrs.is_empty() {
        let cell_shape = &arrs[0].shape;
        if arrs.iter().all(|a| &a.shape == cell_shape) {
            let mut merged_shape = lead_shape;
            merged_shape.extend_from_slice(cell_shape);
            let total: usize = arrs.iter().map(|a| a.ia()).sum();
            let mut flat: Vec<B> = Vec::with_capacity(total);
            for a in &arrs {
                for i in 0..a.ia() {
                    flat.push(a.get(i).unwrap_or(B::SENTINEL));
                }
            }
            return results_to_arr(flat, merged_shape);
        }
    }
    // Fallback: boxed array
    let mut out = BqnArr::new_vec_b(results);
    out.shape = lead_shape;
    crate::vm::tag_arr(out)
}

fn arr_of(x: B) -> std::sync::Arc<BqnArr> {
    crate::vm::get_arr(x)
        .unwrap_or_else(|| rbqn_core::error::throw("Expected array argument"))
}

/// Normalize a scan cell result to match the expected cell_shape.
/// BQN scan always produces an array with the same shape as the input.
/// When F returns a scalar for a cell operation, replicate it cell_size times.
/// When F returns an array matching cell_shape, leave it as-is.
/// Otherwise leave as-is (will be boxed by merge_cells_result).
fn normalize_cell_result(result: B, cell_shape: &[usize], cell_size: usize) -> B {
    if cell_size <= 1 {
        return result;
    }
    // If result matches expected cell shape, nothing to do
    if result.is_arr() {
        let rarr = crate::vm::get_arr(result);
        if let Some(a) = rarr
            && a.shape == cell_shape {
                return result;
            }
        return result;
    }
    // Result is a scalar — replicate it to fill cell_shape
    if result.is_f64() || result.is_c32() || result.is_atom() {
        let vals: Vec<B> = std::iter::repeat_n(result, cell_size).collect();
        if result.is_f64() {
            let fvals: Vec<f64> = vals.iter().map(|b| b.o2f()).collect();
            let mut out = rbqn_core::array::BqnArr::new_vec_f64(fvals);
            out.shape = cell_shape.to_vec();
            return crate::vm::tag_arr(rbqn_core::array::squeeze_num(out));
        }
        if result.is_c32() {
            let cvals: Vec<u32> = vals.iter().map(|b| b.0 as u32).collect();
            let mut out = rbqn_core::array::BqnArr::new_vec_c32(cvals);
            out.shape = cell_shape.to_vec();
            return crate::vm::tag_arr(out);
        }
        // Generic boxed scalar
        let mut out = BqnArr::new_vec_b(vals);
        out.shape = cell_shape.to_vec();
        return crate::vm::tag_arr(out);
    }
    result
}

fn get_elem(arr: &BqnArr, i: usize) -> B {
    arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e))
}

/// Extract major cell `cell_idx` from a higher-rank array.
fn extract_cell(arr: &BqnArr, cell_idx: usize, cell_size: usize, cell_shape: &[usize]) -> BqnArr {
    let start = cell_idx * cell_size;
    let data = match &arr.data {
        ArrData::Bit(v) => {
            let nwords = cell_size.div_ceil(64);
            let mut bits = vec![0u64; nwords];
            for i in 0..cell_size {
                let src = start + i;
                let bit = (v[src / 64] >> (src % 64)) & 1;
                bits[i / 64] |= bit << (i % 64);
            }
            ArrData::Bit(bits)
        }
        ArrData::I8(v) => ArrData::I8(v[start..start + cell_size].to_vec()),
        ArrData::I16(v) => ArrData::I16(v[start..start + cell_size].to_vec()),
        ArrData::I32(v) => ArrData::I32(v[start..start + cell_size].to_vec()),
        ArrData::F64(v) => ArrData::F64(v[start..start + cell_size].to_vec()),
        ArrData::C8(v) => ArrData::C8(v[start..start + cell_size].to_vec()),
        ArrData::C16(v) => ArrData::C16(v[start..start + cell_size].to_vec()),
        ArrData::C32(v) => ArrData::C32(v[start..start + cell_size].to_vec()),
        ArrData::Boxed(v) => ArrData::Boxed(v[start..start + cell_size].to_vec()),
    };
    BqnArr {
        shape: cell_shape.to_vec(),
        data,
        fill: arr.fill,
    }
}

// ============================================================
// 1-modifier: ¨ Each
// ============================================================

fn each_c1(f: B, x: B) -> B {
    // NOTE: BQN f¨ on a scalar returns a rank-0 array (not a plain scalar).
    // This matches `f⌜scalar` behavior.
    if x.is_atom() {
        let result = c1(f, x);
        return results_to_arr(vec![result], vec![]);
    }
    let arr = arr_of(x);
    // Rank-0 array: apply f to its single element, return rank-0 result
    if arr.shape.is_empty() {
        let elem = get_elem(&arr, 0);
        let result = c1(f, elem);
        return results_to_arr(vec![result], vec![]);
    }
    let n = arr.ia();
    let mut results = Vec::with_capacity(n);
    for i in 0..n {
        let elem = get_elem(&arr, i);
        let result = c1(f, elem);
        results.push(result);
    }
    results_to_arr(results, arr.shape.clone())
}

fn each_c2(f: B, w: B, x: B) -> B {
    let w_is_atom = w.is_atom();
    let x_is_atom = x.is_atom();
    // NOTE: When both args are atoms (scalars), BQN w f¨ x returns a rank-0 array.
    if w_is_atom && x_is_atom {
        let result = c2(f, w, x);
        return results_to_arr(vec![result], vec![]);
    }
    if w_is_atom {
        let xarr = arr_of(x);
        // Rank-0 x: apply once, return rank-0
        if xarr.shape.is_empty() {
            let result = c2(f, w, get_elem(&xarr, 0));
            return results_to_arr(vec![result], vec![]);
        }
        let n = xarr.ia();
        let mut results = Vec::with_capacity(n);
        for i in 0..n {
            results.push(c2(f, w, get_elem(&xarr, i)));
        }
        return results_to_arr(results, xarr.shape.clone());
    }
    if x_is_atom {
        let warr = arr_of(w);
        // Rank-0 w: apply once, return rank-0
        if warr.shape.is_empty() {
            let result = c2(f, get_elem(&warr, 0), x);
            return results_to_arr(vec![result], vec![]);
        }
        let n = warr.ia();
        let mut results = Vec::with_capacity(n);
        for i in 0..n {
            results.push(c2(f, get_elem(&warr, i), x));
        }
        return results_to_arr(results, warr.shape.clone());
    }
    let warr = arr_of(w);
    let xarr = arr_of(x);
    // Rank-0 arrays: apply once
    if warr.shape.is_empty() && xarr.shape.is_empty() {
        let result = c2(f, get_elem(&warr, 0), get_elem(&xarr, 0));
        return results_to_arr(vec![result], vec![]);
    }
    // NOTE: BQN ¨ rank-0 broadcast: when one arg is rank-0, it broadcasts to match the other.
    // rank-0 w broadcasts over all elements of x (iterating x, w is constant).
    if warr.shape.is_empty() {
        let w_elem = get_elem(&warr, 0);
        let n = xarr.ia();
        let mut results = Vec::with_capacity(n);
        for i in 0..n {
            results.push(c2(f, w_elem, get_elem(&xarr, i)));
        }
        return results_to_arr(results, xarr.shape.clone());
    }
    // rank-0 x broadcasts over all elements of w.
    if xarr.shape.is_empty() {
        let x_elem = get_elem(&xarr, 0);
        let n = warr.ia();
        let mut results = Vec::with_capacity(n);
        for i in 0..n {
            results.push(c2(f, get_elem(&warr, i), x_elem));
        }
        return results_to_arr(results, warr.shape.clone());
    }
    if warr.shape == xarr.shape {
        let n = warr.ia();
        let mut results = Vec::with_capacity(n);
        for i in 0..n {
            results.push(c2(f, get_elem(&warr, i), get_elem(&xarr, i)));
        }
        return results_to_arr(results, warr.shape.clone());
    }

    // NOTE: BQN ¨ leading-axis broadcast: if w.shape is a prefix of x.shape,
    // iterate over major cells of x, pairing each with the corresponding element of w.
    // Result shape = x.shape (the longer one). Vice versa for x.shape prefix of w.shape.
    let ws = &warr.shape;
    let xs = &xarr.shape;
    if xs.starts_with(ws) && ws.len() < xs.len() {
        // w.shape is a strict prefix of x.shape: w[i] applied to each major cell of x[i]
        let lead = warr.ia(); // total elements of w = major cell count matched
        let cell_size: usize = xs[ws.len()..].iter().product::<usize>().max(1);
        let mut results = Vec::with_capacity(lead * cell_size);
        for i in 0..lead {
            let w_elem = get_elem(&warr, i);
            for j in 0..cell_size {
                results.push(c2(f, w_elem, get_elem(&xarr, i * cell_size + j)));
            }
        }
        return results_to_arr(results, xs.clone());
    }
    if ws.starts_with(xs) && xs.len() < ws.len() {
        // x.shape is a strict prefix of w.shape: x[i] applied to each major cell of w[i]
        let lead = xarr.ia();
        let cell_size: usize = ws[xs.len()..].iter().product::<usize>().max(1);
        let mut results = Vec::with_capacity(lead * cell_size);
        for i in 0..lead {
            let x_elem = get_elem(&xarr, i);
            for j in 0..cell_size {
                results.push(c2(f, get_elem(&warr, i * cell_size + j), x_elem));
            }
        }
        return results_to_arr(results, ws.clone());
    }

    rbqn_core::error::throw(format!(
        "¨: 𝕨 and 𝕩 must have the same shape ({:?} ≡ ≢𝕨, {:?} ≡ ≢𝕩)",
        warr.shape, xarr.shape
    ))
}

// ============================================================
// 1-modifier: ⌜ Table
// ============================================================

fn table_c2(f: B, w: B, x: B) -> B {
    // NOTE: BQN allows scalar (atom) args to ⌜. Scalars have shape [] and contribute
    // no dimensions to the result: result shape = w.shape ∾ x.shape.
    // For scalar w and array x: result has x.shape. For both scalars: result is rank-0.
    match (w.is_atom(), x.is_atom()) {
        (true, true) => {
            // scalar ⌜ scalar → rank-0 result (shape [])
            let val = c2(f, w, x);
            results_to_arr(vec![val], vec![])
        }
        (true, false) => {
            // scalar ⌜ array → iterate x, result shape = x.shape
            let xarr = arr_of(x);
            let xn = xarr.ia();
            let mut results = Vec::with_capacity(xn);
            for j in 0..xn {
                results.push(c2(f, w, get_elem(&xarr, j)));
            }
            results_to_arr(results, xarr.shape.clone())
        }
        (false, true) => {
            // array ⌜ scalar → iterate w, result shape = w.shape
            let warr = arr_of(w);
            let wn = warr.ia();
            let mut results = Vec::with_capacity(wn);
            for i in 0..wn {
                results.push(c2(f, get_elem(&warr, i), x));
            }
            results_to_arr(results, warr.shape.clone())
        }
        (false, false) => {
            // array ⌜ array → original behavior
            let warr = arr_of(w);
            let xarr = arr_of(x);
            let wn = warr.ia();
            let xn = xarr.ia();
            let mut results = Vec::with_capacity(wn * xn);
            for i in 0..wn {
                let wi = get_elem(&warr, i);
                for j in 0..xn {
                    results.push(c2(f, wi, get_elem(&xarr, j)));
                }
            }
            let mut shape = warr.shape.clone();
            shape.extend_from_slice(&xarr.shape);
            results_to_arr(results, shape)
        }
    }
}

// ============================================================
// 1-modifier: ´ Fold
// ============================================================

/// Get the identity element for a function (used for empty-array fold).
fn fold_identity(f: B) -> Option<B> {
    if !f.is_fun() { return None; }
    let fid = (f.0 & 0xFFFFFFFFFFFF) >> 3;
    let d = crate::derive::get_derived(fid);
    match d.kind {
        crate::derive::DerivedKind::NativeFn { prim_idx } => match prim_idx {
            0 => Some(B::m_f64(0.0)),    // + → 0
            1 => Some(B::m_f64(0.0)),    // - → 0
            2 => Some(B::m_f64(1.0)),    // × → 1
            3 => Some(B::m_f64(1.0)),    // ÷ → 1
            4 => Some(B::m_f64(1.0)),    // ⋆ → 1
            6 => Some(B::m_f64(f64::INFINITY)),   // ⌊ → ∞
            7 => Some(B::m_f64(f64::NEG_INFINITY)), // ⌈ → -∞
            9 => Some(B::m_f64(1.0)),    // ¬ → 1
            10 => Some(B::m_f64(1.0)),   // ∧ → 1
            11 => Some(B::m_f64(0.0)),   // ∨ → 0
            // NOTE: Comparison functions as fold identity elements (BQN spec)
            12 => Some(B::m_f64(0.0)),   // < → 0
            13 => Some(B::m_f64(0.0)),   // > → 0
            14 => Some(B::m_f64(0.0)),   // ≠ → 0
            15 => Some(B::m_f64(1.0)),   // = → 1
            16 => Some(B::m_f64(1.0)),   // ≤ → 1
            17 => Some(B::m_f64(1.0)),   // ≥ → 1
            // NOTE: ∾´⟨⟩ should fail — no universal identity for join (type depends on context)
            // Per BQN spec, ∾ has no identity element
            _ => None,
        },
        _ => None,
    }
}

fn fold_c1(f: B, x: B) -> B {
    let arr = arr_of(x);
    if arr.rank() == 0 {
        rbqn_core::error::throw("´: 𝕩 must have rank ≥ 1");
    }
    let n = arr.ia();
    if n == 0 {
        return fold_identity(f).unwrap_or_else(||
            rbqn_core::error::throw("´: empty array with no identity")
        );
    }
    // GPU dispatch for large rank-1 numeric arrays with supported ops
    if arr.rank() == 1
        && let Some(hook) = GPU_FOLD_HOOK.get()
            && let Some(result) = hook(f, &arr) {
                return result;
            }
    if let Some(r) = crate::typed::fold(f, &arr, None) {
        return r;
    }
    let mut acc = get_elem(&arr, n - 1);
    for i in (0..n - 1).rev() {
        acc = c2(f, get_elem(&arr, i), acc);
    }
    acc
}

fn fold_c2(f: B, w: B, x: B) -> B {
    let arr = arr_of(x);
    if w.is_f64()
        && let Some(r) = crate::typed::fold(f, &arr, Some(w.o2f())) {
            return r;
        }
    let n = arr.ia();
    let mut acc = w;
    for i in (0..n).rev() {
        acc = c2(f, get_elem(&arr, i), acc);
    }
    acc
}

// ============================================================
// 1-modifier: ˝ Insert
// ============================================================

/// Identity for ∾˝ (join-insert) on empty leading axis.
/// For input shape 0‿s₁‿s₂‿...‿sₙ, result shape is 0‿s₂‿...‿sₙ.
fn insert_join_identity(f: B, cell_shape: &[usize], arr: &BqnArr) -> Option<B> {
    if !f.is_fun() { return None; }
    let fid = (f.0 & 0xFFFFFFFFFFFF) >> 3;
    let d = crate::derive::get_derived(fid);
    match d.kind {
        crate::derive::DerivedKind::NativeFn { prim_idx: 23 } => {
            // ∾ (join): identity = empty array with shape 0‿cell_shape[1..]
            let mut result_shape = vec![0usize];
            if cell_shape.len() > 1 {
                result_shape.extend_from_slice(&cell_shape[1..]);
            }
            let mut result_arr = rbqn_core::array::typed_arr_from_b_vec(vec![], result_shape, None);
            result_arr.fill = arr.fill;
            Some(crate::vm::tag_arr(result_arr))
        },
        _ => None,
    }
}

fn insert_c1(f: B, x: B) -> B {
    let arr = arr_of(x);
    if arr.rank() == 0 {
        rbqn_core::error::throw("˝: 𝕩 must have rank ≥ 1");
    }
    if arr.rank() == 1 {
        // Insert on rank-1: fold between rank-0 cells (enclosed elements).
        // BQN: F˝ = F´˘, so cells are rank-0 arrays, not bare atoms.
        let n = arr.shape[0];
        if n == 0 {
            if let Some(id) = fold_identity(f) {
                return id;
            }
            rbqn_core::error::throw("˝: empty array with no identity");
        }
        let enclose_elem = |e: B| -> B {
            crate::vm::tag_arr(BqnArr {
                shape: vec![],
                data: ArrData::Boxed(vec![e]),
                fill: None,
            })
        };
        let mut acc = enclose_elem(get_elem(&arr, n - 1));
        for i in (0..n - 1).rev() {
            let cell = enclose_elem(get_elem(&arr, i));
            acc = c2(f, cell, acc);
        }
        return acc;
    }
    let lead = arr.shape[0];
    if lead == 0 {
        // NOTE: Empty leading axis: return identity element in cell shape.
        // CBQN: insert on empty array returns identity element broadcast to cell shape.
        let cell_shape = arr.shape[1..].to_vec();
        let cell_ia: usize = cell_shape.iter().product::<usize>().max(1);

        // Special case: ∾˝ (join insert) on empty leading axis
        // Identity is an empty array with shape 0‿cell_shape[1..]
        if let Some(join_id) = insert_join_identity(f, &cell_shape, &arr) {
            return join_id;
        }

        if let Some(id) = fold_identity(f) {
            // Fill cell_shape with identity value
            let elems: Vec<B> = (0..cell_ia).map(|_| id).collect();
            let cell = crate::vm::tag_arr(rbqn_core::array::typed_arr_from_b_vec(elems, cell_shape, None));
            return cell;
        }
        rbqn_core::error::throw("˝: empty leading axis with no identity");
    }
    let cell_size: usize = arr.shape[1..].iter().product();
    let cell_shape = arr.shape[1..].to_vec();
    let mut acc = crate::vm::tag_arr(extract_cell(&arr, lead - 1, cell_size, &cell_shape));
    for i in (0..lead - 1).rev() {
        let cell = crate::vm::tag_arr(extract_cell(&arr, i, cell_size, &cell_shape));
        acc = c2(f, cell, acc);
    }
    acc
}

fn insert_c2(f: B, w: B, x: B) -> B {
    let arr = arr_of(x);
    if arr.rank() < 2 {
        return fold_c2(f, w, x);
    }
    let lead = arr.shape[0];
    let cell_size: usize = arr.shape[1..].iter().product();
    let cell_shape = arr.shape[1..].to_vec();
    let mut acc = w;
    for i in (0..lead).rev() {
        let cell = crate::vm::tag_arr(extract_cell(&arr, i, cell_size, &cell_shape));
        acc = c2(f, cell, acc);
    }
    acc
}

// ============================================================
// 1-modifier: ` Scan
// ============================================================

fn scan_c1(f: B, x: B) -> B {
    if x.is_atom() {
        rbqn_core::error::throw("`: 𝕩 must be an array");
    }
    let arr = arr_of(x);
    let rank = arr.rank();
    if rank == 0 {
        rbqn_core::error::throw("`: 𝕩 must have rank ≥ 1");
    }
    let lead = arr.shape[0];
    if lead == 0 {
        return crate::vm::tag_arr(BqnArr {
            shape: arr.shape.clone(),
            data: ArrData::Boxed(vec![]),
            fill: arr.fill,
        });
    }
    if rank == 1 {
        // GPU dispatch for large rank-1 numeric arrays with supported ops
        if let Some(hook) = GPU_SCAN_HOOK.get()
            && let Some(result) = hook(f, &arr) {
                return result;
            }
        // Rank-1: scan over individual elements
        let n = arr.ia();
        let mut results = Vec::with_capacity(n);
        results.push(get_elem(&arr, 0));
        for i in 1..n {
            let prev = results[i - 1];
            results.push(c2(f, prev, get_elem(&arr, i)));
        }
        return results_to_arr_fill(results, arr.shape.clone(), arr.fill);
    }
    // Rank > 1: scan operates on major cells (slices along axis 0)
    // Apply F between consecutive cells; result has same shape as input
    let cell_size: usize = arr.shape[1..].iter().product();
    let cell_shape = arr.shape[1..].to_vec();
    // NOTE: when cell_size=0, F is never called (no elements to process), return input as-is
    if cell_size == 0 {
        return x;
    }
    let mut cell_results: Vec<B> = Vec::with_capacity(lead);
    // First cell is copied as-is
    let first_cell = crate::vm::tag_arr(extract_cell(&arr, 0, cell_size, &cell_shape));
    cell_results.push(first_cell);
    for i in 1..lead {
        let prev_cell = cell_results[i - 1];
        let curr_cell = crate::vm::tag_arr(extract_cell(&arr, i, cell_size, &cell_shape));
        // NOTE: normalize result to cell_shape so merge_cells_result can build correct shape
        let result = c2(f, prev_cell, curr_cell);
        cell_results.push(normalize_cell_result(result, &cell_shape, cell_size));
    }
    // Merge cell results back into shape of x
    merge_cells_result(cell_results, vec![lead])
}

fn scan_c2(f: B, w: B, x: B) -> B {
    if x.is_atom() {
        rbqn_core::error::throw("𝕨F`𝕩: 𝕩 must be an array");
    }
    let arr = arr_of(x);
    let rank = arr.rank();
    if rank == 0 {
        return crate::vm::tag_arr((*arr).clone());
    }
    let lead = arr.shape[0];
    if rank == 1 {
        // Rank-1: scan with initial value w over elements
        let n = arr.ia();
        let mut results = Vec::with_capacity(n);
        let mut acc = w;
        for i in 0..n {
            acc = c2(f, acc, get_elem(&arr, i));
            results.push(acc);
        }
        return results_to_arr_fill(results, arr.shape.clone(), arr.fill);
    }
    // Rank > 1: scan operates on major cells with initial cell w
    // w must have shape matching the cell shape of x
    let cell_size: usize = arr.shape[1..].iter().product();
    let cell_shape = arr.shape[1..].to_vec();
    // NOTE: when cell_size=0, F is never called (no elements to process), return input as-is
    if cell_size == 0 {
        return x;
    }
    // Validate that w has the right shape
    if w.is_arr() {
        let warr = arr_of(w);
        if warr.shape != cell_shape {
            rbqn_core::error::throw(format!(
                "𝕨F`𝕩: Shape of 𝕨 must match the cell of 𝕩 ({:?} ≡ ≢𝕨, {:?} ≡ ≢𝕩)",
                warr.shape, arr.shape
            ));
        }
    }
    let mut cell_results: Vec<B> = Vec::with_capacity(lead);
    let mut acc = w;
    for i in 0..lead {
        let curr_cell = crate::vm::tag_arr(extract_cell(&arr, i, cell_size, &cell_shape));
        acc = c2(f, acc, curr_cell);
        // NOTE: normalize result to cell_shape so merge_cells_result can build correct shape
        let norm = normalize_cell_result(acc, &cell_shape, cell_size);
        acc = norm;
        cell_results.push(acc);
    }
    merge_cells_result(cell_results, vec![lead])
}

// ============================================================
// Scan inverse helpers (used by ScanInv dispatch in derive.rs)
// ============================================================

/// Monadic scan inverse: (F`⁼ x) — undo the scan.
/// result[0] = x[0], result[i] = x[i] F⁼ x[i-1]  (or x[i] F⁼ cell[i-1] for rank>1)
pub fn scan_inv_c1(f: B, x: B) -> B {
    if x.is_atom() {
        rbqn_core::error::throw("F`⁼𝕩: 𝕩 must be an array");
    }
    let arr = arr_of(x);
    let rank = arr.rank();
    if rank == 0 {
        return crate::vm::tag_arr((*arr).clone());
    }
    let lead = arr.shape[0];
    if lead == 0 {
        return crate::vm::tag_arr(BqnArr {
            shape: arr.shape.clone(),
            data: ArrData::Boxed(vec![]),
            fill: arr.fill,
        });
    }
    // Get the inverse of F for applying between consecutive elements
    let f_inv = crate::derive::inv_reg(f);
    if rank == 1 {
        let n = arr.ia();
        let mut results = Vec::with_capacity(n);
        results.push(get_elem(&arr, 0));
        for i in 1..n {
            // result[i] = x[i] F⁼ x[i-1]  (dyadic inverse: find y s.t. x[i-1] F y = x[i])
            let prev = get_elem(&arr, i - 1);
            let curr = get_elem(&arr, i);
            results.push(c2(f_inv, prev, curr));
        }
        return results_to_arr(results, arr.shape.clone());
    }
    // Rank > 1: scan inverse on major cells
    let cell_size: usize = arr.shape[1..].iter().product();
    let cell_shape = arr.shape[1..].to_vec();
    let mut cell_results: Vec<B> = Vec::with_capacity(lead);
    // First cell is copied as-is
    let first_cell = crate::vm::tag_arr(extract_cell(&arr, 0, cell_size, &cell_shape));
    cell_results.push(first_cell);
    for i in 1..lead {
        let prev_cell = crate::vm::tag_arr(extract_cell(&arr, i - 1, cell_size, &cell_shape));
        let curr_cell = crate::vm::tag_arr(extract_cell(&arr, i, cell_size, &cell_shape));
        // result[i] = curr_cell F⁼ prev_cell
        cell_results.push(c2(f_inv, prev_cell, curr_cell));
    }
    merge_cells_result(cell_results, vec![lead])
}

/// Dyadic scan inverse: (w F`⁼ x) — undo a scan with initial value w.
/// result[0] = x[0] F⁼ w,  result[i] = x[i] F⁼ x[i-1]
pub fn scan_inv_c2(f: B, w: B, x: B) -> B {
    if x.is_atom() {
        rbqn_core::error::throw("𝕨F`⁼𝕩: 𝕩 must be an array");
    }
    let arr = arr_of(x);
    let rank = arr.rank();
    if rank == 0 {
        return crate::vm::tag_arr((*arr).clone());
    }
    let lead = arr.shape[0];
    let f_inv = crate::derive::inv_reg(f);
    if rank == 1 {
        let n = arr.ia();
        let mut results = Vec::with_capacity(n);
        if n > 0 {
            // result[0] = x[0] F⁼ w
            results.push(c2(f_inv, w, get_elem(&arr, 0)));
            for i in 1..n {
                let prev = get_elem(&arr, i - 1);
                let curr = get_elem(&arr, i);
                results.push(c2(f_inv, prev, curr));
            }
        }
        return results_to_arr(results, arr.shape.clone());
    }
    // Rank > 1: scan inverse on major cells with initial cell w
    let cell_size: usize = arr.shape[1..].iter().product();
    let cell_shape = arr.shape[1..].to_vec();
    // Validate w shape matches cell shape
    if w.is_arr() {
        let warr = arr_of(w);
        if warr.shape != cell_shape {
            rbqn_core::error::throw(format!(
                "𝕨F`⁼𝕩: Shape of 𝕨 must match the cell of 𝕩 ({:?} ≡ ≢𝕨, {:?} ≡ ≢𝕩)",
                warr.shape, arr.shape
            ));
        }
    }
    let mut cell_results: Vec<B> = Vec::with_capacity(lead);
    if lead > 0 {
        // result[0] = x[0] F⁼ w
        let first_cell = crate::vm::tag_arr(extract_cell(&arr, 0, cell_size, &cell_shape));
        cell_results.push(c2(f_inv, w, first_cell));
        for i in 1..lead {
            let prev_cell = crate::vm::tag_arr(extract_cell(&arr, i - 1, cell_size, &cell_shape));
            let curr_cell = crate::vm::tag_arr(extract_cell(&arr, i, cell_size, &cell_shape));
            cell_results.push(c2(f_inv, prev_cell, curr_cell));
        }
    }
    merge_cells_result(cell_results, vec![lead])
}

// ============================================================
// 1-modifier: ˘ Cells
// ============================================================

fn cells_c1(f: B, x: B) -> B {
    if x.is_atom() {
        // BQN: F˘ on atom wraps result in rank-0 array
        let result = c1(f, x);
        if result.is_arr() {
            // Already an array — wrap in rank-0
            let inner = rbqn_core::get_arr(result);
            if let Some(arr) = inner
                && arr.shape.is_empty() {
                    // Already rank-0 — return as-is
                    return result;
                }
            return crate::vm::tag_arr(BqnArr {
                shape: vec![],
                data: ArrData::Boxed(vec![result]),
                fill: Some(rbqn_prim::structural::prototype_of(result)),
            });
        }
        // Scalar result: wrap in rank-0 array
        return crate::vm::tag_arr(BqnArr {
            shape: vec![],
            data: if result.is_f64() {
                ArrData::F64(vec![result.o2f()])
            } else if result.is_c32() {
                ArrData::C32(vec![result.0 as u32])
            } else {
                ArrData::Boxed(vec![result])
            },
            fill: Some(rbqn_prim::structural::prototype_of(result)),
        });
    }
    let arr = arr_of(x);
    // Rank-0 array: apply F to the whole rank-0 array; frame is ⟨⟩, merge result properly
    if arr.shape.is_empty() {
        let result = c1(f, x);
        return merge_cells_result(vec![result], vec![]);
    }
    let lead = arr.shape[0];
    if arr.rank() == 1 {
        // Rank 1: major cells are rank-0 units.
        // For numeric/char arrays, rank-0 cells are just scalar values.
        // For boxed arrays, rank-0 cells are enclosed values (rank-0 arrays wrapping the inner value).
        let is_boxed = arr.el_type() == ElType::B;
        let mut results = Vec::with_capacity(lead);
        for i in 0..lead {
            let elem = get_elem(&arr, i);
            if is_boxed && elem.is_arr() {
                // Wrap array element in rank-0 enclosure (the major cell IS a rank-0 array)
                let cell = crate::vm::tag_arr(BqnArr {
                    shape: vec![],
                    data: ArrData::Boxed(vec![elem]),
                    fill: Some(rbqn_prim::structural::prototype_of(elem)),
                });
                results.push(c1(f, cell));
            } else {
                results.push(c1(f, elem));
            }
        }
        return merge_cells_result(results, vec![lead]);
    }
    // Rank >= 2: cells are subarrays along the leading axis
    let cell_size: usize = arr.shape[1..].iter().product();
    let cell_shape = arr.shape[1..].to_vec();
    let mut results = Vec::with_capacity(lead);
    for i in 0..lead {
        let cell = crate::vm::tag_arr(extract_cell(&arr, i, cell_size, &cell_shape));
        results.push(c1(f, cell));
    }
    merge_cells_result(results, vec![lead])
}

fn cells_c2(f: B, w: B, x: B) -> B {
    // NOTE: ˘ applies F to each major cell of 𝕩 (and 𝕨 if it has matching leading axis).
    // For rank-1 arrays, the cells are individual elements (rank-0 atoms).
    // For rank-0 (atom) 𝕩, just call F directly.
    if x.is_atom() {
        return c2(f, w, x);
    }
    let xarr = arr_of(x);
    // Rank-0 array: apply F to the whole rank-0 array; frame is ⟨⟩, merge result properly
    if xarr.shape.is_empty() {
        let result = c2(f, w, x);
        return merge_cells_result(vec![result], vec![]);
    }
    let lead = xarr.shape[0];
    if xarr.rank() == 1 {
        // Rank 1: major cells are rank-0 units.
        // For boxed arrays, array elements are wrapped in rank-0 enclosures.
        let x_is_boxed = xarr.el_type() == ElType::B;
        let get_x_cell = |arr: &BqnArr, i: usize| -> B {
            let elem = get_elem(arr, i);
            if x_is_boxed && elem.is_arr() {
                crate::vm::tag_arr(BqnArr {
                    shape: vec![],
                    data: ArrData::Boxed(vec![elem]),
                    fill: Some(rbqn_prim::structural::prototype_of(elem)),
                })
            } else {
                elem
            }
        };
        if w.is_arr() {
            let warr = arr_of(w);
            if warr.rank() == 1 && warr.shape[0] == lead {
                // Both rank 1 with same length: pair up elements
                let w_is_boxed = warr.el_type() == ElType::B;
                let mut results = Vec::with_capacity(lead);
                for i in 0..lead {
                    let we = get_elem(&warr, i);
                    let wc = if w_is_boxed && we.is_arr() {
                        crate::vm::tag_arr(BqnArr {
                            shape: vec![],
                            data: ArrData::Boxed(vec![we]),
                            fill: Some(rbqn_prim::structural::prototype_of(we)),
                        })
                    } else { we };
                    results.push(c2(f, wc, get_x_cell(&xarr, i)));
                }
                return merge_cells_result(results, vec![lead]);
            }
            if warr.rank() >= 2 && warr.shape[0] == lead {
                // w has higher rank, extract w cells
                let w_cell_size: usize = warr.shape[1..].iter().product();
                let w_cell_shape = warr.shape[1..].to_vec();
                let mut results = Vec::with_capacity(lead);
                for i in 0..lead {
                    let wc = crate::vm::tag_arr(extract_cell(&warr, i, w_cell_size, &w_cell_shape));
                    results.push(c2(f, wc, get_x_cell(&xarr, i)));
                }
                return merge_cells_result(results, vec![lead]);
            }
        }
        // w is atom or doesn't match: broadcast w to each x element
        let mut results = Vec::with_capacity(lead);
        for i in 0..lead {
            results.push(c2(f, w, get_x_cell(&xarr, i)));
        }
        return merge_cells_result(results, vec![lead]);
    }
    // Rank >= 2: cells are subarrays along the leading axis
    let cell_size: usize = xarr.shape[1..].iter().product();
    let cell_shape = xarr.shape[1..].to_vec();
    if w.is_arr() {
        let warr = arr_of(w);
        if warr.rank() >= 2 && warr.shape[0] == lead {
            let w_cell_size: usize = warr.shape[1..].iter().product();
            let w_cell_shape = warr.shape[1..].to_vec();
            let mut results = Vec::with_capacity(lead);
            for i in 0..lead {
                let wc = crate::vm::tag_arr(extract_cell(&warr, i, w_cell_size, &w_cell_shape));
                let xc = crate::vm::tag_arr(extract_cell(&xarr, i, cell_size, &cell_shape));
                results.push(c2(f, wc, xc));
            }
            return merge_cells_result(results, vec![lead]);
        }
    }
    let mut results = Vec::with_capacity(lead);
    for i in 0..lead {
        let cell = crate::vm::tag_arr(extract_cell(&xarr, i, cell_size, &cell_shape));
        results.push(c2(f, w, cell));
    }
    merge_cells_result(results, vec![lead])
}

// ============================================================
// 2-modifier: ⌾ Under
// ============================================================

// F⌾G x: Apply F under G.
// CBQN approach: delegates to G's fn_uc1 handler for structural under,
// falling back to the BQN runtime's Under for computational under.
// We follow the same pattern: try computational under first, then
// delegate to the BQN runtime's Under implementation.
/// Try to handle structural Under natively for common patterns.
/// Returns Some(result) if handled, None to fall through to runtime.
fn try_structural_under(f: B, g: B, x: B) -> Option<B> {
    if !g.is_fun() { return None; }
    let gid = (g.0 & 0xFFFFFFFFFFFF) >> 3;
    let gd = crate::derive::get_derived(gid);

    // Pattern: F⌾⌽ x — ⌽ is self-inverse (reverse)
    if let crate::derive::DerivedKind::NativeFn { prim_idx: 31 } = gd.kind {
        // F⌾⌽ x: apply F to (⌽ x), then ⌽ result back
        let rev = c1(g, x);
        let frev = c1(f, rev);
        return Some(c1(g, frev));
    }

    // Pattern: F⌾⍉ x — ⍉ is self-inverse for rank≤2
    if let crate::derive::DerivedKind::NativeFn { prim_idx: 32 } = gd.kind {
        // F⌾⍉ x: apply F to (⍉ x), then ⍉ result back
        let trans = c1(g, x);
        let ftrans = c1(f, trans);
        return Some(c1(g, ftrans));
    }

    // Pattern: F⌾< x — enclose under (enclose, apply F, unbox via <⁼)
    if let crate::derive::DerivedKind::NativeFn { prim_idx: 12 } = gd.kind {
        // F⌾< x: enclose x → rank-0 array, apply F, then <⁼ (sys 206) to extract
        // <⁻¹ extracts the single element from a rank-0 array (returns the atom).
        // Using > (merge, prim 13) is wrong — it wraps non-boxed rank-0 arrays.
        let enclosed = c1(g, x);  // < x  (rank-0 boxed array)
        let f_enclosed = c1(f, enclosed);
        let unbox_fn = crate::derive::m_sys_fn(206); // <⁼ = extract rank-0 element
        return Some(c1(unbox_fn, f_enclosed));
    }

    // Pattern: F⌾(k⊸↑) x or F⌾(k⊸↓) x — take/drop under via ⊸ modifier
    if gd.kind == crate::derive::DerivedKind::Md2D {
        let modifier = gd.g;
        if modifier.is_md2() {
            let mid = (modifier.0 & 0xFFFFFFFFFFFF) >> 3;
            let md = crate::derive::get_derived(mid);
            if let crate::derive::DerivedKind::NativeMd2 { prim_idx: 55 } = md.kind {
                // ⊸ (Before) — check right operand
                let right_op = gd.h;
                let left_op = gd.f;  // the k value

                if right_op.is_fun() {
                    let rid = (right_op.0 & 0xFFFFFFFFFFFF) >> 3;
                    let rd = crate::derive::get_derived(rid);

                    // NOTE: left_op is k — may be a literal value or a function.
                    // If it's callable (function/modifier), evaluate k = c1(left_op, x)
                    // to compute the actual k from x's shape before take/drop.
                    // Example: F⌾((2÷˜≠)⊸↑) x — k = ≠x÷2 = half the length.
                    let k = if left_op.is_fun() || left_op.is_md1() || left_op.is_md2() {
                        c1(left_op, x)
                    } else {
                        left_op
                    };

                    match rd.kind {
                        // F⌾(k⊸↑) x — take under (handles 1D and multi-dim)
                        crate::derive::DerivedKind::NativeFn { prim_idx: 26 } => {
                            return Some(take_under(f, k, x, false));
                        }
                        // F⌾(k⊸↓) x — drop under (handles 1D and multi-dim)
                        crate::derive::DerivedKind::NativeFn { prim_idx: 27 } => {
                            return Some(take_under(f, k, x, true));
                        }
                        // F⌾(arr⊸⊏) x — structural select-under (k must be literal indices)
                        crate::derive::DerivedKind::NativeFn { prim_idx: 36 } => {
                            return Some(structural_select_under(f, k, x));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Pattern: F⌾⊑ x — modify first element
    if let crate::derive::DerivedKind::NativeFn { prim_idx: 37 } = gd.kind {
        // F⌾⊑ x: modify first element
        if !x.is_arr() { return None; }
        if let Some(xa) = crate::vm::get_arr(x) {
            if xa.ia() == 0 { return None; }
            let first = xa.get(0).ok()?;
            let modified = c1(f, first);
            let mut elems = Vec::with_capacity(xa.ia());
            elems.push(modified);
            for i in 1..xa.ia() {
                elems.push(xa.get(i).ok()?);
            }
            let out = rbqn_core::array::typed_arr_from_b_vec(elems, xa.shape.clone(), xa.fill);
            return Some(crate::vm::tag_arr(out));
        }
    }

    // Pattern: F⌾(Fork(k, ⊏⎉r, ⊢)) x — rank-select-under.
    // Handles 2‿1⊏⎉1⊢, 2‿1⊏⎉¯1⊢, 2‿1⊏⎉(-˜○=)⊢ etc.
    // Fork structure: gd.f=k(value array), gd.g=⊏⎉r(Md2D), gd.h=⊢(identity).
    if gd.kind == crate::derive::DerivedKind::Fork {
        let fork_f = gd.f; // k value
        let fork_g = gd.g; // center = ⊏⎉r
        let fork_h = gd.h; // right = ⊢
        // Check h = ⊢ (prim 21).
        let h_is_id = fork_h.is_fun() && {
            let hid = (fork_h.0 & 0xFFFFFFFFFFFF) >> 3;
            matches!(crate::derive::get_derived(hid).kind,
                     crate::derive::DerivedKind::NativeFn { prim_idx: 21 })
        };
        // Check g = Md2D with modifier=⎉ (prim 60) and left-op=⊏ (prim 36).
        if h_is_id && fork_g.is_fun() && fork_f.is_arr() {
            let gid2 = (fork_g.0 & 0xFFFFFFFFFFFF) >> 3;
            let gd2 = crate::derive::get_derived(gid2);
            if gd2.kind == crate::derive::DerivedKind::Md2D {
                let g_mod = gd2.g;
                let g_lop = gd2.f;
                let g_rsp = gd2.h; // rank spec
                if g_mod.is_md2() {
                    let mid = (g_mod.0 & 0xFFFFFFFFFFFF) >> 3;
                    let md = crate::derive::get_derived(mid);
                    if let crate::derive::DerivedKind::NativeMd2 { prim_idx: 60 } = md.kind
                        && g_lop.is_fun() {
                            let lid = (g_lop.0 & 0xFFFFFFFFFFFF) >> 3;
                            let ld = crate::derive::get_derived(lid);
                            if let crate::derive::DerivedKind::NativeFn { prim_idx: 36 } = ld.kind {
                                return Some(rank_select_under(f, fork_f, g_rsp, x));
                            }
                        }
                }
            }
        }
    }

    // Pattern: F⌾(a↓b↑⊢) x — drop-after-take chain that may introduce fill.
    // Detect Fork(a, ↓, Fork(b, ↑, ⊢)) and check if b > len(x) → throw fill error.
    if gd.kind == crate::derive::DerivedKind::Fork {
        let fork_g = gd.g; // center function
        // Check center = ↓ (prim 27)
        if fork_g.is_fun() {
            let cgid = (fork_g.0 & 0xFFFFFFFFFFFF) >> 3;
            let cgd = crate::derive::get_derived(cgid);
            if let crate::derive::DerivedKind::NativeFn { prim_idx: 27 } = cgd.kind {
                // Center is ↓. Check h = Fork(b, ↑, ⊢).
                let fork_h = gd.h;
                if fork_h.is_fun() {
                    let hhid = (fork_h.0 & 0xFFFFFFFFFFFF) >> 3;
                    let hhd = crate::derive::get_derived(hhid);
                    if hhd.kind == crate::derive::DerivedKind::Fork {
                        let inner_g = hhd.g;
                        let inner_h = hhd.h;
                        // Check inner center = ↑ (prim 26) and inner right = ⊢ (prim 21).
                        let inner_g_is_take = inner_g.is_fun() && {
                            let igid = (inner_g.0 & 0xFFFFFFFFFFFF) >> 3;
                            matches!(crate::derive::get_derived(igid).kind,
                                     crate::derive::DerivedKind::NativeFn { prim_idx: 26 })
                        };
                        let inner_h_is_id = inner_h.is_fun() && {
                            let ihid = (inner_h.0 & 0xFFFFFFFFFFFF) >> 3;
                            matches!(crate::derive::get_derived(ihid).kind,
                                     crate::derive::DerivedKind::NativeFn { prim_idx: 21 })
                        };
                        if inner_g_is_take && inner_h_is_id {
                            // Inner left = n (the take amount). Check if it introduces fill.
                            let inner_f = hhd.f;
                            let n = if inner_f.is_f64() {
                                Some(inner_f.o2f().abs() as usize)
                            } else { None };
                            if let Some(n_val) = n
                                && x.is_arr()
                                    && let Some(xa) = crate::vm::get_arr(x) {
                                        let len = if xa.shape.is_empty() { 1 } else { xa.shape[0] };
                                        if n_val > len {
                                            rbqn_core::error::throw(format!(
                                                "𝔽⌾(n⊸↑)𝕩: Cannot modify fill with Under ({} ≡ n, {:?} ≡ ≢𝕩)",
                                                n_val, xa.shape
                                            ));
                                        }
                                    }
                        }
                    }
                }
            }
        }
    }

    None
}

/// Structural select-under: F⌾(indices⊸⊏) x
/// Applies F to the elements at the given indices, leaving others unchanged.
fn structural_select_under(f: B, indices_b: B, x: B) -> B {
    if !x.is_arr() {
        rbqn_core::error::throw("⌾(⊸⊏): 𝕩 must be an array");
    }
    let xa = crate::vm::get_arr(x)
        .unwrap_or_else(|| rbqn_core::error::throw("⌾(⊸⊏): 𝕩 array not found"));

    // Get the selected elements
    let selected = c2(crate::derive::m_native_fn(36), indices_b, x); // indices⊏x

    // Apply F to the selected elements
    let modified = c1(f, selected);

    // Scatter the modified values back into a copy of x
    let mod_arr = crate::vm::get_arr(modified);

    // Get indices as i32 array
    let idx_arr = crate::vm::get_arr(indices_b)
        .unwrap_or_else(|| rbqn_core::error::throw("⌾(⊸⊏): indices must be an array"));
    let indices = idx_arr.i32_iter()
        .unwrap_or_else(|_| rbqn_core::error::throw("⌾(⊸⊏): indices must be integers"));

    // NOTE: BQN requires indices to be injective (no duplicates) for structural under.
    // Duplicate indices mean the same position is written twice → ambiguous result → error.
    {
        let n = xa.ia() as i32;
        let mut seen = std::collections::HashSet::new();
        for &idx in &indices {
            let norm = if idx < 0 { idx + n } else { idx };
            if !seen.insert(norm) {
                rbqn_core::error::throw("⌾(⊸⊏): 𝕨 contains duplicate indices");
            }
        }
    }

    // Build result: copy of x with modifications at index positions
    let mut elems: Vec<B> = (0..xa.ia()).map(|i| xa.get(i).unwrap_or(B::SENTINEL)).collect();

    if let Some(ma) = mod_arr {
        // Modified is an array — scatter its elements back
        for (j, &idx) in indices.iter().enumerate() {
            let i = if idx < 0 { (idx + xa.ia() as i32) as usize } else { idx as usize };
            if i < elems.len() && j < ma.ia() {
                elems[i] = ma.get(j).unwrap_or(B::SENTINEL);
            }
        }
    } else if modified.is_f64() || modified.is_c32() {
        // Modified is a scalar — set all indexed positions to this value
        for &idx in &indices {
            let i = if idx < 0 { (idx + xa.ia() as i32) as usize } else { idx as usize };
            if i < elems.len() {
                elems[i] = modified;
            }
        }
    }

    let out = rbqn_core::array::typed_arr_from_b_vec(elems, xa.shape.clone(), xa.fill);
    crate::vm::tag_arr(out)
}

/// Take/drop under: F⌾(k⊸↑) x or F⌾(k⊸↓) x.
/// drop=false → take under, drop=true → drop under.
fn take_under(f: B, k: B, x: B, drop: bool) -> B {
    if !x.is_arr() {
        rbqn_core::error::throw("⌾(⊸↑/↓): 𝕩 must be an array");
    }
    let xa = crate::vm::get_arr(x)
        .unwrap_or_else(|| rbqn_core::error::throw("⌾(⊸↑/↓): 𝕩 array not found"));
    let take_fn = crate::derive::m_native_fn(26); // ↑
    let drop_fn = crate::derive::m_native_fn(27); // ↓
    // Determine if k is multi-dimensional (vector with rank=1 and ≥2 elements).
    let k_is_vector = if let Some(ka) = crate::vm::get_arr(k) {
        ka.rank() == 1 && ka.ia() >= 2
    } else { false };
    if !k_is_vector {
        // Scalar k: 1D join approach with fill check.
        let k_val = if k.is_f64() { k.o2f() }
        else if let Some(ka) = crate::vm::get_arr(k) {
            if ka.ia() == 0 { 0.0 } else { ka.get(0).map(|b| b.o2f()).unwrap_or(0.0) }
        } else { 0.0 };
        let len = if xa.shape.is_empty() { 1 } else { xa.shape[0] };
        let abs_k = k_val.abs() as usize;
        if abs_k > len {
            rbqn_core::error::throw(format!(
                "𝔽⌾(n⊸{}): Cannot modify fill with Under ({} ≡ n, {:?} ≡ ≢𝕩)",
                if drop { "↓" } else { "↑" }, abs_k, xa.shape
            ));
        }
        let join_fn = crate::derive::m_native_fn(23); // ∾
        if drop {
            let prefix = c2(take_fn, k, x);
            let selected = c2(drop_fn, k, x);
            let modified = c1(f, selected);
            c2(join_fn, prefix, modified)
        } else {
            let selected = c2(take_fn, k, x);
            let modified = c1(f, selected);
            let suffix = c2(drop_fn, k, x);
            c2(join_fn, modified, suffix)
        }
    } else {
        // Multi-dimensional vector k: scatter-back approach.
        let ka = crate::vm::get_arr(k).unwrap();
        let rank = xa.rank() as usize;
        if ka.ia() != rank {
            rbqn_core::error::throw(format!(
                "⌾(⊸↑/↓): k has {} elements but 𝕩 has rank {}", ka.ia(), rank
            ));
        }
        let mut take_starts: Vec<usize> = Vec::with_capacity(rank);
        let mut take_lens: Vec<usize> = Vec::with_capacity(rank);
        for i in 0..rank {
            let ki = ka.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e)).o2f();
            let si = xa.shape[i];
            let abs_ki = ki.abs() as usize;
            if abs_ki > si {
                rbqn_core::error::throw(format!(
                    "𝔽⌾(k⊸{}): Cannot modify fill with Under ({} > {} on axis {})",
                    if drop { "↓" } else { "↑" }, abs_ki, si, i
                ));
            }
            if drop {
                if ki >= 0.0 { take_starts.push(abs_ki); take_lens.push(si - abs_ki); }
                else { take_starts.push(0); take_lens.push(si - abs_ki); }
            } else if ki >= 0.0 { take_starts.push(0); take_lens.push(abs_ki); }
            else { take_starts.push(si - abs_ki); take_lens.push(abs_ki); }
        }
        let selected = if drop { c2(drop_fn, k, x) } else { c2(take_fn, k, x) };
        let modified = c1(f, selected);
        let mod_arr = crate::vm::get_arr(modified)
            .unwrap_or_else(|| rbqn_core::error::throw("⌾(⊸↑/↓): F result must be an array"));
        // Strides for x.
        let mut x_strides: Vec<usize> = vec![1; rank];
        for i in (0..rank.saturating_sub(1)).rev() { x_strides[i] = x_strides[i+1] * xa.shape[i+1]; }
        // Strides for mod.
        let mod_shape = &mod_arr.shape;
        let mod_rank = mod_shape.len();
        let mut mod_strides: Vec<usize> = vec![1; mod_rank];
        for i in (0..mod_rank.saturating_sub(1)).rev() { mod_strides[i] = mod_strides[i+1] * mod_shape[i+1]; }
        // Scatter.
        let total = xa.ia();
        let mut elems: Vec<B> = (0..total).map(|i| xa.get(i).unwrap_or(B::SENTINEL)).collect();
        let mod_total = mod_arr.ia();
        for flat_mod in 0..mod_total {
            let mut rem = flat_mod;
            let mut x_flat = 0usize;
            let mut valid = true;
            for dim in 0..rank.min(mod_rank) {
                let mod_i = rem / mod_strides[dim];
                rem %= mod_strides[dim];
                let x_i = mod_i + take_starts[dim];
                if x_i >= xa.shape[dim] { valid = false; break; }
                x_flat += x_i * x_strides[dim];
            }
            if valid && x_flat < elems.len() {
                elems[x_flat] = mod_arr.get(flat_mod).unwrap_or(B::SENTINEL);
            }
        }
        let out = rbqn_core::array::typed_arr_from_b_vec(elems, xa.shape.clone(), xa.fill);
        crate::vm::tag_arr(out)
    }
}

/// Rank-select-under: F⌾(k⊏⎉r⊢) x where G is a Fork(k_val, ⊏⎉r, ⊢).
fn rank_select_under(f: B, k: B, rank_spec: B, x: B) -> B {
    if !x.is_arr() {
        rbqn_core::error::throw("⌾(k⊏⎉r⊢): 𝕩 must be an array");
    }
    let xa = crate::vm::get_arr(x)
        .unwrap_or_else(|| rbqn_core::error::throw("⌾(k⊏⎉r⊢): 𝕩 array not found"));
    let ka = crate::vm::get_arr(k)
        .unwrap_or_else(|| rbqn_core::error::throw("⌾(k⊏⎉r⊢): k must be an array"));
    let xr: usize = xa.rank() as usize;
    let kr: usize = ka.rank() as usize;
    // Compute rank pair (w_r, x_r) for dyadic rank spec called with (k, x).
    let (w_r_val, x_r_val): (f64, f64) = {
        if rank_spec.is_f64() { let v = rank_spec.o2f(); (v, v) }
        else if rank_spec.is_arr() {
            let ra = crate::vm::get_arr(rank_spec).unwrap();
            let n = ra.ia();
            if n == 0 { rbqn_core::error::throw("⌾(k⊏⎉r⊢): rank spec empty"); }
            if n >= 2 { (ra.get(n-2).unwrap().o2f(), ra.get(n-1).unwrap().o2f()) }
            else { let v = ra.get(0).unwrap().o2f(); (v, v) }
        } else if rank_spec.is_fun() {
            let r = c2(rank_spec, k, x);
            if r.is_f64() { let v = r.o2f(); (v, v) }
            else if r.is_arr() {
                let ra = crate::vm::get_arr(r).unwrap();
                let n = ra.ia();
                if n == 0 { rbqn_core::error::throw("⌾(k⊏⎉r⊢): rank spec returned empty"); }
                if n >= 2 { (ra.get(n-2).unwrap().o2f(), ra.get(n-1).unwrap().o2f()) }
                else { let v = ra.get(0).unwrap().o2f(); (v, v) }
            } else { rbqn_core::error::throw("⌾(k⊏⎉r⊢): rank spec must return number or array") }
        } else { rbqn_core::error::throw("⌾(k⊏⎉r⊢): invalid rank spec") }
    };
    // Effective cell ranks.
    let w_cr = if w_r_val < 0.0 { let v = w_r_val + kr as f64; if v < 0.0 { 0 } else { v as usize } }
               else { let v = w_r_val as usize; if v > kr { kr } else { v } };
    let x_cr = if x_r_val < 0.0 { let v = x_r_val + xr as f64; if v < 0.0 { 0 } else { v as usize } }
               else { let v = x_r_val as usize; if v > xr { xr } else { v } };
    // Frame and cell shapes.
    let x_frame_rank = xr - x_cr;
    let x_frame_shape = xa.shape[..x_frame_rank].to_vec();
    let x_cell_shape = xa.shape[x_frame_rank..].to_vec();
    let x_cell_size: usize = x_cell_shape.iter().product::<usize>().max(1);
    let n_x_cells: usize = x_frame_shape.iter().product::<usize>().max(1);
    let k_frame_rank = kr - w_cr;
    let n_k_cells: usize = ka.shape[..k_frame_rank].iter().product::<usize>().max(1);
    if w_cr == kr {
        // Full k used for each x-cell (⎉1 style).
        let indices = ka.i32_iter()
            .unwrap_or_else(|_| rbqn_core::error::throw("⌾(k⊏⎉r⊢): k must be integer indices"));
        {
            let mut seen = std::collections::HashSet::new();
            for &idx in &indices {
                if !seen.insert(idx) {
                    rbqn_core::error::throw("⌾(k⊏⎉r⊢): k contains duplicate indices");
                }
            }
        }
        let select_fn = crate::derive::m_native_fn(36);
        let n_k = ka.ia();
        let mut selected_cells: Vec<B> = Vec::with_capacity(n_x_cells);
        for i in 0..n_x_cells {
            let cell = crate::vm::tag_arr(extract_cell(&xa, i, x_cell_size, &x_cell_shape));
            selected_cells.push(c2(select_fn, k, cell));
        }
        let gx = merge_cells_result(selected_cells, x_frame_shape.clone());
        let modified = c1(f, gx);
        let mod_arr = crate::vm::get_arr(modified)
            .unwrap_or_else(|| rbqn_core::error::throw("⌾(k⊏⎉r⊢): F must return an array"));
        let total = xa.ia();
        let mut elems: Vec<B> = (0..total).map(|i| xa.get(i).unwrap_or(B::SENTINEL)).collect();
        for cell_i in 0..n_x_cells {
            let cell_start = cell_i * x_cell_size;
            let mod_cell_start = cell_i * n_k;
            for (j, &idx) in indices.iter().enumerate() {
                let pos = if idx < 0 { (idx + x_cell_shape[0] as i32) as usize } else { idx as usize };
                let x_pos = cell_start + pos;
                let mod_pos = mod_cell_start + j;
                if x_pos < elems.len() && mod_pos < mod_arr.ia() {
                    elems[x_pos] = mod_arr.get(mod_pos).unwrap_or(B::SENTINEL);
                }
            }
        }
        let out = rbqn_core::array::typed_arr_from_b_vec(elems, xa.shape.clone(), xa.fill);
        crate::vm::tag_arr(out)
    } else {
        // Scalar k cells (⎉¯1 style): ka[i] selects from x_cell[i].
        if n_k_cells != n_x_cells {
            rbqn_core::error::throw(format!(
                "⌾(k⊏⎉r⊢): k frame count {} != x frame count {}", n_k_cells, n_x_cells
            ));
        }
        let mut selected_vals: Vec<B> = Vec::with_capacity(n_x_cells);
        for i in 0..n_x_cells {
            let ki_val = ka.get(i).unwrap_or(B::SENTINEL).o2f() as i64;
            let pos = if ki_val < 0 { (ki_val + x_cell_size as i64) as usize } else { ki_val as usize };
            let x_pos = i * x_cell_size + pos;
            if x_pos < xa.ia() {
                selected_vals.push(xa.get(x_pos).unwrap_or(B::SENTINEL));
            } else {
                rbqn_core::error::throw(format!(
                    "⌾(k⊏⎉r⊢): index {} out of bounds", ki_val
                ));
            }
        }
        let gx = results_to_arr(selected_vals, x_frame_shape.clone());
        let modified = c1(f, gx);
        let mod_arr = crate::vm::get_arr(modified)
            .unwrap_or_else(|| rbqn_core::error::throw("⌾(k⊏⎉r⊢): F must return an array"));
        let total = xa.ia();
        let mut elems: Vec<B> = (0..total).map(|i| xa.get(i).unwrap_or(B::SENTINEL)).collect();
        for i in 0..n_x_cells {
            let ki_val = ka.get(i).unwrap_or(B::SENTINEL).o2f() as i64;
            let pos = if ki_val < 0 { (ki_val + x_cell_size as i64) as usize } else { ki_val as usize };
            let x_pos = i * x_cell_size + pos;
            let mod_val = mod_arr.get(i).unwrap_or(B::SENTINEL);
            if x_pos < elems.len() { elems[x_pos] = mod_val; }
        }
        let out = rbqn_core::array::typed_arr_from_b_vec(elems, xa.shape.clone(), xa.fill);
        crate::vm::tag_arr(out)
    }
}

fn under_c1(f: B, g: B, x: B) -> B {
    // Native structural Under for common patterns before delegating to runtime.
    // F⌾(arr⊸⊏) x: apply F to selected elements, scatter back.
    // F⌾(arr⊸/) x: similar for replicate-based selection.
    if let Some(result) = try_structural_under(f, g, x) {
        return result;
    }

    // If the BQN runtime's Under is available, use it directly.
    // The BQN runtime's Under handles both computational and structural cases
    // correctly, including the roundtrip assertion and all edge cases.
    if let Some(rt_under) = get_rt_under() {
        // Build the Under-derived function: rt_under(f, g)
        let under_fn = crate::derive::m_md2d(rt_under, f, g);
        return c1(under_fn, x);
    }

    // Fallback: basic computational under (pre-runtime1)
    let gx = c1(g, x);
    let fgx = c1(f, gx);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let g_inv = crate::derive::inv_reg(g);
        c1(g_inv, fgx)
    }));
    match result {
        Ok(v) => v,
        Err(_) => rbqn_core::error::throw("⌾: inverse failed and runtime Under not available"),
    }
}

// w F⌾G x: Dyadic under.
// CBQN transforms this to: (G(w)⊸F)⌾G x — binds G(w) as left arg of F,
// then does monadic Under.
fn under_c2(f: B, g: B, w: B, x: B) -> B {
    // If the BQN runtime's Under is available, use CBQN's approach:
    // Build f2 = (G(w))⊸F, then call monadic under: f2⌾G x
    if let Some(rt_under) = get_rt_under() {
        let gw = c1(g, w);
        let before_md2 = crate::derive::m_native_md2(55); // ⊸ (Before)
        let f2 = crate::derive::m_md2d(before_md2, gw, f);
        let under_fn = crate::derive::m_md2d(rt_under, f2, g);
        return c1(under_fn, x);
    }

    // Fallback: basic computational under (pre-runtime1)
    let gx = c1(g, x);
    let comp_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let gw = c1(g, w);
        let fgx = c2(f, gw, gx);
        let g_inv = crate::derive::inv_reg(g);
        c1(g_inv, fgx)
    }));
    match comp_result {
        Ok(v) => v,
        Err(_) => rbqn_core::error::throw("⌾: inverse failed and runtime Under not available"),
    }
}

// ============================================================
// 2-modifier: ◶ Choose
// ============================================================

/// Call dyadic pick (w⊑x) via the primitive dispatch.
/// Used by ◶ when the condition function returns a non-number (array index).
fn pick_from(w: B, x: B) -> B {
    let prims = rbqn_prim::get_runtime();
    // ⊑ is primitive index 37 in the standard BQN ordering
    let pick_prim = &prims[37];
    let c2_fn = pick_prim.c2.unwrap();
    let wa = crate::vm::get_arr(w);
    let xa = crate::vm::get_arr(x);
    let result = c2_fn(w, wa.as_deref(), x, xa.as_deref())
        .unwrap_or_else(|e| rbqn_core::error::throw(format!("◶: pick failed: {}", e)));
    match result {
        rbqn_prim::PrimResult::Scalar(b) => b,
        rbqn_prim::PrimResult::Array(arr) => crate::vm::tag_arr(arr),
    }
}

fn choose_c1(f: B, g: B, x: B) -> B {
    let idx_b = c1(f, x);
    // NOTE: Match CBQN cond_c1 — scalar path (fast) and array path (via pick)
    if idx_b.is_f64() {
        let idx = b_to_index(idx_b);
        let garr = arr_of(g);
        let chosen = get_elem(&garr, idx);
        c1(chosen, x)
    } else {
        let chosen = pick_from(idx_b, g);
        c1(chosen, x)
    }
}

fn choose_c2(f: B, g: B, w: B, x: B) -> B {
    let idx_b = c2(f, w, x);
    if idx_b.is_f64() {
        let idx = b_to_index(idx_b);
        let garr = arr_of(g);
        let chosen = get_elem(&garr, idx);
        c2(chosen, w, x)
    } else {
        let chosen = pick_from(idx_b, g);
        c2(chosen, w, x)
    }
}

/// Extract an integer index from a B value. Handles both scalars and
/// single-element arrays (the VM sometimes wraps scalars in 1-element Boxed arrays).
fn b_to_index(v: B) -> usize {
    if let Ok(u) = v.to_usz() {
        return u;
    }
    // Try unwrapping a single-element array
    if let Some(arr) = crate::vm::get_arr(v)
        && arr.ia() == 1
            && let Ok(elem) = arr.get(0)
                && let Ok(u) = elem.to_usz() {
                    return u;
                }
    rbqn_core::error::throw(format!("◶: Expected number index, got {:#x}", v.0))
}

// ============================================================
// 2-modifier: ⎉ Rank
// ============================================================

/// Compute cell rank from array rank and k parameter.
/// Negative k counts from the end, positive k is clamped to array rank.
fn cell_rank(r: usize, k: f64) -> usize {
    if k.is_finite() && k.fract() != 0.0 {
        rbqn_core::error::throw("⎉: 𝕘 was a fractional number");
    }
    if k < 0.0 {
        let v = k + r as f64;
        if v < 0.0 { 0 } else { v as usize }
    } else {
        let v = k as usize;
        if v > r { r } else { v }
    }
}

/// Extract rank value from g for monadic rank.
/// If g is a number, use directly. If g is an array, use the last element.
fn get_rank_val_c1(g: B, x: B) -> f64 {
    if g.is_f64() {
        return g.o2f();
    }
    if g.is_fun() {
        let r = c1(g, x);
        return r.o2f();
    }
    if g.is_arr() {
        let arr = arr_of(g);
        let n = arr.ia();
        if n == 0 {
            rbqn_core::error::throw("⎉: rank specification array is empty");
        }
        // For monadic: use last element
        return get_elem(&arr, n - 1).o2f();
    }
    rbqn_core::error::throw("⎉: invalid rank specification")
}

/// Extract rank pair from g for dyadic rank.
/// Returns (w_rank_spec, x_rank_spec).
fn get_rank_pair(g: B, w: B, x: B) -> (f64, f64) {
    if g.is_f64() {
        let v = g.o2f();
        return (v, v);
    }
    if g.is_fun() {
        let r = c2(g, w, x);
        if r.is_f64() {
            let v = r.o2f();
            return (v, v);
        }
        if r.is_arr() {
            let arr = arr_of(r);
            let n = arr.ia();
            if n >= 2 {
                return (get_elem(&arr, n - 2).o2f(), get_elem(&arr, n - 1).o2f());
            }
            if n == 1 {
                let v = get_elem(&arr, 0).o2f();
                return (v, v);
            }
        }
        rbqn_core::error::throw("⎉: rank function must return number or array");
    }
    if g.is_arr() {
        let arr = arr_of(g);
        let n = arr.ia();
        if n >= 2 {
            return (get_elem(&arr, n - 2).o2f(), get_elem(&arr, n - 1).o2f());
        }
        if n == 1 {
            let v = get_elem(&arr, 0).o2f();
            return (v, v);
        }
        rbqn_core::error::throw("⎉: rank specification array is empty");
    }
    rbqn_core::error::throw("⎉: invalid rank specification")
}

fn rank_c1(f: B, g: B, x: B) -> B {
    let k = get_rank_val_c1(g, x);

    if x.is_atom() {
        return c1(f, x);
    }
    let xarr = arr_of(x);
    let xr = xarr.rank() as usize;
    let cr = cell_rank(xr, k);
    if cr == xr {
        return c1(f, x);
    }

    let frame_rank = xr - cr;
    let cam: usize = xarr.shape[..frame_rank].iter().product();
    let cell_size: usize = xarr.shape[frame_rank..].iter().product();
    let cell_shape = xarr.shape[frame_rank..].to_vec();
    let frame_shape = xarr.shape[..frame_rank].to_vec();

    let mut results = Vec::with_capacity(cam);
    for i in 0..cam {
        let cell = crate::vm::tag_arr(extract_cell(&xarr, i, cell_size, &cell_shape));
        results.push(c1(f, cell));
    }
    merge_cells_result(results, frame_shape)
}

fn rank_c2(f: B, g: B, w: B, x: B) -> B {
    let (wf, xf) = get_rank_pair(g, w, x);

    let w_atom = w.is_atom();
    let x_atom = x.is_atom();

    let (wr, wcr) = if w_atom {
        (0usize, 0usize)
    } else {
        let warr = arr_of(w);
        let wr = warr.rank() as usize;
        (wr, cell_rank(wr, wf))
    };
    let (xr, xcr) = if x_atom {
        (0usize, 0usize)
    } else {
        let xarr = arr_of(x);
        let xr = xarr.rank() as usize;
        (xr, cell_rank(xr, xf))
    };

    let w_full = w_atom || wcr == wr;
    let x_full = x_atom || xcr == xr;

    if w_full && x_full {
        return c2(f, w, x);
    }
    if w_full {
        return rank_c2_sa(f, w, x, xcr);
    }
    if x_full {
        return rank_c2_as(f, w, x, wcr);
    }
    rank_c2_aa(f, w, x, wcr, xcr)
}

/// Dyadic rank: only x decomposed, w passed whole to each call.
fn rank_c2_sa(f: B, w: B, x: B, xcr: usize) -> B {
    let xarr = arr_of(x);
    let xr = xarr.rank() as usize;
    let frame_rank = xr - xcr;
    let cam: usize = xarr.shape[..frame_rank].iter().product();
    let cell_size: usize = xarr.shape[frame_rank..].iter().product();
    let cell_shape = xarr.shape[frame_rank..].to_vec();
    let frame_shape = xarr.shape[..frame_rank].to_vec();

    let mut results = Vec::with_capacity(cam);
    for i in 0..cam {
        let xc = crate::vm::tag_arr(extract_cell(&xarr, i, cell_size, &cell_shape));
        results.push(c2(f, w, xc));
    }
    merge_cells_result(results, frame_shape)
}

/// Dyadic rank: only w decomposed, x passed whole to each call.
fn rank_c2_as(f: B, w: B, x: B, wcr: usize) -> B {
    let warr = arr_of(w);
    let wr = warr.rank() as usize;
    let frame_rank = wr - wcr;
    let cam: usize = warr.shape[..frame_rank].iter().product();
    let cell_size: usize = warr.shape[frame_rank..].iter().product();
    let cell_shape = warr.shape[frame_rank..].to_vec();
    let frame_shape = warr.shape[..frame_rank].to_vec();

    let mut results = Vec::with_capacity(cam);
    for i in 0..cam {
        let wc = crate::vm::tag_arr(extract_cell(&warr, i, cell_size, &cell_shape));
        results.push(c2(f, wc, x));
    }
    merge_cells_result(results, frame_shape)
}

/// Dyadic rank: both w and x decomposed with frame matching.
fn rank_c2_aa(f: B, w: B, x: B, wcr: usize, xcr: usize) -> B {
    let warr = arr_of(w);
    let xarr = arr_of(x);
    let wr = warr.rank() as usize;
    let xr = xarr.rank() as usize;
    let wk = wr - wcr;
    let xk = xr - xcr;

    // Determine outer and inner frames
    let outer_k = wk.max(xk);
    let inner_k = wk.min(xk);

    // The common frame portion (first inner_k axes) must match
    if warr.shape[..inner_k] != xarr.shape[..inner_k] {
        rbqn_core::error::throw("⎉: frame shapes don't agree");
    }

    // Outer frame shape
    let outer_shape = if wk >= xk {
        warr.shape[..outer_k].to_vec()
    } else {
        xarr.shape[..outer_k].to_vec()
    };
    let outer_cam: usize = outer_shape.iter().product();

    // Cell sizes
    let w_cell_size: usize = warr.shape[wk..].iter().product();
    let w_cell_shape = warr.shape[wk..].to_vec();
    let x_cell_size: usize = xarr.shape[xk..].iter().product();
    let x_cell_shape = xarr.shape[xk..].to_vec();

    // Number of cells for shorter-frame arg within each outer cell
    let w_inner: usize = if wk < outer_k { 1 } else { warr.shape[inner_k..wk].iter().product() };
    let x_inner: usize = if xk < outer_k { 1 } else { xarr.shape[inner_k..xk].iter().product() };

    let mut results = Vec::with_capacity(outer_cam);
    for i in 0..outer_cam {
        let wi = if wk >= xk { i } else { i / x_inner };
        let xi = if xk >= wk { i } else { i / w_inner };
        let wc = crate::vm::tag_arr(extract_cell(&warr, wi, w_cell_size, &w_cell_shape));
        let xc = crate::vm::tag_arr(extract_cell(&xarr, xi, x_cell_size, &x_cell_shape));
        results.push(c2(f, wc, xc));
    }
    merge_cells_result(results, outer_shape)
}

// ============================================================
// 2-modifier: ⚇ Depth
// ============================================================

/// Recursively apply f to all atoms of x (depth-0 monadic).
fn depthf_c1(f: B, x: B) -> B {
    if x.is_arr() {
        let arr = arr_of(x);
        let n = arr.ia();
        let mut results = Vec::with_capacity(n);
        for i in 0..n {
            results.push(depthf_c1(f, get_elem(&arr, i)));
        }
        results_to_arr(results, arr.shape.clone())
    } else {
        c1(f, x)
    }
}

/// Recursively apply f to all atom pairs of w and x (depth-0 dyadic).
fn depthf_c2(f: B, w: B, x: B) -> B {
    let w_arr = w.is_arr();
    let x_arr = x.is_arr();
    if !w_arr && !x_arr {
        return c2(f, w, x);
    }
    if w_arr && x_arr {
        let warr = arr_of(w);
        let xarr = arr_of(x);
        if warr.shape != xarr.shape {
            rbqn_core::error::throw("⚇: 𝕨 and 𝕩 shapes don't match");
        }
        let n = warr.ia();
        let mut results = Vec::with_capacity(n);
        for i in 0..n {
            results.push(depthf_c2(f, get_elem(&warr, i), get_elem(&xarr, i)));
        }
        return results_to_arr(results, warr.shape.clone());
    }
    if w_arr {
        let warr = arr_of(w);
        let n = warr.ia();
        let mut results = Vec::with_capacity(n);
        for i in 0..n {
            results.push(depthf_c2(f, get_elem(&warr, i), x));
        }
        return results_to_arr(results, warr.shape.clone());
    }
    // x_arr
    let xarr = arr_of(x);
    let n = xarr.ia();
    let mut results = Vec::with_capacity(n);
    for i in 0..n {
        results.push(depthf_c2(f, w, get_elem(&xarr, i)));
    }
    results_to_arr(results, xarr.shape.clone())
}

fn depth_c1(f: B, g: B, x: B) -> B {
    if g.is_f64() && g.o2f() == 0.0 {
        return depthf_c1(f, x);
    }
    // Delegate to BQN runtime's Depth for non-zero depths
    if let Some(rt_depth) = get_rt_depth() {
        let depth_fn = crate::derive::m_md2d(rt_depth, f, g);
        return c1(depth_fn, x);
    }
    rbqn_core::error::throw("⚇: non-zero depth requires runtime Depth (not yet loaded)")
}

fn depth_c2(f: B, g: B, w: B, x: B) -> B {
    if g.is_f64() && g.o2f() == 0.0 {
        return depthf_c2(f, w, x);
    }
    // Delegate to BQN runtime's Depth for non-zero depths
    if let Some(rt_depth) = get_rt_depth() {
        let depth_fn = crate::derive::m_md2d(rt_depth, f, g);
        return c2(depth_fn, w, x);
    }
    rbqn_core::error::throw("⚇: non-zero depth requires runtime Depth (not yet loaded)")
}

// ============================================================
// 2-modifier: ⍟ Repeat
// ============================================================

fn repeat_c1(f: B, g: B, x: B) -> B {
    // NOTE: If g is an array, apply f⍟g(i) x for each element of g
    if g.is_arr() {
        let garr = arr_of(g);
        return repeat_c1_arr(f, &garr, x);
    }
    let n = if g.is_f64() {
        g.to_i32().unwrap_or_else(|e| rbqn_core::error::throw_bqn(e))
    } else {
        // g is a function: compute count by calling g(x)
        let n_b = c1(g, x);
        if n_b.is_arr() {
            let narr = arr_of(n_b);
            return repeat_c1_arr(f, &narr, x);
        }
        n_b.to_i32().unwrap_or_else(|e| rbqn_core::error::throw_bqn(e))
    };
    if n < 0 {
        // NOTE: Negative repeat: apply inverse n times
        let f_inv = crate::derive::inv_reg(f);
        let mut acc = x;
        for _ in 0..n.unsigned_abs() {
            acc = c1(f_inv, acc);
        }
        return acc;
    }
    let mut acc = x;
    for _ in 0..n {
        acc = c1(f, acc);
    }
    acc
}

fn repeat_c1_arr(f: B, counts: &rbqn_core::BqnArr, x: B) -> B {
    // Apply repeat for each count value when g is an array.
    // NOTE: BQN evaluates f⍟counts x using the "sorted unique" approach:
    // 1. Sort unique count values
    // 2. Evaluate incrementally from x (minimizes total f calls)
    // 3. Map results back to the original order
    // This matches CBQN: f⍟⟨3,1,2⟩ x makes max(counts)=3 total calls.
    let n = counts.ia();
    if n == 0 {
        return results_to_arr(vec![], counts.shape.clone());
    }
    // Collect all count values
    let mut count_vals: Vec<i32> = Vec::with_capacity(n);
    for i in 0..n {
        let count_b = get_elem(counts, i);
        let count = count_b.to_i32().unwrap_or_else(|e| rbqn_core::error::throw_bqn(e));
        count_vals.push(count);
    }
    // If all counts are negative, or mixed, fall back to independent evaluation
    let all_nonneg = count_vals.iter().all(|&c| c >= 0);
    if !all_nonneg {
        // General case with negatives: apply independently using inverse for negative counts
        let mut results = Vec::with_capacity(n);
        for &count in &count_vals {
            if count < 0 {
                let f_inv = crate::derive::inv_reg(f);
                let mut acc = x;
                for _ in 0..count.unsigned_abs() { acc = c1(f_inv, acc); }
                results.push(acc);
            } else {
                let mut acc = x;
                for _ in 0..count { acc = c1(f, acc); }
                results.push(acc);
            }
        }
        return results_to_arr(results, counts.shape.clone());
    }
    // All non-negative: use sorted-unique incremental approach
    // Sort indices by count value
    let mut sorted_indices: Vec<usize> = (0..n).collect();
    sorted_indices.sort_by_key(|&i| count_vals[i]);
    // Evaluate incrementally
    let mut results_by_index: Vec<B> = vec![x; n]; // placeholder
    let mut acc = x;
    let mut prev_count = 0i32;
    for &idx in &sorted_indices {
        let count = count_vals[idx];
        let delta = count - prev_count;
        for _ in 0..delta { acc = c1(f, acc); }
        results_by_index[idx] = acc;
        prev_count = count;
    }
    results_to_arr(results_by_index, counts.shape.clone())
}

fn repeat_c2(f: B, g: B, w: B, x: B) -> B {
    // Handle array g: apply w F⍟g(i) x for each element of g
    if g.is_arr() {
        let garr = arr_of(g);
        return repeat_c2_arr(f, &garr, w, x);
    }
    let n = if g.is_f64() {
        g.to_i32().unwrap_or_else(|e| rbqn_core::error::throw_bqn(e))
    } else {
        // g is a function: compute count by calling g(w, x) or g(x)
        let n_b = c2(g, w, x);
        if n_b.is_arr() {
            let narr = arr_of(n_b);
            return repeat_c2_arr(f, &narr, w, x);
        }
        n_b.to_i32().unwrap_or_else(|e| rbqn_core::error::throw_bqn(e))
    };
    if n < 0 {
        let f_inv = crate::derive::inv_reg(f);
        let mut acc = x;
        for _ in 0..n.unsigned_abs() {
            acc = c2(f_inv, w, acc);
        }
        return acc;
    }
    let mut acc = x;
    for _ in 0..n {
        acc = c2(f, w, acc);
    }
    acc
}

fn repeat_c2_arr(f: B, counts: &rbqn_core::BqnArr, w: B, x: B) -> B {
    // Same sorted-unique incremental approach as repeat_c1_arr.
    let n = counts.ia();
    if n == 0 {
        return results_to_arr(vec![], counts.shape.clone());
    }
    let mut count_vals: Vec<i32> = Vec::with_capacity(n);
    for i in 0..n {
        let count_b = get_elem(counts, i);
        let count = count_b.to_i32().unwrap_or_else(|e| rbqn_core::error::throw_bqn(e));
        count_vals.push(count);
    }
    let all_nonneg = count_vals.iter().all(|&c| c >= 0);
    if !all_nonneg {
        let mut results = Vec::with_capacity(n);
        for &count in &count_vals {
            if count < 0 {
                let f_inv = crate::derive::inv_reg(f);
                let mut acc = x;
                for _ in 0..count.unsigned_abs() { acc = c2(f_inv, w, acc); }
                results.push(acc);
            } else {
                let mut acc = x;
                for _ in 0..count { acc = c2(f, w, acc); }
                results.push(acc);
            }
        }
        return results_to_arr(results, counts.shape.clone());
    }
    let mut sorted_indices: Vec<usize> = (0..n).collect();
    sorted_indices.sort_by_key(|&i| count_vals[i]);
    let mut results_by_index: Vec<B> = vec![x; n];
    let mut acc = x;
    let mut prev_count = 0i32;
    for &idx in &sorted_indices {
        let count = count_vals[idx];
        let delta = count - prev_count;
        for _ in 0..delta { acc = c2(f, w, acc); }
        results_by_index[idx] = acc;
        prev_count = count;
    }
    results_to_arr(results_by_index, counts.shape.clone())
}

// ============================================================
// 2-modifier: ⎊ Catch
// ============================================================

use std::cell::RefCell;

thread_local! {
    /// Current error message for •CurrentError, set by catch (⎊) handlers.
    /// Stack discipline: save/restore when entering/leaving catch handler G.
    static CURRENT_ERROR: RefCell<Option<B>> = const { RefCell::new(None) };
}

/// Get the current error message (called by •CurrentError dispatch).
pub fn get_current_error() -> Option<B> {
    CURRENT_ERROR.with(|ce| *ce.borrow())
}

/// Extract error message from a panic payload and convert to BQN character array.
fn panic_to_error_string(panic: &Box<dyn std::any::Any + Send>) -> B {
    let msg = if let Some(e) = panic.downcast_ref::<rbqn_core::error::BqnError>() {
        e.current_error_msg()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = panic.downcast_ref::<&str>() {
        s.to_string()
    } else {
        "Unknown error".to_string()
    };
    let chars: Vec<u32> = msg.chars().map(|c| c as u32).collect();
    crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
}

fn catch_c1(f: B, g: B, x: B) -> B {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c1(f, x)));
    match result {
        Ok(v) => v,
        Err(panic) => {
            let msg_b = panic_to_error_string(&panic);
            let old = CURRENT_ERROR.with(|ce| ce.borrow_mut().replace(msg_b));
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c1(g, x)));
            CURRENT_ERROR.with(|ce| *ce.borrow_mut() = old);
            match r {
                Ok(v) => v,
                Err(e) => std::panic::resume_unwind(e),
            }
        }
    }
}

fn catch_c2(f: B, g: B, w: B, x: B) -> B {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c2(f, w, x)));
    match result {
        Ok(v) => v,
        Err(panic) => {
            let msg_b = panic_to_error_string(&panic);
            let old = CURRENT_ERROR.with(|ce| ce.borrow_mut().replace(msg_b));
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c2(g, w, x)));
            CURRENT_ERROR.with(|ce| *ce.borrow_mut() = old);
            match r {
                Ok(v) => v,
                Err(e) => std::panic::resume_unwind(e),
            }
        }
    }
}

// ============================================================
// •bit namespace — bitwise 1-modifier operations
// ============================================================

/// Parse width operand: scalar N → all same; array extends by repeating last.
fn bit_parse_widths(operand: B, _is_dyadic: bool) -> (usize, usize, usize, usize) {
    if operand.is_f64() {
        let w = operand.o2f() as usize;
        return (w, w, w, w);
    }
    if let Some(arr) = crate::vm::get_arr(operand) {
        let n = arr.ia();
        let vals: Vec<usize> = (0..n).map(|i| {
            arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e)).o2f() as usize
        }).collect();
        match n {
            1 => (vals[0], vals[0], vals[0], vals[0]),
            2 => (vals[0], vals[1], vals[1], vals[1]),
            3 => (vals[0], vals[1], vals[2], vals[2]),
            4 => (vals[0], vals[1], vals[2], vals[3]),
            _ => rbqn_core::error::throw("•bit: operand must have 1-4 elements"),
        }
    } else {
        rbqn_core::error::throw("•bit: operand must be a number or array")
    }
}

/// Convert a BQN array to bytes treating elements as `width`-bit values.
fn bit_arr_to_bytes(arr: &rbqn_core::array::BqnArr, width: usize) -> Vec<u8> {
    match width {
        1 => {
            let n = arr.ia();
            let byte_count = (n + 7) / 8;
            let mut bytes = vec![0u8; byte_count];
            for i in 0..n {
                if arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e)).o2f() != 0.0 {
                    bytes[i / 8] |= 1 << (i % 8);
                }
            }
            bytes
        }
        8 => (0..arr.ia()).map(|i| {
            arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e)).o2f() as i8 as u8
        }).collect(),
        16 => {
            let mut bytes = Vec::with_capacity(arr.ia() * 2);
            for i in 0..arr.ia() {
                bytes.extend_from_slice(&(arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e)).o2f() as i16).to_le_bytes());
            }
            bytes
        }
        32 => {
            let mut bytes = Vec::with_capacity(arr.ia() * 4);
            for i in 0..arr.ia() {
                bytes.extend_from_slice(&(arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e)).o2f() as i32).to_le_bytes());
            }
            bytes
        }
        64 => {
            let mut bytes = Vec::with_capacity(arr.ia() * 8);
            for i in 0..arr.ia() {
                bytes.extend_from_slice(&arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e)).o2f().to_le_bytes());
            }
            bytes
        }
        _ => rbqn_core::error::throw(&format!("•bit: unsupported width {}", width)),
    }
}

/// Convert bytes back to a BQN array with `width`-bit elements.
fn bit_bytes_to_arr(bytes: &[u8], width: usize, total_bits: usize) -> B {
    let elem_count = total_bits / width;
    match width {
        1 => {
            let elems: Vec<f64> = (0..elem_count)
                .map(|i| ((bytes[i / 8] >> (i % 8)) & 1) as f64)
                .collect();
            crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_f64(elems))
        }
        8 => {
            let elems: Vec<f64> = bytes.iter().take(elem_count).map(|&b| b as i8 as f64).collect();
            crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_f64(elems))
        }
        16 => {
            let elems: Vec<f64> = bytes.chunks_exact(2).take(elem_count)
                .map(|c| i16::from_le_bytes([c[0], c[1]]) as f64).collect();
            crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_f64(elems))
        }
        32 => {
            let elems: Vec<f64> = bytes.chunks_exact(4).take(elem_count)
                .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f64).collect();
            crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_f64(elems))
        }
        64 => {
            let elems: Vec<f64> = bytes.chunks_exact(8).take(elem_count)
                .map(|c| f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]])).collect();
            crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_f64(elems))
        }
        _ => rbqn_core::error::throw(&format!("•bit: unsupported result width {}", width)),
    }
}

/// Read a signed integer of `ow` bits from bytes at byte offset.
fn bit_read_signed(bytes: &[u8], offset: usize, ow: usize) -> i64 {
    match ow {
        1 => ((bytes[offset / 8] >> (offset % 8)) & 1) as i64,
        8 => bytes[offset] as i8 as i64,
        16 => i16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as i64,
        32 => i32::from_le_bytes([
            bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3],
        ]) as i64,
        64 => i64::from_le_bytes([
            bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3],
            bytes[offset + 4], bytes[offset + 5], bytes[offset + 6], bytes[offset + 7],
        ]),
        _ => rbqn_core::error::throw(&format!("•bit: unsupported width {}", ow)),
    }
}

/// Write a signed integer of `ow` bits to bytes at byte offset.
fn bit_write_signed(bytes: &mut [u8], offset: usize, ow: usize, val: i64) {
    match ow {
        1 => {
            if val & 1 != 0 {
                bytes[offset / 8] |= 1 << (offset % 8);
            } else {
                bytes[offset / 8] &= !(1 << (offset % 8));
            }
        }
        8 => bytes[offset] = val as u8,
        16 => bytes[offset..offset + 2].copy_from_slice(&(val as i16).to_le_bytes()),
        32 => bytes[offset..offset + 4].copy_from_slice(&(val as i32).to_le_bytes()),
        64 => bytes[offset..offset + 8].copy_from_slice(&val.to_le_bytes()),
        _ => rbqn_core::error::throw(&format!("•bit: unsupported width {}", ow)),
    }
}

/// •bit._cast: reinterpret bits (monadic)
fn bit_cast_c1(operand: B, x: B) -> B {
    let (_, rw, xw, _) = bit_parse_widths(operand, false);
    let x_arr = crate::vm::get_arr(x)
        .unwrap_or_else(|| rbqn_core::error::throw("•bit._cast: 𝕩 must be an array"));
    let bytes = bit_arr_to_bytes(&x_arr, xw);
    let total_bits = x_arr.ia() * xw;
    bit_bytes_to_arr(&bytes, rw, total_bits)
}

/// •bit._not: bitwise NOT (monadic)
fn bit_not_c1(operand: B, x: B) -> B {
    let (_, rw, xw, _) = bit_parse_widths(operand, false);
    let x_arr = crate::vm::get_arr(x)
        .unwrap_or_else(|| rbqn_core::error::throw("•bit._not: 𝕩 must be an array"));
    let bytes = bit_arr_to_bytes(&x_arr, xw);
    let result: Vec<u8> = bytes.iter().map(|&b| !b).collect();
    let total_bits = x_arr.ia() * xw;
    bit_bytes_to_arr(&result, rw, total_bits)
}

/// •bit._neg: two's complement negate (monadic)
fn bit_neg_c1(operand: B, x: B) -> B {
    let (ow, rw, xw, _) = bit_parse_widths(operand, false);
    let x_arr = crate::vm::get_arr(x)
        .unwrap_or_else(|| rbqn_core::error::throw("•bit._neg: 𝕩 must be an array"));
    let bytes = bit_arr_to_bytes(&x_arr, xw);
    let unit_bytes = (ow / 8).max(1);
    let n_units = bytes.len() / unit_bytes;
    let mut result = vec![0u8; bytes.len()];
    for i in 0..n_units {
        let offset = i * unit_bytes;
        let val = bit_read_signed(&bytes, offset, ow);
        bit_write_signed(&mut result, offset, ow, val.wrapping_neg());
    }
    let total_bits = x_arr.ia() * xw;
    bit_bytes_to_arr(&result, rw, total_bits)
}

/// Dyadic bitwise binary operation (AND, OR, XOR) — operates on raw bytes.
fn bit_binop_c2(operand: B, w: B, x: B, op: impl Fn(u8, u8) -> u8) -> B {
    let (_ow, rw, xw, ww) = bit_parse_widths(operand, true);
    let x_arr = crate::vm::get_arr(x)
        .unwrap_or_else(|| rbqn_core::error::throw("•bit: 𝕩 must be an array"));
    let w_arr = crate::vm::get_arr(w)
        .unwrap_or_else(|| rbqn_core::error::throw("•bit: 𝕨 must be an array"));
    let xb = bit_arr_to_bytes(&x_arr, xw);
    let wb = bit_arr_to_bytes(&w_arr, ww);
    let n = xb.len().min(wb.len());
    let result: Vec<u8> = (0..n).map(|i| op(wb[i], xb[i])).collect();
    let min_total_bits = (x_arr.ia() * xw).min(w_arr.ia() * ww);
    bit_bytes_to_arr(&result, rw, min_total_bits)
}

/// Dyadic arithmetic operation (_add, _sub, _mul) — operates on width-sized units.
fn bit_arith_c2(operand: B, w: B, x: B, op: impl Fn(i64, i64) -> i64) -> B {
    let (ow, rw, xw, ww) = bit_parse_widths(operand, true);
    let x_arr = crate::vm::get_arr(x)
        .unwrap_or_else(|| rbqn_core::error::throw("•bit: 𝕩 must be an array"));
    let w_arr = crate::vm::get_arr(w)
        .unwrap_or_else(|| rbqn_core::error::throw("•bit: 𝕨 must be an array"));
    let xb = bit_arr_to_bytes(&x_arr, xw);
    let wb = bit_arr_to_bytes(&w_arr, ww);
    let unit_bytes = (ow / 8).max(1);
    let n_units = (xb.len().min(wb.len())) / unit_bytes;
    let mut result = vec![0u8; n_units * unit_bytes];
    for i in 0..n_units {
        let offset = i * unit_bytes;
        let a = bit_read_signed(&wb, offset, ow);
        let b = bit_read_signed(&xb, offset, ow);
        let c = op(a, b);
        bit_write_signed(&mut result, offset, ow, c);
    }
    let total_bits = n_units * ow;
    bit_bytes_to_arr(&result, rw, total_bits)
}
