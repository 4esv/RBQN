use rbqn_core::*;
use rbqn_core::array::typed_arr_from_b_vec;
use crate::dispatch::PrimResult;

// / monad: indices
pub fn indices_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("/𝕩: 𝕩 must be an array".into()))?;
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

// /⁼ monad: inverse of indices — converts sorted index array back to counts
// /⁼ ⟨0,0,1,2,2,2⟩ → ⟨2,1,3⟩ (because / ⟨2,1,3⟩ = ⟨0,0,1,2,2,2⟩)
pub fn indices_inverse_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("/⁼𝕩: 𝕩 must be an array".into()))?;
    if arr.ia() == 0 {
        return Ok(PrimResult::Array(BqnArr::new_vec_i32(vec![])));
    }
    let indices = arr.i32_iter()?;
    let max_idx = *indices.last().unwrap();
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

// / dyad: replicate
pub fn replicate_c2(w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨/𝕩: 𝕩 must be an array".into()))?;

    let counts = if w.is_f64() {
        let n = w.to_i32()?;
        vec![n; arr.ia()]
    } else {
        let warr = wa.ok_or_else(|| BqnError::Type("𝕨/𝕩: 𝕨 must be a number or array".into()))?;
        let c = warr.i32_iter()?;
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
    Ok(PrimResult::Array(typed_arr_from_b_vec(result, vec![len], arr.fill)))
}
