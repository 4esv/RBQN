use rbqn_core::*;
use rbqn_core::array::typed_arr_from_b_vec;
use crate::dispatch::PrimResult;

// ⊏ monad: first cell
pub fn first_cell_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("⊏𝕩: 𝕩 must be an array".into()))?;
    if arr.ia() == 0 {
        return Err(BqnError::Domain("⊏𝕩: 𝕩 is empty".into()));
    }
    let v = arr.get(0)?;
    Ok(PrimResult::Scalar(v))
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

// ⊏ dyad: select
pub fn select_c2(w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨⊏𝕩: 𝕩 must be an array".into()))?;
    // NOTE: BQN ⊏ selects along the first axis
    let first_dim = if arr.shape.is_empty() { arr.ia() } else { arr.shape[0] };
    let cell_shape: &[usize] = if arr.shape.len() > 1 { &arr.shape[1..] } else { &[] };
    let cell_size: usize = cell_shape.iter().product();

    if w.is_f64() {
        let idx = resolve_index(w.o2i(), first_dim)?;
        if arr.rank() <= 1 {
            return Ok(PrimResult::Scalar(arr.get(idx)?));
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
    if arr.ia() == 0 {
        return match arr.fill {
            Some(f) => Ok(PrimResult::Scalar(f)),
            None => Err(BqnError::Domain("⊑𝕩: 𝕩 is empty with no fill".into())),
        };
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
        let idx = resolve_index(w.o2i(), arr.ia())?;
        return Ok(PrimResult::Scalar(arr.get(idx)?));
    }

    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⊑𝕩: 𝕨 must be a number or array".into()))?;
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
