use rbqn_core::*;
use crate::dispatch::PrimResult;

fn grade(arr: &BqnArr, ascending: bool) -> Result<Vec<i32>> {
    let ia = arr.ia();
    let mut indices: Vec<i32> = (0..ia as i32).collect();

    if arr.el_type().is_num() {
        let vals = arr.f64_iter()?;
        indices.sort_by(|&a, &b| {
            let va = vals[a as usize];
            let vb = vals[b as usize];
            if ascending {
                va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                vb.partial_cmp(&va).unwrap_or(std::cmp::Ordering::Equal)
            }
        });
    } else {
        let mut vals = Vec::with_capacity(ia);
        for i in 0..ia {
            vals.push(arr.get(i)?);
        }
        indices.sort_by(|&a, &b| {
            let va = vals[a as usize];
            let vb = vals[b as usize];
            let cmp = compare::compare(va, vb);
            if ascending {
                cmp.cmp(&0)
            } else {
                0.cmp(&cmp)
            }
        });
    }

    Ok(indices)
}

// ⍋ monad: grade up
pub fn grade_up_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("⍋𝕩: 𝕩 must be an array".into()))?;
    let indices = grade(arr, true)?;
    Ok(PrimResult::Array(BqnArr::new_vec_i32(indices)))
}

// ⍒ monad: grade down
pub fn grade_down_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("⍒𝕩: 𝕩 must be an array".into()))?;
    let indices = grade(arr, false)?;
    Ok(PrimResult::Array(BqnArr::new_vec_i32(indices)))
}

// ⍋ dyad: bins (ascending)
pub fn bins_up_c2(_w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⍋𝕩: 𝕨 must be an array".into()))?;
    let xarr = xa.ok_or_else(|| BqnError::Type("𝕨⍋𝕩: 𝕩 must be an array".into()))?;

    if warr.el_type().is_num() && xarr.el_type().is_num() {
        let wvals = warr.f64_iter()?;
        let xvals = xarr.f64_iter()?;
        let mut result = Vec::with_capacity(xvals.len());
        for &xv in &xvals {
            let pos = wvals.partition_point(|&wv| wv <= xv);
            result.push(pos as i32);
        }
        let mut out = BqnArr::new_vec_i32(result);
        out.shape = xarr.shape.clone();
        return Ok(PrimResult::Array(out));
    }

    // Non-numeric bins: use compare::compare for ordering
    let wia = warr.ia();
    let xia = xarr.ia();
    let mut wvals = Vec::with_capacity(wia);
    for i in 0..wia {
        wvals.push(warr.get(i)?);
    }
    let mut result = Vec::with_capacity(xia);
    for i in 0..xia {
        let xv = xarr.get(i)?;
        // Binary search: find first position where wvals[pos] > xv
        let pos = wvals.partition_point(|&wv| compare::compare(wv, xv) <= 0);
        result.push(pos as i32);
    }
    let mut out = BqnArr::new_vec_i32(result);
    out.shape = xarr.shape.clone();
    Ok(PrimResult::Array(out))
}

// ⍒ dyad: bins (descending)
pub fn bins_down_c2(_w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⍒𝕩: 𝕨 must be an array".into()))?;
    let xarr = xa.ok_or_else(|| BqnError::Type("𝕨⍒𝕩: 𝕩 must be an array".into()))?;

    if warr.el_type().is_num() && xarr.el_type().is_num() {
        let wvals = warr.f64_iter()?;
        let xvals = xarr.f64_iter()?;
        let mut result = Vec::with_capacity(xvals.len());
        for &xv in &xvals {
            let pos = wvals.partition_point(|&wv| wv >= xv);
            result.push(pos as i32);
        }
        let mut out = BqnArr::new_vec_i32(result);
        out.shape = xarr.shape.clone();
        return Ok(PrimResult::Array(out));
    }

    // Non-numeric bins descending: use compare::compare for ordering
    let wia = warr.ia();
    let xia = xarr.ia();
    let mut wvals = Vec::with_capacity(wia);
    for i in 0..wia {
        wvals.push(warr.get(i)?);
    }
    let mut result = Vec::with_capacity(xia);
    for i in 0..xia {
        let xv = xarr.get(i)?;
        // Binary search: find first position where wvals[pos] < xv (descending)
        let pos = wvals.partition_point(|&wv| compare::compare(wv, xv) >= 0);
        result.push(pos as i32);
    }
    let mut out = BqnArr::new_vec_i32(result);
    out.shape = xarr.shape.clone();
    Ok(PrimResult::Array(out))
}
