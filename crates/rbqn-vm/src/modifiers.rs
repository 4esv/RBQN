// Native modifier dispatch: implements all 1-modifiers (idx 44-52) and 2-modifiers (idx 53-63).
// Modifier logic lives here rather than in rbqn-prim because modifiers need to call
// derive::c1/c2 to apply operand functions, which creates a circular dep if in rbqn-prim.

use rbqn_core::{B, BqnArr, ArrData};

use crate::derive::{c1, c2};

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
        _ => rbqn_core::error::throw(format!(
            "native 2-modifier idx {} c2 not implemented", prim_idx
        )),
    }
}

// ============================================================
// Helpers
// ============================================================

/// Convert a Vec<B> of results into a typed array.
/// If all results are numeric scalars, produces a numeric array (applying squeeze).
/// If all results are characters, produces a character array.
/// Otherwise, keeps as Boxed.
fn results_to_arr(results: Vec<B>, shape: Vec<usize>) -> B {
    if results.is_empty() {
        let mut out = BqnArr::new_vec_b(results);
        out.shape = shape;
        return crate::vm::tag_arr(out);
    }
    if results.iter().all(|b| b.is_f64()) {
        let vals: Vec<f64> = results.iter().map(|b| b.o2f()).collect();
        let mut out = rbqn_core::array::BqnArr::new_vec_f64(vals);
        out.shape = shape;
        return crate::vm::tag_arr(rbqn_core::array::squeeze_num(out));
    }
    if results.iter().all(|b| b.is_c32()) {
        let vals: Vec<u32> = results.iter().map(|b| b.0 as u32).collect();
        let mut out = rbqn_core::array::BqnArr::new_vec_c32(vals);
        out.shape = shape;
        return crate::vm::tag_arr(out);
    }
    let mut out = BqnArr::new_vec_b(results);
    out.shape = shape;
    crate::vm::tag_arr(out)
}

/// Merge cell results into a higher-rank array.
/// Used by ˘ (cells) where results from each cell are combined with
/// shape = leading_shape ∾ cell_result_shape.
/// If all results are scalars, produces a simple array.
/// If all results are arrays of the same shape, flattens and concatenates.
fn merge_cells_result(results: Vec<B>, lead_shape: Vec<usize>) -> B {
    if results.is_empty() {
        let mut out = BqnArr::new_vec_b(results);
        out.shape = lead_shape;
        return crate::vm::tag_arr(out);
    }
    // All scalar numbers
    if results.iter().all(|b| b.is_f64()) {
        let vals: Vec<f64> = results.iter().map(|b| b.o2f()).collect();
        let mut out = rbqn_core::array::BqnArr::new_vec_f64(vals);
        out.shape = lead_shape;
        return crate::vm::tag_arr(rbqn_core::array::squeeze_num(out));
    }
    // All scalar characters
    if results.iter().all(|b| b.is_c32()) {
        let vals: Vec<u32> = results.iter().map(|b| b.0 as u32).collect();
        let mut out = rbqn_core::array::BqnArr::new_vec_c32(vals);
        out.shape = lead_shape;
        return crate::vm::tag_arr(out);
    }
    // All arrays with same cell shape → merge into higher-rank
    if results.iter().all(|b| b.is_arr()) {
        let arrs: Vec<BqnArr> = results.iter()
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
                if flat.iter().all(|b| b.is_f64()) {
                    let vals: Vec<f64> = flat.iter().map(|b| b.o2f()).collect();
                    let mut out = rbqn_core::array::BqnArr::new_vec_f64(vals);
                    out.shape = merged_shape;
                    return crate::vm::tag_arr(rbqn_core::array::squeeze_num(out));
                }
                if flat.iter().all(|b| b.is_c32()) {
                    let vals: Vec<u32> = flat.iter().map(|b| b.0 as u32).collect();
                    let mut out = rbqn_core::array::BqnArr::new_vec_c32(vals);
                    out.shape = merged_shape;
                    return crate::vm::tag_arr(out);
                }
                let mut out = BqnArr::new_vec_b(flat);
                out.shape = merged_shape;
                return crate::vm::tag_arr(out);
            }
        }
    }
    // Fallback: boxed array
    let mut out = BqnArr::new_vec_b(results);
    out.shape = lead_shape;
    crate::vm::tag_arr(out)
}

fn arr_of(x: B) -> BqnArr {
    crate::vm::get_arr(x)
        .unwrap_or_else(|| rbqn_core::error::throw("Expected array argument"))
}

fn get_elem(arr: &BqnArr, i: usize) -> B {
    arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()))
}

/// Extract major cell `cell_idx` from a higher-rank array.
fn extract_cell(arr: &BqnArr, cell_idx: usize, cell_size: usize, cell_shape: &[usize]) -> BqnArr {
    let start = cell_idx * cell_size;
    let data = match &arr.data {
        ArrData::Bit(v) => {
            let nwords = (cell_size + 63) / 64;
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
    if x.is_atom() {
        return c1(f, x);
    }
    let arr = arr_of(x);
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
    if w_is_atom && x_is_atom {
        return c2(f, w, x);
    }
    if w_is_atom {
        let xarr = arr_of(x);
        let n = xarr.ia();
        let mut results = Vec::with_capacity(n);
        for i in 0..n {
            results.push(c2(f, w, get_elem(&xarr, i)));
        }
        return results_to_arr(results, xarr.shape.clone());
    }
    if x_is_atom {
        let warr = arr_of(w);
        let n = warr.ia();
        let mut results = Vec::with_capacity(n);
        for i in 0..n {
            results.push(c2(f, get_elem(&warr, i), x));
        }
        return results_to_arr(results, warr.shape.clone());
    }
    let warr = arr_of(w);
    let xarr = arr_of(x);
    if warr.shape != xarr.shape {
        rbqn_core::error::throw("¨: 𝕨 and 𝕩 must have the same shape");
    }
    let n = warr.ia();
    let mut results = Vec::with_capacity(n);
    for i in 0..n {
        results.push(c2(f, get_elem(&warr, i), get_elem(&xarr, i)));
    }
    results_to_arr(results, warr.shape.clone())
}

// ============================================================
// 1-modifier: ⌜ Table
// ============================================================

fn table_c2(f: B, w: B, x: B) -> B {
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
    let n = arr.ia();
    if n == 0 {
        return fold_identity(f).unwrap_or_else(||
            rbqn_core::error::throw("´: empty array with no identity")
        );
    }
    let mut acc = get_elem(&arr, n - 1);
    for i in (0..n - 1).rev() {
        acc = c2(f, get_elem(&arr, i), acc);
    }
    acc
}

fn fold_c2(f: B, w: B, x: B) -> B {
    let arr = arr_of(x);
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

fn insert_c1(f: B, x: B) -> B {
    let arr = arr_of(x);
    if arr.rank() < 2 {
        return fold_c1(f, x);
    }
    let lead = arr.shape[0];
    if lead == 0 {
        // NOTE: Empty leading axis: return identity element in cell shape.
        // CBQN: insert on empty array returns identity element broadcast to cell shape.
        let cell_shape = arr.shape[1..].to_vec();
        let cell_ia: usize = cell_shape.iter().product::<usize>().max(1);
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
    let arr = arr_of(x);
    let n = arr.ia();
    if n == 0 {
        return crate::vm::tag_arr(BqnArr {
            shape: arr.shape.clone(),
            data: ArrData::Boxed(vec![]),
            fill: arr.fill,
        });
    }
    let mut results = Vec::with_capacity(n);
    results.push(get_elem(&arr, 0));
    for i in 1..n {
        let prev = results[i - 1];
        results.push(c2(f, prev, get_elem(&arr, i)));
    }
    results_to_arr(results, arr.shape.clone())
}

fn scan_c2(f: B, w: B, x: B) -> B {
    let arr = arr_of(x);
    let n = arr.ia();
    let mut results = Vec::with_capacity(n);
    let mut acc = w;
    for i in 0..n {
        acc = c2(f, acc, get_elem(&arr, i));
        results.push(acc);
    }
    results_to_arr(results, arr.shape.clone())
}

// ============================================================
// 1-modifier: ˘ Cells
// ============================================================

fn cells_c1(f: B, x: B) -> B {
    if x.is_atom() {
        // Rank 0: the single cell is the atom itself
        return c1(f, x);
    }
    let arr = arr_of(x);
    let lead = arr.shape[0];
    if arr.rank() == 1 {
        // Rank 1: cells are individual elements (rank-0 atoms)
        let mut results = Vec::with_capacity(lead);
        for i in 0..lead {
            let elem = get_elem(&arr, i);
            results.push(c1(f, elem));
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
    let lead = xarr.shape[0];
    if xarr.rank() == 1 {
        // Rank 1: cells are individual elements
        if w.is_arr() {
            let warr = arr_of(w);
            if warr.rank() == 1 && warr.shape[0] == lead {
                // Both rank 1 with same length: pair up elements
                let mut results = Vec::with_capacity(lead);
                for i in 0..lead {
                    results.push(c2(f, get_elem(&warr, i), get_elem(&xarr, i)));
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
                    results.push(c2(f, wc, get_elem(&xarr, i)));
                }
                return merge_cells_result(results, vec![lead]);
            }
        }
        // w is atom or doesn't match: broadcast w to each x element
        let mut results = Vec::with_capacity(lead);
        for i in 0..lead {
            results.push(c2(f, w, get_elem(&xarr, i)));
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

    // Pattern: F⌾< x — enclose under (enclose, apply F, unbox result)
    if let crate::derive::DerivedKind::NativeFn { prim_idx: 12 } = gd.kind {
        // F⌾< x: enclose x, apply F, then > (merge) to unbox
        let enclosed = c1(g, x);  // < x
        let f_enclosed = c1(f, enclosed);
        let merge_fn = crate::derive::m_native_fn(13); // >
        return Some(c1(merge_fn, f_enclosed));
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

                    match rd.kind {
                        // F⌾(k⊸↑) x — take under
                        crate::derive::DerivedKind::NativeFn { prim_idx: 26 } => {
                            // Take k elements, apply F, then stitch back with remaining suffix
                            // Result: (k↑ F(k↑x)) ∾ (k↓x)
                            let take_fn = right_op;  // ↑
                            let drop_fn = crate::derive::m_native_fn(27); // ↓
                            let join_fn = crate::derive::m_native_fn(23);  // ∾
                            let selected = c2(take_fn, left_op, x);       // k↑x
                            let modified = c1(f, selected);                // F(k↑x)
                            let suffix = c2(drop_fn, left_op, x);         // k↓x
                            return Some(c2(join_fn, modified, suffix));    // modified ∾ suffix
                        }
                        // F⌾(k⊸↓) x — drop under
                        crate::derive::DerivedKind::NativeFn { prim_idx: 27 } => {
                            // Drop k elements, apply F to suffix, prefix back
                            // Result: (k↑x) ∾ (k↓ F(k↓x))
                            let take_fn = crate::derive::m_native_fn(26); // ↑
                            let drop_fn = right_op;  // ↓
                            let join_fn = crate::derive::m_native_fn(23);  // ∾
                            let prefix = c2(take_fn, left_op, x);         // k↑x
                            let selected = c2(drop_fn, left_op, x);       // k↓x
                            let modified = c1(f, selected);                // F(k↓x)
                            return Some(c2(join_fn, prefix, modified));    // prefix ∾ modified
                        }
                        // F⌾(arr⊸⊏) x — structural select-under
                        crate::derive::DerivedKind::NativeFn { prim_idx: 36 } => {
                            return Some(structural_select_under(f, left_op, x));
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
    let result = c2_fn(w, wa.as_ref(), x, xa.as_ref())
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
    if let Some(arr) = crate::vm::get_arr(v) {
        if arr.ia() == 1 {
            if let Ok(elem) = arr.get(0) {
                if let Ok(u) = elem.to_usz() {
                    return u;
                }
            }
        }
    }
    rbqn_core::error::throw(format!("◶: Expected number index, got {:#x}", v.0))
}

// ============================================================
// 2-modifier: ⎉ Rank
// ============================================================

/// Compute cell rank from array rank and k parameter.
/// Negative k counts from the end, positive k is clamped to array rank.
fn cell_rank(r: usize, k: f64) -> usize {
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
        g.to_i32().unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()))
    } else {
        // g is a function: compute count by calling g(x)
        let n_b = c1(g, x);
        if n_b.is_arr() {
            let narr = arr_of(n_b);
            return repeat_c1_arr(f, &narr, x);
        }
        n_b.to_i32().unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()))
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
    // Apply repeat for each count value when g is an array
    // Result shape = g shape, each element is f⍟count(i) x
    let n = counts.ia();
    let mut results = Vec::with_capacity(n);
    for i in 0..n {
        let count_b = get_elem(counts, i);
        let count = count_b.to_i32().unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()));
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
    results_to_arr(results, counts.shape.clone())
}

fn repeat_c2(f: B, g: B, w: B, x: B) -> B {
    let n = if g.is_f64() {
        g.to_i32().unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()))
    } else {
        let n_b = c2(g, w, x);
        n_b.to_i32().unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()))
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
        acc = c2(f, w, acc);
    }
    acc
}

// ============================================================
// 2-modifier: ⎊ Catch
// ============================================================

fn catch_c1(f: B, g: B, x: B) -> B {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c1(f, x)));
    match result {
        Ok(v) => v,
        Err(_) => c1(g, x),
    }
}

fn catch_c2(f: B, g: B, w: B, x: B) -> B {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c2(f, w, x)));
    match result {
        Ok(v) => v,
        Err(_) => c2(g, w, x),
    }
}
