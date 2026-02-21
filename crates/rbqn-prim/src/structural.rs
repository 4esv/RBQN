use rbqn_core::*;
use crate::dispatch::PrimResult;

// = monad: rank
pub fn rank_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Ok(PrimResult::Scalar(B::m_i32(0)));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("=𝕩: 𝕩 must be an array".into()))?;
    Ok(PrimResult::Scalar(B::m_i32(arr.rank() as i32)))
}

// ≠ monad: length (first axis)
pub fn length_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Ok(PrimResult::Scalar(B::m_i32(1)));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("≠𝕩: 𝕩 must be an array".into()))?;
    let len = if arr.shape.is_empty() { 1 } else { arr.shape[0] };
    Ok(PrimResult::Scalar(B::m_f64(len as f64)))
}

// ≢ monad: shape
pub fn shape_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Ok(PrimResult::Array(BqnArr::new_vec_i32(vec![])));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("≢𝕩: 𝕩 must be an array".into()))?;
    let shape: Vec<i32> = arr.shape.iter().map(|&s| s as i32).collect();
    Ok(PrimResult::Array(BqnArr::new_vec_i32(shape)))
}

// ≡ monad: depth
pub fn depth_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    fn compute_depth(x: B, xa: Option<&BqnArr>) -> i32 {
        if x.is_atom() && xa.is_none() {
            return 0;
        }
        match xa {
            Some(arr) => {
                if arr.el_type() != ElType::B {
                    return 1;
                }
                let mut max_d = 0i32;
                let ia = arr.ia();
                for i in 0..ia {
                    if let Ok(v) = arr.get(i) {
                        let d = compute_depth(v, None);
                        max_d = max_d.max(d);
                    }
                }
                max_d + 1
            }
            None => 0,
        }
    }
    Ok(PrimResult::Scalar(B::m_i32(compute_depth(x, xa))))
}

// < monad: enclose
pub fn enclose_c1(x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    Ok(PrimResult::Array(BqnArr {
        shape: vec![],
        data: ArrData::Boxed(vec![x]),
        fill: None,
    }))
}

// > monad: merge (stub - just returns for atoms)
pub fn merge_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Ok(PrimResult::Scalar(x));
    }
    let _arr = xa.ok_or_else(|| BqnError::Type(">𝕩: 𝕩 must be an array".into()))?;
    Err(error::throw_nyi(">: merge not yet implemented"))
}

// ⊣ monad/dyad: identity / left
pub fn identity_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    match xa {
        Some(arr) => Ok(PrimResult::Array(arr.clone())),
        None => Ok(PrimResult::Scalar(x)),
    }
}

pub fn ltack_c2(w: B, wa: Option<&BqnArr>, _x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    match wa {
        Some(arr) => Ok(PrimResult::Array(arr.clone())),
        None => Ok(PrimResult::Scalar(w)),
    }
}

pub fn rtack_c2(_w: B, _wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    match xa {
        Some(arr) => Ok(PrimResult::Array(arr.clone())),
        None => Ok(PrimResult::Scalar(x)),
    }
}

// ⥊ monad: deshape (flatten to list)
pub fn deshape_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Ok(PrimResult::Array(BqnArr {
            shape: vec![1],
            data: ArrData::Boxed(vec![x]),
            fill: None,
        }));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("⥊𝕩: 𝕩 must be an array".into()))?;
    let ia = arr.ia();
    let mut out = arr.clone();
    out.shape = vec![ia];
    Ok(PrimResult::Array(out))
}

// ⥊ dyad: reshape
pub fn reshape_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let new_shape = if w.is_f64() {
        vec![w.to_usz()?]
    } else {
        let warr = wa.ok_or_else(|| BqnError::Type("𝕨⥊𝕩: 𝕨 must be a number or array of numbers".into()))?;
        warr.i32_iter()?.iter().map(|&s| s as usize).collect()
    };
    let new_ia: usize = new_shape.iter().product();

    if x.is_atom() {
        let vals = vec![x.o2f(); new_ia];
        let mut out = BqnArr::new_vec_f64(vals);
        out.shape = new_shape;
        return Ok(PrimResult::Array(array::squeeze_num(out)));
    }

    let arr = xa.ok_or_else(|| BqnError::Type("𝕨⥊𝕩: 𝕩 must be an array".into()))?;
    let old_ia = arr.ia();
    if old_ia == 0 {
        return Err(BqnError::Domain("𝕨⥊𝕩: 𝕩 can't be empty".into()));
    }

    let mut result = Vec::with_capacity(new_ia);
    for i in 0..new_ia {
        result.push(arr.get(i % old_ia)?);
    }
    let mut out = BqnArr::new_vec_b(result);
    out.shape = new_shape;
    Ok(PrimResult::Array(out))
}

// ∾ monad: join (flatten one level of nesting)
pub fn join_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Err(BqnError::Type("∾𝕩: 𝕩 must be an array".into()));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("∾𝕩: 𝕩 must be an array".into()))?;
    if arr.rank() != 1 {
        return Err(error::throw_nyi("∾: rank>1 not yet implemented"));
    }
    Err(error::throw_nyi("∾: monadic join not yet implemented"))
}

// ∾ dyad: join to
pub fn join_to_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    match (wa, xa) {
        (Some(warr), Some(xarr)) => {
            if warr.rank() != 1 || xarr.rank() != 1 {
                return Err(error::throw_nyi("∾: non-vector join not yet implemented"));
            }
            let wia = warr.ia();
            let xia = xarr.ia();
            let mut result = Vec::with_capacity(wia + xia);
            for i in 0..wia {
                result.push(warr.get(i)?);
            }
            for i in 0..xia {
                result.push(xarr.get(i)?);
            }
            Ok(PrimResult::Array(BqnArr::new_vec_b(result)))
        }
        (None, Some(xarr)) => {
            if xarr.rank() != 1 {
                return Err(error::throw_nyi("∾: non-vector join not yet implemented"));
            }
            let xia = xarr.ia();
            let mut result = Vec::with_capacity(1 + xia);
            result.push(w);
            for i in 0..xia {
                result.push(xarr.get(i)?);
            }
            Ok(PrimResult::Array(BqnArr::new_vec_b(result)))
        }
        (Some(warr), None) => {
            if warr.rank() != 1 {
                return Err(error::throw_nyi("∾: non-vector join not yet implemented"));
            }
            let wia = warr.ia();
            let mut result = Vec::with_capacity(wia + 1);
            for i in 0..wia {
                result.push(warr.get(i)?);
            }
            result.push(x);
            Ok(PrimResult::Array(BqnArr::new_vec_b(result)))
        }
        (None, None) => Err(BqnError::Type("𝕨∾𝕩: Arguments must include an array".into())),
    }
}

// ≍ monad: solo (wrap in 1-element list)
pub fn solo_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    match xa {
        Some(arr) => {
            let mut new_shape = vec![1];
            new_shape.extend_from_slice(&arr.shape);
            let mut out = arr.clone();
            out.shape = new_shape;
            Ok(PrimResult::Array(out))
        }
        None => Ok(PrimResult::Array(BqnArr {
            shape: vec![1],
            data: ArrData::Boxed(vec![x]),
            fill: None,
        })),
    }
}

// ≍ dyad: couple
pub fn couple_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    match (wa, xa) {
        (None, None) => {
            if w.is_f64() && x.is_f64() {
                Ok(PrimResult::Array(BqnArr::new_vec_f64(vec![w.o2f(), x.o2f()])))
            } else {
                Ok(PrimResult::Array(BqnArr::new_vec_b(vec![w, x])))
            }
        }
        _ => Err(error::throw_nyi("≍: array couple not yet implemented")),
    }
}

// ⋈ dyad: pair
pub fn pair_c2(w: B, _wa: Option<&BqnArr>, x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    Ok(PrimResult::Array(BqnArr::new_vec_b(vec![w, x])))
}

// ↑ monad: prefixes
pub fn prefixes_c1(_x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    Err(error::throw_nyi("↑: prefixes not yet implemented"))
}

// ↑ dyad: take
pub fn take_c2(w: B, _wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let n = w.to_i32()?;
    let arr = if x.is_atom() {
        BqnArr {
            shape: vec![1],
            data: ArrData::Boxed(vec![x]),
            fill: None,
        }
    } else {
        xa.ok_or_else(|| BqnError::Type("𝕨↑𝕩: 𝕩 must be an array".into()))?.clone()
    };

    if arr.rank() != 1 {
        return Err(error::throw_nyi("↑: rank>1 take not yet implemented"));
    }

    let ia = arr.ia() as i32;
    let (start, len) = if n >= 0 {
        (0, n.min(ia) as usize)
    } else {
        let s = (ia + n).max(0);
        (s as usize, (ia - s) as usize)
    };

    let mut result = Vec::with_capacity(n.unsigned_abs() as usize);
    let take_len = n.unsigned_abs() as usize;
    for i in 0..take_len {
        let idx = start + i;
        if idx < len + start && idx < arr.ia() {
            result.push(arr.get(idx)?);
        } else {
            result.push(arr.fill.unwrap_or(B::m_i32(0)));
        }
    }
    Ok(PrimResult::Array(BqnArr::new_vec_b(result)))
}

// ↓ monad: suffixes
pub fn suffixes_c1(_x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    Err(error::throw_nyi("↓: suffixes not yet implemented"))
}

// ↓ dyad: drop
pub fn drop_c2(w: B, _wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let n = w.to_i32()?;
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨↓𝕩: 𝕩 must be an array".into()))?;

    if arr.rank() != 1 {
        return Err(error::throw_nyi("↓: rank>1 drop not yet implemented"));
    }

    let ia = arr.ia() as i32;
    let (start, end) = if n >= 0 {
        (n.min(ia) as usize, ia as usize)
    } else {
        (0, (ia + n).max(0) as usize)
    };

    let mut result = Vec::with_capacity(end.saturating_sub(start));
    for i in start..end {
        result.push(arr.get(i)?);
    }
    Ok(PrimResult::Array(BqnArr::new_vec_b(result)))
}

// ↕ monad: range
pub fn range_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_f64() {
        let n = x.to_usz()?;
        let vals: Vec<i32> = (0..n as i32).collect();
        return Ok(PrimResult::Array(BqnArr::new_vec_i32(vals)));
    }
    let _arr = xa.ok_or_else(|| BqnError::Type("↕𝕩: 𝕩 must be a number or array".into()))?;
    Err(error::throw_nyi("↕: multi-dimensional range not yet implemented"))
}

// ↕ dyad: windows
pub fn windows_c2(_w: B, _wa: Option<&BqnArr>, _x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    Err(error::throw_nyi("↕: windows not yet implemented"))
}

// « dyad: shift after
pub fn shifta_c2(w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨«𝕩: 𝕩 must be an array".into()))?;
    if arr.rank() != 1 {
        return Err(error::throw_nyi("«: rank>1 not yet implemented"));
    }
    let ia = arr.ia();
    let fill_vals = match wa {
        Some(warr) => {
            let wia = warr.ia();
            let mut v = Vec::with_capacity(wia);
            for i in 0..wia {
                v.push(warr.get(i)?);
            }
            v
        }
        None => vec![w],
    };
    let shift = fill_vals.len();
    let mut result = Vec::with_capacity(ia);
    for i in shift..ia {
        result.push(arr.get(i)?);
    }
    for v in fill_vals.iter().take(ia.saturating_sub(0)).skip(0) {
        result.push(*v);
    }
    while result.len() < ia {
        result.push(arr.fill.unwrap_or(B::m_i32(0)));
    }
    result.truncate(ia);
    let mut out = BqnArr::new_vec_b(result);
    out.shape = arr.shape.clone();
    Ok(PrimResult::Array(out))
}

// » dyad: shift before
pub fn shiftb_c2(w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨»𝕩: 𝕩 must be an array".into()))?;
    if arr.rank() != 1 {
        return Err(error::throw_nyi("»: rank>1 not yet implemented"));
    }
    let ia = arr.ia();
    let fill_vals = match wa {
        Some(warr) => {
            let wia = warr.ia();
            let mut v = Vec::with_capacity(wia);
            for i in 0..wia {
                v.push(warr.get(i)?);
            }
            v
        }
        None => vec![w],
    };
    let shift = fill_vals.len();
    let mut result = Vec::with_capacity(ia);
    for v in fill_vals.iter().take(ia) {
        result.push(*v);
    }
    for i in 0..ia.saturating_sub(shift) {
        result.push(arr.get(i)?);
    }
    result.truncate(ia);
    let mut out = BqnArr::new_vec_b(result);
    out.shape = arr.shape.clone();
    Ok(PrimResult::Array(out))
}

// ⌽ monad: reverse
pub fn reverse_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("⌽𝕩: 𝕩 must be an array".into()))?;
    if arr.rank() != 1 {
        return Err(error::throw_nyi("⌽: rank>1 reverse not yet implemented"));
    }
    let ia = arr.ia();
    let mut result = Vec::with_capacity(ia);
    for i in (0..ia).rev() {
        result.push(arr.get(i)?);
    }
    Ok(PrimResult::Array(BqnArr::new_vec_b(result)))
}

// ⌽ dyad: rotate
pub fn rotate_c2(w: B, _wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let n = w.to_i32()?;
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨⌽𝕩: 𝕩 must be an array".into()))?;
    if arr.rank() != 1 {
        return Err(error::throw_nyi("⌽: rank>1 rotate not yet implemented"));
    }
    let ia = arr.ia();
    if ia == 0 {
        return Ok(PrimResult::Array(arr.clone()));
    }
    let shift = ((n % ia as i32) + ia as i32) as usize % ia;
    let mut result = Vec::with_capacity(ia);
    for i in 0..ia {
        result.push(arr.get((i + shift) % ia)?);
    }
    Ok(PrimResult::Array(BqnArr::new_vec_b(result)))
}

// ⍉ monad: transpose
pub fn transpose_c1(_x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    Err(error::throw_nyi("⍉: transpose not yet implemented"))
}

// ⍉ dyad: reorder axes
pub fn reorder_c2(_w: B, _wa: Option<&BqnArr>, _x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    Err(error::throw_nyi("⍉: reorder axes not yet implemented"))
}
