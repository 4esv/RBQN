use rbqn_core::*;
use rbqn_core::array::typed_arr_from_b_vec;
use crate::dispatch::PrimResult;

/// Compare major cell i with major cell j in an array (high-rank cell comparison).
fn cells_equal(arr: &BqnArr, i: usize, j: usize, cell_size: usize) -> bool {
    let base_i = i * cell_size;
    let base_j = j * cell_size;
    for k in 0..cell_size {
        if let (Ok(a), Ok(b)) = (arr.get(base_i + k), arr.get(base_j + k)) {
            if !rbqn_core::compare::deep_equal(a, b) { return false; }
        } else { return false; }
    }
    true
}

// ⊐ monad: classify (supports high-rank — compares major cells)
#[allow(non_snake_case)]
pub fn self_indexOf_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Err(BqnError::Type("⊐𝕩: 𝕩 must be an array".into()));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("⊐𝕩: 𝕩 must be an array".into()))?;
    if arr.rank() == 0 {
        return Err(BqnError::Rank("⊐𝕩: 𝕩 must have rank ≥ 1".into()));
    }
    let lead = arr.shape[0];
    let cell_size: usize = if arr.rank() > 1 { arr.shape[1..].iter().product() } else { 1 };
    let mut result = Vec::with_capacity(lead);
    let mut class_map: Vec<i32> = Vec::with_capacity(lead);
    let mut next_class: i32 = 0;
    for i in 0..lead {
        let mut found_class: Option<i32> = None;
        if cell_size == 1 {
            let v = arr.get(i)?;
            for j in 0..i {
                if rbqn_core::compare::deep_equal(v, arr.get(j)?) {
                    found_class = Some(class_map[j]);
                    break;
                }
            }
        } else {
            for j in 0..i {
                if cells_equal(arr, i, j, cell_size) {
                    found_class = Some(class_map[j]);
                    break;
                }
            }
        }
        let cls = found_class.unwrap_or_else(|| { let c = next_class; next_class += 1; c });
        class_map.push(cls);
        result.push(cls);
    }
    Ok(PrimResult::Array(BqnArr::new_vec_i32(result)))
}

// ⊐ dyad: index of
// NOTE: 𝕨 must be rank-1 OR rank-N where cells (rank-N-1 sub-arrays) match 𝕩 element shape.
// For rank-1 𝕨: each element of 𝕩 (or 𝕩 itself if atom) is searched for.
#[allow(non_snake_case)]
pub fn indexOf_c2(_w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⊐𝕩: 𝕨 must be an array".into()))?;

    // Validate: rank-0 w is not valid for ⊐
    if warr.rank() == 0 {
        return Err(BqnError::Rank("𝕨⊐𝕩: 𝕨 must be rank-1 or higher".into()));
    }

    // Validate: w must be rank-1 (for simple indexOf) or rank-N with matching cell shapes
    if warr.rank() > 1 {
        // Higher-rank w: cell shape must match x's shape
        let w_cell_shape = &warr.shape[1..];
        let x_shape: &[usize] = if x.is_atom() {
            &[]
        } else if let Some(xarr) = xa {
            &xarr.shape
        } else {
            &[]
        };
        if w_cell_shape != x_shape {
            return Err(BqnError::Rank(format!(
                "𝕨⊐𝕩: 𝕨 cell shape {:?} doesn't match 𝕩 shape {:?}",
                w_cell_shape, x_shape
            )));
        }
    }

    let wia = warr.ia();

    // Trace: detect glyph lookups during compilation
    if std::env::var("RBQN_COMP_TRACE").is_ok() && x.is_c32() {
        let ch = char::from_u32(x.0 as u32).unwrap_or('?');
        eprintln!("[⊐ TRACE] atom char '{}' (u32={}) in array of {} elems", ch, x.0 as u32, wia);
    }

    // Handle atom x: w⊐atom returns scalar index
    if x.is_atom() {
        let mut found = wia as i32;
        for j in 0..wia {
            if rbqn_core::compare::deep_equal(x, warr.get(j)?) {
                found = j as i32;
                break;
            }
        }
        return Ok(PrimResult::Scalar(B::m_i32(found)));
    }

    let xarr = xa.ok_or_else(|| BqnError::Type("𝕨⊐𝕩: 𝕩 must be an array".into()))?;
    let xia = xarr.ia();
    let mut result = Vec::with_capacity(xia);
    for i in 0..xia {
        let xv = xarr.get(i)?;
        let mut found = wia as i32;
        for j in 0..wia {
            if rbqn_core::compare::deep_equal(xv, warr.get(j)?) {
                found = j as i32;
                break;
            }
        }
        result.push(found);
    }

    // Trace: detect glyph lookups during compilation (array ⊐ array with chars)
    if std::env::var("RBQN_COMP_TRACE").is_ok() && xia > 0 {
        if let Ok(x0) = xarr.get(0) {
            if x0.is_c32() && wia >= 10 {
                let xchars: String = (0..xia.min(10)).filter_map(|i| {
                    xarr.get(i).ok().and_then(|b| {
                        if b.is_c32() { char::from_u32(b.0 as u32) } else { None }
                    })
                }).collect();
                let wchars: String = (0..wia.min(20)).filter_map(|i| {
                    warr.get(i).ok().and_then(|b| {
                        if b.is_c32() { char::from_u32(b.0 as u32) } else { None }
                    })
                }).collect();
                eprintln!("[⊐ TRACE arr] w=\"{}\"({}) ⊐ x=\"{}\"({}) → {:?}", wchars, wia, xchars, xia, &result);
            }
        }
    }

    let mut out = BqnArr::new_vec_i32(result);
    out.shape = xarr.shape.clone();
    Ok(PrimResult::Array(out))
}

// ⊒ monad: self-count (supports high-rank — compares major cells)
pub fn self_count_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Err(BqnError::Type("⊒𝕩: 𝕩 must be an array".into()));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("⊒𝕩: 𝕩 must be an array".into()))?;
    if arr.rank() == 0 {
        return Err(BqnError::Rank("⊒𝕩: 𝕩 must have rank ≥ 1".into()));
    }
    let lead = arr.shape[0];
    let cell_size: usize = if arr.rank() > 1 { arr.shape[1..].iter().product() } else { 1 };
    let mut result = Vec::with_capacity(lead);
    for i in 0..lead {
        let mut count = 0i32;
        if cell_size == 1 {
            let v = arr.get(i)?;
            for j in 0..i {
                if rbqn_core::compare::deep_equal(v, arr.get(j)?) { count += 1; }
            }
        } else {
            for j in 0..i {
                if cells_equal(arr, i, j, cell_size) { count += 1; }
            }
        }
        result.push(count);
    }
    Ok(PrimResult::Array(BqnArr::new_vec_i32(result)))
}

// ⊒ dyad: progressive index of (count)
// NOTE: Like ⊐, 𝕨 and 𝕩 must have compatible shapes.
// 𝕨 must be rank-1 for simple element search, or rank-N with cell shape matching 𝕩 elements.
pub fn count_c2(_w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⊒𝕩: 𝕨 must be an array".into()))?;

    // Validate rank compatibility
    if warr.rank() > 1 {
        let w_cell_shape = &warr.shape[1..];
        let x_shape: &[usize] = if x.is_atom() {
            &[]
        } else if let Some(xarr) = xa {
            &xarr.shape
        } else {
            &[]
        };
        if w_cell_shape != x_shape {
            return Err(BqnError::Rank(format!(
                "𝕨⊒𝕩: 𝕨 cell shape {:?} doesn't match 𝕩 shape {:?}",
                w_cell_shape, x_shape
            )));
        }
    }

    let xarr = xa.ok_or_else(|| BqnError::Type("𝕨⊒𝕩: 𝕩 must be an array".into()))?;
    let wia = warr.ia();
    let xia = xarr.ia();
    let mut used = vec![false; wia];
    let mut result = Vec::with_capacity(xia);
    for i in 0..xia {
        let xv = xarr.get(i)?;
        let mut found = wia as i32;
        for j in 0..wia {
            if !used[j] && rbqn_core::compare::deep_equal(xv, warr.get(j)?) {
                found = j as i32;
                used[j] = true;
                break;
            }
        }
        result.push(found);
    }
    let mut out = BqnArr::new_vec_i32(result);
    out.shape = xarr.shape.clone();
    Ok(PrimResult::Array(out))
}

// ∊ monad: mark firsts (supports high-rank — compares major cells)
pub fn mark_firsts_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Err(BqnError::Type("∊𝕩: 𝕩 must be an array".into()));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("∊𝕩: 𝕩 must be an array".into()))?;
    if arr.rank() == 0 {
        return Err(BqnError::Rank("∊𝕩: 𝕩 must have rank ≥ 1".into()));
    }
    let lead = arr.shape[0];
    let cell_size: usize = if arr.rank() > 1 { arr.shape[1..].iter().product() } else { 1 };
    let mut result = Vec::with_capacity(lead);
    for i in 0..lead {
        let mut is_first = true;
        if cell_size == 1 {
            let v = arr.get(i)?;
            for j in 0..i {
                if rbqn_core::compare::deep_equal(v, arr.get(j)?) { is_first = false; break; }
            }
        } else {
            for j in 0..i {
                if cells_equal(arr, i, j, cell_size) { is_first = false; break; }
            }
        }
        result.push(is_first as i32);
    }
    Ok(PrimResult::Array(BqnArr::new_vec_i32(result)))
}

// ∊ dyad: member of
pub fn member_of_c2(_w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨∊𝕩: 𝕨 must be an array".into()))?;
    let xarr = xa.ok_or_else(|| BqnError::Type("𝕨∊𝕩: 𝕩 must be an array".into()))?;
    let wia = warr.ia();
    let xia = xarr.ia();
    let mut result = Vec::with_capacity(wia);
    for i in 0..wia {
        let wv = warr.get(i)?;
        let mut found = false;
        for j in 0..xia {
            if rbqn_core::compare::deep_equal(wv, xarr.get(j)?) {
                found = true;
                break;
            }
        }
        result.push(found as i32);
    }
    let mut out = BqnArr::new_vec_i32(result);
    out.shape = warr.shape.clone();
    Ok(PrimResult::Array(out))
}

// ⍷ monad: deduplicate
pub fn deduplicate_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("⍷𝕩: 𝕩 must be an array".into()))?;
    let ia = arr.ia();
    let mut result = Vec::new();
    for i in 0..ia {
        let v = arr.get(i)?;
        let mut is_dup = false;
        for j in 0..i {
            if rbqn_core::compare::deep_equal(v, arr.get(j)?) {
                is_dup = true;
                break;
            }
        }
        if !is_dup {
            result.push(v);
        }
    }
    let len = result.len();
    Ok(PrimResult::Array(typed_arr_from_b_vec(result, vec![len], arr.fill)))
}

// ⍷ dyad: find
// w⍷x marks positions where w occurs as contiguous subsequence in x.
// For vectors: substring search. Returns boolean array same length as x.
// NOTE: w and x must have the same rank (or w.rank == x.rank), and trailing shapes match.
pub fn find_c2(w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if w.is_atom() {
        return Err(BqnError::Type("𝕨⍷𝕩: 𝕨 must be an array".into()));
    }
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⍷𝕩: 𝕨 must be an array".into()))?;
    let xarr = xa.ok_or_else(|| BqnError::Type("𝕨⍷𝕩: 𝕩 must be an array".into()))?;

    // NOTE: w and x must have the same rank for find to make sense
    if warr.rank() != xarr.rank() {
        return Err(BqnError::Rank(format!(
            "𝕨⍷𝕩: 𝕨 and 𝕩 must have the same rank ({} vs {})",
            warr.rank(), xarr.rank()
        )));
    }

    let rank = warr.rank() as usize;

    // Result shape: 1+≢x-≢w per axis (clamped to 0)
    let mut result_shape = Vec::with_capacity(rank);
    for r in 0..rank {
        let d = 1isize + xarr.shape[r] as isize - warr.shape[r] as isize;
        result_shape.push(if d < 0 { 0 } else { d as usize });
    }
    let result_ia: usize = result_shape.iter().product();

    if result_ia == 0 {
        return Ok(PrimResult::Array(BqnArr { shape: result_shape, data: ArrData::I32(vec![]), fill: None }));
    }

    // For rank-1: sliding window match
    if rank <= 1 {
        let wlen = warr.ia();
        let mut result = Vec::with_capacity(result_ia);
        for i in 0..result_ia {
            let mut matches = true;
            for j in 0..wlen {
                if !rbqn_core::compare::deep_equal(warr.get(j)?, xarr.get(i + j)?) {
                    matches = false;
                    break;
                }
            }
            result.push(matches as i32);
        }
        return Ok(PrimResult::Array(BqnArr { shape: result_shape, data: ArrData::I32(result), fill: None }));
    }

    // For rank>1: multi-dimensional sliding window match
    let w_shape = &warr.shape;
    let x_shape = &xarr.shape;
    let wia = warr.ia();

    // Compute x strides
    let mut x_strides = vec![1usize; rank];
    for r in (0..rank - 1).rev() {
        x_strides[r] = x_strides[r + 1] * x_shape[r + 1];
    }

    let mut result = Vec::with_capacity(result_ia);
    for flat_pos in 0..result_ia {
        // Convert flat position to multi-dim result index
        let mut rem = flat_pos;
        let mut result_idx = vec![0usize; rank];
        for r in (0..rank).rev() {
            result_idx[r] = rem % result_shape[r];
            rem /= result_shape[r];
        }
        // Check if w matches x at this offset
        let mut matches = true;
        for w_flat in 0..wia {
            let mut w_idx = vec![0usize; rank];
            let mut w_rem = w_flat;
            for r in (0..rank).rev() {
                w_idx[r] = w_rem % w_shape[r];
                w_rem /= w_shape[r];
            }
            // x position = result_idx + w_idx per axis
            let mut x_flat = 0;
            for r in 0..rank {
                x_flat += (result_idx[r] + w_idx[r]) * x_strides[r];
            }
            if !rbqn_core::compare::deep_equal(warr.get(w_flat)?, xarr.get(x_flat)?) {
                matches = false;
                break;
            }
        }
        result.push(matches as i32);
    }

    Ok(PrimResult::Array(BqnArr { shape: result_shape, data: ArrData::I32(result), fill: None }))
}
