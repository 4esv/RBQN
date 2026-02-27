use rbqn_core::*;
use rbqn_core::array::typed_arr_from_b_vec;
use crate::dispatch::PrimResult;

// ⊏ monad: first cell
// For rank-1:
//   - Flat (non-boxed) arrays: returns rank-0 unit containing the element
//   - Boxed arrays: returns the first element directly (the inner array)
// For rank-n (n>1): returns first major cell (rank n-1 array)
pub fn first_cell_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("⊏𝕩: 𝕩 must be an array".into()))?;
    if arr.shape.is_empty() || arr.shape[0] == 0 {
        return Err(BqnError::Domain("⊏𝕩: 𝕩 is empty along first axis".into()));
    }
    if arr.rank() == 1 {
        let v = arr.get(0)?;
        if arr.el_type() == ElType::B {
            // NOTE: Boxed rank-1: first cell = first element directly (the inner value)
            // The element v is already a B value that IS the inner array/scalar
            if v.is_arr() {
                // Return the inner array directly
                if let Some(inner) = get_arr(v) {
                    return Ok(PrimResult::Array(inner));
                }
            }
            return Ok(PrimResult::Scalar(v));
        }
        // NOTE: Flat rank-1: first cell = rank-0 unit containing the element
        let out = BqnArr {
            shape: vec![],
            data: ArrData::Boxed(vec![v]),
            fill: None,
        };
        return Ok(PrimResult::Array(out));
    }
    // Rank >= 2: return first major cell (rank n-1 subarray)
    let cell_shape = &arr.shape[1..];
    let cell_size: usize = cell_shape.iter().product::<usize>().max(1);
    let mut result = Vec::with_capacity(cell_size);
    for j in 0..cell_size {
        result.push(arr.get(j)?);
    }
    let out = typed_arr_from_b_vec(result, cell_shape.to_vec(), arr.fill);
    Ok(PrimResult::Array(out))
}

fn resolve_index(i: i32, len: usize) -> Result<usize> {
    let idx = if i < 0 { i + len as i32 } else { i };
    if idx < 0 || idx as usize >= len {
        return Err(BqnError::Domain(format!(
            "Index {i} out of bounds for length {len}"
        )));
    }
    Ok(idx as usize)
}

/// Validate that a B value is an integer index (not fractional).
fn validate_integer_index(v: B, name: &str) -> Result<i32> {
    if !v.is_f64() {
        return Err(BqnError::Type(format!("{name}: index must be a number")));
    }
    let f = v.o2f();
    let i = f as i32;
    if f != i as f64 {
        return Err(BqnError::Domain(format!("{name}: index must be an integer, got {f}")));
    }
    Ok(i)
}

// ⊏ dyad: select
// NOTE: Selects major cells along first axis of 𝕩.
// When 𝕨 is a scalar: returns rank-0 cell (for rank-1 𝕩) or rank(𝕩)-1 cell.
// When 𝕨 is a rank-1 array of integers: returns rank(𝕩) result with selected cells.
// 𝕨 must not be rank-0 (enclosed) or rank>1 non-boxed.
pub fn select_c2(w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨⊏𝕩: 𝕩 must be an array".into()))?;
    // NOTE: BQN ⊏ selects along the first axis
    let first_dim = if arr.shape.is_empty() { arr.ia() } else { arr.shape[0] };
    let cell_shape: &[usize] = if arr.shape.len() > 1 { &arr.shape[1..] } else { &[] };
    let cell_size: usize = cell_shape.iter().product::<usize>().max(1);

    if w.is_f64() {
        let wi = validate_integer_index(w, "𝕨⊏𝕩")?;
        let idx = resolve_index(wi, first_dim)?;
        if arr.rank() <= 1 {
            // NOTE: scalar select from rank-1 returns rank-0 cell (unit array)
            let v = arr.get(idx)?;
            let out = BqnArr {
                shape: vec![],
                data: if v.is_f64() {
                    ArrData::F64(vec![v.o2f()])
                } else if v.is_c32() {
                    ArrData::C32(vec![v.0 as u32])
                } else {
                    ArrData::Boxed(vec![v])
                },
                fill: arr.fill,
            };
            return Ok(PrimResult::Array(out));
        }
        // Multi-dimensional: return the selected cell
        let mut result = Vec::with_capacity(cell_size);
        for j in 0..cell_size {
            result.push(arr.get(idx * cell_size + j)?);
        }
        let out = typed_arr_from_b_vec(result, cell_shape.to_vec(), arr.fill);
        return Ok(PrimResult::Array(out));
    }

    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⊏𝕩: 𝕨 must be a number or array".into()))?;

    // rank-0 boxed 𝕨: unbox and use as index array for first axis
    if warr.rank() == 0 {
        if warr.el_type() == ElType::B {
            let inner = warr.get(0)?;
            if inner.is_f64() {
                let idx = resolve_index(validate_integer_index(inner, "𝕨⊏𝕩")?, first_dim)?;
                if arr.rank() <= 1 {
                    return Ok(PrimResult::Scalar(arr.get(idx)?));
                }
                let mut result = Vec::with_capacity(cell_size);
                for j in 0..cell_size {
                    result.push(arr.get(idx * cell_size + j)?);
                }
                let out = typed_arr_from_b_vec(result, cell_shape.to_vec(), arr.fill);
                return Ok(PrimResult::Array(out));
            }
            if inner.is_arr() {
                let inner_arr = get_arr(inner)
                    .ok_or_else(|| BqnError::Type("𝕨⊏𝕩: index not found".into()))?;
                let indices = inner_arr.i32_iter()?;
                if cell_shape.is_empty() && arr.rank() <= 1 {
                    let mut result = Vec::with_capacity(indices.len());
                    for &i in &indices {
                        let idx = resolve_index(i, first_dim)?;
                        result.push(arr.get(idx)?);
                    }
                    let out = typed_arr_from_b_vec(result, inner_arr.shape.clone(), arr.fill);
                    return Ok(PrimResult::Array(out));
                } else {
                    let mut result = Vec::with_capacity(indices.len() * cell_size);
                    for &i in &indices {
                        let idx = resolve_index(i, first_dim)?;
                        for j in 0..cell_size {
                            result.push(arr.get(idx * cell_size + j)?);
                        }
                    }
                    let mut out_shape = inner_arr.shape.clone();
                    out_shape.extend_from_slice(cell_shape);
                    let out = typed_arr_from_b_vec(result, out_shape, arr.fill);
                    return Ok(PrimResult::Array(out));
                }
            }
        }
        return Err(BqnError::Rank("𝕨⊏𝕩: 𝕨 must be a number or rank≥1 array".into()));
    }

    // NOTE: rank>1 boxed 𝕨 is not a standard ⊏ pattern
    if warr.rank() > 1 && warr.el_type() == ElType::B {
        return Err(BqnError::Rank(format!(
            "𝕨⊏𝕩: rank-{} boxed 𝕨 not supported for ⊏",
            warr.rank()
        )));
    }

    // NOTE: BQN multi-axis select: when 𝕨 is a boxed rank-1 list, each element is a
    // numeric index array selecting along one axis of 𝕩.
    // Result shape = concat of each element's shape, plus trailing axes of 𝕩.
    // Special case: empty list ⟨⟩ means "select 0 rows along axis 0" = shape [0]∾(≢𝕩)[1..]
    if warr.el_type() == ElType::B {
        let wia = warr.ia();

        if wia == 0 {
            // Empty boxed list: select 0 rows along axis 0
            let mut out_shape = vec![0usize];
            if arr.rank() > 1 {
                out_shape.extend_from_slice(&arr.shape[1..]);
            }
            let out = typed_arr_from_b_vec(vec![], out_shape, arr.fill);
            return Ok(PrimResult::Array(out));
        }

        // Check for mixed type: having both scalar numbers and arrays is invalid
        {
            let has_num = (0..wia).any(|i| warr.get(i).map(|v| v.is_f64() || v.is_c32()).unwrap_or(false));
            let has_arr = (0..wia).any(|i| warr.get(i).map(|v| {
                if v.is_arr() {
                    get_arr(v).map(|a| a.rank() >= 1).unwrap_or(false)
                } else { false }
            }).unwrap_or(false));
            if has_num && has_arr {
                return Err(BqnError::Type(
                    "𝕨⊏𝕩: 𝕨 must be an array of numbers or list of such (𝕨 contained both an array and number)".into()
                ));
            }
        }

        // Gather per-axis index arrays and validate
        let mut axis_indices: Vec<Vec<i32>> = Vec::with_capacity(wia);
        let mut axis_shapes: Vec<Vec<usize>> = Vec::with_capacity(wia);
        for i in 0..wia {
            let idx_b = warr.get(i)?;
            if idx_b.is_f64() {
                // Scalar index: treated as rank-1 length-1 array
                let iv = validate_integer_index(idx_b, "𝕨⊏𝕩")?;
                axis_indices.push(vec![iv]);
                axis_shapes.push(vec![1]);
            } else if idx_b.is_arr() {
                let sub_arr = get_arr(idx_b)
                    .ok_or_else(|| BqnError::Type("𝕨⊏𝕩: index element not found".into()))?;
                // rank-0 enclosed: error
                if sub_arr.rank() == 0 {
                    return Err(BqnError::Rank("𝕨⊏𝕩: index element must not be rank-0 (enclosed)".into()));
                }
                let sub_indices = sub_arr.i32_iter()?;
                axis_shapes.push(sub_arr.shape.clone());
                axis_indices.push(sub_indices);
            } else {
                return Err(BqnError::Type("𝕨⊏𝕩: index must be number or array".into()));
            }
        }

        // Build result shape: concat of all axis index shapes, plus trailing axes of x
        let arr_rank = arr.rank() as usize;
        let n_axes = wia.min(arr_rank);
        let mut out_shape: Vec<usize> = Vec::new();
        for sh in &axis_shapes {
            out_shape.extend_from_slice(sh);
        }
        // Trailing axes of x (beyond what w covers)
        for k in n_axes..arr_rank {
            out_shape.push(arr.shape[k]);
        }

        // Validate: each axis index fits within x's axis
        for (a, indices) in axis_indices.iter().enumerate() {
            let axis_len = if a < arr_rank { arr.shape[a] } else { 1 };
            for &idx in indices {
                let _ = resolve_index(idx, axis_len)?;
            }
        }

        // Resolve all negative indices
        let resolved: Vec<Vec<usize>> = axis_indices.iter().enumerate().map(|(a, idxs)| {
            let axis_len = if a < arr_rank { arr.shape[a] } else { 1 };
            idxs.iter().map(|&idx| {
                resolve_index(idx, axis_len).unwrap()
            }).collect()
        }).collect();

        // Compute element counts per axis index group
        let per_axis_counts: Vec<usize> = resolved.iter().map(|v| v.len()).collect();

        let total: usize = out_shape.iter().product();
        let mut result = vec![arr.fill.unwrap_or(B::SENTINEL); total];

        // Trailing cell size (axes of x not covered by w)
        let trailing_size: usize = if n_axes < arr_rank {
            arr.shape[n_axes..].iter().product()
        } else {
            1
        };

        // Compute x strides (for all axes of x)
        let x_strides: Vec<usize> = {
            let n = arr_rank;
            let mut s = vec![1usize; n];
            for k in (0..n.saturating_sub(1)).rev() {
                s[k] = s[k+1] * arr.shape[k+1];
            }
            s
        };

        // Check if any axis has 0 indices → empty result
        let combined_count: usize = per_axis_counts.iter().product();
        if combined_count == 0 || total == 0 {
            let out = typed_arr_from_b_vec(result, out_shape, arr.fill);
            return Ok(PrimResult::Array(out));
        }

        // Iterate over all combinations of axis indices (row-major) × trailing cells
        let mut combo: Vec<usize> = vec![0; n_axes];
        loop {
            // Compute x flat base from current combo
            let mut x_base = 0usize;
            for a in 0..n_axes {
                x_base += resolved[a][combo[a]] * x_strides[a];
            }
            // Compute result flat base from current combo
            let mut r_base = 0usize;
            {
                let mut mul = trailing_size;
                for a in (0..n_axes).rev() {
                    r_base += combo[a] * mul;
                    mul *= per_axis_counts[a];
                }
            }
            // Copy trailing cell
            for trail in 0..trailing_size {
                let x_flat = x_base + trail;
                let r_flat = r_base + trail;
                if r_flat < result.len() && x_flat < arr.ia() {
                    result[r_flat] = arr.get(x_flat)?;
                }
            }
            // Advance combo (big-endian / row-major)
            let mut carry = true;
            for a in (0..n_axes).rev() {
                if carry {
                    combo[a] += 1;
                    if combo[a] < per_axis_counts[a] {
                        carry = false;
                    } else {
                        combo[a] = 0;
                    }
                }
            }
            if carry { break; }
        }

        let out = typed_arr_from_b_vec(result, out_shape, arr.fill);
        return Ok(PrimResult::Array(out));
    }

    let indices = warr.i32_iter()?;

    if cell_shape.is_empty() && arr.rank() <= 1 {
        // Rank-1: simple element selection
        let mut result = Vec::with_capacity(indices.len());
        for &i in &indices {
            let idx = resolve_index(i, first_dim)?;
            result.push(arr.get(idx)?);
        }
        let out = typed_arr_from_b_vec(result, warr.shape.clone(), arr.fill);
        Ok(PrimResult::Array(out))
    } else {
        // Multi-dimensional: select cells along first axis
        let mut result = Vec::with_capacity(indices.len() * cell_size);
        for &i in &indices {
            let idx = resolve_index(i, first_dim)?;
            for j in 0..cell_size {
                result.push(arr.get(idx * cell_size + j)?);
            }
        }
        let mut out_shape = warr.shape.clone();
        out_shape.extend_from_slice(cell_shape);
        let out = typed_arr_from_b_vec(result, out_shape, arr.fill);
        Ok(PrimResult::Array(out))
    }
}

// ⊑ monad: first
pub fn first_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Ok(PrimResult::Scalar(x));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("⊑𝕩: 𝕩 must be an array".into()))?;
    // NOTE: BQN spec: ⊑ on empty array always errors (no fill element access)
    if arr.ia() == 0 {
        return Err(BqnError::Domain("⊑𝕩: 𝕩 is empty".into()));
    }
    Ok(PrimResult::Scalar(arr.get(0)?))
}

// ⊑ dyad: pick
// BQN pick semantics:
// - w is scalar (f64): pick from rank-1 x by index
// - w is rank-1 NUMERIC array: multi-axis index (length must match rank of x), returns scalar
// - w is BOXED array (any rank): iterate elements, each picks from x independently, result shape = shape of w
// - w is rank-0 boxed: unwrap and recurse
pub fn pick_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if w.is_f64() {
        // Scalar pick: x must be rank-1
        if x.is_atom() { return Ok(PrimResult::Scalar(x)); }
        let arr = xa.ok_or_else(|| BqnError::Type("𝕨⊑𝕩: 𝕩 must be an array".into()))?;
        if arr.rank() != 1 {
            return Err(BqnError::Rank(format!(
                "𝕨⊑𝕩: 𝕩 must be a list when 𝕨 is a number ({:?} ≡ ≢𝕩)", arr.shape
            )));
        }
        let wi = validate_integer_index(w, "𝕨⊑𝕩")?;
        let idx = resolve_index(wi, arr.ia())?;
        return Ok(PrimResult::Scalar(arr.get(idx)?));
    }

    if !w.is_arr() {
        return Err(BqnError::Type("𝕨⊑𝕩: 𝕨 must be a number or array".into()));
    }

    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⊑𝕩: 𝕨 must be a number or array".into()))?;

    // rank-0 boxed w: apply inner as index, then ENCLOSE result
    // <1‿2⊑x = <(x[1,2]) -- enclose the picked element
    // Constraint: inner must be a rank-1 array (not scalar, not rank-0)
    if warr.rank() == 0 {
        if warr.el_type() == ElType::B && warr.ia() > 0 {
            let inner = warr.get(0)?;
            // Validate: inner must be a rank-1+ array (not scalar)
            if inner.is_f64() || inner.is_c32() {
                return Err(BqnError::Rank(format!(
                    "𝕨⊑𝕩: Leaf arrays in 𝕨 must have rank 1 (element: {:?})", warr.shape
                )));
            }
            // Pick using inner index, then enclose result
            let picked = pick_elem(inner, x)?;
            return Ok(PrimResult::Array(BqnArr {
                shape: vec![],
                data: ArrData::Boxed(vec![picked]),
                fill: Some(crate::structural::prototype_of(picked)),
            }));
        }
        // empty rank-0 boxed (<⟨⟩): must be applied to a unit (atom or rank-0 enclosed)
        if warr.el_type() == ElType::B {
            if x.is_atom() {
                // Atom is a unit: enclose it
                return Ok(PrimResult::Array(BqnArr {
                    shape: vec![],
                    data: ArrData::Boxed(vec![x]),
                    fill: Some(crate::structural::prototype_of(x)),
                }));
            }
            if let Some(xarr) = xa {
                if xarr.rank() == 0 {
                    // rank-0 enclosed is a unit: return as-is (the enclosure)
                    return Ok(PrimResult::Array(xarr.clone()));
                }
            }
            return Err(BqnError::Domain("𝕨⊑𝕩: 𝕩 must be a unit if 𝕨 is empty enclosed".into()));
        }
        return Err(BqnError::Rank("𝕨⊑𝕩: rank-0 𝕨 must be boxed".into()));
    }

    // rank-1+ NUMERIC: multi-axis index (single pick)
    if warr.el_type() != ElType::B {
        let arr = xa.ok_or_else(|| BqnError::Type("𝕨⊑𝕩: 𝕩 must be an array".into()))?;
        let indices = warr.i32_iter()?;
        if warr.rank() == 1 {
            if indices.len() != arr.rank() as usize {
                return Err(BqnError::Rank(format!(
                    "𝕨⊑𝕩: Picking item at wrong rank (index {:?} in array of shape {:?})",
                    indices, arr.shape
                )));
            }
            let mut flat_idx = 0usize;
            let mut stride = 1usize;
            for i in (0..indices.len()).rev() {
                let idx = resolve_index(indices[i], arr.shape[i])?;
                flat_idx += idx * stride;
                stride *= arr.shape[i];
            }
            return Ok(PrimResult::Scalar(arr.get(flat_idx)?));
        }
        return Err(BqnError::Rank("𝕨⊑𝕩: numeric 𝕨 must be rank-1".into()));
    }

    // BOXED w: iterate elements, each independently picks from x
    let wia = warr.ia();
    if wia == 0 {
        // Empty boxed w: ⟨⟩⊑x -- x must be a "unit" (atom or rank-0 enclosed)
        if x.is_atom() {
            return Ok(PrimResult::Scalar(x));
        }
        if let Some(xarr) = xa {
            if xarr.rank() == 0 && xarr.ia() > 0 {
                return Ok(PrimResult::Scalar(xarr.get(0)?));
            }
            if xarr.rank() == 0 {
                return Ok(PrimResult::Scalar(x));
            }
        }
        return Err(BqnError::Domain("𝕨⊑𝕩: 𝕩 must be a unit if 𝕨 contains an empty array".into()));
    }
    // Validate: all elements must be the same "kind" (no mixed numeric+boxed),
    // and any array element must be rank 1 (not rank 2+).
    // (CBQN errors with "mixed-type elements" for lists with scalar ints and boxed arrays,
    // and "Leaf arrays in 𝕨 must have rank 1" for rank-2+ array elements.)
    {
        let mut has_num = false;
        let mut has_arr = false;
        for i in 0..wia {
            let elem = warr.get(i)?;
            if elem.is_f64() || elem.is_c32() {
                has_num = true;
            } else if elem.is_arr() {
                let ea = get_arr(elem)
                    .ok_or_else(|| BqnError::Type("𝕨⊑𝕩: index element not found".into()))?;
                // rank-0 enclosed is OK (path into nested structure)
                if ea.rank() > 1 {
                    return Err(BqnError::Rank(format!(
                        "𝕨⊑𝕩: Leaf arrays in 𝕨 must have rank 1 (element: {:?})", ea.shape
                    )));
                }
                if ea.rank() == 1 {
                    has_arr = true;
                }
                // rank-0 boxed: not counted as "array" for mixed-type purposes
            }
        }
        if has_num && has_arr {
            return Err(BqnError::Type("𝕨⊑𝕩: 𝕨 contained list with mixed-type elements".into()));
        }
    }
    let mut result = Vec::with_capacity(wia);
    for i in 0..wia {
        let idx_b = warr.get(i)?;
        result.push(pick_elem(idx_b, x)?);
    }
    let out = typed_arr_from_b_vec(result, warr.shape.clone(), None);
    Ok(PrimResult::Array(out))
}

/// Pick an element using idx as the index value applied to x.
/// Used by the boxed-w iteration in pick_c2.
/// Rules:
/// - idx is f64 scalar: pick from rank-1 x (index into first axis)
/// - idx is numeric array: multi-axis pick (length = rank of x)
/// - idx is rank-0 boxed: unwrap and apply inner to x (this accesses enclosed x)
/// - idx is rank-1+ boxed: iterate elements, each picks from x, result is array
fn pick_elem(idx: B, x: B) -> Result<B> {
    if idx.is_f64() {
        // Scalar: pick from rank-1 (or first axis of higher-rank)
        if x.is_atom() {
            // Any index into an atom = the atom itself (BQN: index into enclosed scalar)
            return Ok(x);
        }
        let arr = get_arr(x)
            .ok_or_else(|| BqnError::Type("𝕨⊑𝕩: 𝕩 must be an array".into()))?;
        let i = idx.to_i32()?;
        if arr.rank() != 1 {
            return Err(BqnError::Rank(format!(
                "𝕨⊑𝕩: Picking item at wrong rank (scalar index in array of shape {:?})",
                arr.shape
            )));
        }
        let pos = resolve_index(i, arr.ia())?;
        return Ok(arr.get(pos)?);
    }

    if let Some(idx_arr) = get_arr(idx) {
        if idx_arr.rank() == 0 {
            // rank-0 boxed index: applies inner index to x, then ENCLOSES result
            // Constraint: inner must be rank-1+ array (not plain scalar)
            if idx_arr.ia() == 0 {
                // <⟨⟩⊑x: empty enclosed, x must be unit
                if x.is_atom() {
                    return Ok(tag_arr(BqnArr {
                        shape: vec![],
                        data: ArrData::Boxed(vec![x]),
                        fill: Some(crate::structural::prototype_of(x)),
                    }));
                }
                let xarr = get_arr(x).ok_or_else(|| BqnError::Type("𝕨⊑𝕩: 𝕩 must be an array".into()))?;
                if xarr.rank() != 0 {
                    return Err(BqnError::Domain("𝕨⊑𝕩: 𝕩 must be a unit if 𝕨 contains an empty array".into()));
                }
                return Ok(x);
            }
            // Non-empty rank-0 boxed: inner must be rank-1+ (not scalar)
            let inner = idx_arr.get(0)?;
            if inner.is_f64() || inner.is_c32() {
                return Err(BqnError::Rank("𝕨⊑𝕩: Leaf arrays in 𝕨 must have rank 1".into()));
            }
            let picked = pick_elem(inner, x)?;
            return Ok(tag_arr(BqnArr {
                shape: vec![],
                data: ArrData::Boxed(vec![picked]),
                fill: Some(crate::structural::prototype_of(picked)),
            }));
        }

        if idx_arr.el_type() == ElType::B {
            // rank-1+ boxed: iterate elements, each picks from x independently
            let ia = idx_arr.ia();
            if ia == 0 {
                // Empty boxed list ⟨⟩⊑x:
                // - if x is atom: return x (identity)
                // - if x is rank-0 enclosed: unwrap and return content
                // - otherwise: error
                if x.is_atom() { return Ok(x); }
                let xarr = get_arr(x).ok_or_else(|| BqnError::Type("⊑: 𝕩 must be array".into()))?;
                if xarr.rank() == 0 && xarr.ia() > 0 {
                    return Ok(xarr.get(0)?);
                }
                if xarr.rank() == 0 && xarr.ia() == 0 {
                    return Ok(x); // empty enclosed unit
                }
                return Err(BqnError::Domain("𝕨⊑𝕩: 𝕩 must be a unit if 𝕨 contains an empty array".into()));
            }
            let mut elems = Vec::with_capacity(ia);
            for i in 0..ia {
                let step = idx_arr.get(i)?;
                elems.push(pick_elem(step, x)?);
            }
            let out = typed_arr_from_b_vec(elems, idx_arr.shape.clone(), None);
            return Ok(tag_arr(out));
        }

        // Numeric array: multi-axis index
        if x.is_atom() { return Ok(x); }
        let arr = get_arr(x).ok_or_else(|| BqnError::Type("𝕨⊑𝕩: 𝕩 must be array".into()))?;
        let indices = idx_arr.i32_iter()?;
        if indices.len() != arr.rank() as usize {
            return Err(BqnError::Rank(format!(
                "𝕨⊑𝕩: Picking item at wrong rank (index {:?} in array of shape {:?})",
                indices, arr.shape
            )));
        }
        let mut flat_idx = 0usize;
        let mut stride = 1usize;
        for j in (0..indices.len()).rev() {
            let pos = resolve_index(indices[j], arr.shape[j])?;
            flat_idx += pos * stride;
            stride *= arr.shape[j];
        }
        return Ok(arr.get(flat_idx)?);
    }

    Err(BqnError::Type("𝕨⊑𝕩: index must be a number or array".into()))
}

/// Deep pick helper: given an index value and a target, pick one element.
/// Handles: scalar index (pick from rank-1), rank-1 int list (multi-axis),
/// rank-0 boxed (unbox and recurse), rank-1 boxed (path walk).
fn deep_pick_one(idx: B, target: B) -> Result<B> {
    if idx.is_f64() {
        // Scalar index: pick from first axis
        if target.is_atom() {
            return Ok(target);
        }
        let arr = get_arr(target)
            .ok_or_else(|| BqnError::Type("𝕨⊑𝕩: 𝕩 must be an array".into()))?;
        let i = validate_integer_index(idx, "𝕨⊑𝕩")?;
        let resolved = resolve_index(i, if arr.rank() <= 1 { arr.ia() } else { arr.shape[0] })?;
        if arr.rank() <= 1 {
            return Ok(arr.get(resolved)?);
        }
        // Multi-dimensional: return cell
        let cell_shape = &arr.shape[1..];
        let cell_size: usize = cell_shape.iter().product::<usize>().max(1);
        let mut result = Vec::with_capacity(cell_size);
        for j in 0..cell_size {
            result.push(arr.get(resolved * cell_size + j)?);
        }
        let out = typed_arr_from_b_vec(result, cell_shape.to_vec(), arr.fill);
        Ok(tag_arr(out))
    } else if idx.is_arr() {
        let idx_arr = get_arr(idx)
            .ok_or_else(|| BqnError::Type("𝕨⊑𝕩: index not found".into()))?;
        if idx_arr.rank() == 0 && idx_arr.el_type() == ElType::B {
            // Rank-0 boxed: unbox and recurse
            let inner = idx_arr.get(0)?;
            return deep_pick_one(inner, target);
        }
        if target.is_atom() {
            return Ok(target);
        }
        let arr = get_arr(target)
            .ok_or_else(|| BqnError::Type("𝕨⊑𝕩: 𝕩 must be an array".into()))?;
        if idx_arr.el_type() != ElType::B {
            // Rank-1 numeric: multi-axis pick
            let indices = idx_arr.i32_iter()?;
            if indices.len() != arr.rank() as usize {
                return Err(BqnError::Rank("𝕨⊑𝕩: index length must equal rank of 𝕩".into()));
            }
            let mut flat_idx = 0usize;
            let mut stride = 1usize;
            for j in (0..indices.len()).rev() {
                let resolved = resolve_index(indices[j], arr.shape[j])?;
                flat_idx += resolved * stride;
                stride *= arr.shape[j];
            }
            return Ok(arr.get(flat_idx)?);
        }
        // Rank-1 boxed: path walk
        let wia = idx_arr.ia();
        let mut current = target;
        for i in 0..wia {
            let step = idx_arr.get(i)?;
            current = deep_pick_one(step, current)?;
        }
        Ok(current)
    } else {
        Err(BqnError::Type("𝕨⊑𝕩: index must be number or array".into()))
    }
}

/// Deep pick entry: used when w is rank-0 boxed
fn deep_pick(w: B, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let result = deep_pick_one(w, x)?;
    Ok(PrimResult::Scalar(result))
}
