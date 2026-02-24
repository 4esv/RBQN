use rbqn_core::*;
use rbqn_core::array::typed_arr_from_b_vec;
use crate::dispatch::PrimResult;

// ⊐ monad: classify
// Returns sequential class indices: first unique → 0, second unique → 1, etc.
// Duplicates get the same class as their first occurrence.
// NOTE: 𝕩 must be rank-1. Rank-0 errors.
#[allow(non_snake_case)]
pub fn self_indexOf_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Err(BqnError::Type("⊐𝕩: 𝕩 must be a rank-1 array".into()));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("⊐𝕩: 𝕩 must be an array".into()))?;
    if arr.rank() == 0 {
        return Err(BqnError::Rank("⊐𝕩: 𝕩 must be rank-1".into()));
    }
    let ia = arr.ia();
    let mut result = Vec::with_capacity(ia);
    // NOTE: class_map[i] = class number assigned to position i
    let mut class_map: Vec<i32> = Vec::with_capacity(ia);
    let mut next_class: i32 = 0;
    for i in 0..ia {
        let v = arr.get(i)?;
        let mut found_class: Option<i32> = None;
        for j in 0..i {
            if rbqn_core::compare::deep_equal(v, arr.get(j)?) {
                found_class = Some(class_map[j]);
                break;
            }
        }
        let cls = match found_class {
            Some(c) => c,
            None => {
                let c = next_class;
                next_class += 1;
                c
            }
        };
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

// ⊒ monad: self-count (occurrence count)
// NOTE: 𝕩 must be rank-1. Rank-0 errors.
pub fn self_count_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Err(BqnError::Type("⊒𝕩: 𝕩 must be a rank-1 array".into()));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("⊒𝕩: 𝕩 must be an array".into()))?;
    if arr.rank() == 0 {
        return Err(BqnError::Rank("⊒𝕩: 𝕩 must be rank-1".into()));
    }
    let ia = arr.ia();
    let mut result = Vec::with_capacity(ia);
    for i in 0..ia {
        let v = arr.get(i)?;
        let mut count = 0i32;
        for j in 0..i {
            if rbqn_core::compare::deep_equal(v, arr.get(j)?) {
                count += 1;
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

// ∊ monad: mark firsts
// NOTE: 𝕩 must be a rank-1 array. Rank-0 (atom or enclosed) errors.
pub fn mark_firsts_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Err(BqnError::Type("∊𝕩: 𝕩 must be a rank-1 array".into()));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("∊𝕩: 𝕩 must be an array".into()))?;
    if arr.rank() == 0 {
        return Err(BqnError::Rank("∊𝕩: 𝕩 must be rank-1".into()));
    }
    let ia = arr.ia();
    let mut result = Vec::with_capacity(ia);
    for i in 0..ia {
        let v = arr.get(i)?;
        let mut is_first = true;
        for j in 0..i {
            if rbqn_core::compare::deep_equal(v, arr.get(j)?) {
                is_first = false;
                break;
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

    let wia = warr.ia();
    let xia = xarr.ia();

    if wia == 0 {
        // Empty needle: all positions match
        let result = vec![1i32; xia];
        let mut out = BqnArr::new_vec_i32(result);
        out.shape = xarr.shape.clone();
        return Ok(PrimResult::Array(out));
    }

    if wia > xia {
        let result = vec![0i32; xia];
        let mut out = BqnArr::new_vec_i32(result);
        out.shape = xarr.shape.clone();
        return Ok(PrimResult::Array(out));
    }

    let mut result = vec![0i32; xia];
    let num_positions = xia - wia + 1;

    for i in 0..num_positions {
        let mut matches = true;
        for j in 0..wia {
            let wv = warr.get(j)?;
            let xv = xarr.get(i + j)?;
            if !rbqn_core::compare::deep_equal(wv, xv) {
                matches = false;
                break;
            }
        }
        if matches {
            result[i] = 1;
        }
    }

    let mut out = BqnArr::new_vec_i32(result);
    out.shape = xarr.shape.clone();
    Ok(PrimResult::Array(out))
}
