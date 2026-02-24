use rbqn_core::*;
use rbqn_core::array::typed_arr_from_b_vec;
use crate::dispatch::PrimResult;

// ⊐ monad: self-index-of (classify)
#[allow(non_snake_case)]
pub fn self_indexOf_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("⊐𝕩: 𝕩 must be an array".into()))?;
    let ia = arr.ia();
    let mut result = Vec::with_capacity(ia);
    for i in 0..ia {
        let v = arr.get(i)?;
        let mut found = i;
        for j in 0..i {
            if rbqn_core::compare::deep_equal(v, arr.get(j)?) {
                found = j;
                break;
            }
        }
        result.push(found as i32);
    }
    Ok(PrimResult::Array(BqnArr::new_vec_i32(result)))
}

// ⊐ dyad: index of
#[allow(non_snake_case)]
pub fn indexOf_c2(_w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⊐𝕩: 𝕨 must be an array".into()))?;
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
pub fn self_count_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("⊒𝕩: 𝕩 must be an array".into()))?;
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
pub fn count_c2(_w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⊒𝕩: 𝕨 must be an array".into()))?;
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
pub fn mark_firsts_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("∊𝕩: 𝕩 must be an array".into()))?;
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
pub fn find_c2(_w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⍷𝕩: 𝕨 must be an array".into()))?;
    let xarr = xa.ok_or_else(|| BqnError::Type("𝕨⍷𝕩: 𝕩 must be an array".into()))?;

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
