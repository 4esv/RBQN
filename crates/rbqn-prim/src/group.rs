use rbqn_core::*;
use crate::dispatch::PrimResult;

// ⊔ monad: group indices
// If 𝕩 is a rank-1 integer array: groups ↕≠𝕩 by values in 𝕩.
// If 𝕩 is a boxed array (list of integer arrays): multi-dimensional group.
// NOTE: 𝕩 must be a rank-1 integer array OR a rank-1 boxed array of integer arrays.
pub fn group_indices_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Err(BqnError::Type("⊔𝕩: 𝕩 must be an integer array".into()));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("⊔𝕩: 𝕩 must be an array".into()))?;

    // NOTE: Multi-dim group: 𝕩 is a boxed array where each element is an integer array or scalar.
    // Dispatch when 𝕩 has boxed element type.
    if arr.el_type() == ElType::B && arr.rank() == 1 {
        return group_indices_multidim(arr);
    }

    if arr.rank() != 1 {
        return Err(BqnError::Rank(format!(
            "⊔𝕩: 𝕩 must be rank-1, got rank {}",
            arr.rank()
        )));
    }
    let indices = arr.i32_iter().map_err(|_| BqnError::Type("⊔𝕩: 𝕩 must be an integer array".into()))?;
    // NOTE: BQN spec: ⊔ requires natural numbers (¯1 is allowed to exclude)
    // Values < ¯1 are domain errors
    for &g in &indices {
        if g < -1 {
            return Err(BqnError::Domain(format!(
                "⊔𝕩: 𝕩 must be ¯1 or non-negative integers, got {}",
                g
            )));
        }
    }

    let max_idx = indices.iter().copied().max().unwrap_or(-1);
    let n = (max_idx + 1).max(0) as usize;

    let mut groups: Vec<Vec<i32>> = vec![vec![]; n];
    for (i, &g) in indices.iter().enumerate() {
        if g >= 0 {
            groups[g as usize].push(i as i32);
        }
    }

    let result: Vec<B> = groups
        .into_iter()
        .map(|g| tag_arr(BqnArr::new_vec_i32(g)))
        .collect();
    Ok(PrimResult::Array(BqnArr::new_vec_b(result)))
}

// Multi-dimensional group: 𝕩 is a list of integer arrays or scalars ⟨l₁,l₂,...,lₙ⟩.
//
// Per CBQN behavior:
//
// When element k is a scalar n:
//   - It specifies 1 "position" (position 0) which goes to group n.
//   - result_shape[k] = n+1.
//   - count_k(g) = 1 if g==n, else 0 (contributes to cell shape).
//   - The scalar position is NOT included in the index tuple.
//
// When element k is an array lₖ:
//   - result_shape[k] = 1+max(lₖ)
//   - matches_k[g] = {i | lₖ[i] = g}
//   - count_k(g) = |matches_k[g]|
//   - positions ARE included in the index tuple.
//
// Cell at (g₁,...,gₙ): outer product over ALL dims.
//   - Cell shape = ⟨count₁(g₁), ..., countₙ(gₙ)⟩ (ALL dims)
//   - Each element of the cell is a tuple of positions from ARRAY dims only.
fn group_indices_multidim(arr: &BqnArr) -> Result<PrimResult> {
    let ndim = arr.ia();

    // Empty list: ⊔⟨⟩ → empty result
    if ndim == 0 {
        return Ok(PrimResult::Array(BqnArr::new_vec_b(vec![])));
    }

    // Per-dimension specification.
    // Each element of the input can be:
    //   - a scalar n: contributes 0 axes to the tuple, 1 result axis
    //   - an array of any rank r: contributes r axes to the tuple, 1 result axis
    //     The array is raveled to get flat group indices.
    //     When building tuples, flat positions are expanded back to multi-dim indices.
    enum DimSpec {
        Scalar {
            target_group: usize,
        },
        Array {
            matches: Vec<Vec<usize>>, // matches[g] = flat positions with index g
            elem_shape: Vec<usize>,   // original shape of the element (for expanding flat→multi-dim)
        },
    }

    let mut dims: Vec<DimSpec> = Vec::with_capacity(ndim);
    let mut result_shape: Vec<usize> = Vec::with_capacity(ndim);

    for k in 0..ndim {
        let elem = arr.get(k)?;
        if elem.is_f64() {
            let n = elem.to_i32().map_err(|_| BqnError::Type(format!(
                "⊔𝕩: element {} must be an integer", k
            )))?;
            if n < 0 {
                return Err(BqnError::Domain(format!(
                    "⊔𝕩: scalar element must be ≥ 0, got {}", n
                )));
            }
            let target = n as usize;
            let size = target + 1;
            result_shape.push(size);
            dims.push(DimSpec::Scalar { target_group: target });
        } else {
            let elem_arr = get_arr(elem)
                .ok_or_else(|| BqnError::Type(format!("⊔𝕩: element {} must be an integer array", k)))?;
            let indices = elem_arr.i32_iter()
                .map_err(|_| BqnError::Type(format!("⊔𝕩: element {} must be an integer array", k)))?;
            for &v in &indices {
                if v < -1 {
                    return Err(BqnError::Domain(format!(
                        "⊔𝕩: values must be ≥¯1, got {}", v
                    )));
                }
            }
            let max_idx = indices.iter().copied().max().unwrap_or(-1);
            let result_size = (max_idx + 1).max(0) as usize;
            result_shape.push(result_size);

            let mut matches: Vec<Vec<usize>> = vec![vec![]; result_size];
            for (i, &g) in indices.iter().enumerate() {
                if g >= 0 && (g as usize) < result_size {
                    matches[g as usize].push(i);
                }
            }
            dims.push(DimSpec::Array { matches, elem_shape: elem_arr.shape.clone() });
        }
    }

    // Helper: convert flat position to multi-dimensional index tuple
    fn flat_to_multi(flat: usize, shape: &[usize]) -> Vec<i32> {
        if shape.is_empty() {
            return vec![];
        }
        let mut result = vec![0i32; shape.len()];
        let mut rem = flat;
        for k in (0..shape.len()).rev() {
            result[k] = (rem % shape[k]) as i32;
            rem /= shape[k];
        }
        result
    }

    // Compute result strides for flat cell index
    let result_strides: Vec<usize> = {
        let mut s = vec![1usize; ndim];
        for k in (0..ndim.saturating_sub(1)).rev() {
            s[k] = s[k + 1] * result_shape[k + 1];
        }
        s
    };

    // Compute tuple length: sum of ranks of array elements
    let tuple_len: usize = dims.iter().map(|d| match d {
        DimSpec::Scalar { .. } => 0,
        DimSpec::Array { elem_shape, .. } => elem_shape.len(),
    }).sum();

    let total_cells: usize = result_shape.iter().product();
    let mut result_cells: Vec<B> = Vec::with_capacity(total_cells);

    for cell_flat in 0..total_cells {
        // Decode cell multi-index (g₁,...,gₙ)
        let mut group_indices: Vec<usize> = Vec::with_capacity(ndim);
        let mut remaining = cell_flat;
        for &stride in &result_strides {
            let g = remaining / stride;
            remaining %= stride;
            group_indices.push(g);
        }

        // For each dim, compute the count (for cell shape) and matches
        let mut cell_counts: Vec<usize> = Vec::with_capacity(ndim);
        let mut array_matches_for_cell: Vec<(Vec<usize>, Vec<usize>)> = Vec::new(); // (flat_positions, elem_shape)

        for k in 0..ndim {
            let g = group_indices[k];
            match &dims[k] {
                DimSpec::Scalar { target_group, .. } => {
                    cell_counts.push(if g == *target_group { 1 } else { 0 });
                }
                DimSpec::Array { matches, elem_shape, .. } => {
                    let m = &matches[g];
                    cell_counts.push(m.len());
                    array_matches_for_cell.push((m.clone(), elem_shape.clone()));
                }
            }
        }

        let cell_total: usize = cell_counts.iter().product();

        // Cell strides
        let cell_strides: Vec<usize> = {
            let mut s = vec![1usize; ndim];
            for k in (0..ndim.saturating_sub(1)).rev() {
                s[k] = s[k + 1] * cell_counts[k + 1];
            }
            s
        };

        let mut cell_elems: Vec<B> = Vec::with_capacity(cell_total);

        for elem_flat in 0..cell_total {
            let mut r_per_dim: Vec<usize> = Vec::with_capacity(ndim);
            let mut rem = elem_flat;
            for (k, &cs) in cell_strides.iter().enumerate() {
                let stride = if k < ndim - 1 { cs } else { 1 };
                let r = if stride > 0 { rem / stride } else { 0 };
                rem %= stride.max(1);
                r_per_dim.push(r);
            }

            // Build tuple: for each array dim, expand flat position to multi-dim index
            let mut pos_tuple: Vec<i32> = Vec::with_capacity(tuple_len);
            let mut arr_dim_idx = 0usize;
            for k in 0..ndim {
                match &dims[k] {
                    DimSpec::Scalar { .. } => {} // skip
                    DimSpec::Array { .. } => {
                        let r = r_per_dim[k];
                        let (ref flat_positions, ref elem_shape) = array_matches_for_cell[arr_dim_idx];
                        let flat_pos = flat_positions[r];
                        // Expand flat position back to multi-dimensional index
                        let multi_idx = flat_to_multi(flat_pos, elem_shape);
                        pos_tuple.extend_from_slice(&multi_idx);
                        arr_dim_idx += 1;
                    }
                }
            }
            cell_elems.push(tag_arr(BqnArr::new_vec_i32(pos_tuple)));
        }

        let cell_arr = {
            let mut a = BqnArr::new_vec_b(cell_elems);
            a.shape = cell_counts;
            tag_arr(a)
        };
        result_cells.push(cell_arr);
    }

    let mut out = BqnArr::new_vec_b(result_cells);
    out.shape = result_shape;
    Ok(PrimResult::Array(out))
}

// ⊔ dyad: group
// 𝕨⊔𝕩: groups elements of 𝕩 by index list 𝕨.
//
// Cases:
//   - scalar 𝕨 = n: all elements of 𝕩 go to group n; result has n+1 groups
//   - rank-1 𝕨: groups major cells of 𝕩 by 𝕨
//     (≠𝕨 may be ≠𝕩 or 1+≠𝕩; last element in the latter case is min result length)
//   - rank-k 𝕨 (k>1): 𝕨 must have shape prefix of 𝕩; group by ⥊𝕨
//   - boxed list 𝕨 (depth 2): multi-axis group
pub fn group_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    // Case: scalar w = n (not an array)
    if w.is_f64() {
        let n = w.to_i32().map_err(|_| BqnError::Domain("𝕨⊔𝕩: scalar 𝕨 must be an integer".into()))?;
        if n < 0 {
            return Err(BqnError::Domain(format!("𝕨⊔𝕩: scalar 𝕨 must be ≥ 0, got {}", n)));
        }
        return group_scalar_w(n as usize, x, xa);
    }

    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⊔𝕩: 𝕨 must be an array".into()))?;

    // Case: boxed list w (depth 2) → multi-axis group
    if warr.rank() == 1 && warr.el_type() == ElType::B {
        return group_multi_axis(warr, x, xa);
    }

    // Case: rank>1 simple w → ravel leading dims
    if warr.rank() > 1 {
        return group_high_rank_w(warr, x, xa);
    }

    // Case: rank-1 simple w
    group_rank1_w(warr, x, xa)
}

/// Scalar w=n: the entire array x is placed as one unit into group n.
/// Groups 0..n-1 are empty with shape ⟨0, ≢x...⟩.
/// Group n has shape ⟨1, ≢x...⟩ containing x.
/// This matches CBQN: `n⊔x` treats x as a single "major cell".
fn group_scalar_w(n: usize, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    // x_shape = shape of x (empty vec for scalar x)
    let (x_shape, x_fill, xia): (Vec<usize>, Option<B>, usize) = if let Some(a) = xa {
        (a.shape.clone(), a.fill, a.ia())
    } else {
        // Scalar x: shape is empty, treat as rank-0
        (vec![], None, 1)
    };

    let n_groups = n + 1;
    let mut result: Vec<B> = Vec::with_capacity(n_groups);

    // Empty groups 0..n-1: shape = ⟨0, x_shape...⟩
    let empty_shape = {
        let mut s = vec![0usize];
        s.extend_from_slice(&x_shape);
        s
    };
    for _ in 0..n {
        let empty = array::typed_arr_from_b_vec(vec![], empty_shape.clone(), x_fill);
        result.push(tag_arr(empty));
    }

    // Group n: shape = ⟨1, x_shape...⟩, containing all elements of x
    let group_n_shape = {
        let mut s = vec![1usize];
        s.extend_from_slice(&x_shape);
        s
    };
    let group_n = if let Some(arr) = xa {
        let mut elems: Vec<B> = Vec::with_capacity(xia);
        for i in 0..xia {
            elems.push(arr.get(i)?);
        }
        array::typed_arr_from_b_vec(elems, group_n_shape, x_fill)
    } else {
        // Scalar x: 1-element array containing x
        array::typed_arr_from_b_vec(vec![x], group_n_shape, x_fill)
    };
    result.push(tag_arr(group_n));

    let group_fill_arr = array::typed_arr_from_b_vec(vec![], empty_shape, x_fill);
    let group_fill = rbqn_core::tag_arr(group_fill_arr);
    let mut out = BqnArr::new_vec_b(result);
    out.fill = Some(group_fill);
    Ok(PrimResult::Array(out))
}

/// Rank-1 w: standard dyadic group of major cells.
fn group_rank1_w(warr: &BqnArr, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let xarr = if let Some(a) = xa {
        a
    } else {
        return Err(BqnError::Type("𝕨⊔𝕩: 𝕩 must be an array when 𝕨 is rank-1".into()));
    };

    let all_indices = warr.i32_iter()?;
    let x_n_major = xarr.shape.first().copied().unwrap_or(1); // number of major cells

    // trailing shape = shape[1..]
    let trailing_shape = if xarr.rank() > 1 { xarr.shape[1..].to_vec() } else { vec![] };
    let cell_size: usize = trailing_shape.iter().product::<usize>().max(1);

    // Determine indices for grouping and minimum result length
    let (indices, min_len) = if all_indices.len() == x_n_major + 1 {
        let last = *all_indices.last().unwrap();
        let min = last.max(0) as usize;
        (&all_indices[..x_n_major], min)
    } else if all_indices.len() == x_n_major {
        (&all_indices[..], 0usize)
    } else {
        return Err(BqnError::Shape(format!(
            "𝕨⊔𝕩: ≠𝕨 must be ≠𝕩 or 1+≠𝕩 ({} vs {})",
            all_indices.len(),
            x_n_major
        )));
    };

    for &g in indices {
        if g < -1 {
            return Err(BqnError::Domain(format!(
                "𝕨⊔𝕩: indices must be ≥¯1, got {}", g
            )));
        }
    }

    let max_idx = indices.iter().copied().max().unwrap_or(-1);
    let n = ((max_idx + 1).max(0) as usize).max(min_len);

    let mut groups: Vec<Vec<B>> = vec![vec![]; n];
    for (i, &g) in indices.iter().enumerate() {
        if g >= 0 {
            let g_u = g as usize;
            // Extract the major cell at position i
            let base = i * cell_size;
            for j in 0..cell_size {
                groups[g_u].push(xarr.get(base + j)?);
            }
        }
    }

    let empty_cell_shape = {
        let mut s = vec![0usize];
        s.extend_from_slice(&trailing_shape);
        s
    };

    let result: Vec<B> = groups
        .into_iter()
        .map(|g| {
            let n_cells = g.len() / cell_size.max(1);
            let mut shape = vec![n_cells];
            shape.extend_from_slice(&trailing_shape);
            tag_arr(array::typed_arr_from_b_vec(g, shape, xarr.fill))
        })
        .collect();

    let group_fill_arr = array::typed_arr_from_b_vec(vec![], empty_cell_shape, xarr.fill);
    let group_fill = rbqn_core::tag_arr(group_fill_arr);
    let mut out = BqnArr::new_vec_b(result);
    out.fill = Some(group_fill);
    Ok(PrimResult::Array(out))
}

/// Rank-k (k>1) simple w: w's shape must be a prefix of x's shape.
/// Treat as ⥊w ⊔ (x reshaped to flatten leading k dims).
fn group_high_rank_w(warr: &BqnArr, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let xarr = xa.ok_or_else(|| BqnError::Type("𝕨⊔𝕩: 𝕩 must be an array".into()))?;
    let w_rank = warr.rank() as usize;
    let x_rank = xarr.rank() as usize;

    if w_rank > x_rank {
        return Err(BqnError::Rank(format!(
            "𝕨⊔𝕩: Rank of simple 𝕨 must be at most rank of 𝕩 (𝕨 rank {}, 𝕩 rank {})",
            w_rank, x_rank
        )));
    }

    // Check that warr.shape is a prefix of xarr.shape
    for k in 0..w_rank {
        if warr.shape[k] != xarr.shape[k] {
            return Err(BqnError::Shape(format!(
                "𝕨⊔𝕩: Expected 𝕨's shape to be a prefix of 𝕩's ({:?} ≡ ≢𝕨, {:?} ≡ ≢𝕩)",
                warr.shape, xarr.shape
            )));
        }
    }

    // NOTE: Create a flattened rank-1 view of w (as indices)
    // and treat x with its leading w_rank dims flattened.
    let flat_indices = warr.i32_iter()?;
    let _leading_count: usize = warr.shape.iter().product();
    let trailing_shape = xarr.shape[w_rank..].to_vec();
    let cell_size: usize = trailing_shape.iter().product::<usize>().max(1);

    for &g in &flat_indices {
        if g < -1 {
            return Err(BqnError::Domain(format!("𝕨⊔𝕩: indices must be ≥¯1, got {}", g)));
        }
    }

    let max_idx = flat_indices.iter().copied().max().unwrap_or(-1);
    let n = (max_idx + 1).max(0) as usize;

    let mut groups: Vec<Vec<B>> = vec![vec![]; n];
    for (i, &g) in flat_indices.iter().enumerate() {
        if g >= 0 {
            let g_u = g as usize;
            if g_u < n {
                let base = i * cell_size;
                for j in 0..cell_size {
                    groups[g_u].push(xarr.get(base + j)?);
                }
            }
        }
    }

    let empty_cell_shape = {
        let mut s = vec![0usize];
        s.extend_from_slice(&trailing_shape);
        s
    };

    let result: Vec<B> = groups
        .into_iter()
        .map(|g| {
            let n_cells = g.len() / cell_size.max(1);
            let mut shape = vec![n_cells];
            shape.extend_from_slice(&trailing_shape);
            tag_arr(array::typed_arr_from_b_vec(g, shape, xarr.fill))
        })
        .collect();

    let group_fill_arr = array::typed_arr_from_b_vec(vec![], empty_cell_shape, xarr.fill);
    let group_fill = rbqn_core::tag_arr(group_fill_arr);
    let mut out = BqnArr::new_vec_b(result);
    out.fill = Some(group_fill);
    Ok(PrimResult::Array(out))
}

/// Multi-axis group: w is a boxed list where each element indexes one axis of x.
/// Each element wₖ can be:
///   - a scalar n: axis k groups all positions into group n (result size = n+1)
///   - an integer array: groups axis k positions by value (result size = max+1)
///   - an integer array with +1 length: last element is min result size for that axis
///
/// The cell at (g₁,...,gₙ) is a submatrix of x:
///   shape = ⟨count₁,...,countₙ, trailing_shape...⟩
///   where countₖ = number of positions in axis k matching group gₖ
///
/// For scalar wₖ = n: position j matches group n for all j in axis k.
fn group_multi_axis(warr: &BqnArr, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let n_dims = warr.ia();
    if n_dims == 0 {
        // BQN: ⟨⟩⊔x returns ⟨⟩ (empty result with fill derived from x)
        let fill_b = if let Some(a) = xa {
            let empty_cell = array::typed_arr_from_b_vec(vec![], a.shape.clone(), a.fill);
            Some(rbqn_core::tag_arr(empty_cell))
        } else {
            None
        };
        let mut out = BqnArr::new_vec_b(vec![]);
        out.fill = fill_b;
        return Ok(PrimResult::Array(out));
    }

    // Parse w elements. Each can be:
    //   - scalar (f64): target group index, doesn't consume an x axis
    //   - rank-0 boxed (enclosed scalar like <2): same as scalar
    //   - rank-1+ array: index array, consumes one x axis
    enum WElem {
        Scalar(i32),         // target group index
        Array(Vec<i32>),     // index array (length matches one x axis)
    }

    let mut w_elems: Vec<WElem> = Vec::with_capacity(n_dims);
    for a in 0..n_dims {
        let w_elem = warr.get(a)?;
        if w_elem.is_f64() {
            let n = w_elem.to_i32().map_err(|_| BqnError::Domain(format!(
                "𝕨⊔𝕩: axis {} w element must be an integer", a
            )))?;
            w_elems.push(WElem::Scalar(n));
        } else {
            let w_arr = get_arr(w_elem)
                .ok_or_else(|| BqnError::Type(format!("𝕨⊔𝕩: axis {} w element must be an array", a)))?;
            // Rank-0 boxed (enclosed scalar): treat as scalar
            if w_arr.rank() == 0 {
                if w_arr.ia() > 0 {
                    let inner = w_arr.get(0)?;
                    if inner.is_f64() {
                        w_elems.push(WElem::Scalar(inner.to_i32()?));
                        continue;
                    }
                }
                return Err(BqnError::Type(format!("𝕨⊔𝕩: axis {} w element must be integer", a)));
            }
            // Rank-1+ array: integer indices
            let indices = w_arr.i32_iter().map_err(|_| BqnError::Type(format!(
                "𝕨⊔𝕩: axis {} w element must be an integer array", a
            )))?;
            w_elems.push(WElem::Array(indices));
        }
    }

    // Count how many array w elements → must be <= x rank
    let n_array_dims = w_elems.iter().filter(|e| matches!(e, WElem::Array(_))).count();

    // Get x info
    let x_rank;
    let x_shape: Vec<usize>;
    let x_fill;
    // For scalar x, synthesize a rank-0 array
    let scalar_arr;
    let xarr: &BqnArr = if let Some(a) = xa {
        x_rank = a.rank() as usize;
        x_shape = a.shape.clone();
        x_fill = a.fill;
        a
    } else {
        // Atom x: rank 0
        x_rank = 0;
        x_shape = vec![];
        x_fill = None;
        scalar_arr = BqnArr {
            shape: vec![],
            data: ArrData::Boxed(vec![x]),
            fill: None,
        };
        &scalar_arr
    };

    if n_array_dims > x_rank {
        return Err(BqnError::Rank(format!(
            "𝕨⊔𝕩: Total rank of 𝕨 must be at most rank of 𝕩 ({} > {})",
            n_array_dims, x_rank
        )));
    }

    // Map each w element to its x axis (or None for scalars)
    // Array elements consume consecutive x axes; scalar elements don't
    let mut x_axis_for_dim: Vec<Option<usize>> = Vec::with_capacity(n_dims);
    let mut next_x_axis = 0usize;
    for e in &w_elems {
        match e {
            WElem::Scalar(_) => { x_axis_for_dim.push(None); }
            WElem::Array(_) => {
                x_axis_for_dim.push(Some(next_x_axis));
                next_x_axis += 1;
            }
        }
    }

    // trailing shape = x dims beyond the consumed axes
    let trailing_shape = if x_rank > n_array_dims { x_shape[n_array_dims..].to_vec() } else { vec![] };
    let cell_size: usize = trailing_shape.iter().product::<usize>().max(1);

    // Build per-dimension specs
    struct DimSpec {
        matches: Vec<Vec<usize>>,  // matches[g] = positions for group g
    }

    let mut dims: Vec<DimSpec> = Vec::with_capacity(n_dims);
    let mut result_shape: Vec<usize> = Vec::with_capacity(n_dims);

    for (a, (we, xa_idx)) in w_elems.iter().zip(x_axis_for_dim.iter()).enumerate() {
        match we {
            WElem::Scalar(n) => {
                if *n < 0 {
                    return Err(BqnError::Domain(format!(
                        "𝕨⊔𝕩: scalar w for axis {} must be ≥ 0, got {}", a, n
                    )));
                }
                let n_u = *n as usize;
                let result_size = n_u + 1;
                // Scalar: match[n_u] contains all positions (but positions don't consume x axis)
                // For cell construction: a scalar dim contributes count=1 if g==n, else 0
                let mut matches: Vec<Vec<usize>> = vec![vec![]; result_size];
                matches[n_u] = vec![0]; // single "position" placeholder
                result_shape.push(result_size);
                dims.push(DimSpec { matches });
            }
            WElem::Array(indices) => {
                let xa_k = xa_idx.unwrap();
                let x_dim = if xa_k < x_rank { x_shape[xa_k] } else { 1 };

                let (idx_vals, min_len) = if indices.len() == x_dim + 1 {
                    let last = *indices.last().unwrap();
                    let min = last.max(0) as usize;
                    (indices[..x_dim].to_vec(), min)
                } else if indices.len() == x_dim {
                    (indices.clone(), 0usize)
                } else {
                    return Err(BqnError::Shape(format!(
                        "𝕨⊔𝕩: axis {} index length {} doesn't match 𝕩 dimension {}",
                        a, indices.len(), x_dim
                    )));
                };

                for &v in &idx_vals {
                    if v < -1 {
                        return Err(BqnError::Domain(format!(
                            "𝕨⊔𝕩: axis {} values must be ≥¯1, got {}", a, v
                        )));
                    }
                }

                let max_idx = idx_vals.iter().copied().max().unwrap_or(-1);
                let result_size = ((max_idx + 1).max(0) as usize).max(min_len);
                result_shape.push(result_size);

                let mut matches: Vec<Vec<usize>> = vec![vec![]; result_size];
                for (j, &g) in idx_vals.iter().enumerate() {
                    if g >= 0 && (g as usize) < result_size {
                        matches[g as usize].push(j);
                    }
                }
                dims.push(DimSpec { matches });
            }
        }
    }

    // Compute result strides
    let result_ia: usize = result_shape.iter().product();
    let result_strides: Vec<usize> = {
        let mut s = vec![1usize; n_dims];
        for k in (0..n_dims.saturating_sub(1)).rev() {
            s[k] = s[k + 1] * result_shape[k + 1];
        }
        s
    };

    // Compute x strides for each x axis
    let x_dim_strides: Vec<usize> = {
        let mut s = vec![1usize; x_rank + 1];
        for k in (0..x_rank).rev() {
            s[k] = s[k + 1] * x_shape[k];
        }
        (0..x_rank).map(|k| s[k + 1]).collect()
    };

    // Build result
    let mut result: Vec<B> = Vec::with_capacity(result_ia);

    for cell_flat in 0..result_ia {
        // Decode group multi-index
        let mut group_idxs: Vec<usize> = Vec::with_capacity(n_dims);
        let mut rem = cell_flat;
        for &stride in &result_strides {
            let g = rem / stride;
            rem %= stride;
            group_idxs.push(g);
        }

        // Get matching positions per dim
        let dim_matches: Vec<&[usize]> = (0..n_dims)
            .map(|k| dims[k].matches[group_idxs[k]].as_slice())
            .collect();

        // Cell shape: for each dim, count of matches
        // For scalar dims with wrong group, count=0 (empty matches)
        let counts: Vec<usize> = dim_matches.iter().map(|m| m.len()).collect();
        let cell_count: usize = counts.iter().product();
        let mut cell_shape = counts.clone();
        cell_shape.extend_from_slice(&trailing_shape);
        let cell_total = cell_count * cell_size;

        let mut cell_elems: Vec<B> = Vec::with_capacity(cell_total);

        // Enumerate outer product of dim_matches
        enumerate_product(&dim_matches, |positions| {
            // Compute x offset from the array-dim positions only
            let mut x_base = 0usize;
            for (k, &pos) in positions.iter().enumerate() {
                if let Some(xa_k) = x_axis_for_dim[k] {
                    // This is an array dim — pos is an actual x axis position
                    x_base += pos * x_dim_strides.get(xa_k).copied().unwrap_or(1);
                }
                // Scalar dims: pos is a placeholder (0), doesn't contribute to x offset
            }
            for j in 0..cell_size {
                cell_elems.push(xarr.get(x_base + j).unwrap_or(B::SENTINEL));
            }
        });

        let cell_arr = array::typed_arr_from_b_vec(cell_elems, cell_shape, x_fill);
        result.push(tag_arr(cell_arr));
    }

    let empty_cell_shape = {
        let mut s = vec![0usize; n_dims];
        s.extend_from_slice(&trailing_shape);
        s
    };
    let group_fill_arr = array::typed_arr_from_b_vec(vec![], empty_cell_shape, x_fill);
    let group_fill = rbqn_core::tag_arr(group_fill_arr);
    let mut out = BqnArr::new_vec_b(result);
    out.shape = result_shape;
    out.fill = Some(group_fill);
    Ok(PrimResult::Array(out))
}


/// Enumerate over the outer product of slices, calling f with each combination.
/// Each combination is a Vec<usize> with one position from each slice.
fn enumerate_product(slices: &[&[usize]], mut f: impl FnMut(&[usize])) {
    if slices.is_empty() {
        f(&[]);
        return;
    }
    let n = slices.len();
    let counts: Vec<usize> = slices.iter().map(|s| s.len()).collect();
    let total: usize = counts.iter().product();
    let mut indices = vec![0usize; n];

    for _ in 0..total {
        let positions: Vec<usize> = (0..n).map(|k| slices[k][indices[k]]).collect();
        f(&positions);

        // Increment indices (last dimension varies fastest)
        for k in (0..n).rev() {
            indices[k] += 1;
            if indices[k] < counts[k] {
                break;
            }
            indices[k] = 0;
        }
    }
}

// •GroupLen system function
pub fn group_len(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("•GroupLen: 𝕩 must be an array".into()))?;
    let indices = arr.i32_iter()?;
    let max_idx = indices.iter().copied().max().unwrap_or(-1);
    let n = (max_idx + 1).max(0) as usize;

    let mut counts = vec![0i32; n];
    for &g in &indices {
        if g >= 0 {
            counts[g as usize] += 1;
        }
    }
    Ok(PrimResult::Array(BqnArr::new_vec_i32(counts)))
}

// •GroupOrd system function
// Takes ⟨lengths, indices⟩ where lengths is group lengths and indices is the group
// assignment for each element. Returns ordered indices such that elements of each
// group are contiguous.
pub fn group_ord(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("•GroupOrd: 𝕩 must be an array".into()))?;

    // Expect a 2-element boxed array: ⟨lengths, indices⟩
    if arr.ia() < 2 {
        return Err(BqnError::Domain("•GroupOrd: Expected ⟨lengths, indices⟩".into()));
    }

    let lengths_b = arr.get(0)?;
    let indices_b = arr.get(1)?;

    let lengths_arr = get_arr(lengths_b)
        .ok_or_else(|| BqnError::Type("•GroupOrd: lengths must be an array".into()))?;
    let indices_arr = get_arr(indices_b)
        .ok_or_else(|| BqnError::Type("•GroupOrd: indices must be an array".into()))?;

    let lengths = lengths_arr.i32_iter()?;
    let indices = indices_arr.i32_iter()?;

    // Compute prefix sums of lengths to get offsets
    let n_groups = lengths.len();
    let mut offsets = Vec::with_capacity(n_groups + 1);
    offsets.push(0usize);
    for &l in &lengths {
        offsets.push(offsets.last().unwrap() + l as usize);
    }
    let total = *offsets.last().unwrap();

    // Place each element into its group's position
    let mut result = vec![0i32; total];
    let mut pos = offsets[..n_groups].to_vec();
    for (i, &g) in indices.iter().enumerate() {
        if g >= 0 && (g as usize) < n_groups {
            let gu = g as usize;
            if pos[gu] < offsets[gu + 1] {
                result[pos[gu]] = i as i32;
                pos[gu] += 1;
            }
        }
    }

    Ok(PrimResult::Array(BqnArr::new_vec_i32(result)))
}
