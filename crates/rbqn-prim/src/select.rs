use rbqn_core::*;
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
    let ia = arr.ia();

    if w.is_f64() {
        let idx = resolve_index(w.o2i(), ia)?;
        return Ok(PrimResult::Scalar(arr.get(idx)?));
    }

    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⊏𝕩: 𝕨 must be a number or array".into()))?;
    let indices = warr.i32_iter()?;
    let mut result = Vec::with_capacity(indices.len());
    for &i in &indices {
        let idx = resolve_index(i, ia)?;
        result.push(arr.get(idx)?);
    }
    let mut out = BqnArr::new_vec_b(result);
    out.shape = warr.shape.clone();
    out.fill = arr.fill;
    Ok(PrimResult::Array(out))
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

    Err(error::throw_nyi("⊑: nested pick not yet implemented"))
}
