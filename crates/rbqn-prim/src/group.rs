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
