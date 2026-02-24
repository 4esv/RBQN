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

    // NOTE: rank-0 𝕨 is not a valid index for ⊏
    if warr.rank() == 0 {
        return Err(BqnError::Rank("𝕨⊏𝕩: 𝕨 must be a number or rank≥1 array".into()));
    }

    // BQN select with non-numeric indices: recurse per-element for boxed arrays
    if warr.el_type() == ElType::B {
        let wia = warr.ia();
        let mut result = Vec::with_capacity(wia * cell_size.max(1));
        for i in 0..wia {
            let idx_b = warr.get(i)?;
            if idx_b.is_f64() {
                let idx = resolve_index(idx_b.o2i(), first_dim)?;
                if cell_shape.is_empty() && arr.rank() <= 1 {
                    result.push(arr.get(idx)?);
                } else {
                    for j in 0..cell_size {
                        result.push(arr.get(idx * cell_size + j)?);
                    }
                }
            } else if idx_b.is_arr() {
                let sub_arr = get_arr(idx_b)
                    .ok_or_else(|| BqnError::Type("𝕨⊏𝕩: index element not found".into()))?;
                let sub_indices = sub_arr.i32_iter()?;
                for &si in &sub_indices {
                    let idx = resolve_index(si, first_dim)?;
                    if cell_shape.is_empty() && arr.rank() <= 1 {
                        result.push(arr.get(idx)?);
                    } else {
                        for j in 0..cell_size {
                            result.push(arr.get(idx * cell_size + j)?);
                        }
                    }
                }
            } else {
                return Err(BqnError::Type("𝕨⊏𝕩: index must be number or array".into()));
            }
        }
        let mut out_shape = warr.shape.clone();
        out_shape.extend_from_slice(cell_shape);
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
pub fn pick_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Ok(PrimResult::Scalar(x));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨⊑𝕩: 𝕩 must be an array".into()))?;

    if w.is_f64() {
        // NOTE: BQN spec: scalar pick requires 𝕩 to be rank-1 (a list)
        if arr.rank() != 1 {
            return Err(BqnError::Rank(format!(
                "𝕨⊑𝕩: 𝕩 must be a list when 𝕨 is a number ({:?} ≡ ≢𝕩)", arr.shape
            )));
        }
        let wi = validate_integer_index(w, "𝕨⊑𝕩")?;
        let idx = resolve_index(wi, arr.ia())?;
        return Ok(PrimResult::Scalar(arr.get(idx)?));
    }

    // NOTE: w must be an array (not another scalar type)
    if !w.is_arr() {
        return Err(BqnError::Type("𝕨⊑𝕩: 𝕨 must be a number or array".into()));
    }

    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⊑𝕩: 𝕨 must be a number or array".into()))?;

    // NOTE: rank-0 w (enclosed index) is not valid for ⊑
    if warr.rank() == 0 {
        return Err(BqnError::Rank("𝕨⊑𝕩: 𝕨 must be a number or rank-1 list, not rank-0".into()));
    }

    // Multi-dimensional pick: w is a list of indices
    if warr.rank() == 1 {
        let indices = warr.i32_iter()?;
        if indices.len() != arr.rank() as usize {
            return Err(BqnError::Rank("𝕨⊑𝕩: 𝕨 must have length equal to rank of 𝕩".into()));
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

    // Nested pick: w is a higher-rank array or contains boxed index lists.
    // Each element of w (if boxed) is an index-list into x.
    // The result has the outer shape of w, with each element being the picked value.
    if warr.el_type() == ElType::B {
        let wia = warr.ia();
        let mut result = Vec::with_capacity(wia);
        for i in 0..wia {
            let idx_b = warr.get(i)?;
            if idx_b.is_f64() {
                // Simple integer index
                let idx = resolve_index(idx_b.o2i(), arr.ia())?;
                result.push(arr.get(idx)?);
            } else if idx_b.is_arr() {
                // Nested index list
                let idx_arr = get_arr(idx_b)
                    .ok_or_else(|| BqnError::Type("𝕨⊑𝕩: index element not found".into()))?;
                let indices = idx_arr.i32_iter()?;
                if indices.len() != arr.rank() as usize {
                    return Err(BqnError::Rank(
                        "𝕨⊑𝕩: index length must equal rank of 𝕩".into(),
                    ));
                }
                let mut flat_idx = 0usize;
                let mut stride = 1usize;
                for j in (0..indices.len()).rev() {
                    let idx = resolve_index(indices[j], arr.shape[j])?;
                    flat_idx += idx * stride;
                    stride *= arr.shape[j];
                }
                result.push(arr.get(flat_idx)?);
            } else {
                return Err(BqnError::Type("𝕨⊑𝕩: index must be number or array".into()));
            }
        }
        let out = typed_arr_from_b_vec(result, warr.shape.clone(), None);
        return Ok(PrimResult::Array(out));
    }

    Err(BqnError::Type("𝕨⊑𝕩: unsupported index type".into()))
}
