// Native modifier dispatch: implements all 1-modifiers (idx 44-52) and 2-modifiers (idx 53-63).
// Modifier logic lives here rather than in rbqn-prim because modifiers need to call
// derive::c1/c2 to apply operand functions, which creates a circular dep if in rbqn-prim.

use rbqn_core::{B, BqnArr, ArrData};

use crate::derive::{c1, c2};

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
const MD2_ATOP: usize = 53;   // ∘
const MD2_OVER: usize = 54;   // ○
const MD2_BEFORE: usize = 55; // ⊸
const MD2_AFTER: usize = 56;  // ⟜
const MD2_UNDER: usize = 57;  // ⌾
const MD2_VAL: usize = 58;    // ⊘
const MD2_COND: usize = 59;   // ◶
const MD2_RANK: usize = 60;   // ⎉
const MD2_DEPTH: usize = 61;  // ⚇
const MD2_REPEAT: usize = 62; // ⍟
const MD2_CATCH: usize = 63;  // ⎊

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
        MD1_UNDO => rbqn_core::error::throw("⁼: inverse not yet implemented"),
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
        MD1_UNDO => rbqn_core::error::throw("⁼: inverse not yet implemented"),
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
        MD2_UNDER => rbqn_core::error::throw("⌾: under not yet implemented"),
        MD2_VAL => c1(f, x),
        MD2_COND => choose_c1(f, g, x),
        MD2_RANK => rbqn_core::error::throw("⎉: rank not yet implemented"),
        MD2_DEPTH => rbqn_core::error::throw("⚇: depth not yet implemented"),
        MD2_REPEAT => repeat_c1(f, g, x),
        MD2_CATCH => catch_c1(f, g, x),
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
        MD2_UNDER => rbqn_core::error::throw("⌾: under not yet implemented"),
        MD2_VAL => c2(g, w, x),
        MD2_COND => choose_c2(f, g, w, x),
        MD2_RANK => rbqn_core::error::throw("⎉: rank not yet implemented"),
        MD2_DEPTH => rbqn_core::error::throw("⚇: depth not yet implemented"),
        MD2_REPEAT => repeat_c2(f, g, w, x),
        MD2_CATCH => catch_c2(f, g, w, x),
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
        results.push(c1(f, get_elem(&arr, i)));
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

fn fold_c1(f: B, x: B) -> B {
    let arr = arr_of(x);
    let n = arr.ia();
    if n == 0 {
        rbqn_core::error::throw("´: empty array with no identity");
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
    let arr = arr_of(x);
    if arr.rank() <= 1 {
        return c1(f, x);
    }
    let lead = arr.shape[0];
    let cell_size: usize = arr.shape[1..].iter().product();
    let cell_shape = arr.shape[1..].to_vec();
    let mut results = Vec::with_capacity(lead);
    for i in 0..lead {
        let cell = crate::vm::tag_arr(extract_cell(&arr, i, cell_size, &cell_shape));
        results.push(c1(f, cell));
    }
    results_to_arr(results, vec![lead])
}

fn cells_c2(f: B, w: B, x: B) -> B {
    let xarr = arr_of(x);
    if xarr.rank() <= 1 {
        return c2(f, w, x);
    }
    let lead = xarr.shape[0];
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
            return results_to_arr(results, vec![lead]);
        }
    }
    let mut results = Vec::with_capacity(lead);
    for i in 0..lead {
        let cell = crate::vm::tag_arr(extract_cell(&xarr, i, cell_size, &cell_shape));
        results.push(c2(f, w, cell));
    }
    results_to_arr(results, vec![lead])
}

// ============================================================
// 2-modifier: ◶ Choose
// ============================================================

fn choose_c1(f: B, g: B, x: B) -> B {
    let idx_b = c1(f, x);
    let idx = idx_b.to_usz().unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()));
    let garr = arr_of(g);
    let chosen = get_elem(&garr, idx);
    c1(chosen, x)
}

fn choose_c2(f: B, g: B, w: B, x: B) -> B {
    let idx_b = c2(f, w, x);
    let idx = idx_b.to_usz().unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()));
    let garr = arr_of(g);
    let chosen = get_elem(&garr, idx);
    c2(chosen, w, x)
}

// ============================================================
// 2-modifier: ⍟ Repeat
// ============================================================

fn repeat_c1(f: B, g: B, x: B) -> B {
    let n = if g.is_f64() {
        g.to_i32().unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()))
    } else {
        let n_b = c1(g, x);
        n_b.to_i32().unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()))
    };
    if n < 0 {
        rbqn_core::error::throw("⍟: negative repeat count requires inverse");
    }
    let mut acc = x;
    for _ in 0..n {
        acc = c1(f, acc);
    }
    acc
}

fn repeat_c2(f: B, g: B, w: B, x: B) -> B {
    let n = if g.is_f64() {
        g.to_i32().unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()))
    } else {
        let n_b = c2(g, w, x);
        n_b.to_i32().unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()))
    };
    if n < 0 {
        rbqn_core::error::throw("⍟: negative repeat count requires inverse");
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
