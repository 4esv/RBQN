use rbqn_core::*;
use crate::dispatch::PrimResult;

// ⊔ monad: group indices
pub fn group_indices_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("⊔𝕩: 𝕩 must be an array".into()))?;
    let indices = arr.i32_iter()?;

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
        .map(|g| {
            // Each group becomes an array; encode as sentinel for now
            let _ = BqnArr::new_vec_i32(g);
            B::SENTINEL
        })
        .collect();
    Ok(PrimResult::Array(BqnArr::new_vec_b(result)))
}

// ⊔ dyad: group
pub fn group_c2(_w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⊔𝕩: 𝕨 must be an array".into()))?;
    let xarr = xa.ok_or_else(|| BqnError::Type("𝕨⊔𝕩: 𝕩 must be an array".into()))?;

    let indices = warr.i32_iter()?;
    if indices.len() != xarr.ia() {
        return Err(BqnError::Shape(format!(
            "𝕨⊔𝕩: 𝕨 must have same length as 𝕩 ({} vs {})",
            indices.len(),
            xarr.ia()
        )));
    }

    let max_idx = indices.iter().copied().max().unwrap_or(-1);
    let n = (max_idx + 1).max(0) as usize;

    let mut groups: Vec<Vec<B>> = vec![vec![]; n];
    for (i, &g) in indices.iter().enumerate() {
        if g >= 0 {
            groups[g as usize].push(xarr.get(i)?);
        }
    }

    let result: Vec<B> = groups
        .into_iter()
        .map(|_g| {
            // Each group is a nested array
            B::SENTINEL
        })
        .collect();
    Ok(PrimResult::Array(BqnArr::new_vec_b(result)))
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
pub fn group_ord(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("•GroupOrd: 𝕩 must be an array".into()))?;
    let ia = arr.ia();
    if ia < 2 {
        return Err(BqnError::Domain("•GroupOrd: Expected ⟨lengths, indices⟩".into()));
    }
    // Stub: this needs both group lengths and indices
    let _ = arr;
    Err(error::throw_nyi("•GroupOrd not yet implemented"))
}
