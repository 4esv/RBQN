use rbqn_core::*;
use crate::dispatch::PrimResult;

// Helper: tag_arr convenience (delegates to rbqn_core::tag_arr)
fn box_arr(arr: BqnArr) -> B {
    tag_arr(arr)
}

// Helper: convert Vec<B> to the most specific typed array (numeric, char, or boxed).
// Use this anywhere elements are collected via arr.get(i) to preserve element types.
fn typed_arr(elems: Vec<B>, shape: Vec<usize>, fill: Option<B>) -> BqnArr {
    rbqn_core::array::typed_arr_from_b_vec(elems, shape, fill)
}

// = monad: rank
pub fn rank_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Ok(PrimResult::Scalar(B::m_i32(0)));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("=𝕩: 𝕩 must be an array".into()))?;
    Ok(PrimResult::Scalar(B::m_i32(arr.rank() as i32)))
}

// ≠ monad: length (first axis)
pub fn length_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Ok(PrimResult::Scalar(B::m_i32(1)));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("≠𝕩: 𝕩 must be an array".into()))?;
    let len = if arr.shape.is_empty() { 1 } else { arr.shape[0] };
    Ok(PrimResult::Scalar(B::m_f64(len as f64)))
}

// ≢ monad: shape
pub fn shape_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Ok(PrimResult::Array(BqnArr::new_vec_i32(vec![])));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("≢𝕩: 𝕩 must be an array".into()))?;
    let shape: Vec<i32> = arr.shape.iter().map(|&s| s as i32).collect();
    Ok(PrimResult::Array(BqnArr::new_vec_i32(shape)))
}

// ≡ monad: depth
pub fn depth_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    fn compute_depth(x: B, xa: Option<&BqnArr>) -> i32 {
        if x.is_atom() && xa.is_none() {
            return 0;
        }
        match xa {
            Some(arr) => {
                if arr.el_type() != ElType::B {
                    return 1;
                }
                let mut max_d = 0i32;
                let ia = arr.ia();
                for i in 0..ia {
                    if let Ok(v) = arr.get(i) {
                        let d = compute_depth(v, None);
                        max_d = max_d.max(d);
                    }
                }
                max_d + 1
            }
            None => 0,
        }
    }
    Ok(PrimResult::Scalar(B::m_i32(compute_depth(x, xa))))
}

// < monad: enclose
pub fn enclose_c1(x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    Ok(PrimResult::Array(BqnArr {
        shape: vec![],
        data: ArrData::Boxed(vec![x]),
        fill: None,
    }))
}

// > monad: merge
// Takes an array of arrays (all same shape), produces a single higher-rank array.
// Result shape = outer_shape ++ inner_shape.
// For atom elements: result shape = outer_shape.
pub fn merge_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Ok(PrimResult::Scalar(x));
    }
    let arr = xa.ok_or_else(|| BqnError::Type(">𝕩: 𝕩 must be an array".into()))?;
    let ia = arr.ia();

    if ia == 0 {
        // Empty merge: keep outer shape, inner shape is unknown so just return as-is
        return Ok(PrimResult::Array(arr.clone()));
    }

    // Non-boxed arrays: all elements are atoms, result is just the array as-is
    if arr.el_type() != ElType::B {
        return Ok(PrimResult::Array(arr.clone()));
    }

    // Check first element to determine inner shape
    let first = arr.get(0)?;
    if first.is_atom() {
        // All elements are atoms, result shape = outer_shape, data is the same
        return Ok(PrimResult::Array(arr.clone()));
    }

    // First element is an array — get its shape
    let first_inner = get_arr(first)
        .ok_or_else(|| BqnError::Type(">𝕩: element is tagged as array but not found in store".into()))?;
    let inner_shape = first_inner.shape.clone();
    let inner_size: usize = inner_shape.iter().product::<usize>().max(1);

    // Collect all elements, checking shape consistency
    let mut result_data = Vec::with_capacity(ia * inner_size);
    for i in 0..ia {
        let elem = arr.get(i)?;
        if elem.is_atom() {
            return Err(BqnError::Shape(">𝕩: element shapes don't match".into()));
        }
        let elem_arr = get_arr(elem)
            .ok_or_else(|| BqnError::Type(">𝕩: element is tagged as array but not found in store".into()))?;
        if elem_arr.shape != inner_shape {
            return Err(BqnError::Shape(format!(
                ">𝕩: element shapes don't match ({:?} vs {:?})",
                elem_arr.shape, inner_shape
            )));
        }
        for j in 0..inner_size {
            result_data.push(elem_arr.get(j)?);
        }
    }

    let mut new_shape = arr.shape.clone();
    new_shape.extend_from_slice(&inner_shape);

    Ok(PrimResult::Array(typed_arr(result_data, new_shape, first_inner.fill)))
}

// ⊣ monad/dyad: identity / left
pub fn identity_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    match xa {
        Some(arr) => Ok(PrimResult::Array(arr.clone())),
        None => Ok(PrimResult::Scalar(x)),
    }
}

pub fn ltack_c2(w: B, wa: Option<&BqnArr>, _x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    match wa {
        Some(arr) => Ok(PrimResult::Array(arr.clone())),
        None => Ok(PrimResult::Scalar(w)),
    }
}

pub fn rtack_c2(_w: B, _wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    match xa {
        Some(arr) => Ok(PrimResult::Array(arr.clone())),
        None => Ok(PrimResult::Scalar(x)),
    }
}

// ⥊ monad: deshape (flatten to list)
pub fn deshape_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Ok(PrimResult::Array(typed_arr(vec![x], vec![1], None)));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("⥊𝕩: 𝕩 must be an array".into()))?;
    let ia = arr.ia();
    let mut out = arr.clone();
    out.shape = vec![ia];
    Ok(PrimResult::Array(out))
}

// ⥊ dyad: reshape
pub fn reshape_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let new_shape = if w.is_f64() {
        vec![w.to_usz()?]
    } else {
        let warr = wa.ok_or_else(|| BqnError::Type("𝕨⥊𝕩: 𝕨 must be a number or array of numbers".into()))?;
        // BQN spec: shape can contain ∘ (exact), ⌊ (floor), or ⌈ (ceil) to compute a dimension
        if warr.is_num_arr() {
            warr.i32_iter()?.iter().map(|&s| s as usize).collect()
        } else {
            // Shape contains non-numeric element(s) — handle computed dimension
            return reshape_computed(warr, x, xa);
        }
    };
    let new_ia: usize = new_shape.iter().product();

    if x.is_atom() {
        let vals = vec![x.o2f(); new_ia];
        let mut out = BqnArr::new_vec_f64(vals);
        out.shape = new_shape;
        return Ok(PrimResult::Array(array::squeeze_num(out)));
    }

    let arr = xa.ok_or_else(|| BqnError::Type("𝕨⥊𝕩: 𝕩 must be an array".into()))?;
    let old_ia = arr.ia();
    if old_ia == 0 {
        return Err(BqnError::Domain("𝕨⥊𝕩: 𝕩 can't be empty".into()));
    }

    let mut result = Vec::with_capacity(new_ia);
    for i in 0..new_ia {
        result.push(arr.get(i % old_ia)?);
    }
    Ok(PrimResult::Array(typed_arr(result, new_shape, arr.fill)))
}

/// Handle reshape with computed dimension (shape contains ∘, ⌊, or ⌈).
/// BQN spec: at most one element in shape can be a non-number.
/// ∘ = exact division, ⌊ = floor, ⌈ = ceil.
/// We identify the mode by the B value's type tag:
///   - MD2 (tag 0xfff3) → ∘ (atop) → exact division
///   - FUN → ⌊ or ⌈ → floor or ceil (default floor)
/// For robustness, any non-numeric value defaults to exact (∘) behavior.
fn reshape_computed(warr: &BqnArr, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let ia = warr.ia();
    let mut computed_idx: Option<usize> = None;
    let mut computed_mode = 0u8; // 0=exact(∘), 1=floor(⌊), 2=ceil(⌈)
    let mut known_dims: Vec<usize> = Vec::with_capacity(ia);

    for i in 0..ia {
        let v = warr.get(i)?;
        if v.is_f64() {
            known_dims.push(v.to_usz()?);
        } else {
            // Non-numeric element: this is the computed dimension
            if computed_idx.is_some() {
                return Err(BqnError::Domain("𝕨⥊𝕩: 𝕨 can have at most one computed dimension".into()));
            }
            computed_idx = Some(i);
            known_dims.push(0); // placeholder

            // NOTE: Determine mode. FUN tag with prim_idx 6=⌊, 7=⌈.
            // MD2 tag = ∘ (exact). Default to exact for any other non-numeric.
            if v.is_fun() {
                // FUN value in shape: could be ⌊ or ⌈
                // TODO: proper primitive identification for ⌊ vs ⌈
                computed_mode = 1; // assume floor for now
            }
            // MD2 → exact (∘), which is the default (0)
        }
    }

    let ci = computed_idx
        .ok_or_else(|| BqnError::Domain("𝕨⥊𝕩: shape has no computed dimension but contains non-numeric".into()))?;

    // Get total element count from x
    let total = if x.is_atom() {
        1
    } else {
        let xarr = xa.ok_or_else(|| BqnError::Type("𝕨⥊𝕩: 𝕩 must be an array".into()))?;
        xarr.ia()
    };

    // Product of known dimensions
    let known_product: usize = known_dims.iter().enumerate()
        .filter(|&(j, _)| j != ci)
        .map(|(_, &d)| d)
        .product();

    if known_product == 0 {
        return Err(BqnError::Domain("𝕨⥊𝕩: known dimensions product is 0".into()));
    }

    // Compute the missing dimension
    let computed_dim = match computed_mode {
        0 => { // exact (∘): must divide evenly
            if total % known_product != 0 {
                return Err(BqnError::Domain(format!(
                    "𝕨⥊𝕩: 𝕩 length ({}) not divisible by known shape product ({})",
                    total, known_product
                )));
            }
            total / known_product
        }
        1 => { // floor (⌊)
            total / known_product
        }
        2 => { // ceil (⌈)
            (total + known_product - 1) / known_product
        }
        _ => unreachable!(),
    };

    known_dims[ci] = computed_dim;
    let new_shape = known_dims;
    let new_ia: usize = new_shape.iter().product();

    if x.is_atom() {
        let vals = vec![x.o2f(); new_ia];
        let mut out = BqnArr::new_vec_f64(vals);
        out.shape = new_shape;
        return Ok(PrimResult::Array(array::squeeze_num(out)));
    }

    let arr = xa.ok_or_else(|| BqnError::Type("𝕨⥊𝕩: 𝕩 must be an array".into()))?;
    let old_ia = arr.ia();
    if old_ia == 0 {
        return Err(BqnError::Domain("𝕨⥊𝕩: 𝕩 can't be empty".into()));
    }

    let mut result = Vec::with_capacity(new_ia);
    for i in 0..new_ia {
        result.push(arr.get(i % old_ia)?);
    }
    Ok(PrimResult::Array(typed_arr(result, new_shape, arr.fill)))
}

// ∾ monad: join (flatten one level of nesting)
// For a list of arrays, concatenates all along first axis.
// For a list of atoms, wraps into a flat list.
pub fn join_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Err(BqnError::Type("∾𝕩: 𝕩 must be an array".into()));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("∾𝕩: 𝕩 must be an array".into()))?;

    if arr.rank() != 1 {
        return Err(BqnError::Nyi("∾: rank>1 not yet implemented".into()));
    }

    let outer_len = arr.ia();
    if outer_len == 0 {
        return Ok(PrimResult::Array(BqnArr::empty_vec()));
    }

    // Non-boxed arrays can't contain sub-arrays; they're lists of atoms
    if arr.el_type() != ElType::B {
        // Already a flat array of atoms — just return as-is
        return Ok(PrimResult::Array(arr.clone()));
    }

    // Collect all elements from sub-arrays
    let mut result = Vec::new();
    let mut inner_tail_shape: Option<Vec<usize>> = None;
    let mut total_first = 0usize;

    for i in 0..outer_len {
        let elem = arr.get(i)?;
        if elem.is_atom() {
            // Atom in list: treat as single element
            if let Some(ref ts) = inner_tail_shape {
                if !ts.is_empty() {
                    return Err(BqnError::Shape("∾𝕩: incompatible element shapes".into()));
                }
            } else {
                inner_tail_shape = Some(vec![]);
            }
            result.push(elem);
            total_first += 1;
        } else {
            let sub = get_arr(elem)
                .ok_or_else(|| BqnError::Type("∾𝕩: element is tagged as array but not found".into()))?;
            let sub_tail = sub.shape[1..].to_vec();
            if let Some(ref ts) = inner_tail_shape {
                if *ts != sub_tail {
                    return Err(BqnError::Shape("∾𝕩: incompatible trailing shapes".into()));
                }
            } else {
                inner_tail_shape = Some(sub_tail);
            }
            let first_dim = if sub.shape.is_empty() { 1 } else { sub.shape[0] };
            total_first += first_dim;
            let sub_ia = sub.ia();
            for j in 0..sub_ia {
                result.push(sub.get(j)?);
            }
        }
    }

    let tail = inner_tail_shape.unwrap_or_default();
    let mut new_shape = vec![total_first];
    new_shape.extend_from_slice(&tail);

    Ok(PrimResult::Array(typed_arr(result, new_shape, arr.fill)))
}

// ∾ dyad: join to
// BQN rank rules for dyadic ∾:
//   - Equal ranks: concatenate along first axis (trailing shapes must match)
//   - Rank differs by 1: lower-rank arg treated as single cell (prepend 1 to its shape)
//   - Atom + array: atom treated as rank-0 cell
//   - Rank differs by >1: error
pub fn join_to_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    match (wa, xa) {
        (Some(warr), Some(xarr)) => {
            let wr = warr.rank();
            let xr = xarr.rank();

            // Both rank 1: simple vector concatenation
            if wr <= 1 && xr <= 1 {
                let wia = warr.ia();
                let xia = xarr.ia();
                let mut result = Vec::with_capacity(wia + xia);
                for i in 0..wia {
                    result.push(warr.get(i)?);
                }
                for i in 0..xia {
                    result.push(xarr.get(i)?);
                }
                let new_len = wia + xia;
                return Ok(PrimResult::Array(typed_arr(result, vec![new_len], warr.fill)));
            }

            // Multi-rank join along first axis
            // Determine effective shapes after rank promotion
            let (w_shape, x_shape) = if wr == xr {
                // Equal ranks: use shapes as-is
                (warr.shape.clone(), xarr.shape.clone())
            } else if wr == xr + 1 {
                // 𝕨 has one more rank: promote 𝕩 by prepending 1
                let mut xs = vec![1usize];
                xs.extend_from_slice(&xarr.shape);
                (warr.shape.clone(), xs)
            } else if xr == wr + 1 {
                // 𝕩 has one more rank: promote 𝕨 by prepending 1
                let mut ws = vec![1usize];
                ws.extend_from_slice(&warr.shape);
                (ws, xarr.shape.clone())
            } else {
                return Err(BqnError::Rank(format!(
                    "𝕨∾𝕩: rank difference too large ({} vs {})",
                    wr, xr
                )));
            };

            // Check trailing shapes match
            if w_shape[1..] != x_shape[1..] {
                return Err(BqnError::Shape(format!(
                    "𝕨∾𝕩: trailing shapes don't match ({:?} vs {:?})",
                    &w_shape[1..], &x_shape[1..]
                )));
            }

            let mut new_shape = vec![w_shape[0] + x_shape[0]];
            new_shape.extend_from_slice(&w_shape[1..]);
            let wia = warr.ia();
            let xia = xarr.ia();
            let mut result = Vec::with_capacity(wia + xia);
            for i in 0..wia {
                result.push(warr.get(i)?);
            }
            for i in 0..xia {
                result.push(xarr.get(i)?);
            }
            Ok(PrimResult::Array(typed_arr(result, new_shape, warr.fill)))
        }
        (None, Some(xarr)) => {
            // Atom ∾ array: atom is rank 0, array is rank r
            // If r == 1: prepend atom to vector
            // If r == 1 (after considering rank-0 as cell of rank-1): same as above
            // For higher rank: rank 0 can only join rank 1 (differs by 1)
            if xarr.rank() > 1 {
                return Err(BqnError::Rank(format!(
                    "𝕨∾𝕩: rank difference too large (0 vs {})",
                    xarr.rank()
                )));
            }
            let xia = xarr.ia();
            let mut result = Vec::with_capacity(1 + xia);
            result.push(w);
            for i in 0..xia {
                result.push(xarr.get(i)?);
            }
            let len = 1 + xia;
            Ok(PrimResult::Array(typed_arr(result, vec![len], xarr.fill)))
        }
        (Some(warr), None) => {
            // Array ∾ atom: same logic, atom is rank 0
            if warr.rank() > 1 {
                return Err(BqnError::Rank(format!(
                    "𝕨∾𝕩: rank difference too large ({} vs 0)",
                    warr.rank()
                )));
            }
            let wia = warr.ia();
            let mut result = Vec::with_capacity(wia + 1);
            for i in 0..wia {
                result.push(warr.get(i)?);
            }
            result.push(x);
            let len = wia + 1;
            Ok(PrimResult::Array(typed_arr(result, vec![len], warr.fill)))
        }
        (None, None) => Err(BqnError::Type("𝕨∾𝕩: Arguments must include an array".into())),
    }
}

// ≍ monad: solo (wrap in 1-element list)
pub fn solo_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    match xa {
        Some(arr) => {
            let mut new_shape = vec![1];
            new_shape.extend_from_slice(&arr.shape);
            let mut out = arr.clone();
            out.shape = new_shape;
            Ok(PrimResult::Array(out))
        }
        None => Ok(PrimResult::Array(typed_arr(vec![x], vec![1], None))),
    }
}

// ≍ dyad: couple
// In BQN, couple adds a leading axis of length 2.
// Each argument is treated as a cell. Atoms (rank 0) are valid cells;
// they get broadcast (replicated) to match the other argument's shape.
pub fn couple_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    match (wa, xa) {
        // atom ≍ atom → 2-element list
        (None, None) => {
            if w.is_f64() && x.is_f64() {
                Ok(PrimResult::Array(BqnArr::new_vec_f64(vec![w.o2f(), x.o2f()])))
            } else {
                Ok(PrimResult::Array(BqnArr::new_vec_b(vec![w, x])))
            }
        }
        // array ≍ array → shapes must match
        (Some(warr), Some(xarr)) => {
            if warr.shape != xarr.shape {
                return Err(BqnError::Shape(format!(
                    "𝕨≍𝕩: 𝕨 and 𝕩 must have equal shapes ({:?} ≡ ≢𝕨, {:?} ≡ ≢𝕩)",
                    warr.shape, xarr.shape
                )));
            }
            let wia = warr.ia();
            let xia = xarr.ia();
            let mut result = Vec::with_capacity(wia + xia);
            for i in 0..wia {
                result.push(warr.get(i)?);
            }
            for i in 0..xia {
                result.push(xarr.get(i)?);
            }
            let mut new_shape = vec![2];
            new_shape.extend_from_slice(&warr.shape);
            Ok(PrimResult::Array(typed_arr(result, new_shape, warr.fill)))
        }
        // atom ≍ array → broadcast atom to match array shape, then couple
        (None, Some(xarr)) => {
            let xia = xarr.ia();
            let mut result = Vec::with_capacity(xia + xia);
            // First row: atom broadcast to fill xarr.shape
            for _ in 0..xia {
                result.push(w);
            }
            // Second row: array elements
            for i in 0..xia {
                result.push(xarr.get(i)?);
            }
            let mut new_shape = vec![2];
            new_shape.extend_from_slice(&xarr.shape);
            Ok(PrimResult::Array(typed_arr(result, new_shape, xarr.fill)))
        }
        // array ≍ atom → broadcast atom to match array shape, then couple
        (Some(warr), None) => {
            let wia = warr.ia();
            let mut result = Vec::with_capacity(wia + wia);
            // First row: array elements
            for i in 0..wia {
                result.push(warr.get(i)?);
            }
            // Second row: atom broadcast to fill warr.shape
            for _ in 0..wia {
                result.push(x);
            }
            let mut new_shape = vec![2];
            new_shape.extend_from_slice(&warr.shape);
            Ok(PrimResult::Array(typed_arr(result, new_shape, warr.fill)))
        }
    }
}

// ⋈ dyad: pair
pub fn pair_c2(w: B, _wa: Option<&BqnArr>, x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    Ok(PrimResult::Array(typed_arr(vec![w, x], vec![2], None)))
}

// ↑ monad: prefixes
// Returns array of all prefixes: ↑ "abc" → ⟨""‿"a"‿"ab"‿"abc"⟩
pub fn prefixes_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Err(BqnError::Type("↑𝕩: 𝕩 must be an array".into()));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("↑𝕩: 𝕩 must be an array".into()))?;

    let first_dim = if arr.shape.is_empty() { 1 } else { arr.shape[0] };
    let cell_shape = &arr.shape[1..];
    let cell_size: usize = cell_shape.iter().product::<usize>().max(1);

    let mut prefixes = Vec::with_capacity(first_dim + 1);
    for i in 0..=first_dim {
        // Prefix i: first i major cells
        let n_elems = i * cell_size;
        let mut data = Vec::with_capacity(n_elems);
        for j in 0..n_elems {
            data.push(arr.get(j)?);
        }
        let mut prefix_shape = vec![i];
        prefix_shape.extend_from_slice(cell_shape);
        prefixes.push(box_arr(BqnArr {
            shape: prefix_shape,
            data: ArrData::Boxed(data),
            fill: arr.fill,
        }));
    }

    Ok(PrimResult::Array(BqnArr::new_vec_b(prefixes)))
}

// ↑ dyad: take
pub fn take_c2(w: B, _wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let n = w.to_i32()?;
    let arr = if x.is_atom() {
        typed_arr(vec![x], vec![1], None)
    } else {
        xa.ok_or_else(|| BqnError::Type("𝕨↑𝕩: 𝕩 must be an array".into()))?.clone()
    };

    let first_dim = if arr.shape.is_empty() { 1 } else { arr.shape[0] };

    if arr.rank() <= 1 {
        // Vector take
        let ia = arr.ia() as i32;
        let (start, len) = if n >= 0 {
            (0, n.min(ia) as usize)
        } else {
            let s = (ia + n).max(0);
            (s as usize, (ia - s) as usize)
        };

        let mut result = Vec::with_capacity(n.unsigned_abs() as usize);
        let take_len = n.unsigned_abs() as usize;
        for i in 0..take_len {
            let idx = start + i;
            if idx < len + start && idx < arr.ia() {
                result.push(arr.get(idx)?);
            } else {
                result.push(arr.fill.unwrap_or(B::m_i32(0)));
            }
        }
        let take_len = n.unsigned_abs() as usize;
        return Ok(PrimResult::Array(typed_arr(result, vec![take_len], arr.fill)));
    }

    // Multi-rank take: operates along first axis
    let cell_shape = &arr.shape[1..];
    let cell_size: usize = cell_shape.iter().product::<usize>().max(1);
    let abs_n = n.unsigned_abs() as usize;
    let fill_val = arr.fill.unwrap_or(B::m_i32(0));

    let mut result = Vec::with_capacity(abs_n * cell_size);
    if n >= 0 {
        for i in 0..abs_n {
            if i < first_dim {
                for j in 0..cell_size {
                    result.push(arr.get(i * cell_size + j)?);
                }
            } else {
                for _ in 0..cell_size {
                    result.push(fill_val);
                }
            }
        }
    } else {
        let start = (first_dim as i32 + n).max(0) as usize;
        for i in 0..abs_n {
            let src = start + i;
            if src < first_dim {
                for j in 0..cell_size {
                    result.push(arr.get(src * cell_size + j)?);
                }
            } else {
                for _ in 0..cell_size {
                    result.push(fill_val);
                }
            }
        }
    }

    let mut new_shape = vec![abs_n];
    new_shape.extend_from_slice(cell_shape);
    Ok(PrimResult::Array(typed_arr(result, new_shape, arr.fill)))
}

// ↓ monad: suffixes
// Returns array of all suffixes: ↓ "abc" → ⟨"abc"‿"bc"‿"c"‿""⟩
pub fn suffixes_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Err(BqnError::Type("↓𝕩: 𝕩 must be an array".into()));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("↓𝕩: 𝕩 must be an array".into()))?;

    let first_dim = if arr.shape.is_empty() { 1 } else { arr.shape[0] };
    let cell_shape = &arr.shape[1..];
    let cell_size: usize = cell_shape.iter().product::<usize>().max(1);

    let mut suffixes = Vec::with_capacity(first_dim + 1);
    for i in 0..=first_dim {
        // Suffix i: elements from cell i to end
        let remaining = first_dim - i;
        let start = i * cell_size;
        let n_elems = remaining * cell_size;
        let mut data = Vec::with_capacity(n_elems);
        for j in 0..n_elems {
            data.push(arr.get(start + j)?);
        }
        let mut suffix_shape = vec![remaining];
        suffix_shape.extend_from_slice(cell_shape);
        suffixes.push(box_arr(BqnArr {
            shape: suffix_shape,
            data: ArrData::Boxed(data),
            fill: arr.fill,
        }));
    }

    Ok(PrimResult::Array(BqnArr::new_vec_b(suffixes)))
}

// ↓ dyad: drop
pub fn drop_c2(w: B, _wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let n = w.to_i32()?;
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨↓𝕩: 𝕩 must be an array".into()))?;

    let first_dim = if arr.shape.is_empty() { 1 } else { arr.shape[0] };

    if arr.rank() <= 1 {
        let ia = arr.ia() as i32;
        let (start, end) = if n >= 0 {
            (n.min(ia) as usize, ia as usize)
        } else {
            (0, (ia + n).max(0) as usize)
        };

        let mut result = Vec::with_capacity(end.saturating_sub(start));
        for i in start..end {
            result.push(arr.get(i)?);
        }
        let len = result.len();
        return Ok(PrimResult::Array(typed_arr(result, vec![len], arr.fill)));
    }

    // Multi-rank drop: operates along first axis
    let cell_shape = &arr.shape[1..];
    let cell_size: usize = cell_shape.iter().product::<usize>().max(1);

    let fd = first_dim as i32;
    let (start, end) = if n >= 0 {
        (n.min(fd) as usize, first_dim)
    } else {
        (0, (fd + n).max(0) as usize)
    };

    let remaining = end - start;
    let mut result = Vec::with_capacity(remaining * cell_size);
    for i in start..end {
        for j in 0..cell_size {
            result.push(arr.get(i * cell_size + j)?);
        }
    }

    let mut new_shape = vec![remaining];
    new_shape.extend_from_slice(cell_shape);
    Ok(PrimResult::Array(typed_arr(result, new_shape, arr.fill)))
}

// ↕ monad: range
pub fn range_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_f64() {
        let n = x.to_usz()?;
        let vals: Vec<i32> = (0..n as i32).collect();
        return Ok(PrimResult::Array(BqnArr::new_vec_i32(vals)));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("↕𝕩: 𝕩 must be a number or array".into()))?;

    // Multi-dimensional range: ↕ s produces array of index lists
    let dims = arr.i32_iter()?;
    let total: usize = dims.iter().map(|&d| d as usize).product();
    let rank = dims.len();

    let mut result = Vec::with_capacity(total);
    for flat in 0..total {
        let mut idx_vals = vec![0i32; rank];
        let mut rem = flat;
        for r in (0..rank).rev() {
            let d = dims[r] as usize;
            idx_vals[r] = (rem % d) as i32;
            rem /= d;
        }
        result.push(box_arr(BqnArr::new_vec_i32(idx_vals)));
    }

    let out_shape: Vec<usize> = dims.iter().map(|&d| d as usize).collect();
    Ok(PrimResult::Array(BqnArr {
        shape: out_shape,
        data: ArrData::Boxed(result),
        fill: None,
    }))
}

// ↕ dyad: windows
// n↕x returns sliding windows of length n along x.
// Result has (len-n+1) windows, each of length n.
pub fn windows_c2(w: B, _wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let n = w.to_usz()?;
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨↕𝕩: 𝕩 must be an array".into()))?;

    let first_dim = if arr.shape.is_empty() { 1 } else { arr.shape[0] };

    // BQN: result first-dim = max(0, 1 + first_dim - n)
    // When n > first_dim, result has 0 windows (empty along first axis)
    let num_windows = if n > first_dim { 0 } else { first_dim - n + 1 };

    if num_windows == 0 {
        // Return empty array with correct shape: ⟨0, n, ...cell_shape⟩
        let mut out_shape = vec![0, n];
        if arr.shape.len() > 1 {
            out_shape.extend_from_slice(&arr.shape[1..]);
        }
        let out = BqnArr { shape: out_shape, data: ArrData::I32(vec![]), fill: arr.fill };
        return Ok(PrimResult::Array(out));
    }

    if arr.rank() <= 1 {
        // Vector windows: result is a flat numeric array with shape ⟨num_windows, n⟩
        let total = num_windows * n;
        let mut result = Vec::with_capacity(total);
        for start in 0..num_windows {
            for j in 0..n {
                result.push(arr.get(start + j)?);
            }
        }
        let out_shape = vec![num_windows, n];
        let out = typed_arr(result, out_shape, arr.fill);
        return Ok(PrimResult::Array(out));
    }

    // Multi-rank windows: each window is n major cells
    let cell_shape = &arr.shape[1..];
    let cell_size: usize = cell_shape.iter().product();

    let total = num_windows * n * cell_size;
    let mut result = Vec::with_capacity(total);
    for start in 0..num_windows {
        for row in start..start + n {
            for j in 0..cell_size {
                result.push(arr.get(row * cell_size + j)?);
            }
        }
    }
    let mut out_shape = vec![num_windows, n];
    out_shape.extend_from_slice(cell_shape);
    let out = typed_arr(result, out_shape, arr.fill);
    Ok(PrimResult::Array(out))
}

// « monad: shift after (shift left, fill from right with type fill)
// «⟨1,2,3⟩ → ⟨2,3,0⟩
pub fn shifta_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("«𝕩: 𝕩 must be an array".into()))?;
    let ia = arr.ia();
    if ia == 0 {
        return Ok(PrimResult::Array(arr.clone()));
    }
    let fill_val = arr.fill.unwrap_or(B::m_i32(0));

    if arr.rank() > 1 {
        let first_dim = arr.shape[0];
        let cell_size: usize = arr.shape[1..].iter().product::<usize>().max(1);
        let mut result = Vec::with_capacity(ia);
        // Copy cells [1..] from original
        for i in 1..first_dim {
            for j in 0..cell_size {
                result.push(arr.get(i * cell_size + j)?);
            }
        }
        // Fill last cell
        for _ in 0..cell_size {
            result.push(fill_val);
        }
        return Ok(PrimResult::Array(typed_arr(result, arr.shape.clone(), arr.fill)));
    }

    let mut result = Vec::with_capacity(ia);
    for i in 1..ia {
        result.push(arr.get(i)?);
    }
    result.push(fill_val);
    Ok(PrimResult::Array(typed_arr(result, arr.shape.clone(), arr.fill)))
}

// « dyad: shift after
pub fn shifta_c2(w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨«𝕩: 𝕩 must be an array".into()))?;

    if arr.rank() > 1 {
        // Multi-rank: shift major cells along first axis
        let first_dim = arr.shape[0];
        let cell_shape = &arr.shape[1..];
        let cell_size: usize = cell_shape.iter().product::<usize>().max(1);
        let fill_val = arr.fill.unwrap_or(B::m_i32(0));

        let shift = match wa {
            Some(warr) => {
                if warr.rank() == arr.rank() {
                    warr.shape[0]
                } else {
                    1
                }
            }
            None => 1,
        };

        let mut result = Vec::with_capacity(arr.ia());
        // Copy cells [shift..] from original
        for i in shift..first_dim {
            for j in 0..cell_size {
                result.push(arr.get(i * cell_size + j)?);
            }
        }
        // Fill remaining cells
        let fill_cells = shift.min(first_dim);
        for _ in 0..fill_cells * cell_size {
            result.push(fill_val);
        }

        return Ok(PrimResult::Array(typed_arr(result, arr.shape.clone(), arr.fill)));
    }

    let ia = arr.ia();
    let fill_vals = match wa {
        Some(warr) => {
            let wia = warr.ia();
            let mut v = Vec::with_capacity(wia);
            for i in 0..wia {
                v.push(warr.get(i)?);
            }
            v
        }
        None => vec![w],
    };
    let shift = fill_vals.len();
    let mut result = Vec::with_capacity(ia);
    for i in shift..ia {
        result.push(arr.get(i)?);
    }
    for v in fill_vals.iter().take(ia.saturating_sub(0)).skip(0) {
        result.push(*v);
    }
    while result.len() < ia {
        result.push(arr.fill.unwrap_or(B::m_i32(0)));
    }
    result.truncate(ia);
    Ok(PrimResult::Array(typed_arr(result, arr.shape.clone(), arr.fill)))
}

// » monad: shift before (shift right, fill from left with type fill)
// »⟨1,2,3⟩ → ⟨0,1,2⟩
pub fn shiftb_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("»𝕩: 𝕩 must be an array".into()))?;
    let ia = arr.ia();
    if ia == 0 {
        return Ok(PrimResult::Array(arr.clone()));
    }
    let fill_val = arr.fill.unwrap_or(B::m_i32(0));

    if arr.rank() > 1 {
        let first_dim = arr.shape[0];
        let cell_size: usize = arr.shape[1..].iter().product::<usize>().max(1);
        let mut result = Vec::with_capacity(ia);
        // Fill first cell
        for _ in 0..cell_size {
            result.push(fill_val);
        }
        // Copy cells [0..first_dim-1] from original
        for i in 0..first_dim - 1 {
            for j in 0..cell_size {
                result.push(arr.get(i * cell_size + j)?);
            }
        }
        return Ok(PrimResult::Array(typed_arr(result, arr.shape.clone(), arr.fill)));
    }

    let mut result = Vec::with_capacity(ia);
    result.push(fill_val);
    for i in 0..ia - 1 {
        result.push(arr.get(i)?);
    }
    Ok(PrimResult::Array(typed_arr(result, arr.shape.clone(), arr.fill)))
}

// » dyad: shift before
pub fn shiftb_c2(w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨»𝕩: 𝕩 must be an array".into()))?;

    if arr.rank() > 1 {
        // Multi-rank: shift major cells along first axis
        let first_dim = arr.shape[0];
        let cell_shape = &arr.shape[1..];
        let cell_size: usize = cell_shape.iter().product::<usize>().max(1);
        let fill_val = arr.fill.unwrap_or(B::m_i32(0));

        let shift = match wa {
            Some(warr) => {
                if warr.rank() == arr.rank() {
                    warr.shape[0]
                } else {
                    1
                }
            }
            None => 1,
        };

        let mut result = Vec::with_capacity(arr.ia());
        // Fill first cells
        let fill_cells = shift.min(first_dim);
        for _ in 0..fill_cells * cell_size {
            result.push(fill_val);
        }
        // Copy cells [0..first_dim-shift] from original
        let copy_end = first_dim.saturating_sub(shift);
        for i in 0..copy_end {
            for j in 0..cell_size {
                result.push(arr.get(i * cell_size + j)?);
            }
        }

        return Ok(PrimResult::Array(typed_arr(result, arr.shape.clone(), arr.fill)));
    }

    let ia = arr.ia();
    let fill_vals = match wa {
        Some(warr) => {
            let wia = warr.ia();
            let mut v = Vec::with_capacity(wia);
            for i in 0..wia {
                v.push(warr.get(i)?);
            }
            v
        }
        None => vec![w],
    };
    let shift = fill_vals.len();
    let mut result = Vec::with_capacity(ia);
    for v in fill_vals.iter().take(ia) {
        result.push(*v);
    }
    for i in 0..ia.saturating_sub(shift) {
        result.push(arr.get(i)?);
    }
    result.truncate(ia);
    Ok(PrimResult::Array(typed_arr(result, arr.shape.clone(), arr.fill)))
}

// ⌽ monad: reverse (along first axis)
pub fn reverse_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("⌽𝕩: 𝕩 must be an array".into()))?;

    if arr.rank() <= 1 {
        let ia = arr.ia();
        let mut result = Vec::with_capacity(ia);
        for i in (0..ia).rev() {
            result.push(arr.get(i)?);
        }
        return Ok(PrimResult::Array(typed_arr(result, vec![ia], arr.fill)));
    }

    // Multi-rank: reverse major cells
    let first_dim = arr.shape[0];
    let cell_size: usize = arr.shape[1..].iter().product::<usize>().max(1);
    let mut result = Vec::with_capacity(arr.ia());
    for i in (0..first_dim).rev() {
        for j in 0..cell_size {
            result.push(arr.get(i * cell_size + j)?);
        }
    }
    Ok(PrimResult::Array(typed_arr(result, arr.shape.clone(), arr.fill)))
}

// ⌽ dyad: rotate (along first axis)
pub fn rotate_c2(w: B, _wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let n = w.to_i32()?;
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨⌽𝕩: 𝕩 must be an array".into()))?;

    let first_dim = if arr.shape.is_empty() { 1 } else { arr.shape[0] };
    if first_dim == 0 {
        return Ok(PrimResult::Array(arr.clone()));
    }

    if arr.rank() <= 1 {
        let ia = arr.ia();
        let shift = ((n % ia as i32) + ia as i32) as usize % ia;
        let mut result = Vec::with_capacity(ia);
        for i in 0..ia {
            result.push(arr.get((i + shift) % ia)?);
        }
        return Ok(PrimResult::Array(typed_arr(result, vec![ia], arr.fill)));
    }

    // Multi-rank: rotate major cells
    let cell_size: usize = arr.shape[1..].iter().product::<usize>().max(1);
    let shift = ((n % first_dim as i32) + first_dim as i32) as usize % first_dim;
    let mut result = Vec::with_capacity(arr.ia());
    for i in 0..first_dim {
        let src = (i + shift) % first_dim;
        for j in 0..cell_size {
            result.push(arr.get(src * cell_size + j)?);
        }
    }
    Ok(PrimResult::Array(typed_arr(result, arr.shape.clone(), arr.fill)))
}

// ⍉ monad: transpose (reverse axis order)
// For rank ≤ 1: identity.
// For rank 2: swap rows/cols.
// For rank n: reverse all axes.
pub fn transpose_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return Ok(PrimResult::Scalar(x));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("⍉𝕩: 𝕩 must be an array".into()))?;

    if arr.rank() <= 1 {
        return Ok(PrimResult::Array(arr.clone()));
    }

    let rank = arr.rank() as usize;
    let old_shape = &arr.shape;
    let ia = arr.ia();

    // New shape is reversed
    let new_shape: Vec<usize> = old_shape.iter().rev().copied().collect();

    // Compute strides for original array
    let mut old_strides = vec![1usize; rank];
    for i in (0..rank - 1).rev() {
        old_strides[i] = old_strides[i + 1] * old_shape[i + 1];
    }

    // For transposition: new axis i corresponds to old axis (rank-1-i)
    // new_strides[i] = old_strides[rank-1-i]
    let mut result = vec![B::m_i32(0); ia];
    for flat in 0..ia {
        // Convert flat index to multi-dimensional index in new array
        let mut rem = flat;
        let mut old_flat = 0;
        for new_axis in 0..rank {
            let idx = rem / {
                let mut s = 1;
                for a in (new_axis + 1)..rank {
                    s *= new_shape[a];
                }
                s
            };
            rem %= {
                let mut s = 1;
                for a in (new_axis + 1)..rank {
                    s *= new_shape[a];
                }
                s
            };
            // new_axis corresponds to old_axis = rank - 1 - new_axis
            let old_axis = rank - 1 - new_axis;
            old_flat += idx * old_strides[old_axis];
        }
        result[flat] = arr.get(old_flat)?;
    }

    Ok(PrimResult::Array(typed_arr(result, new_shape, arr.fill)))
}

// ⍉ dyad: reorder axes
// p⍉x reorders axes of x according to permutation p.
// p[i] says where axis i of x ends up in the result.
pub fn reorder_c2(w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨⍉𝕩: 𝕩 must be an array".into()))?;
    let rank = arr.rank() as usize;

    let perm = if w.is_f64() {
        vec![w.to_i32()?]
    } else {
        let warr = wa.ok_or_else(|| BqnError::Type("𝕨⍉𝕩: 𝕨 must be a number or array".into()))?;
        warr.i32_iter()?
    };

    if perm.len() != rank {
        return Err(BqnError::Rank(format!(
            "𝕨⍉𝕩: 𝕨 length ({}) must equal rank of 𝕩 ({})",
            perm.len(),
            rank
        )));
    }

    let max_p = perm.iter().copied().max().unwrap_or(0);
    let new_rank = (max_p + 1) as usize;

    // Build new shape: for each new axis, take the min of all old axes mapped to it
    let mut new_shape = vec![usize::MAX; new_rank];
    for (old_axis, &p) in perm.iter().enumerate() {
        if p < 0 {
            return Err(BqnError::Domain("𝕨⍉𝕩: axis indices must be non-negative".into()));
        }
        let na = p as usize;
        new_shape[na] = new_shape[na].min(arr.shape[old_axis]);
    }

    // Replace any remaining MAX (shouldn't happen with valid input)
    for s in &mut new_shape {
        if *s == usize::MAX {
            *s = 0;
        }
    }

    let ia: usize = new_shape.iter().product();
    let old_shape = &arr.shape;

    // Compute strides for old array
    let mut old_strides = vec![1usize; rank];
    for i in (0..rank.saturating_sub(1)).rev() {
        old_strides[i] = old_strides[i + 1] * old_shape[i + 1];
    }

    // Compute strides for new array
    let mut new_strides = vec![1usize; new_rank];
    for i in (0..new_rank.saturating_sub(1)).rev() {
        new_strides[i] = new_strides[i + 1] * new_shape[i + 1];
    }

    let mut result = Vec::with_capacity(ia);
    for flat in 0..ia {
        // Decompose flat index into new multi-index
        let mut new_idx = vec![0usize; new_rank];
        let mut rem = flat;
        for a in 0..new_rank {
            new_idx[a] = rem / new_strides[a];
            rem %= new_strides[a];
        }

        // Map to old multi-index: old_axis[i] index = new_idx[perm[i]]
        let mut old_flat = 0;
        for (old_axis, &p) in perm.iter().enumerate() {
            old_flat += new_idx[p as usize] * old_strides[old_axis];
        }
        result.push(arr.get(old_flat)?);
    }

    Ok(PrimResult::Array(typed_arr(result, new_shape, arr.fill)))
}
