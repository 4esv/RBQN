use rbqn_core::*;
use crate::dispatch::PrimResult;

fn grade(arr: &BqnArr, ascending: bool) -> Result<Vec<i32>> {
    // NOTE: BQN grade operates on the first axis (rows for 2D arrays).
    // For rank-1 arrays, each element is a cell. For rank-N, each cell is a
    // row-major sub-array. The result length equals arr.shape[0] (nrows).
    let nrows = if arr.rank() == 0 { 1 } else { arr.shape[0] };
    let cell_size: usize = if arr.rank() <= 1 { 1 } else { arr.shape[1..].iter().product() };
    let mut indices: Vec<i32> = (0..nrows as i32).collect();

    if cell_size == 1 && arr.el_type().is_num() {
        // Fast path: rank-1 numeric array — compare scalars directly
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
    } else if cell_size == 1 {
        // rank-1 non-numeric: compare elements
        let mut vals = Vec::with_capacity(nrows);
        for i in 0..nrows {
            vals.push(arr.get(i)?);
        }
        indices.sort_by(|&a, &b| {
            let va = vals[a as usize];
            let vb = vals[b as usize];
            let cmp = compare::compare(va, vb);
            if ascending { cmp.cmp(&0) } else { 0.cmp(&cmp) }
        });
    } else {
        // Higher-rank: compare rows lexicographically, cell by cell
        indices.sort_by(|&row_a, &row_b| {
            let base_a = row_a as usize * cell_size;
            let base_b = row_b as usize * cell_size;
            for col in 0..cell_size {
                let va = arr.get(base_a + col).unwrap_or(B::SENTINEL);
                let vb = arr.get(base_b + col).unwrap_or(B::SENTINEL);
                let cmp = compare::compare(va, vb);
                if cmp != 0 {
                    return if ascending { cmp.cmp(&0) } else { 0.cmp(&cmp) };
                }
            }
            std::cmp::Ordering::Equal
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
