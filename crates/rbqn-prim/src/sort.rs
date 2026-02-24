use rbqn_core::*;
use rbqn_core::array;
use crate::dispatch::PrimResult;

fn grade(arr: &BqnArr, ascending: bool) -> Result<Vec<i32>> {
    // NOTE: BQN grade operates on the first axis (rows for 2D arrays).
    // For rank-1 arrays, each element is a cell. For rank-N, each cell is a
    // row-major sub-array. The result length equals arr.shape[0] (nrows).

    // Validate: elements must be sortable (numbers, chars, or arrays thereof).
    // Functions and modifiers are not sortable.
    if arr.el_type() == ElType::B {
        // Boxed array: check each element
        for i in 0..arr.ia() {
            let v = arr.get(i)?;
            if !v.is_f64() && !v.is_c32() && !v.is_arr() {
                return Err(BqnError::Type(
                    "⍋/⍒𝕩: elements must be numbers, characters, or arrays".into()
                ));
            }
        }
    }

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

/// Apply a row permutation to an array.
/// `indices` contains row indices (from grade). For rank-1, each "row" is one element.
/// For higher-rank, each "row" is a cell of `cell_size` elements along the first axis.
fn apply_row_permutation(arr: &BqnArr, indices: &[i32]) -> Result<BqnArr> {
    let cell_size: usize = if arr.rank() <= 1 { 1 } else { arr.shape[1..].iter().product() };
    let ia = arr.ia();
    let mut result = Vec::with_capacity(ia);
    for &row_idx in indices {
        let base = row_idx as usize * cell_size;
        for col in 0..cell_size {
            result.push(arr.get(base + col)?);
        }
    }
    Ok(array::typed_arr_from_b_vec(result, arr.shape.clone(), arr.fill))
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

// ∧ monad: sort up (ascending) — returns sorted values, NOT the grade
pub fn sort_up_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("∧𝕩: 𝕩 must be an array".into()))?;
    let indices = grade(arr, true)?;
    let out = apply_row_permutation(arr, &indices)?;
    Ok(PrimResult::Array(out))
}

// ∨ monad: sort down (descending) — returns sorted values, NOT the grade
pub fn sort_down_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("∨𝕩: 𝕩 must be an array".into()))?;
    let indices = grade(arr, false)?;
    let out = apply_row_permutation(arr, &indices)?;
    Ok(PrimResult::Array(out))
}

// ⍋ dyad: bins (ascending)
// NOTE: 𝕨 and 𝕩 must be compatible sorted arrays. Elements must be comparable (numbers, chars, arrays).
pub fn bins_up_c2(_w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⍋𝕩: 𝕨 must be an array".into()))?;
    let xarr = xa.ok_or_else(|| BqnError::Type("𝕨⍋𝕩: 𝕩 must be an array".into()))?;

    // Validate: elements must be comparable (not functions/modifiers)
    if warr.el_type() == ElType::B {
        for i in 0..warr.ia() {
            let v = warr.get(i)?;
            if !v.is_f64() && !v.is_c32() && !v.is_arr() {
                return Err(BqnError::Type("𝕨⍋𝕩: elements must be numbers, characters, or arrays".into()));
            }
        }
    }

    // Validate rank compatibility: x.rank must be >= w.rank-1, and x.shape must end with w.shape[1..]
    let wr = warr.rank() as usize;
    let xr = xarr.rank() as usize;
    let w_cell_shape = if wr > 0 { &warr.shape[1..] } else { &[] as &[usize] };
    let w_cell_rank = w_cell_shape.len();
    if xr < w_cell_rank {
        return Err(BqnError::Rank(format!(
            "𝕨⍋𝕩: 𝕩 rank {} too low for 𝕨 cell rank {} (need rank ≥ {})",
            xr, w_cell_rank, w_cell_rank
        )));
    }
    // x's trailing shape must match w's cell shape
    let x_tail = &xarr.shape[xr - w_cell_rank..];
    if x_tail != w_cell_shape {
        return Err(BqnError::Shape(format!(
            "𝕨⍋𝕩: 𝕩 trailing shape {:?} doesn't match 𝕨 cell shape {:?}",
            x_tail, w_cell_shape
        )));
    }

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
