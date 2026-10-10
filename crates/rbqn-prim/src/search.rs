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
    if arr.rank() == 1
        && let Some(r) = fast_classify(arr) {
            return Ok(r);
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
            for (j, &cls) in class_map.iter().enumerate() {
                if rbqn_core::compare::deep_equal(v, arr.get(j)?) {
                    found_class = Some(cls);
                    break;
                }
            }
        } else {
            for (j, &cls) in class_map.iter().enumerate() {
                if cells_equal(arr, i, j, cell_size) {
                    found_class = Some(cls);
                    break;
                }
            }
        }
        let cls = found_class.unwrap_or_else(|| { let c = next_class; next_class += 1; c });
        class_map.push(cls);
        result.push(cls);
    }
    let mut out = BqnArr::new_vec_i32(result);
    out.fill = Some(B::m_i32(0));
    Ok(PrimResult::Array(out))
}

// ⊐ dyad: index of
// BQN semantics: cell shape = w.shape[1..]. Each "query" is a cell of x with x.shape[1..].
// w.shape[1..] must equal x.shape[x.rank - (w.rank-1)..] (trailing x shape = w cell shape).
// Result shape = x.shape[..(x.rank - (w.rank-1))] (leading x axes).
#[allow(non_snake_case)]
pub fn indexOf_c2(_w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⊐𝕩: 𝕨 must be an array".into()))?;

    if warr.rank() == 0 {
        return Err(BqnError::Rank("𝕨⊐𝕩: 𝕨 must have rank ≥ 1".into()));
    }

    // Trace: detect glyph lookups during compilation
    if std::env::var("RBQN_COMP_TRACE").is_ok() && x.is_c32() {
        let ch = char::from_u32(x.0 as u32).unwrap_or('?');
        eprintln!("[⊐ TRACE] atom char '{}' (u32={}) in array of {} elems", ch, x.0 as u32, warr.ia());
    }

    let w_rank = warr.rank() as usize;
    let w_cell_rank = w_rank - 1;
    let w_cell_shape = &warr.shape[1..];
    let w_cell_size: usize = w_cell_shape.iter().product::<usize>().max(1);
    let w_lead = warr.shape[0];
    if w_rank == 1 && !x.is_atom()
        && let Some(xarr) = xa
        && let Some(r) = fast_index_of(warr, xarr) {
            return Ok(r);
        }

    // Determine x's rank and shape
    let (x_rank, x_shape_owned) = if x.is_atom() {
        (0usize, vec![])
    } else if let Some(xarr) = xa {
        (xarr.rank() as usize, xarr.shape.clone())
    } else {
        (0usize, vec![])
    };

    // x.rank must be >= w_cell_rank
    if x_rank < w_cell_rank {
        return Err(BqnError::Rank(format!(
            "𝕨⊐𝕩: 𝕩 rank {} too low for 𝕨 cell rank {}", x_rank, w_cell_rank
        )));
    }

    // x's trailing shape must match w's cell shape
    if x_rank >= w_cell_rank {
        let x_tail = &x_shape_owned[x_rank - w_cell_rank..];
        if x_tail != w_cell_shape {
            return Err(BqnError::Rank(format!(
                "𝕨⊐𝕩: 𝕩 trailing shape {:?} doesn't match 𝕨 cell shape {:?}",
                x_tail, w_cell_shape
            )));
        }
    }

    // Result shape = x's leading axes (x.shape[..x.rank - w_cell_rank])
    let result_lead_dims = x_rank - w_cell_rank;
    let result_shape: Vec<usize> = x_shape_owned[..result_lead_dims].to_vec();
    let result_ia: usize = result_shape.iter().product::<usize>().max(if result_lead_dims == 0 { 1 } else { 0 });

    // For each query cell in x, search in w's major cells
    let mut result = Vec::with_capacity(result_ia);
    for i in 0..result_ia {
        let x_cell_offset = i * w_cell_size;
        let mut found = w_lead as i32;
        for j in 0..w_lead {
            let mut eq = true;
            for k in 0..w_cell_size {
                let wv = warr.get(j * w_cell_size + k)?;
                let xv = if let Some(xarr) = xa {
                    xarr.get(x_cell_offset + k)?
                } else {
                    // atom x: single query element
                    x
                };
                if !rbqn_core::compare::deep_equal(xv, wv) { eq = false; break; }
            }
            if eq { found = j as i32; break; }
        }
        result.push(found);
    }

    if result_shape.is_empty() {
        // rank-0 result
        return Ok(PrimResult::Array(BqnArr {
            shape: vec![],
            data: rbqn_core::array::ArrData::I32(result),
            fill: Some(B::m_i32(0)),
        }));
    }
    let mut out = BqnArr::new_vec_i32(result);
    out.shape = result_shape;
    out.fill = Some(B::m_i32(0));
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
    if arr.rank() == 1
        && let Some(r) = fast_occurrence(arr) {
            return Ok(r);
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
    let mut out = BqnArr::new_vec_i32(result);
    out.fill = Some(B::m_i32(0));
    Ok(PrimResult::Array(out))
}

// ⊒ dyad: progressive index of
// Same cell semantics as ⊐, but each matched w cell is "used up" (can only match once).
// Result shape = x's leading axes (same formula as ⊐).
pub fn count_c2(_w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⊒𝕩: 𝕨 must be an array".into()))?;

    if warr.rank() == 0 {
        return Err(BqnError::Rank("𝕨⊒𝕩: 𝕨 must have rank ≥ 1".into()));
    }

    let w_rank = warr.rank() as usize;
    let w_cell_rank = w_rank - 1;
    let w_cell_shape = &warr.shape[1..];
    let w_cell_size: usize = w_cell_shape.iter().product::<usize>().max(1);
    let w_lead = warr.shape[0];
    if w_rank == 1 && !x.is_atom()
        && let Some(xarr) = xa
        && let Some(r) = fast_progressive_index_of(warr, xarr) {
            return Ok(r);
        }

    // Determine x's rank and shape
    let (x_rank, x_shape_owned) = if x.is_atom() {
        (0usize, vec![])
    } else if let Some(xarr) = xa {
        (xarr.rank() as usize, xarr.shape.clone())
    } else {
        (0usize, vec![])
    };

    // x.rank must be >= w_cell_rank
    if x_rank < w_cell_rank {
        return Err(BqnError::Rank(format!(
            "𝕨⊒𝕩: 𝕩 rank {} too low for 𝕨 cell rank {}", x_rank, w_cell_rank
        )));
    }

    // x's trailing shape must match w's cell shape
    let x_tail = &x_shape_owned[x_rank - w_cell_rank..];
    if x_tail != w_cell_shape {
        return Err(BqnError::Rank(format!(
            "𝕨⊒𝕩: 𝕩 trailing shape {:?} doesn't match 𝕨 cell shape {:?}",
            x_tail, w_cell_shape
        )));
    }

    // Result shape = x's leading axes
    let result_lead_dims = x_rank - w_cell_rank;
    let result_shape: Vec<usize> = x_shape_owned[..result_lead_dims].to_vec();
    let result_ia: usize = result_shape.iter().product::<usize>().max(if result_lead_dims == 0 { 1 } else { 0 });

    // Progressive search: each w cell can only match once
    let mut used = vec![false; w_lead];
    let mut result = Vec::with_capacity(result_ia);
    for i in 0..result_ia {
        let x_cell_offset = i * w_cell_size;
        let mut found = w_lead as i32;
        for (j, used_j) in used.iter_mut().enumerate() {
            if *used_j { continue; }
            let mut eq = true;
            for k in 0..w_cell_size {
                let wv = warr.get(j * w_cell_size + k)?;
                let xv = if let Some(xarr) = xa {
                    xarr.get(x_cell_offset + k)?
                } else {
                    x
                };
                if !rbqn_core::compare::deep_equal(xv, wv) { eq = false; break; }
            }
            if eq { found = j as i32; *used_j = true; break; }
        }
        result.push(found);
    }

    if result_shape.is_empty() {
        return Ok(PrimResult::Array(BqnArr {
            shape: vec![],
            data: rbqn_core::array::ArrData::I32(result),
            fill: Some(B::m_i32(0)),
        }));
    }
    let mut out = BqnArr::new_vec_i32(result);
    out.shape = result_shape;
    out.fill = Some(B::m_i32(0));
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
    if arr.rank() == 1
        && let Some(r) = fast_mark_firsts(arr) {
            return Ok(r);
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
    let mut out = BqnArr::new_vec_i32(result);
    out.fill = Some(B::m_i32(0));
    Ok(PrimResult::Array(out))
}

// ∊ dyad: member of
// BQN semantics: cell shape = x.shape[1..]. Each "query" is a cell of w with w.shape[1..].
// x.shape[1..] must match w.shape[w.rank - (x.rank-1)..] (w's trailing shape = x's cell shape).
// Result shape = w's leading axes (all of w's shape except the matching trailing axes).
// NOTE: When x.rank==1, cell shape is [] (scalars), so we check elements of w against elements of x.
pub fn member_of_c2(_w: B, wa: Option<&BqnArr>, w_raw: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let xarr = xa.ok_or_else(|| BqnError::Type("𝕨∊𝕩: 𝕩 must be an array".into()))?;
    let x_rank = xarr.rank() as usize;

    // Cell shape of x (what each query must match)
    let x_cell_rank = if x_rank > 0 { x_rank - 1 } else { 0 };
    let x_cell_shape: &[usize] = if x_rank > 0 { &xarr.shape[1..] } else { &[] };
    let x_cell_size: usize = x_cell_shape.iter().product::<usize>().max(1);
    let x_lead = if x_rank > 0 { xarr.shape[0] } else { 1 };

    // Handle atom w: rank-0 query
    if w_raw.is_atom() {
        if x_cell_rank != 0 {
            return Err(BqnError::Rank(format!(
                "𝕨∊𝕩: atom 𝕨 can only be compared to rank-1 𝕩, but 𝕩 has rank {}", x_rank
            )));
        }
        // Search for w_raw in elements of x
        let mut found = false;
        for j in 0..x_lead {
            if rbqn_core::compare::deep_equal(w_raw, xarr.get(j * x_cell_size)?) { found = true; break; }
        }
        return Ok(PrimResult::Array(BqnArr {
            shape: vec![],
            data: rbqn_core::array::ArrData::I32(vec![found as i32]),
            fill: Some(B::m_i32(0)),
        }));
    }

    let warr = wa.ok_or_else(|| BqnError::Type("𝕨∊𝕩: 𝕨 must be an array".into()))?;
    let w_rank = warr.rank() as usize;
    if x_rank == 1
        && let Some(r) = fast_member_of(warr, xarr) {
            return Ok(r);
        }

    // w's trailing shape must match x's cell shape
    if w_rank < x_cell_rank {
        return Err(BqnError::Rank(format!(
            "𝕨∊𝕩: 𝕨 rank {} too low for 𝕩 cell rank {}", w_rank, x_cell_rank
        )));
    }
    let w_tail = &warr.shape[w_rank - x_cell_rank..];
    if w_tail != x_cell_shape {
        return Err(BqnError::Rank(format!(
            "𝕨∊𝕩: 𝕨 trailing shape {:?} doesn't match 𝕩 cell shape {:?}",
            w_tail, x_cell_shape
        )));
    }

    // Result shape = w's leading axes
    let result_lead_dims = w_rank - x_cell_rank;
    let result_shape: Vec<usize> = warr.shape[..result_lead_dims].to_vec();
    let result_ia: usize = result_shape.iter().product::<usize>().max(if result_lead_dims == 0 { 1 } else { 0 });

    let mut result = Vec::with_capacity(result_ia);
    for i in 0..result_ia {
        let w_cell_offset = i * x_cell_size;
        let mut found = false;
        for j in 0..x_lead {
            let mut eq = true;
            for k in 0..x_cell_size {
                let wv = warr.get(w_cell_offset + k)?;
                let xv = xarr.get(j * x_cell_size + k)?;
                if !rbqn_core::compare::deep_equal(wv, xv) { eq = false; break; }
            }
            if eq { found = true; break; }
        }
        result.push(found as i32);
    }

    if result_shape.is_empty() {
        return Ok(PrimResult::Array(BqnArr {
            shape: vec![],
            data: rbqn_core::array::ArrData::I32(result),
            fill: Some(B::m_i32(0)),
        }));
    }
    let mut out = BqnArr::new_vec_i32(result);
    out.shape = result_shape;
    out.fill = Some(B::m_i32(0));
    Ok(PrimResult::Array(out))
}

// ⍷ monad: deduplicate
pub fn deduplicate_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("⍷𝕩: 𝕩 must be an array".into()))?;
    if arr.rank() == 1
        && let Some(idx) = fast_first_indices(arr) {
            let mut result = Vec::with_capacity(idx.len());
            for i in idx { result.push(arr.get(i)?); }
            let len = result.len();
            return Ok(PrimResult::Array(typed_arr_from_b_vec(result, vec![len], arr.fill)));
        }
    if arr.rank() == 0 {
        return Err(BqnError::Rank("⍷𝕩: 𝕩 must have rank ≥ 1".into()));
    }
    if arr.rank() > 1 {
        // Keep the first occurrence of each major cell.
        let lead = arr.shape[0];
        let cell_size: usize = arr.shape[1..].iter().product();
        let mut result = Vec::new();
        let mut kept = 0;
        for i in 0..lead {
            if (0..i).any(|j| cells_equal(arr, i, j, cell_size)) { continue; }
            for k in 0..cell_size { result.push(arr.get(i * cell_size + k)?); }
            kept += 1;
        }
        let mut shape = arr.shape.clone();
        shape[0] = kept;
        return Ok(PrimResult::Array(typed_arr_from_b_vec(result, shape, arr.fill)));
    }
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
// w⍷x: result shape = (1 + ≢x) - ≢w (after rank-promoting w to x's rank).
// If w has lower rank than x, w's shape is left-padded with 1s to match x's rank.
// NOTE: Result shape per axis = (1 + x_dim) - w_dim, clamped to 0.
pub fn find_c2(w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let xarr = xa.ok_or_else(|| BqnError::Type("𝕨⍷𝕩: 𝕩 must be an array".into()))?;
    let x_rank = xarr.rank() as usize;

    // Build w_shape: atom or array, left-padded with 1s to match x_rank
    let (w_shape, w_data_shape) = if w.is_atom() {
        // atom w: treat as rank-x_rank with shape [1,1,...,1]
        let ws: Vec<usize> = vec![1; x_rank];
        (ws.clone(), ws)
    } else if let Some(warr) = wa {
        let wr = warr.rank() as usize;
        if wr > x_rank {
            return Err(BqnError::Rank(format!(
                "𝕨⍷𝕩: 𝕨 rank {} > 𝕩 rank {}", wr, x_rank
            )));
        }
        // Left-pad w's shape with 1s to match x's rank
        let mut padded = vec![1usize; x_rank - wr];
        padded.extend_from_slice(&warr.shape);
        (padded, warr.shape.clone())
    } else {
        (vec![1; x_rank], vec![1; x_rank])
    };

    let rank = x_rank;

    // Result shape: 1+x_dim - w_dim per axis (clamped to 0), using padded w_shape
    let result_shape: Vec<usize> = xarr.shape.iter().zip(w_shape.iter())
        .map(|(&xs, &ws)| {
            let d = 1isize + xs as isize - ws as isize;
            if d < 0 { 0 } else { d as usize }
        })
        .collect();
    let result_ia: usize = result_shape.iter().product();

    if result_ia == 0 {
        return Ok(PrimResult::Array(BqnArr { shape: result_shape, data: ArrData::I32(vec![]), fill: None }));
    }

    // w's actual data shape (before padding) and ia
    let wia: usize = w_data_shape.iter().product::<usize>().max(if w.is_atom() { 1 } else { 0 });

    // Compute x strides
    let x_shape = &xarr.shape;
    let mut x_strides = vec![1usize; rank];
    for r in (0..rank - 1).rev() {
        x_strides[r] = x_strides[r + 1] * x_shape[r + 1];
    }

    // For rank-1 (no padding needed): simple sliding window
    if rank <= 1 && w_shape == w_data_shape {
        let mut result = Vec::with_capacity(result_ia);
        for i in 0..result_ia {
            let mut matches = true;
            for j in 0..wia {
                let wv = if w.is_atom() { w } else if let Some(warr) = wa { warr.get(j)? } else { w };
                if !rbqn_core::compare::deep_equal(wv, xarr.get(i + j)?) {
                    matches = false;
                    break;
                }
            }
            result.push(matches as i32);
        }
        return Ok(PrimResult::Array(BqnArr { shape: result_shape, data: ArrData::I32(result), fill: None }));
    }

    // Multi-dimensional sliding window match (handles rank promotion via padded w_shape)
    // w_shape is the padded shape (for iteration), w_data_shape is the actual data shape
    let mut result = Vec::with_capacity(result_ia);
    for flat_pos in 0..result_ia {
        // Convert flat result position to multi-dim index
        let mut rem = flat_pos;
        let mut result_idx = vec![0usize; rank];
        for r in (0..rank).rev() {
            result_idx[r] = rem % result_shape[r];
            rem /= result_shape[r];
        }
        // Check if w matches x starting at result_idx
        let mut matches = true;
        // Iterate over w's padded shape (total wia_padded elements in padded window)
        let wia_padded: usize = w_shape.iter().product::<usize>().max(1);
        'outer: for w_flat in 0..wia_padded {
            // Convert w_flat to multi-dim index in padded w_shape
            let mut w_idx = vec![0usize; rank];
            let mut w_rem = w_flat;
            for r in (0..rank).rev() {
                w_idx[r] = w_rem % w_shape[r];
                w_rem /= w_shape[r];
            }
            // Map w_idx to actual w data index (offset by padding: leading 1s are always index 0)
            let padding = rank - w_data_shape.len();
            let mut w_data_flat = 0usize;
            if !w_data_shape.is_empty() {
                for r in padding..rank {
                    let stride: usize = w_data_shape[r-padding+1..].iter().product::<usize>().max(1);
                    w_data_flat += w_idx[r] * stride;
                }
            }
            // Get w value: atom or from warr
            let wv = if w.is_atom() {
                w
            } else if let Some(warr) = wa {
                if w_data_flat < warr.ia() { warr.get(w_data_flat)? } else { continue }
            } else {
                continue
            };
            // Compute x position
            let mut x_flat = 0;
            for r in 0..rank {
                x_flat += (result_idx[r] + w_idx[r]) * x_strides[r];
            }
            if !rbqn_core::compare::deep_equal(wv, xarr.get(x_flat)?) {
                matches = false;
                break 'outer;
            }
        }
        result.push(matches as i32);
    }

    Ok(PrimResult::Array(BqnArr { shape: result_shape, data: ArrData::I32(result), fill: None }))
}

// ---------------------------------------------------------------------------
// Fast paths: intern atom elements to dense ids, then search with flat tables.
// Matches deep_equal on atoms: numbers compare by f64 value (so 0 = ¯0 and NaN
// never matches), characters by code point, and numbers never equal characters.
// Arrays holding anything else (nested arrays, functions) return None and use
// the generic deep_equal path.
// ---------------------------------------------------------------------------

/// Id for an element that can never match anything (NaN).
const NO_ID: u32 = u32::MAX;

/// Borrowed view of a small-int array. Bit arrays are widened to i8 (1 byte/elem).
enum IntView<'a> {
    I8(std::borrow::Cow<'a, [i8]>),
    I16(&'a [i16]),
    I32(&'a [i32]),
}

fn int_view(a: &BqnArr) -> Option<IntView<'_>> {
    match &a.data {
        ArrData::Bit(_) => Some(IntView::I8(std::borrow::Cow::Owned(
            a.i32_iter().ok()?.into_iter().map(|x| x as i8).collect()))),
        ArrData::I8(v) => Some(IntView::I8(std::borrow::Cow::Borrowed(v))),
        ArrData::I16(v) => Some(IntView::I16(v)),
        ArrData::I32(v) => Some(IntView::I32(v)),
        _ => None,
    }
}

/// Run a generic body over the narrow slice type of an IntView.
macro_rules! with_slice {
    ($view:expr, $v:ident => $body:expr) => {
        match $view {
            IntView::I8($v) => { let $v: &[i8] = &$v; $body }
            IntView::I16($v) => { let $v: &[i16] = $v; $body }
            IntView::I32($v) => { let $v: &[i32] = $v; $body }
        }
    };
}

fn min_max<T: Copy + Into<i32>>(v: &[T]) -> (i32, i32) {
    let mut lo = i32::MAX;
    let mut hi = i32::MIN;
    for &x in v { let x: i32 = x.into(); lo = lo.min(x); hi = hi.max(x); }
    (lo, hi)
}

fn dense_ids<T: Copy + Into<i32>>(v: &[T], lo: i32) -> Vec<u32> {
    v.iter().map(|&x| (x.into() as i64 - lo as i64) as u32).collect()
}

fn dense_classify<T: Copy + Into<i32>>(v: &[T], lo: i32, range: usize) -> Vec<i32> {
    let mut cls = vec![-1i32; range];
    let mut next = 0i32;
    let mut out = Vec::with_capacity(v.len());
    for &x in v {
        let c = &mut cls[(x.into() as i64 - lo as i64) as usize];
        if *c < 0 { *c = next; next += 1; }
        out.push(*c);
    }
    out
}

#[inline]
fn num_key(f: f64) -> Option<u128> {
    if f.is_nan() { return None; }
    let f = if f == 0.0 { 0.0 } else { f };
    Some(f.to_bits() as u128)
}

#[inline]
fn chr_key(c: u32) -> Option<u128> {
    Some((1u128 << 64) | c as u128)
}

/// Per-element keys, or None if some element is not a number or character.
fn atom_keys(a: &BqnArr) -> Option<Vec<Option<u128>>> {
    Some(match &a.data {
        ArrData::Bit(_) | ArrData::I8(_) | ArrData::I16(_) | ArrData::I32(_) =>
            a.i32_iter().ok()?.into_iter().map(|x| num_key(x as f64)).collect(),
        ArrData::F64(v) => v.iter().map(|&x| num_key(x)).collect(),
        ArrData::C8(v) => v.iter().map(|&c| chr_key(c as u32)).collect(),
        ArrData::C16(v) => v.iter().map(|&c| chr_key(c as u32)).collect(),
        ArrData::C32(v) => v.iter().map(|&c| chr_key(c)).collect(),
        ArrData::Boxed(v) => {
            let mut out = Vec::with_capacity(v.len());
            for &b in v {
                if b.is_f64() { out.push(num_key(b.o2f())); }
                else if b.is_c32() { out.push(chr_key(b.0 as u32)); }
                else { return None; }
            }
            out
        }
    })
}

/// Map the elements of every array to ids in 0..k (NO_ID for NaN), with equal
/// elements getting equal ids across all arrays. Returns (ids per array, k).
fn intern(arrs: &[&BqnArr]) -> Option<(Vec<Vec<u32>>, usize)> {
    // Dense path: all small ints in a compact range, id = value - min.
    let views: Vec<Option<IntView>> = arrs.iter().map(|a| int_view(a)).collect();
    if views.iter().all(|v| v.is_some()) {
        let views: Vec<IntView> = views.into_iter().map(|v| v.unwrap()).collect();
        let total: usize = arrs.iter().map(|a| a.ia()).sum();
        if total == 0 { return Some((views.iter().map(|_| vec![]).collect(), 0)); }
        let (mut lo, mut hi) = (i32::MAX, i32::MIN);
        for v in &views {
            let (l, h) = with_slice!(v, s => min_max(s));
            lo = lo.min(l); hi = hi.max(h);
        }
        let range = (hi as i64 - lo as i64 + 1) as usize;
        if range <= (4 * total).max(1 << 16) {
            let ids = views.iter().map(|v| with_slice!(v, s => dense_ids(s, lo))).collect();
            return Some((ids, range));
        }
    }
    // Hash path.
    let mut map: std::collections::HashMap<u128, u32, std::hash::BuildHasherDefault<IdHasher>> =
        Default::default();
    let mut out = Vec::with_capacity(arrs.len());
    for a in arrs {
        let keys = atom_keys(a)?;
        let ids = keys.into_iter().map(|k| match k {
            None => NO_ID,
            Some(k) => { let n = map.len() as u32; *map.entry(k).or_insert(n) }
        }).collect();
        out.push(ids);
    }
    let k = map.len();
    Some((out, k))
}

fn i32_result(v: Vec<i32>, shape: Vec<usize>) -> PrimResult {
    PrimResult::Array(BqnArr { shape, data: ArrData::I32(v), fill: Some(B::m_i32(0)) })
}

fn dense_occurrence<T: Copy + Into<i32>>(v: &[T], lo: i32, range: usize) -> Vec<i32> {
    let mut cnt = vec![0i32; range];
    let mut out = Vec::with_capacity(v.len());
    for &x in v {
        let c = &mut cnt[(x.into() as i64 - lo as i64) as usize];
        out.push(*c);
        *c += 1;
    }
    out
}

fn dense_mark_firsts<T: Copy + Into<i32>>(v: &[T], lo: i32, range: usize) -> Vec<i32> {
    let mut seen = vec![false; range];
    let mut out = Vec::with_capacity(v.len());
    for &x in v {
        let sn = &mut seen[(x.into() as i64 - lo as i64) as usize];
        out.push(!*sn as i32);
        *sn = true;
    }
    out
}

fn dense_first_indices<T: Copy + Into<i32>>(v: &[T], lo: i32, range: usize) -> Vec<usize> {
    let mut seen = vec![false; range];
    let mut out = Vec::new();
    for (i, &x) in v.iter().enumerate() {
        let sn = &mut seen[(x.into() as i64 - lo as i64) as usize];
        if !*sn { *sn = true; out.push(i); }
    }
    out
}

/// Min/max + range check shared by the single-pass dense paths.
fn dense_params(view: &IntView, n: usize) -> Option<(i32, usize)> {
    let (lo, hi) = with_slice!(view, s => min_max(s));
    let range = (hi as i64 - lo as i64 + 1) as usize;
    if range <= (4 * n).max(1 << 16) { Some((lo, range)) } else { None }
}

/// ⊐𝕩 on a list of atoms.
fn fast_classify(arr: &BqnArr) -> Option<PrimResult> {
    if let Some(view) = int_view(arr) {
        let n = arr.ia();
        if n == 0 { return Some(i32_result(vec![], vec![0])); }
        let (lo, hi) = with_slice!(&view, s => min_max(s));
        let range = (hi as i64 - lo as i64 + 1) as usize;
        if range <= (4 * n).max(1 << 16) {
            let r = with_slice!(&view, s => dense_classify(s, lo, range));
            return Some(i32_result(r, vec![n]));
        }
    }
    let (ids, k) = intern(&[arr])?;
    let mut cls = vec![-1i32; k];
    let mut next = 0i32;
    let r = ids[0].iter().map(|&id| {
        if id == NO_ID { next += 1; return next - 1; }
        let c = &mut cls[id as usize];
        if *c < 0 { *c = next; next += 1; }
        *c
    }).collect();
    Some(i32_result(r, vec![arr.ia()]))
}

/// ⊒𝕩 on a list of atoms.
fn fast_occurrence(arr: &BqnArr) -> Option<PrimResult> {
    if let Some(view) = int_view(arr) {
        let n = arr.ia();
        if n == 0 { return Some(i32_result(vec![], vec![0])); }
        if let Some((lo, range)) = dense_params(&view, n) {
            let r = with_slice!(&view, s => dense_occurrence(s, lo, range));
            return Some(i32_result(r, vec![n]));
        }
    }
    let (ids, k) = intern(&[arr])?;
    let mut cnt = vec![0i32; k];
    let r = ids[0].iter().map(|&id| {
        if id == NO_ID { return 0; }
        let c = &mut cnt[id as usize];
        *c += 1;
        *c - 1
    }).collect();
    Some(i32_result(r, vec![arr.ia()]))
}

/// ∊𝕩 on a list of atoms.
fn fast_mark_firsts(arr: &BqnArr) -> Option<PrimResult> {
    if let Some(view) = int_view(arr) {
        let n = arr.ia();
        if n == 0 { return Some(i32_result(vec![], vec![0])); }
        if let Some((lo, range)) = dense_params(&view, n) {
            let r = with_slice!(&view, s => dense_mark_firsts(s, lo, range));
            return Some(i32_result(r, vec![n]));
        }
    }
    let (ids, k) = intern(&[arr])?;
    let mut seen = vec![false; k];
    let r = ids[0].iter().map(|&id| {
        if id == NO_ID { return 1; }
        let s = &mut seen[id as usize];
        let first = !*s;
        *s = true;
        first as i32
    }).collect();
    Some(i32_result(r, vec![arr.ia()]))
}

/// Indices of first occurrences, for ⍷𝕩.
fn fast_first_indices(arr: &BqnArr) -> Option<Vec<usize>> {
    if let Some(view) = int_view(arr) {
        let n = arr.ia();
        if n == 0 { return Some(vec![]); }
        if let Some((lo, range)) = dense_params(&view, n) {
            return Some(with_slice!(&view, s => dense_first_indices(s, lo, range)));
        }
    }
    let (ids, k) = intern(&[arr])?;
    let mut seen = vec![false; k];
    Some(ids[0].iter().enumerate().filter_map(|(i, &id)| {
        if id == NO_ID { return Some(i); }
        let s = &mut seen[id as usize];
        if *s { None } else { *s = true; Some(i) }
    }).collect())
}

/// 𝕨⊐𝕩 with 𝕨 a list of atoms and 𝕩 an array of atoms.
fn fast_index_of(warr: &BqnArr, xarr: &BqnArr) -> Option<PrimResult> {
    let (ids, k) = intern(&[warr, xarr])?;
    let n = warr.ia() as i32;
    let mut pos = vec![n; k];
    for (i, &id) in ids[0].iter().enumerate().rev() {
        if id != NO_ID { pos[id as usize] = i as i32; }
    }
    let r = ids[1].iter().map(|&id| if id == NO_ID { n } else { pos[id as usize] }).collect();
    Some(i32_result(r, xarr.shape.clone()))
}

/// 𝕨⊒𝕩 with 𝕨 a list of atoms and 𝕩 an array of atoms.
fn fast_progressive_index_of(warr: &BqnArr, xarr: &BqnArr) -> Option<PrimResult> {
    let (ids, k) = intern(&[warr, xarr])?;
    let n = warr.ia() as i32;
    // Bucket w's indices by id (counting sort), then hand them out in order.
    let mut start = vec![0usize; k + 1];
    for &id in &ids[0] { if id != NO_ID { start[id as usize + 1] += 1; } }
    for i in 0..k { start[i + 1] += start[i]; }
    let mut fill = start.clone();
    let mut slots = vec![0i32; start[k]];
    for (i, &id) in ids[0].iter().enumerate() {
        if id != NO_ID { slots[fill[id as usize]] = i as i32; fill[id as usize] += 1; }
    }
    let mut next = start.clone();
    let r = ids[1].iter().map(|&id| {
        if id == NO_ID { return n; }
        let j = id as usize;
        if next[j] < start[j + 1] { next[j] += 1; slots[next[j] - 1] } else { n }
    }).collect();
    Some(i32_result(r, xarr.shape.clone()))
}

/// 𝕨∊𝕩 with 𝕩 a list of atoms and 𝕨 an array of atoms.
fn fast_member_of(warr: &BqnArr, xarr: &BqnArr) -> Option<PrimResult> {
    let (ids, k) = intern(&[warr, xarr])?;
    let mut present = vec![false; k];
    for &id in &ids[1] { if id != NO_ID { present[id as usize] = true; } }
    let r = ids[0].iter().map(|&id| (id != NO_ID && present[id as usize]) as i32).collect();
    Some(i32_result(r, warr.shape.clone()))
}
