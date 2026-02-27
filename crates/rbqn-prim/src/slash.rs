use rbqn_core::*;
use rbqn_core::array::typed_arr_from_b_vec;
use crate::dispatch::PrimResult;
use crate::structural::prototype_of;

// / monad: indices
// NOTE: 𝕩 must be a rank-1 integer array of non-negative values. Rank-0 (atoms or enclosed) errors.
pub fn indices_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Err(BqnError::Type("/𝕩: 𝕩 must be an array".into()));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("/𝕩: 𝕩 must be an array".into()))?;
    if arr.rank() != 1 {
        return Err(BqnError::Rank(format!(
            "/𝕩: 𝕩 must be rank-1, got rank {}",
            arr.rank()
        )));
    }
    let counts = arr.i32_iter()?;
    let total: i32 = counts.iter().sum();
    if total < 0 {
        return Err(BqnError::Domain("/𝕩: 𝕩 must consist of natural numbers".into()));
    }
    let mut result = Vec::with_capacity(total as usize);
    for (i, &c) in counts.iter().enumerate() {
        if c < 0 {
            return Err(BqnError::Domain("/𝕩: 𝕩 must consist of natural numbers".into()));
        }
        for _ in 0..c {
            result.push(i as i32);
        }
    }
    // NOTE: empty result from / is legitimate BQN behavior
    Ok(PrimResult::Array(BqnArr::new_vec_i32(result)))
}

// /⁼ monad: group counts — for each value v in 𝕩, count how many times it appears.
// Result[v] = number of occurrences of v in 𝕩.
// /⁼ ⟨0,0,1,2,2,2⟩ → ⟨2,1,3⟩  (sorted case: matches / ⟨2,1,3⟩ = ⟨0,0,1,2,2,2⟩)
// /⁼ ⟨2,3,1,1⟩ → ⟨0,2,1,1⟩  (unsorted: count by value index)
pub fn indices_inverse_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("/⁼𝕩: 𝕩 must be an array".into()))?;
    if arr.ia() == 0 {
        return Ok(PrimResult::Array(BqnArr::new_vec_i32(vec![])));
    }
    let indices = arr.i32_iter()?;
    // Find actual max (not just last element — array may be unsorted)
    let max_idx = indices.iter().copied().max().unwrap_or(0);
    if max_idx < 0 {
        return Err(BqnError::Domain("/⁼𝕩: 𝕩 must consist of natural numbers".into()));
    }
    let mut counts = vec![0i32; (max_idx + 1) as usize];
    for &idx in &indices {
        if idx < 0 {
            return Err(BqnError::Domain("/⁼𝕩: 𝕩 must consist of natural numbers".into()));
        }
        counts[idx as usize] += 1;
    }
    Ok(PrimResult::Array(BqnArr::new_vec_i32(counts)))
}

// /⁼ dyad: w /⁼ x — find replicate counts r such that r / x ≡ w
// When w is a number: compute monadic /⁼ extended to length w
// When w is an array: compute counts per element of x
pub fn indices_inverse_c2(w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if w.is_f64() {
        // Scalar w: compute /⁼ x extended to length w
        let arr = xa.ok_or_else(|| BqnError::Type("𝕨/⁼𝕩: 𝕩 must be an array".into()))?;
        let n = w.to_i32()? as usize;
        if arr.ia() == 0 {
            return Ok(PrimResult::Array(BqnArr::new_vec_i32(vec![0i32; n])));
        }
        let indices = arr.i32_iter()?;
        let mut counts = vec![0i32; n];
        for &idx in &indices {
            if idx < 0 {
                return Err(BqnError::Domain("𝕨/⁼𝕩: 𝕩 must consist of natural numbers".into()));
            }
            if (idx as usize) < n {
                counts[idx as usize] += 1;
            }
        }
        Ok(PrimResult::Array(BqnArr::new_vec_i32(counts)))
    } else {
        // Array w: find r such that r / x ≡ w
        // r[i] = number of consecutive elements in w matching x[i]
        let x_arr = xa.ok_or_else(|| BqnError::Type("𝕨/⁼𝕩: 𝕩 must be an array".into()))?;
        let w_arr = wa.ok_or_else(|| BqnError::Type("𝕨/⁼𝕩: 𝕨 must be a number or array".into()))?;
        let x_len = x_arr.ia();
        let w_len = w_arr.ia();
        let mut counts = vec![0i32; x_len];
        let mut wi = 0usize;
        for xi in 0..x_len {
            while wi < w_len {
                // Count matching elements
                let w_val = w_arr.get(wi)?;
                let x_val = x_arr.get(xi)?;
                // Compare: for the replicate inverse, we need w[wi] == x[xi]
                if w_val.0 == x_val.0 || (w_val.is_f64() && x_val.is_f64() && w_val.o2f() == x_val.o2f()) {
                    counts[xi] += 1;
                    wi += 1;
                } else {
                    break;
                }
            }
        }
        Ok(PrimResult::Array(BqnArr::new_vec_i32(counts)))
    }
}

// / dyad: replicate
// NOTE: 𝕨 must be rank-1 or a scalar. 𝕩 must be rank-1 or an atom (atom treated as ⟨𝕩⟩).
pub fn replicate_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    // NOTE: BQN allows atom 𝕩: treat scalar as rank-1 length-1 array ⟨𝕩⟩.
    // This handles cases like `⟨⟩⊸/ 'a'` and `<⊸/3‿2‿1`.
    if x.is_atom() {
        // Determine count: 𝕨 for a length-1 target must be length 1 or scalar
        let count = if w.is_f64() {
            w.to_i32()?
        } else if let Some(warr) = wa {
            // Allow rank-0 (unit) or rank-1 length-1 𝕨
            let c = warr.i32_iter()?;
            if c.len() == 0 {
                // empty 𝕨 on atom: return <x (rank-0 enclosure)
                return Ok(PrimResult::Array(BqnArr {
                    shape: vec![],
                    data: ArrData::Boxed(vec![x]),
                    fill: Some(prototype_of(x)),
                }));
            }
            if c.len() != 1 {
                return Err(BqnError::Shape(format!(
                    "𝕨/𝕩: 𝕨 must have same length as 𝕩 (1), got {}",
                    c.len()
                )));
            }
            c[0]
        } else {
            return Err(BqnError::Type("𝕨/𝕩: 𝕨 must be a number or array".into()));
        };
        if count < 0 {
            return Err(BqnError::Domain("𝕨/𝕩: 𝕨 must consist of natural numbers".into()));
        }
        let result: Vec<B> = (0..count).map(|_| x).collect();
        let len = result.len();
        return Ok(PrimResult::Array(typed_arr_from_b_vec(result, vec![len], None)));
    }

    let arr = xa.ok_or_else(|| BqnError::Type("𝕨/𝕩: 𝕩 must be an array".into()))?;

    // Multi-axis replicate: when 𝕨 is a boxed list, each element replicates along one axis.
    if let Some(warr) = wa {
        if warr.rank() == 1 {
            if warr.ia() == 0 {
                // ⟨⟩/x = x (no axes to replicate along = identity)
                return Ok(PrimResult::Array(arr.clone()));
            }
            if warr.el_type() == ElType::B {
                // Check if first element is an array (multi-axis) vs number (standard)
                if let Ok(first) = warr.get(0) {
                    if first.is_arr() {
                        return replicate_multi_axis(warr, arr);
                    }
                }
            }
        }
    }

    // NOTE: BQN / on rank>1 𝕩: replicate along first axis.
    // Scalar 𝕨 replicates each major cell 𝕨 times.
    // Rank-1 𝕨 replicates major cell i by 𝕨[i] times.
    if arr.rank() > 1 {
        let first_dim = arr.shape[0];
        let cell_shape = arr.shape[1..].to_vec();
        let cell_size: usize = cell_shape.iter().product::<usize>().max(1);

        let counts: Vec<i32> = if w.is_f64() {
            let n = w.to_i32()?;
            if n < 0 {
                return Err(BqnError::Domain("𝕨/𝕩: 𝕨 must consist of natural numbers".into()));
            }
            vec![n; first_dim]
        } else {
            let warr = wa.ok_or_else(|| BqnError::Type("𝕨/𝕩: 𝕨 must be a number or array".into()))?;
            let w_ref: &BqnArr;
            let effective_warr;
            if warr.rank() == 0 {
                let inner_b = warr.get(0).map_err(|_| BqnError::Type("𝕨/𝕩: rank-0 𝕨 must enclose an array".into()))?;
                effective_warr = get_arr(inner_b)
                    .ok_or_else(|| BqnError::Type("𝕨/𝕩: rank-0 𝕨 must enclose an integer array".into()))?;
                w_ref = &effective_warr;
            } else {
                w_ref = warr;
            }
            if w_ref.rank() != 1 {
                return Err(BqnError::Rank(format!(
                    "𝕨/𝕩: 𝕨 must be rank-1, got rank {}",
                    w_ref.rank()
                )));
            }
            let c = w_ref.i32_iter()?;
            if c.len() != first_dim {
                return Err(BqnError::Shape(format!(
                    "𝕨/𝕩: 𝕨 must have same length as first axis of 𝕩 ({} vs {})",
                    c.len(), first_dim
                )));
            }
            c
        };

        for &c in &counts {
            if c < 0 {
                return Err(BqnError::Domain("𝕨/𝕩: 𝕨 must consist of natural numbers".into()));
            }
        }

        let total_rows: usize = counts.iter().map(|&c| c as usize).sum();
        let mut result = Vec::with_capacity(total_rows * cell_size);
        for (row, &c) in counts.iter().enumerate() {
            let base = row * cell_size;
            for _ in 0..c as usize {
                for j in 0..cell_size {
                    result.push(arr.get(base + j)?);
                }
            }
        }
        let mut new_shape = vec![total_rows];
        new_shape.extend_from_slice(&cell_shape);
        return Ok(PrimResult::Array(typed_arr_from_b_vec(result, new_shape, arr.fill)));
    }

    // NOTE: BQN allows rank-0 𝕨 (enclosed array like <arr) — unwrap to get the inner array.
    // `<arr /x` where arr is an integer array: use arr as the replication counts.
    let counts = if w.is_f64() {
        let n = w.to_i32()?;
        vec![n; arr.ia()]
    } else {
        let warr = wa.ok_or_else(|| BqnError::Type("𝕨/𝕩: 𝕨 must be a number or array".into()))?;
        // Rank-0 𝕨: enclosed array. Unwrap the single boxed element and use it as counts.
        let effective_warr;
        let w_ref: &BqnArr = if warr.rank() == 0 {
            // Get the single enclosed element
            let inner_b = warr.get(0).map_err(|_| BqnError::Type("𝕨/𝕩: rank-0 𝕨 must enclose an array".into()))?;
            effective_warr = get_arr(inner_b)
                .ok_or_else(|| BqnError::Type("𝕨/𝕩: rank-0 𝕨 must enclose an integer array".into()))?;
            &effective_warr
        } else {
            warr
        };
        // 𝕨 must be rank-1
        if w_ref.rank() != 1 {
            return Err(BqnError::Rank(format!(
                "𝕨/𝕩: 𝕨 must be rank-1, got rank {}",
                w_ref.rank()
            )));
        }
        let c = w_ref.i32_iter()?;
        if c.len() != arr.ia() {
            return Err(BqnError::Shape(format!(
                "𝕨/𝕩: 𝕨 must have same length as 𝕩 ({} vs {})",
                c.len(),
                arr.ia()
            )));
        }
        c
    };

    let total: usize = counts.iter().map(|&c| c.max(0) as usize).sum();
    let mut result = Vec::with_capacity(total);
    for (i, &c) in counts.iter().enumerate() {
        if c < 0 {
            return Err(BqnError::Domain("𝕨/𝕩: 𝕨 must consist of natural numbers".into()));
        }
        let v = arr.get(i)?;
        for _ in 0..c {
            result.push(v);
        }
    }
    let len = result.len();
    // NOTE: Preserve fill from source array. Use arr.fill if set, otherwise compute via arr_fill.
    // This ensures fill propagates correctly through replicate even when source has fill=None.
    let fill = if arr.fill.is_some() {
        arr.fill
    } else if arr.el_type() == ElType::B {
        Some(crate::structural::arr_fill(arr))
    } else {
        arr.fill
    };
    Ok(PrimResult::Array(typed_arr_from_b_vec(result, vec![len], fill)))
}

/// Multi-axis replicate: w is a boxed list, each element replicates along one axis of x.
fn replicate_multi_axis(warr: &BqnArr, xarr: &BqnArr) -> Result<PrimResult> {
    let n_axes = warr.ia();

    // Process axis-by-axis: start with x, replicate along each axis
    let mut current_data: Vec<B> = (0..xarr.ia()).map(|i| xarr.get(i).unwrap_or(B::SENTINEL)).collect();
    let mut current_shape = xarr.shape.clone();

    for a in 0..n_axes {
        if a >= current_shape.len() {
            return Err(BqnError::Rank("𝕨/𝕩: too many axes in 𝕨".into()));
        }
        let idx_b = warr.get(a)?;
        let counts = if idx_b.is_f64() {
            // Scalar: replicate entire axis by this amount
            let n = idx_b.o2i();
            if n < 0 {
                return Err(BqnError::Domain("𝕨/𝕩: counts must be non-negative".into()));
            }
            vec![n; current_shape[a]]
        } else if idx_b.is_arr() {
            let idx_arr = get_arr(idx_b)
                .ok_or_else(|| BqnError::Type("𝕨/𝕩: axis element must be number or array".into()))?;
            // NOTE: rank-0 enclosed scalar: treat as a uniform replication count (same as scalar).
            if idx_arr.shape.is_empty() {
                let inner = idx_arr.get(0)?;
                let n = if inner.is_f64() {
                    inner.to_i32()?
                } else {
                    return Err(BqnError::Type("𝕨/𝕩: rank-0 axis element must contain a number".into()));
                };
                if n < 0 {
                    return Err(BqnError::Domain("𝕨/𝕩: counts must be non-negative".into()));
                }
                vec![n; current_shape[a]]
            } else {
                let c = idx_arr.i32_iter()?;
                if c.len() != current_shape[a] {
                    return Err(BqnError::Shape(format!(
                        "𝕨/𝕩: axis {} length {} doesn't match 𝕩 dimension {}",
                        a, c.len(), current_shape[a]
                    )));
                }
                c
            }
        } else {
            return Err(BqnError::Type("𝕨/𝕩: axis element must be number or array".into()));
        };

        // Replicate along axis a
        let dim = current_shape[a];
        let total: usize = counts.iter().map(|&c| c.max(0) as usize).sum();

        // Compute strides
        let outer_size: usize = current_shape[..a].iter().product::<usize>().max(1);
        let inner_size: usize = current_shape[a+1..].iter().product::<usize>().max(1);
        let slice_size = dim * inner_size;

        let mut new_data = Vec::with_capacity(outer_size * total * inner_size);
        for o in 0..outer_size {
            let base = o * slice_size;
            for (i, &c) in counts.iter().enumerate() {
                if c < 0 {
                    return Err(BqnError::Domain("𝕨/𝕩: counts must be non-negative".into()));
                }
                for _ in 0..c as usize {
                    for j in 0..inner_size {
                        new_data.push(current_data[base + i * inner_size + j]);
                    }
                }
            }
        }
        current_shape[a] = total;
        current_data = new_data;
    }

    Ok(PrimResult::Array(typed_arr_from_b_vec(current_data, current_shape, xarr.fill)))
}
