use rbqn_core::*;
use rbqn_core::array;
use crate::dispatch::PrimResult;

/// Recursively check if a B value contains any non-data (function/modifier) values.
/// Returns true if the value or any nested array element is a function/modifier.
fn contains_non_data(v: B) -> bool {
    if v.is_f64() || v.is_c32() { return false; }
    if v.is_fun() || v.is_md1() || v.is_md2() { return true; }
    if v.is_arr() {
        if let Some(arr) = get_arr(v) {
            for i in 0..arr.ia() {
                if let Ok(elem) = arr.get(i) {
                    if contains_non_data(elem) { return true; }
                }
            }
        }
    }
    false
}

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
    let mut out = BqnArr::new_vec_i32(indices);
    out.fill = Some(B::m_i32(0));
    Ok(PrimResult::Array(out))
}

// ⍒ monad: grade down
pub fn grade_down_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("⍒𝕩: 𝕩 must be an array".into()))?;
    let indices = grade(arr, false)?;
    let mut out = BqnArr::new_vec_i32(indices);
    out.fill = Some(B::m_i32(0));
    Ok(PrimResult::Array(out))
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
pub fn bins_up_c2(_w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⍋𝕩: 𝕨 must be an array".into()))?;
    // Handle atom x: result is rank-0
    if xa.is_none() {
        let w_lead = if warr.rank() > 0 { warr.shape[0] } else { 1 };
        let w_cell_size: usize = if warr.rank() > 1 { warr.shape[1..].iter().product() } else { 1 };
        if w_cell_size != 1 {
            return Err(BqnError::Rank("𝕨⍋𝕩: atom 𝕩 incompatible with non-scalar 𝕨 cells".into()));
        }
        let mut lo = 0usize;
        let mut hi = w_lead;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let wv = warr.get(mid)?;
            if compare::compare(wv, x) <= 0 { lo = mid + 1; } else { hi = mid; }
        }
        return Ok(PrimResult::Array(BqnArr {
            shape: vec![],
            data: array::ArrData::I32(vec![lo as i32]),
            fill: Some(B::m_i32(0)),
        }));
    }
    let xarr = xa.unwrap();

    // Validate: elements must be comparable (no functions/modifiers, even nested)
    if warr.el_type() == ElType::B {
        for i in 0..warr.ia() {
            let v = warr.get(i)?;
            if contains_non_data(v) {
                return Err(BqnError::Type("𝕨⍋𝕩: elements must be numbers, characters, or arrays".into()));
            }
        }
    }
    // Validate x elements recursively
    if xarr.el_type() == ElType::B {
        for i in 0..xarr.ia() {
            let v = xarr.get(i)?;
            if contains_non_data(v) {
                return Err(BqnError::Type("𝕨⍋𝕩: 𝕩 elements must be numbers, characters, or arrays".into()));
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

    let w_lead = if wr > 0 { warr.shape[0] } else { 1 };
    let w_cell_size: usize = w_cell_shape.iter().product::<usize>().max(1);
    let x_lead_dims = xr - w_cell_rank;
    let x_lead_shape = &xarr.shape[..x_lead_dims];
    let x_lead_count: usize = x_lead_shape.iter().product::<usize>().max(1);

    // w_leq_x: returns true if w cell j <= x cell at offset (ascending bins condition)
    let w_leq_x = |x_offset: usize, w_j: usize| -> bool {
        for k in 0..w_cell_size {
            let wv = warr.get(w_j * w_cell_size + k).unwrap_or(B::SENTINEL);
            let xv = xarr.get(x_offset + k).unwrap_or(B::SENTINEL);
            let c = compare::compare(wv, xv);
            if c < 0 { return true; }   // w < x → w <= x
            if c > 0 { return false; }  // w > x → not w <= x
        }
        true // equal → w <= x
    };

    let mut result = Vec::with_capacity(x_lead_count);
    for i in 0..x_lead_count {
        let x_off = i * w_cell_size;
        // Binary search: find first w_j where w_cell[w_j] > x_cell
        let mut lo = 0usize;
        let mut hi = w_lead;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if w_leq_x(x_off, mid) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        result.push(lo as i32);
    }

    if x_lead_dims == 0 {
        // Result is rank-0
        return Ok(PrimResult::Array(BqnArr {
            shape: vec![],
            data: array::ArrData::I32(result),
            fill: Some(B::m_i32(0)),
        }));
    }
    let mut out = BqnArr::new_vec_i32(result);
    out.shape = x_lead_shape.to_vec();
    out.fill = Some(B::m_i32(0));
    Ok(PrimResult::Array(out))
}

// ⍒ dyad: bins (descending)
pub fn bins_down_c2(_w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⍒𝕩: 𝕨 must be an array".into()))?;
    // Handle atom x: result is rank-0
    if xa.is_none() {
        let w_lead = if warr.rank() > 0 { warr.shape[0] } else { 1 };
        let w_cell_size: usize = if warr.rank() > 1 { warr.shape[1..].iter().product() } else { 1 };
        if w_cell_size != 1 {
            return Err(BqnError::Rank("𝕨⍒𝕩: atom 𝕩 incompatible with non-scalar 𝕨 cells".into()));
        }
        let mut lo = 0usize;
        let mut hi = w_lead;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let wv = warr.get(mid)?;
            if compare::compare(wv, x) >= 0 { lo = mid + 1; } else { hi = mid; }
        }
        return Ok(PrimResult::Array(BqnArr {
            shape: vec![],
            data: array::ArrData::I32(vec![lo as i32]),
            fill: Some(B::m_i32(0)),
        }));
    }
    let xarr = xa.unwrap();

    // Validate x elements recursively for non-data values
    if xarr.el_type() == ElType::B {
        for i in 0..xarr.ia() {
            let v = xarr.get(i)?;
            if contains_non_data(v) {
                return Err(BqnError::Type("𝕨⍒𝕩: 𝕩 elements must be numbers, characters, or arrays".into()));
            }
        }
    }

    let wr = warr.rank() as usize;
    let xr = xarr.rank() as usize;
    let w_cell_shape = if wr > 0 { &warr.shape[1..] } else { &[] as &[usize] };
    let w_cell_rank = w_cell_shape.len();
    if xr < w_cell_rank {
        return Err(BqnError::Rank(format!(
            "𝕨⍒𝕩: 𝕩 rank {} too low for 𝕨 cell rank {}",
            xr, w_cell_rank
        )));
    }
    let x_tail = &xarr.shape[xr - w_cell_rank..];
    if x_tail != w_cell_shape {
        return Err(BqnError::Shape(format!(
            "𝕨⍒𝕩: 𝕩 trailing shape {:?} doesn't match 𝕨 cell shape {:?}",
            x_tail, w_cell_shape
        )));
    }

    let w_lead = if wr > 0 { warr.shape[0] } else { 1 };
    let w_cell_size: usize = w_cell_shape.iter().product::<usize>().max(1);
    let x_lead_dims = xr - w_cell_rank;
    let x_lead_shape = &xarr.shape[..x_lead_dims];
    let x_lead_count: usize = x_lead_shape.iter().product::<usize>().max(1);

    // Validate w elements as well
    if warr.el_type() == ElType::B {
        for i in 0..warr.ia() {
            let v = warr.get(i)?;
            if contains_non_data(v) {
                return Err(BqnError::Type("𝕨⍒𝕩: 𝕨 elements must be numbers, characters, or arrays".into()));
            }
        }
    }

    // Compare descending: w[j] >= x means "keep going"
    let compare_cell_desc = |x_offset: usize, w_j: usize| -> bool {
        for k in 0..w_cell_size {
            let xv = xarr.get(x_offset + k).unwrap_or(B::SENTINEL);
            let wv = warr.get(w_j * w_cell_size + k).unwrap_or(B::SENTINEL);
            let c = compare::compare(wv, xv);
            if c > 0 { return true; }   // w > x → w >= x, keep going
            if c < 0 { return false; }  // w < x → stop
        }
        true // equal → w >= x
    };

    let mut result = Vec::with_capacity(x_lead_count);
    for i in 0..x_lead_count {
        let x_off = i * w_cell_size;
        let mut lo = 0usize;
        let mut hi = w_lead;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if compare_cell_desc(x_off, mid) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        result.push(lo as i32);
    }

    if x_lead_dims == 0 {
        return Ok(PrimResult::Array(BqnArr {
            shape: vec![],
            data: array::ArrData::I32(result),
            fill: Some(B::m_i32(0)),
        }));
    }
    let mut out = BqnArr::new_vec_i32(result);
    out.shape = x_lead_shape.to_vec();
    out.fill = Some(B::m_i32(0));
    Ok(PrimResult::Array(out))
}
