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

    // NOTE: Multi-dim group: 𝕩 is a boxed array where each element is an integer array.
    // Dispatch to multi-dim implementation when 𝕩 has boxed element type.
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

// Multi-dimensional group: 𝕩 is a list of integer arrays ⟨l₁,l₂,...,lₙ⟩.
// Result shape: result_shape[k] = 1+max(lₖ)
// Cell at (g₁,...,gₙ): let matchingₖ = {i | lₖ[i]=gₖ}
//   Cell shape = (|matching₁|,...,|matchingₙ|)
//   Cell element at (r₁,...,rₙ) = ⟨matching₁[r₁],...,matchingₙ[rₙ]⟩ (rank-1 tuple)
// This creates an outer-product structure where each cell is a rank-n array of position tuples.
fn group_indices_multidim(arr: &BqnArr) -> Result<PrimResult> {
    let ndim = arr.ia();

    // Empty list: ⊔⟨⟩ → empty result
    if ndim == 0 {
        return Ok(PrimResult::Array(BqnArr::new_vec_b(vec![])));
    }

    // Extract each dimension's index array and compute result shape
    let mut dim_arrays: Vec<Vec<i32>> = Vec::with_capacity(ndim);
    let mut result_shape: Vec<usize> = Vec::with_capacity(ndim);

    for k in 0..ndim {
        let elem = arr.get(k)?;
        // NOTE: scalar element treated as length-1 array ⟨elem⟩
        let indices: Vec<i32> = if elem.is_f64() {
            vec![elem.to_i32().map_err(|_| BqnError::Type(format!("⊔𝕩: element {} must be an integer", k)))?]
        } else {
            let elem_arr = get_arr(elem)
                .ok_or_else(|| BqnError::Type(format!("⊔𝕩: element {} must be an integer array", k)))?;
            elem_arr.i32_iter()
                .map_err(|_| BqnError::Type(format!("⊔𝕩: element {} must be an integer array", k)))?
        };
        for &v in &indices {
            if v < -1 {
                return Err(BqnError::Domain(format!(
                    "⊔𝕩: values must be ≥¯1, got {}", v
                )));
            }
        }
        let max_idx = indices.iter().copied().max().unwrap_or(-1);
        result_shape.push((max_idx + 1).max(0) as usize);
        dim_arrays.push(indices);
    }

    // Precompute: for each dimension k and each group value g,
    // which positions in lₖ have value g.
    // matching_positions[k][g] = sorted list of positions i where dim_arrays[k][i] = g
    let mut matching_positions: Vec<Vec<Vec<usize>>> = Vec::with_capacity(ndim);
    for k in 0..ndim {
        let n_groups = result_shape[k];
        let mut matches: Vec<Vec<usize>> = vec![vec![]; n_groups];
        for (i, &g) in dim_arrays[k].iter().enumerate() {
            if g >= 0 && (g as usize) < n_groups {
                matches[g as usize].push(i);
            }
        }
        matching_positions.push(matches);
    }

    // Compute result strides for flat cell index
    let result_strides: Vec<usize> = {
        let mut s = vec![1usize; ndim];
        for k in (0..ndim.saturating_sub(1)).rev() {
            s[k] = s[k + 1] * result_shape[k + 1];
        }
        s
    };

    // For each result cell (g₁,...,gₙ): build outer-product array of position tuples
    let total_cells: usize = result_shape.iter().product();
    let mut result_cells: Vec<B> = Vec::with_capacity(total_cells);

    // Enumerate cells via flat index
    for cell_flat in 0..total_cells {
        // Decode cell multi-index (g₁,...,gₙ)
        let mut group_indices: Vec<usize> = Vec::with_capacity(ndim);
        let mut remaining = cell_flat;
        for k in 0..ndim {
            let g = remaining / result_strides[k];
            remaining %= result_strides[k];
            group_indices.push(g);
        }

        // Get matching positions for each dimension
        let mut dim_matches: Vec<&[usize]> = Vec::with_capacity(ndim);
        for k in 0..ndim {
            let g = group_indices[k];
            dim_matches.push(&matching_positions[k][g]);
        }

        // Cell shape = lengths of matching_k for each k
        let cell_shape: Vec<usize> = dim_matches.iter().map(|m| m.len()).collect();
        let cell_total: usize = cell_shape.iter().product();

        // Cell strides for iterating over the outer product
        let cell_strides: Vec<usize> = {
            let mut s = vec![1usize; ndim];
            for k in (0..ndim.saturating_sub(1)).rev() {
                s[k] = s[k + 1] * cell_shape[k + 1];
            }
            s
        };

        // Build cell elements: for each (r₁,...,rₙ), element = ⟨dim_matches[0][r₁],...⟩
        let mut cell_elems: Vec<B> = Vec::with_capacity(cell_total);
        for elem_flat in 0..cell_total {
            let mut pos_tuple: Vec<i32> = Vec::with_capacity(ndim);
            let mut remaining2 = elem_flat;
            for k in 0..ndim {
                let r_k = if ndim > 0 && cell_strides[k] > 0 { remaining2 / cell_strides[k] } else { 0 };
                remaining2 %= cell_strides[k].max(1);
                pos_tuple.push(dim_matches[k][r_k] as i32);
            }
            cell_elems.push(tag_arr(BqnArr::new_vec_i32(pos_tuple)));
        }

        // Build the cell as a rank-n boxed array
        let cell_arr = {
            let mut a = BqnArr::new_vec_b(cell_elems);
            a.shape = cell_shape;
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
// BQN spec: ≠𝕨 can equal ≠𝕩 or 1+≠𝕩.
// If ≠𝕨 = 1+≠𝕩, the last element of 𝕨 directly specifies the minimum result length.
// (i.e., the result has at least max(0, last_element) groups)
// NOTE: 𝕨 must be rank-1. If it contains scalars that are functions/arrays, error.
pub fn group_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if w.is_atom() {
        return Err(BqnError::Type("𝕨⊔𝕩: 𝕨 must be an integer array".into()));
    }
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⊔𝕩: 𝕨 must be an array".into()))?;
    // Multi-axis group: w is a boxed list of index arrays.
    // Detect: w is rank-1 boxed AND has at least one element that is an array.
    if warr.rank() == 1 && warr.el_type() == ElType::B && warr.ia() > 0 {
        // Check if first element is an array (not a scalar) — multi-axis indicator
        if let Ok(first) = warr.get(0) {
            if first.is_arr() {
                return group_multi_axis(warr, x, xa);
            }
        }
    }
    if warr.rank() != 1 {
        return Err(BqnError::Rank(format!(
            "𝕨⊔𝕩: 𝕨 must be rank-1, got rank {}",
            warr.rank()
        )));
    }
    let xarr = xa.ok_or_else(|| BqnError::Type("𝕨⊔𝕩: 𝕩 must be an array".into()))?;

    let all_indices = warr.i32_iter()?;
    let xia = xarr.ia();

    // Determine indices for grouping and minimum result length
    let (indices, min_len) = if all_indices.len() == xia + 1 {
        // FIX: Last element directly specifies minimum result length (not length-1).
        // Verified against CBQN: ⟨0⟩⊔⟨⟩ → ⟨⟩ (len 0), ⟨1⟩⊔⟨⟩ → ⟨⟨⟩⟩ (len 1)
        let last = *all_indices.last().unwrap();
        let min = last.max(0) as usize;
        (&all_indices[..xia], min)
    } else if all_indices.len() == xia {
        (&all_indices[..], 0usize)
    } else {
        return Err(BqnError::Shape(format!(
            "𝕨⊔𝕩: ≠𝕨 must be ≠𝕩 or 1+≠𝕩 ({} vs {})",
            all_indices.len(),
            xia
        )));
    };

    let max_idx = indices.iter().copied().max().unwrap_or(-1);
    let n = ((max_idx + 1).max(0) as usize).max(min_len);

    let mut groups: Vec<Vec<B>> = vec![vec![]; n];
    for (i, &g) in indices.iter().enumerate() {
        if g >= 0 {
            groups[g as usize].push(xarr.get(i)?);
        }
    }

    let result: Vec<B> = groups
        .into_iter()
        .map(|g| {
            let len = g.len();
            tag_arr(array::typed_arr_from_b_vec(g, vec![len], xarr.fill))
        })
        .collect();
    // NOTE: Fill of group result = prototype of an empty group with same element fill as x.
    // This allows 1↑(w⊔x) to fill with the correct empty-group prototype.
    let group_fill_arr = array::typed_arr_from_b_vec(vec![], vec![0], xarr.fill);
    let group_fill = rbqn_core::tag_arr(group_fill_arr);
    let mut out = BqnArr::new_vec_b(result);
    out.fill = Some(group_fill);
    Ok(PrimResult::Array(out))
}

/// Multi-axis group: w is a boxed list of index arrays.
/// Each boxed element specifies grouping along one axis.
/// Result is a multi-dimensional array of groups.
fn group_multi_axis(warr: &BqnArr, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let n_axes = warr.ia();
    if n_axes == 0 {
        return Err(BqnError::Domain("𝕨⊔𝕩: empty 𝕨 not supported".into()));
    }

    let xarr = xa.ok_or_else(|| BqnError::Type("𝕨⊔𝕩: 𝕩 must be an array".into()))?;

    // Validate: total rank of w index arrays must be ≤ rank of x
    let mut total_w_rank: usize = 0;
    for a in 0..n_axes {
        let idx_b = warr.get(a)?;
        if idx_b.is_arr() {
            if let Some(ia) = get_arr(idx_b) {
                total_w_rank += ia.rank() as usize;
            }
        }
        // Scalar indices count as rank 0, contributing nothing
    }
    if total_w_rank > xarr.rank() as usize {
        return Err(BqnError::Rank(
            "𝕨⊔𝕩: Total rank of 𝕨 must be at most rank of 𝕩".into()
        ));
    }

    // Extract index arrays for each axis
    let mut axes: Vec<(Vec<i32>, usize)> = Vec::with_capacity(n_axes);
    for a in 0..n_axes {
        let idx_b = warr.get(a)?;
        if idx_b.is_f64() {
            // Scalar: treat as single-element array
            let v = idx_b.o2i();
            let x_dim = if (xarr.rank() as usize) > a { xarr.shape[a] } else { 1 };
            if 1 != x_dim && 1 != x_dim + 1 {
                return Err(BqnError::Shape("𝕨⊔𝕩: index length mismatch".into()));
            }
            let max_idx = v.max(0) as usize;
            axes.push((vec![v], max_idx));
            continue;
        }
        let idx_arr = get_arr(idx_b)
            .ok_or_else(|| BqnError::Type("𝕨⊔𝕩: index element must be an array".into()))?;
        let indices = idx_arr.i32_iter()?;
        let x_dim = if (xarr.rank() as usize) > a { xarr.shape[a] } else { 1 };
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
        let max_idx = idx_vals.iter().copied().max().unwrap_or(-1);
        let dim = ((max_idx + 1).max(0) as usize).max(min_len);
        axes.push((idx_vals, dim));
    }

    // Build result shape
    let result_shape: Vec<usize> = axes.iter().map(|(_, dim)| *dim).collect();
    let result_ia: usize = result_shape.iter().product();

    // Determine cell shape (remaining x dimensions after the grouped axes)
    let cell_axes = n_axes.min(xarr.rank() as usize);
    let cell_shape = xarr.shape[cell_axes..].to_vec();
    let cell_size: usize = cell_shape.iter().product::<usize>().max(1);

    // For each element position in x, compute which group it belongs to
    let x_lead_shape = &xarr.shape[..cell_axes];
    let mut groups: Vec<Vec<Vec<B>>> = vec![vec![]; result_ia]; // groups[flat_group_idx] = list of cells

    // Iterate over all leading positions in x
    let x_lead_ia: usize = x_lead_shape.iter().product::<usize>().max(1);
    for pos in 0..x_lead_ia {
        // Convert flat position to multi-dimensional indices
        let mut remaining = pos;
        let mut group_flat = 0usize;
        let mut valid = true;
        let mut group_stride = 1;
        for a in (0..cell_axes).rev() {
            let dim = x_lead_shape[a];
            let idx_in_x = remaining % dim;
            remaining /= dim;
            let g = axes[a].0[idx_in_x];
            if g < 0 { valid = false; break; }
            group_flat += g as usize * group_stride;
            group_stride *= result_shape[a];
        }
        if !valid { continue; }
        if group_flat >= result_ia { continue; }

        // Extract cell at this position
        let base = pos * cell_size;
        let mut cell = Vec::with_capacity(cell_size);
        for j in 0..cell_size {
            cell.push(xarr.get(base + j)?);
        }
        groups[group_flat].push(cell);
    }

    // Build result: each group becomes an array
    let result: Vec<B> = groups.into_iter().map(|cells| {
        let n_cells = cells.len();
        let mut elems = Vec::with_capacity(n_cells * cell_size);
        for cell in &cells {
            elems.extend_from_slice(cell);
        }
        let mut shape = vec![n_cells];
        shape.extend_from_slice(&cell_shape);
        tag_arr(array::typed_arr_from_b_vec(elems, shape, xarr.fill))
    }).collect();

    let group_fill_arr = array::typed_arr_from_b_vec(vec![], vec![0], xarr.fill);
    let group_fill = rbqn_core::tag_arr(group_fill_arr);
    let mut out = BqnArr::new_vec_b(result);
    out.shape = result_shape;
    out.fill = Some(group_fill);
    Ok(PrimResult::Array(out))
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
