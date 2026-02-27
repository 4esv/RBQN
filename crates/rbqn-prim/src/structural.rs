use rbqn_core::*;
use crate::dispatch::PrimResult;

// NOTE: Primitive B values stored after bootstrap so reshape_computed can identify reshape modes.
// In reshape, a non-numeric element selects the computed dimension mode:
//   ∘ (MD2) = exact, ⌊ = floor, ⌽ = ceil+cycle, ↑ = ceil+pad.
static FLOOR_PRIM_B: std::sync::OnceLock<B> = std::sync::OnceLock::new();
static TAKE_PRIM_B: std::sync::OnceLock<B> = std::sync::OnceLock::new();

/// Set the B value for the ⌊ (floor) primitive.
pub fn set_floor_prim(b: B) {
    let _ = FLOOR_PRIM_B.set(b);
}

/// Set the B value for the ↑ (take) primitive.
pub fn set_take_prim(b: B) {
    let _ = TAKE_PRIM_B.set(b);
}

fn is_floor_prim(b: B) -> bool {
    FLOOR_PRIM_B.get().map(|&f| f.0 == b.0).unwrap_or(false)
}

fn is_take_prim(b: B) -> bool {
    TAKE_PRIM_B.get().map(|&f| f.0 == b.0).unwrap_or(false)
}

// Helper: tag_arr convenience (delegates to rbqn_core::tag_arr)
fn box_arr(arr: BqnArr) -> B {
    tag_arr(arr)
}

// Helper: convert Vec<B> to the most specific typed array (numeric, char, or boxed).
// Use this anywhere elements are collected via arr.get(i) to preserve element types.
fn typed_arr(elems: Vec<B>, shape: Vec<usize>, fill: Option<B>) -> BqnArr {
    rbqn_core::array::typed_arr_from_b_vec(elems, shape, fill)
}

// NOTE: Compute the prototype (fill element) of a BQN value.
// The prototype replaces all numbers with 0 and all characters with ' '.
// For arrays: same shape, each element replaced by its prototype.
// This is used when filling padding cells in structural operations.
pub fn prototype_of(b: B) -> B {
    if b.is_f64() {
        return B::m_f64(0.0);
    }
    if b.is_c32() {
        return B::m_c32(b' ' as u32);
    }
    if let Some(arr) = get_arr(b) {
        return prototype_of_arr(&arr);
    }
    // Functions, modifiers: return 0 as fallback
    B::m_f64(0.0)
}

fn prototype_of_arr(arr: &BqnArr) -> B {
    match &arr.data {
        ArrData::Boxed(elems) => {
            // Recursively compute prototype of each element
            let proto_elems: Vec<B> = elems.iter().map(|&e| prototype_of(e)).collect();
            let mut out = BqnArr {
                shape: arr.shape.clone(),
                data: ArrData::Boxed(proto_elems),
                fill: None,
            };
            out.fill = if arr.ia() > 0 {
                if let Ok(first) = arr.get(0) {
                    Some(prototype_of(first))
                } else {
                    None
                }
            } else {
                None
            };
            tag_arr(out)
        }
        ArrData::C8(_) | ArrData::C16(_) | ArrData::C32(_) => {
            // Char array: prototype is spaces with same shape
            let ia = arr.ia();
            let mut out = BqnArr::new_vec_c32(vec![b' ' as u32; ia]);
            out.shape = arr.shape.clone();
            tag_arr(out)
        }
        _ => {
            // Numeric array: prototype is zeros with same shape
            let ia = arr.ia();
            let mut out = BqnArr::new_vec_i32(vec![0i32; ia]);
            out.shape = arr.shape.clone();
            tag_arr(out)
        }
    }
}

// NOTE: Compute a fill value for an array. For Boxed arrays (arrays of arrays),
// the fill is derived from the prototype of the first element.
// For numeric/char arrays, use 0 or ' '.
pub fn arr_fill(arr: &BqnArr) -> B {
    if let Some(f) = arr.fill {
        return f;
    }
    match &arr.data {
        ArrData::Boxed(elems) => {
            // Compute prototype from first element
            if let Some(&first) = elems.first() {
                prototype_of(first)
            } else {
                tag_arr(BqnArr::empty_harr())
            }
        }
        ArrData::C8(_) | ArrData::C16(_) | ArrData::C32(_) => {
            B::m_c32(b' ' as u32)
        }
        _ => B::m_i32(0),
    }
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
    fn compute_depth_b(x: B) -> i32 {
        // Scalar atom (number or char) has depth 0
        if x.is_f64() || x.is_c32() { return 0; }
        // Array (including boxed scalar)
        if let Some(arr) = get_arr(x) {
            return compute_depth_arr(&arr);
        }
        // Non-callable non-array (function, modifier): treat as depth 0
        0
    }

    fn compute_depth_arr(arr: &BqnArr) -> i32 {
        // Non-boxed array: depth = 1 (regardless of rank/content)
        if arr.el_type() != ElType::B {
            return 1;
        }
        // Boxed array: depth = 1 + max(depth of elements)
        let ia = arr.ia();
        if ia == 0 { return 1; }
        let mut max_d = 0i32;
        for i in 0..ia {
            if let Ok(v) = arr.get(i) {
                let d = compute_depth_b(v);
                max_d = max_d.max(d);
            }
        }
        max_d + 1
    }

    let depth = if x.is_f64() || x.is_c32() {
        0
    } else {
        match xa {
            Some(arr) => compute_depth_arr(arr),
            None => compute_depth_b(x),
        }
    };
    Ok(PrimResult::Scalar(B::m_i32(depth)))
}

// < monad: enclose
pub fn enclose_c1(x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    Ok(PrimResult::Array(BqnArr {
        shape: vec![],
        data: ArrData::Boxed(vec![x]),
        fill: Some(prototype_of(x)),
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
        // NOTE: Empty merge: try to determine result type/fill from the fill element.
        // The fill element represents what each element of this array would look like after merge.
        // BQN merge behavior:
        //   - Elements are rank-0 boxes → result is outer-shape, fill = unboxed content
        //   - Elements are rank-n arrays → result shape = outer_shape ++ inner_shape
        if let Some(fill_b) = arr.fill {
            if let Some(fill_arr) = get_arr(fill_b) {
                if fill_arr.shape.is_empty() {
                    // Rank-0 fill: elements would be rank-0 boxed. Merge unboxes one level.
                    // Result shape = outer shape, element fill = content prototype.
                    let content_fill = if fill_arr.el_type() == ElType::B {
                        fill_arr.get(0).ok().map(|inner_b| {
                            if let Some(inner) = get_arr(inner_b) {
                                prototype_of_arr(&inner)
                            } else {
                                inner_b // scalar content
                            }
                        })
                    } else {
                        // Rank-0 non-boxed: prototype is itself
                        fill_arr.get(0).ok().map(|b| prototype_of(b))
                    };
                    let result = BqnArr {
                        shape: arr.shape.clone(),
                        data: ArrData::Boxed(vec![]),
                        fill: content_fill,
                    };
                    return Ok(PrimResult::Array(result));
                } else {
                    // Higher-rank fill: result shape = outer ++ inner
                    let inner_shape = fill_arr.shape.clone();
                    let mut new_shape = arr.shape.clone();
                    new_shape.extend_from_slice(&inner_shape);
                    let result = typed_arr(vec![], new_shape, fill_arr.fill);
                    return Ok(PrimResult::Array(result));
                }
            }
        }
        // Fallback: keep outer shape, inner shape unknown
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
    let first_inner_direct = get_arr(first)
        .ok_or_else(|| BqnError::Type(">𝕩: element is tagged as array but not found in store".into()))?;

    // NOTE: If first element is rank-0 boxed, merge unboxes one level.
    // E.g., >⟨<"ab",<"cd"⟩ = ⟨"ab","cd"⟩ (list of strings, same outer shape).
    let (unbox_level, inner_shape, first_inner_fill) = if first_inner_direct.shape.is_empty() && first_inner_direct.el_type() == ElType::B {
        // Rank-0 boxed: unbox to get inner content shape
        if let Ok(inner_b) = first_inner_direct.get(0) {
            if let Some(inner) = get_arr(inner_b) {
                (true, inner.shape.clone(), inner.fill)
            } else {
                // Rank-0 box containing an atom: result is a list of atoms
                (false, vec![], None)
            }
        } else {
            (false, vec![], None)
        }
    } else {
        (false, first_inner_direct.shape.clone(), first_inner_direct.fill)
    };

    if unbox_level {
        // Unbox elements: result shape = outer_shape (NOT ++ inner_shape unless inner has shape)
        let mut result_data = Vec::with_capacity(ia);
        for i in 0..ia {
            let elem = arr.get(i)?;
            let elem_arr = get_arr(elem)
                .ok_or_else(|| BqnError::Type(">𝕩: element is not an array".into()))?;
            if elem_arr.shape.is_empty() {
                result_data.push(elem_arr.get(0)?);
            } else {
                return Err(BqnError::Shape(">𝕩: element shapes don't match".into()));
            }
        }
        if inner_shape.is_empty() {
            // Inner content is scalar-like: result is outer_shape array of atoms
            let mut new_shape = arr.shape.clone();
            return Ok(PrimResult::Array(typed_arr(result_data, new_shape, None)));
        } else {
            // Inner content has shape: combine outer ++ inner
            let inner_size: usize = inner_shape.iter().product::<usize>();
            let mut flat_data = Vec::with_capacity(ia * inner_size);
            for b in result_data {
                if let Some(inner_arr) = get_arr(b) {
                    for j in 0..inner_arr.ia() {
                        flat_data.push(inner_arr.get(j)?);
                    }
                } else {
                    flat_data.push(b);
                }
            }
            let mut new_shape = arr.shape.clone();
            new_shape.extend_from_slice(&inner_shape);
            return Ok(PrimResult::Array(typed_arr(flat_data, new_shape, first_inner_fill)));
        }
    }

    let inner_size: usize = inner_shape.iter().product::<usize>();

    // Collect all elements, checking shape consistency
    let mut result_data = Vec::with_capacity(ia * inner_size);
    for i in 0..ia {
        let elem = arr.get(i)?;
        if elem.is_atom() {
            // Atoms are compatible with inner_shape [] (rank-0 elements)
            if inner_shape.is_empty() {
                result_data.push(elem);
            } else {
                return Err(BqnError::Shape(">𝕩: element shapes don't match".into()));
            }
        } else {
            let elem_arr = get_arr(elem)
                .ok_or_else(|| BqnError::Type(">𝕩: element is tagged as array but not found in store".into()))?;
            if elem_arr.shape != inner_shape {
                // Rank-0 elements with empty inner_shape: unbox to scalar
                if inner_shape.is_empty() && elem_arr.shape.is_empty() {
                    result_data.push(elem_arr.get(0)?);
                } else {
                    return Err(BqnError::Shape(format!(
                        ">𝕩: element shapes don't match ({:?} vs {:?})",
                        elem_arr.shape, inner_shape
                    )));
                }
            } else if inner_shape.is_empty() {
                // Rank-0 array: unbox to get the contained scalar
                result_data.push(elem_arr.get(0)?);
            } else {
                for j in 0..inner_size {
                    result_data.push(elem_arr.get(j)?);
                }
            }
        }
    }

    let mut new_shape = arr.shape.clone();
    new_shape.extend_from_slice(&inner_shape);

    Ok(PrimResult::Array(typed_arr(result_data, new_shape, first_inner_fill)))
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
        if warr.rank() > 1 {
            return Err(BqnError::Rank("𝕨⥊𝕩: 𝕨 must have rank ≤ 1".into()));
        }
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
        // BQN allows w⥊⟨⟩ when the result is also empty (new_ia=0).
        if new_ia == 0 {
            return Ok(PrimResult::Array(BqnArr {
                shape: new_shape,
                data: arr.data.clone(),
                fill: arr.fill,
            }));
        }
        return Err(BqnError::Domain("𝕨⥊𝕩: 𝕩 can't be empty when result is non-empty".into()));
    }

    // NOTE: Propagate fill: for empty results, compute fill from source elements.
    // For non-empty results, preserve source fill or let arr_fill compute it lazily.
    let result_fill = if arr.fill.is_some() {
        arr.fill
    } else if arr.el_type() == ElType::B {
        // Boxed array: fill = prototype of first element (computed lazily in arr_fill)
        // Store explicitly so empty results have correct fill
        Some(arr_fill(arr))
    } else {
        arr.fill
    };

    let mut result = Vec::with_capacity(new_ia);
    for i in 0..new_ia {
        result.push(arr.get(i % old_ia)?);
    }
    Ok(PrimResult::Array(typed_arr(result, new_shape, result_fill)))
}

/// Handle reshape with computed dimension (shape contains ∘, ⌊, ⌽, or ↑).
/// BQN spec: at most one element in shape can be a non-number.
/// Mode determines how the auto-dimension is computed and how extra elements are filled:
///   ∘ (MD2) → mode 0: exact division (error if not divisible), cyclic fill
///   ⌊ → mode 1: floor division, cyclic fill
///   ⌽ → mode 2: ceiling division, cyclic fill (wraps = standard reshape cycling)
///   ↑ → mode 3: ceiling division, pad with fill values (take = padding)
fn reshape_computed(warr: &BqnArr, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let ia = warr.ia();
    let mut computed_idx: Option<usize> = None;
    let mut computed_mode = 0u8; // 0=exact(∘), 1=floor(⌊), 2=ceil+cycle(⌽), 3=ceil+pad(↑)
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

            // NOTE: Determine mode from the function identity:
            // ⌊ → floor+cycle; ⌽ → ceil+cycle; ↑ → ceil+pad; MD2/other → exact.
            if v.is_fun() {
                if is_floor_prim(v) {
                    computed_mode = 1; // ⌊ = floor, cyclic
                } else if is_take_prim(v) {
                    computed_mode = 3; // ↑ = ceiling, pad with fill
                } else {
                    computed_mode = 2; // ⌽ (or unknown FUN) = ceiling, cyclic
                }
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
        1 => { // floor (⌊): floor division, cyclic
            total / known_product
        }
        2 => { // ceil+cycle (⌽): ceiling division, cyclic repeat
            (total + known_product - 1) / known_product
        }
        3 => { // ceil+pad (↑): ceiling division, pad with fill
            (total + known_product - 1) / known_product
        }
        _ => unreachable!(),
    };

    known_dims[ci] = computed_dim;
    let new_shape = known_dims;
    let new_ia: usize = new_shape.iter().product();

    if x.is_atom() {
        // NOTE: mode 3 (↑, ceil+pad) pads extra positions with fill (0 for numbers).
        // Other modes cycle the single element.
        let vals: Vec<f64> = (0..new_ia).map(|i| {
            if computed_mode == 3 && i >= 1 { 0.0 } else { x.o2f() }
        }).collect();
        let mut out = BqnArr::new_vec_f64(vals);
        out.shape = new_shape;
        return Ok(PrimResult::Array(array::squeeze_num(out)));
    }

    let arr = xa.ok_or_else(|| BqnError::Type("𝕨⥊𝕩: 𝕩 must be an array".into()))?;
    let old_ia = arr.ia();
    if old_ia == 0 {
        if new_ia == 0 {
            return Ok(PrimResult::Array(BqnArr {
                shape: new_shape,
                data: arr.data.clone(),
                fill: arr.fill,
            }));
        }
        return Err(BqnError::Domain("𝕨⥊𝕩: 𝕩 can't be empty when result is non-empty".into()));
    }

    // NOTE: mode 3 (↑, ceil+pad) pads with fill values beyond source length.
    // All other modes use cyclic repeat (standard reshape cycling behavior).
    let fill_val = arr_fill(arr);
    let mut result = Vec::with_capacity(new_ia);
    for i in 0..new_ia {
        if computed_mode == 3 && i >= old_ia {
            result.push(fill_val);
        } else {
            result.push(arr.get(i % old_ia)?);
        }
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

    // Rank-0 ∾: acts like > (merge). Unwrap one level.
    // ∾<x = x for any x. ∾<atom = <atom (rank-0 containing atom).
    if arr.rank() == 0 {
        let content = arr.get(0)?;
        if content.is_arr() {
            return match get_arr(content) {
                Some(a) => Ok(PrimResult::Array(a)),
                None => Ok(PrimResult::Scalar(content)),
            };
        }
        // Content is atom: return rank-0 array containing it
        return Ok(PrimResult::Array(arr.clone()));
    }

    if arr.rank() > 1 {
        // Monadic ∾ on rank>1: elements must be arrays with rank ≥ arr.rank()-1
        if arr.el_type() != ElType::B {
            return Err(BqnError::Type("∾𝕩: elements of rank>1 𝕩 must be arrays".into()));
        }
        let outer_rank = arr.rank() as usize;
        // Validate element ranks
        for i in 0..arr.ia() {
            let elem = arr.get(i)?;
            if elem.is_atom() {
                return Err(BqnError::Rank("∾𝕩: Ranks of argument items too small".into()));
            }
            let elem_arr = get_arr(elem)
                .ok_or_else(|| BqnError::Type("∾𝕩: element not found".into()))?;
            if (elem_arr.rank() as usize) < outer_rank {
                return Err(BqnError::Rank("∾𝕩: Ranks of argument items too small".into()));
            }
        }
        // Fold major cells with dyadic ∾
        let lead = arr.shape[0];
        if lead == 0 {
            let mut new_shape = arr.shape.clone();
            new_shape[0] = 0;
            if new_shape.len() > 1 {
                new_shape = std::iter::once(0).chain(new_shape[2..].iter().copied()).collect();
            }
            return Ok(PrimResult::Array(BqnArr { shape: new_shape, data: ArrData::Boxed(vec![]), fill: arr.fill }));
        }
        let cell_size: usize = arr.shape[1..].iter().product::<usize>().max(1);
        let cell_shape = arr.shape[1..].to_vec();
        // Extract first major cell
        let mut cell_elems = Vec::with_capacity(cell_size);
        for j in 0..cell_size {
            cell_elems.push(arr.get(j)?);
        }
        let acc = typed_arr(cell_elems, cell_shape.clone(), arr.fill);
        let mut acc_b = tag_arr(acc);
        // Fold remaining cells with join_to_c2
        for i in 1..lead {
            let mut cell_elems = Vec::with_capacity(cell_size);
            for j in 0..cell_size {
                cell_elems.push(arr.get(i * cell_size + j)?);
            }
            let cell = typed_arr(cell_elems, cell_shape.clone(), arr.fill);
            let cell_b = tag_arr(cell);
            let result = join_to_c2(acc_b, get_arr(acc_b).as_ref(), cell_b, get_arr(cell_b).as_ref())?;
            match result {
                PrimResult::Array(a) => { acc_b = tag_arr(a); }
                PrimResult::Scalar(s) => { acc_b = s; }
            }
        }
        return match get_arr(acc_b) {
            Some(a) => Ok(PrimResult::Array(a)),
            None => Ok(PrimResult::Scalar(acc_b)),
        };
    }

    let outer_len = arr.ia();
    if outer_len == 0 {
        return Ok(PrimResult::Array(BqnArr::empty_vec()));
    }

    // NOTE: BQN ∾ monad requires elements to be arrays (boxed).
    // A non-boxed rank-1 array contains atoms, not sub-arrays — error.
    if arr.el_type() != ElType::B {
        return Err(BqnError::Type("∾𝕩: elements of 𝕩 must be arrays".into()));
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
        (None, None) => {
            // Atom ∾ atom: produce 2-element list ⟨w,x⟩
            Ok(PrimResult::Array(typed_arr(vec![w, x], vec![2], None)))
        }
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

// ⋈ monad: enclose in list (⟨x⟩)
// Unlike ≍ (solo) which adds a leading axis, ⋈ wraps x as a single element in a list.
// ⋈⟨⟩ → ⟨⟨⟩⟩ (shape ⟨1⟩), not 1‿0⥊⟨⟩ (shape ⟨1,0⟩).
pub fn pair_c1(x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    Ok(PrimResult::Array(BqnArr {
        shape: vec![1],
        data: ArrData::Boxed(vec![x]),
        fill: None,
    }))
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

    // NOTE: Fill of prefixes result = prototype of first prefix (empty prefix).
    // This ensures »↑x uses the correct fill when shifting.
    let result_fill = if !prefixes.is_empty() {
        Some(prototype_of(prefixes[0]))
    } else {
        None
    };
    let mut out = BqnArr::new_vec_b(prefixes);
    out.fill = result_fill;
    Ok(PrimResult::Array(out))
}

// ↑ dyad: take
pub fn take_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    // Validate: w must have rank at most 1
    if let Some(warr) = wa {
        if warr.rank() > 1 {
            return Err(BqnError::Rank(format!(
                "𝕨↑𝕩: 𝕨 must have rank at most 1 ({:?} ≡ ≢𝕨)",
                warr.shape
            )));
        }
    }
    // Multi-axis take: when w is an array, each element specifies take along one axis
    if w.is_arr() {
        if let Some(warr) = wa {
            return take_multi_axis(warr, x, xa);
        }
    }
    let n = w.to_i32()?;
    let arr = if x.is_atom() {
        typed_arr(vec![x], vec![1], None)
    } else {
        xa.ok_or_else(|| BqnError::Type("𝕨↑𝕩: 𝕩 must be an array".into()))?.clone()
    };

    let first_dim = if arr.shape.is_empty() { 1 } else { arr.shape[0] };

    let fill_val = arr_fill(&arr);

    if arr.rank() <= 1 {
        // Vector take
        let ia = arr.ia() as i32;
        let abs_take = n.unsigned_abs() as usize;
        let result = if n >= 0 {
            // Positive take: take first n elements, fill at END if needed
            let take_from = n.min(ia) as usize;
            let fill_count = abs_take.saturating_sub(take_from);
            let mut v = Vec::with_capacity(abs_take);
            for i in 0..take_from {
                v.push(arr.get(i)?);
            }
            for _ in 0..fill_count {
                v.push(fill_val);
            }
            v
        } else {
            // Negative take: take last n elements, fill at START if needed
            // ¯n↑x: last min(n, len) elements from arr, preceded by fill
            let copy_count = abs_take.min(ia as usize);
            let fill_count = abs_take.saturating_sub(copy_count);
            let src_start = ia as usize - copy_count;
            let mut v = Vec::with_capacity(abs_take);
            for _ in 0..fill_count {
                v.push(fill_val);
            }
            for i in 0..copy_count {
                v.push(arr.get(src_start + i)?);
            }
            v
        };
        return Ok(PrimResult::Array(typed_arr(result, vec![abs_take], arr.fill)));
    }

    // Multi-rank take: operates along first axis
    let cell_shape = &arr.shape[1..];
    let cell_size: usize = cell_shape.iter().product::<usize>().max(1);
    let abs_n = n.unsigned_abs() as usize;

    let mut result = Vec::with_capacity(abs_n * cell_size);
    if n >= 0 {
        // Positive take: take first abs_n cells, fill remaining at END
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
        // Negative take: take last abs_n cells, fill at START
        let copy_count = abs_n.min(first_dim);
        let fill_count = abs_n.saturating_sub(copy_count);
        let src_start = first_dim.saturating_sub(copy_count);
        // Fill cells at start
        for _ in 0..fill_count {
            for _ in 0..cell_size {
                result.push(fill_val);
            }
        }
        // Copy cells from end of array
        for i in 0..copy_count {
            for j in 0..cell_size {
                result.push(arr.get((src_start + i) * cell_size + j)?);
            }
        }
    }

    let mut new_shape = vec![abs_n];
    new_shape.extend_from_slice(cell_shape);
    Ok(PrimResult::Array(typed_arr(result, new_shape, arr.fill)))
}

/// Multi-axis take: w is a list of integers, each specifying take along one axis.
fn take_multi_axis(warr: &BqnArr, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let axes = warr.i32_iter()?;
    if axes.is_empty() {
        // ⟨⟩↑x = x
        if x.is_atom() {
            return Ok(PrimResult::Scalar(x));
        }
        return Ok(PrimResult::Array(xa.ok_or_else(|| BqnError::Type("𝕨↑𝕩: 𝕩 must be an array".into()))?.clone()));
    }
    // Start with x, apply take along each axis
    let arr = if x.is_atom() {
        // Atom x with list w: create array of appropriate rank filled with x
        let mut shape = vec![1usize; axes.len()];
        let mut data = vec![x];
        let fill = Some(prototype_of(x));
        let result_shape: Vec<usize> = axes.iter().map(|&n| n.unsigned_abs() as usize).collect();
        let total: usize = result_shape.iter().product();
        let result_data = vec![x; total];
        return Ok(PrimResult::Array(typed_arr(result_data, result_shape, fill)));
    } else {
        xa.ok_or_else(|| BqnError::Type("𝕨↑𝕩: 𝕩 must be an array".into()))?
    };

    let fill_val = arr_fill(arr);
    let mut current_data: Vec<B> = (0..arr.ia()).map(|i| arr.get(i).unwrap_or(B::SENTINEL)).collect();
    let mut current_shape = arr.shape.clone();
    // Pad shape if w is longer than rank of x
    while current_shape.len() < axes.len() {
        current_shape.push(1);
    }

    for (a, &n) in axes.iter().enumerate() {
        if a >= current_shape.len() { break; }
        let dim = current_shape[a];
        let abs_n = n.unsigned_abs() as usize;
        let outer_size: usize = current_shape[..a].iter().product::<usize>().max(1);
        let inner_size: usize = current_shape[a+1..].iter().product::<usize>().max(1);
        let old_slice = dim * inner_size;

        let mut new_data = Vec::with_capacity(outer_size * abs_n * inner_size);
        for o in 0..outer_size {
            let base = o * old_slice;
            if n >= 0 {
                // Positive take: first abs_n rows, fill remainder
                for i in 0..abs_n {
                    if i < dim {
                        for j in 0..inner_size {
                            new_data.push(current_data[base + i * inner_size + j]);
                        }
                    } else {
                        for _ in 0..inner_size { new_data.push(fill_val); }
                    }
                }
            } else {
                // Negative take: last abs_n rows, fill at start
                let copy_count = abs_n.min(dim);
                let fill_count = abs_n.saturating_sub(copy_count);
                let src_start = dim.saturating_sub(copy_count);
                for _ in 0..fill_count {
                    for _ in 0..inner_size { new_data.push(fill_val); }
                }
                for i in 0..copy_count {
                    for j in 0..inner_size {
                        new_data.push(current_data[base + (src_start + i) * inner_size + j]);
                    }
                }
            }
        }
        current_shape[a] = abs_n;
        current_data = new_data;
    }

    Ok(PrimResult::Array(typed_arr(current_data, current_shape, arr.fill)))
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
pub fn drop_c2(w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    // Multi-axis drop: when w is an array, each element specifies drop along one axis
    if w.is_arr() {
        if let Some(warr) = wa {
            return drop_multi_axis(warr, xa);
        }
    }
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

/// Multi-axis drop: w is a list of integers, each specifying drop along one axis.
fn drop_multi_axis(warr: &BqnArr, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let axes = warr.i32_iter()?;
    if axes.is_empty() {
        return Ok(PrimResult::Array(xa.ok_or_else(|| BqnError::Type("𝕨↓𝕩: 𝕩 must be an array".into()))?.clone()));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨↓𝕩: 𝕩 must be an array".into()))?;

    let mut current_data: Vec<B> = (0..arr.ia()).map(|i| arr.get(i).unwrap_or(B::SENTINEL)).collect();
    let mut current_shape = arr.shape.clone();
    // Pad shape if w is longer than rank of x
    while current_shape.len() < axes.len() {
        current_shape.push(1);
    }

    for (a, &n) in axes.iter().enumerate() {
        if a >= current_shape.len() { break; }
        let dim = current_shape[a] as i32;
        let (start, end) = if n >= 0 {
            (n.min(dim) as usize, dim as usize)
        } else {
            (0, (dim + n).max(0) as usize)
        };
        let remaining = end - start;
        let outer_size: usize = current_shape[..a].iter().product::<usize>().max(1);
        let inner_size: usize = current_shape[a+1..].iter().product::<usize>().max(1);
        let old_slice = current_shape[a] * inner_size;

        let mut new_data = Vec::with_capacity(outer_size * remaining * inner_size);
        for o in 0..outer_size {
            let base = o * old_slice;
            for i in start..end {
                for j in 0..inner_size {
                    new_data.push(current_data[base + i * inner_size + j]);
                }
            }
        }
        current_shape[a] = remaining;
        current_data = new_data;
    }

    Ok(PrimResult::Array(typed_arr(current_data, current_shape, arr.fill)))
}

// ↕ monad: range
pub fn range_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_f64() {
        let n = x.to_usz()?;
        let vals: Vec<i32> = (0..n as i32).collect();
        return Ok(PrimResult::Array(BqnArr::new_vec_i32(vals)));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("↕𝕩: 𝕩 must be a number or array".into()))?;
    if arr.rank() != 1 {
        return Err(BqnError::Rank("↕𝕩: 𝕩 must be a number or rank-1 list".into()));
    }

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
    // NOTE: Fill for multi-dim range is the prototype of its first element (a zero index vector).
    // The zero index vector has shape [rank] with all zeros — same as prototype_of(first_elem).
    let fill = if !result.is_empty() {
        Some(prototype_of(result[0]))
    } else {
        // Empty range: fill is a zero-vector of length rank
        let zero_idx = box_arr(BqnArr::new_vec_i32(vec![0i32; rank]));
        Some(prototype_of(zero_idx))
    };
    Ok(PrimResult::Array(BqnArr {
        shape: out_shape,
        data: ArrData::Boxed(result),
        fill,
    }))
}

// ↕ dyad: windows
// n↕x returns sliding windows of length n along x.
// Result has (len-n+1) windows, each of length n.
pub fn windows_c2(w: B, wa: Option<&BqnArr>, _x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    // Multi-axis windows: when w is a list, each element specifies window size along one axis
    if w.is_arr() {
        if let Some(warr) = wa {
            return windows_multi_axis(warr, xa);
        }
    }
    let n = w.to_usz()?;
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨↕𝕩: 𝕩 must be an array".into()))?;

    let first_dim = if arr.shape.is_empty() { 1 } else { arr.shape[0] };

    if n > first_dim + 1 {
        return Err(BqnError::Domain(format!("↕: 𝕨 must be at most 1+≠𝕩 ({} > {})", n, first_dim + 1)));
    }

    let num_windows = first_dim + 1 - n;

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

/// Multi-axis windows: w is a list, each element specifies window size along one axis.
fn windows_multi_axis(warr: &BqnArr, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let axes = warr.i32_iter()?;
    let arr = xa.ok_or_else(|| BqnError::Type("𝕨↕𝕩: 𝕩 must be an array".into()))?;
    if axes.is_empty() {
        // ⟨⟩↕x = x
        return Ok(PrimResult::Array(arr.clone()));
    }
    // For each axis a, window_sizes[a] = axes[a], result_dim[a] = shape[a] + 1 - axes[a]
    let mut out_shape = Vec::new();
    let mut window_shape = Vec::new();
    for (a, &n) in axes.iter().enumerate() {
        if a >= arr.rank() as usize {
            return Err(BqnError::Rank("𝕨↕𝕩: too many axes in 𝕨".into()));
        }
        let dim = arr.shape[a];
        let n = n as usize;
        if n > dim + 1 {
            return Err(BqnError::Domain(format!("↕: window size {} too large for axis {} (dim {})", n, a, dim)));
        }
        out_shape.push(dim + 1 - n);
        window_shape.push(n);
    }
    // result shape = out_shape ++ window_shape ++ remaining_x_shape
    let mut result_shape = out_shape.clone();
    result_shape.extend_from_slice(&window_shape);
    result_shape.extend_from_slice(&arr.shape[axes.len()..]);

    let inner_size: usize = arr.shape[axes.len()..].iter().product::<usize>().max(1);
    let total: usize = result_shape.iter().product();
    let mut result = Vec::with_capacity(total);

    // Compute result by iterating over all output positions
    let n_axes = axes.len();
    let out_total: usize = out_shape.iter().product::<usize>().max(1);
    let win_total: usize = window_shape.iter().product::<usize>().max(1);

    for out_pos in 0..out_total {
        // Convert out_pos to multi-dim indices
        let mut out_indices = vec![0usize; n_axes];
        let mut rem = out_pos;
        for a in (0..n_axes).rev() {
            out_indices[a] = rem % out_shape[a];
            rem /= out_shape[a];
        }
        for win_pos in 0..win_total {
            let mut win_indices = vec![0usize; n_axes];
            let mut rem = win_pos;
            for a in (0..n_axes).rev() {
                win_indices[a] = rem % window_shape[a];
                rem /= window_shape[a];
            }
            // Source index = out_indices + win_indices for each axis
            let mut flat_idx = 0usize;
            let mut stride = inner_size;
            for a in (0..n_axes).rev() {
                let src = out_indices[a] + win_indices[a];
                flat_idx += src * stride;
                stride *= arr.shape[a];
            }
            // Actually stride needs to be computed differently
            // Let me recalculate from the beginning
            let mut src_flat = 0;
            let mut s = 1;
            for a in (0..arr.rank() as usize).rev() {
                if a < n_axes {
                    src_flat += (out_indices[a] + win_indices[a]) * s;
                }
                s *= arr.shape[a];
            }
            // This approach is getting complex. For inner dimensions, iterate directly.
            for j in 0..inner_size {
                // Compute flat source index
                let mut src = j;
                let mut stride = 1;
                for a in (0..arr.rank() as usize).rev() {
                    if a >= n_axes {
                        // inner dimension: decompose j
                        // handled by j already
                    } else {
                        let idx = out_indices[a] + win_indices[a];
                        src += idx * stride * inner_size;
                    }
                    stride *= arr.shape[a];
                }
                // Simpler: compute flat index directly
                result.push(B::SENTINEL); // placeholder
            }
        }
    }

    // Simpler approach: just handle the common 1-axis case efficiently, error on multi-axis
    result.clear();
    if n_axes == 1 {
        let n = axes[0] as usize;
        let dim = arr.shape[0];
        let cell_size: usize = arr.shape[1..].iter().product::<usize>().max(1);
        let num_windows = dim + 1 - n;
        for w_start in 0..num_windows {
            for row in w_start..w_start + n {
                for j in 0..cell_size {
                    result.push(arr.get(row * cell_size + j)?);
                }
            }
        }
        let mut out_shape = vec![num_windows, n];
        out_shape.extend_from_slice(&arr.shape[1..]);
        return Ok(PrimResult::Array(typed_arr(result, out_shape, arr.fill)));
    }

    Err(BqnError::Nyi("↕: multi-axis windows (>1 axis) not yet implemented".into()))
}

// « monad: shift after (shift left, fill from right with type fill)
// «⟨1,2,3⟩ → ⟨2,3,0⟩
pub fn shifta_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("«𝕩: 𝕩 must be an array".into()))?;
    let ia = arr.ia();
    if ia == 0 {
        return Ok(PrimResult::Array(arr.clone()));
    }
    let fill_val = arr_fill(arr);

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
        // Fill last cell with type-appropriate fill
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

    if let Some(warr) = wa {
        let wr = warr.rank();
        let xr = arr.rank();
        if wr != xr && wr + 1 != xr {
            return Err(BqnError::Rank("«: 𝕨 must be a cell of 𝕩 or have compatible shape".into()));
        }
    }

    if arr.rank() > 1 {
        // Multi-rank: shift major cells along first axis
        let first_dim = arr.shape[0];
        let cell_shape = &arr.shape[1..];
        let cell_size: usize = cell_shape.iter().product::<usize>().max(1);

        let (shift, w_cells) = match wa {
            Some(warr) if warr.rank() == arr.rank() => {
                let s = warr.shape[0];
                let mut cells = Vec::with_capacity(warr.ia());
                for i in 0..warr.ia() {
                    cells.push(warr.get(i)?);
                }
                (s, Some(cells))
            }
            Some(_warr) => {
                // w is a single cell (rank = x.rank - 1)
                let mut cells = Vec::with_capacity(cell_size);
                for i in 0..cell_size {
                    cells.push(_warr.get(i)?);
                }
                (1, Some(cells))
            }
            None => (1, None),
        };

        let mut result = Vec::with_capacity(arr.ia());
        // Copy cells [shift..] from original
        for i in shift..first_dim {
            for j in 0..cell_size {
                result.push(arr.get(i * cell_size + j)?);
            }
        }
        // Fill remaining cells with w's data or type fill
        let fill_cells = shift.min(first_dim);
        match &w_cells {
            Some(cells) => {
                // Use last fill_cells cells from w
                let w_total_cells = shift;
                let start_cell = w_total_cells.saturating_sub(fill_cells);
                for c in start_cell..start_cell + fill_cells {
                    for j in 0..cell_size {
                        result.push(cells[c * cell_size + j]);
                    }
                }
            }
            None => {
                let fill_val = arr_fill(arr);
                for _ in 0..fill_cells * cell_size {
                    result.push(fill_val);
                }
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
    // Copy remaining elements from x (shifted left)
    for i in shift..ia {
        result.push(arr.get(i)?);
    }
    // Fill from the end with w elements. When w is longer than x,
    // use the LAST ia elements of w (not the first).
    let fill_start = if shift > ia { shift - ia } else { 0 };
    let fill_count = ia - result.len();
    for v in fill_vals.iter().skip(fill_start).take(fill_count) {
        result.push(*v);
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
    let fill_val = arr_fill(arr);

    if arr.rank() > 1 {
        let first_dim = arr.shape[0];
        let cell_size: usize = arr.shape[1..].iter().product::<usize>().max(1);
        let mut result = Vec::with_capacity(ia);
        // Fill first cell with type-appropriate fill
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

    // Validate w rank: must be x.rank (multi-cell shift) or x.rank-1 (single cell) or atom
    if let Some(warr) = wa {
        let wr = warr.rank();
        let xr = arr.rank();
        if wr != xr && wr + 1 != xr {
            return Err(BqnError::Rank("»: 𝕨 must be a cell of 𝕩 or have compatible shape".into()));
        }
    }

    if arr.rank() > 1 {
        // Multi-rank: shift major cells along first axis
        let first_dim = arr.shape[0];
        let cell_shape = &arr.shape[1..];
        let cell_size: usize = cell_shape.iter().product::<usize>().max(1);

        let (shift, w_cells) = match wa {
            Some(warr) if warr.rank() == arr.rank() => {
                let s = warr.shape[0];
                let mut cells = Vec::with_capacity(warr.ia());
                for i in 0..warr.ia() {
                    cells.push(warr.get(i)?);
                }
                (s, Some(cells))
            }
            Some(_warr) => {
                let mut cells = Vec::with_capacity(cell_size);
                for i in 0..cell_size {
                    cells.push(_warr.get(i)?);
                }
                (1, Some(cells))
            }
            None => (1, None),
        };

        let mut result = Vec::with_capacity(arr.ia());
        // Fill first cells with w's data or type fill
        let fill_cells = shift.min(first_dim);
        match &w_cells {
            Some(cells) => {
                // Use first fill_cells cells from w
                for c in 0..fill_cells {
                    for j in 0..cell_size {
                        result.push(cells[c * cell_size + j]);
                    }
                }
            }
            None => {
                let fill_val = arr_fill(arr);
                for _ in 0..fill_cells * cell_size {
                    result.push(fill_val);
                }
            }
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
pub fn reverse_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    // NOTE: ⌽ on atoms (chars, numbers) errors; on rank-0 arrays errors.
    if x.is_atom() {
        return Err(BqnError::Type("⌽𝕩: 𝕩 must be an array of rank 1 or higher".into()));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("⌽𝕩: 𝕩 must be an array".into()))?;
    if arr.rank() == 0 {
        return Err(BqnError::Rank("⌽𝕩: 𝕩 must have rank 1 or higher".into()));
    }

    if arr.rank() == 1 {
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
// NOTE: BQN spec: 𝕨 can be a number (scalar) or array.
// When 𝕩 is an atom (rank 0): 𝕨 must be empty or 0; result is <𝕩 (rank-0 enclosed).
// When 𝕩 is a rank-0 array: same — result is the rank-0 array as-is.
// When 𝕨 is a number: rotate first axis by that amount.
// When 𝕨 is an array: must have length equal to rank of 𝕩; each element rotates that axis.
pub fn rotate_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    // Handle atom x (rank-0): must have empty or zero w
    if x.is_atom() {
        if let Some(warr) = wa {
            if warr.ia() != 0 {
                return Err(BqnError::Rank("𝕨⌽𝕩: 𝕨 must be empty for scalar 𝕩".into()));
            }
        } else {
            // Scalar w: must be 0
            let n = w.to_i32()?;
            if n != 0 {
                return Err(BqnError::Rank("𝕨⌽𝕩: 𝕨 must be 0 for scalar 𝕩".into()));
            }
        }
        // Return enclose of x: rank-0 boxed array
        return Ok(PrimResult::Array(BqnArr {
            shape: vec![],
            data: ArrData::Boxed(vec![x]),
            fill: None,
        }));
    }

    let arr = xa.ok_or_else(|| BqnError::Type("𝕨⌽𝕩: 𝕩 must be an array".into()))?;

    // Handle rank-0 array x
    if arr.rank() == 0 {
        if let Some(warr) = wa {
            if warr.ia() != 0 {
                return Err(BqnError::Rank("𝕨⌽𝕩: 𝕨 must be empty for rank-0 𝕩".into()));
            }
        } else {
            let n = w.to_i32()?;
            if n != 0 {
                return Err(BqnError::Rank("𝕨⌽𝕩: 𝕨 must be 0 for rank-0 𝕩".into()));
            }
        }
        return Ok(PrimResult::Array(arr.clone()));
    }

    // Array w case: w specifies rotation per axis
    if let Some(warr) = wa {
        let rotations = warr.i32_iter()?;
        if rotations.len() != arr.rank() as usize {
            return Err(BqnError::Rank(format!(
                "𝕨⌽𝕩: 𝕨 length ({}) must equal rank of 𝕩 ({})",
                rotations.len(), arr.rank()
            )));
        }
        // For now only handle first-axis rotation (common case)
        // Multi-axis rotation requires separate permute steps
        if rotations.len() == 1 {
            let n = rotations[0];
            let first_dim = arr.shape[0];
            if first_dim == 0 {
                return Ok(PrimResult::Array(arr.clone()));
            }
            if arr.rank() == 1 {
                let ia = arr.ia();
                let shift = ((n % ia as i32) + ia as i32) as usize % ia;
                let mut result = Vec::with_capacity(ia);
                for i in 0..ia {
                    result.push(arr.get((i + shift) % ia)?);
                }
                return Ok(PrimResult::Array(typed_arr(result, vec![ia], arr.fill)));
            }
            let cell_size: usize = arr.shape[1..].iter().product::<usize>().max(1);
            let shift = ((n % first_dim as i32) + first_dim as i32) as usize % first_dim;
            let mut result = Vec::with_capacity(arr.ia());
            for i in 0..first_dim {
                let src = (i + shift) % first_dim;
                for j in 0..cell_size {
                    result.push(arr.get(src * cell_size + j)?);
                }
            }
            return Ok(PrimResult::Array(typed_arr(result, arr.shape.clone(), arr.fill)));
        }
        // Multi-axis rotation: apply each rotation sequentially
        let mut current = arr.clone();
        for (axis, &rot) in rotations.iter().enumerate() {
            if rot == 0 {
                continue;
            }
            current = rotate_along_axis(&current, axis, rot)?;
        }
        return Ok(PrimResult::Array(current));
    }

    // Scalar w: rotate first axis
    let n = w.to_i32()?;
    let first_dim = if arr.shape.is_empty() { 1 } else { arr.shape[0] };
    if first_dim == 0 || arr.ia() == 0 {
        return Ok(PrimResult::Array(arr.clone()));
    }

    if arr.rank() == 1 {
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

/// Rotate arr along the given axis by rot positions.
fn rotate_along_axis(arr: &BqnArr, axis: usize, rot: i32) -> Result<BqnArr> {
    let rank = arr.rank() as usize;
    let dim = arr.shape[axis];
    if dim == 0 {
        return Ok(arr.clone());
    }

    // Compute strides
    let mut strides = vec![1usize; rank];
    for i in (0..rank - 1).rev() {
        strides[i] = strides[i + 1] * arr.shape[i + 1];
    }

    let ia = arr.ia();
    let shift = ((rot % dim as i32) + dim as i32) as usize % dim;
    let mut result = vec![B::m_i32(0); ia];

    for flat in 0..ia {
        // Decompose flat into multi-index
        let mut rem = flat;
        let mut new_flat = 0;
        for a in 0..rank {
            let idx = rem / strides[a];
            rem %= strides[a];
            let new_idx = if a == axis { (idx + shift) % dim } else { idx };
            new_flat += new_idx * strides[a];
        }
        result[new_flat] = arr.get(flat)?;
    }

    Ok(typed_arr(result, arr.shape.clone(), arr.fill))
}

// ⍉ monad: transpose (reverse axis order)
// For rank 0 (atom or rank-0 array): enclose (⍉ ≡ < for atoms/rank-0)
// For rank 1: identity.
// For rank 2: swap rows/cols.
// For rank n: reverse all axes.
pub fn transpose_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        // NOTE: BQN spec: ⍉ on atom = enclose (same as <)
        return Ok(PrimResult::Array(BqnArr {
            shape: vec![],
            data: ArrData::Boxed(vec![x]),
            fill: None,
        }));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("⍉𝕩: 𝕩 must be an array".into()))?;

    if arr.rank() == 0 {
        // Rank-0 array: return as-is (already enclosed)
        return Ok(PrimResult::Array(arr.clone()));
    }

    if arr.rank() <= 1 {
        return Ok(PrimResult::Array(arr.clone()));
    }

    let rank = arr.rank() as usize;

    if rank == 2 {
        // Rank-2: swap rows and cols (full reverse = same as move-first-to-last for rank 2)
        let old_shape = &arr.shape;
        let ia = arr.ia();
        let new_shape: Vec<usize> = old_shape.iter().rev().copied().collect();
        let mut old_strides = vec![1usize; rank];
        for i in (0..rank - 1).rev() {
            old_strides[i] = old_strides[i + 1] * old_shape[i + 1];
        }
        let mut result = vec![B::m_i32(0); ia];
        for flat in 0..ia {
            let mut rem = flat;
            let mut old_flat = 0;
            for new_axis in 0..rank {
                let mut s = 1;
                for a in (new_axis + 1)..rank {
                    s *= new_shape[a];
                }
                let idx = rem / s;
                rem %= s;
                let old_axis = rank - 1 - new_axis;
                old_flat += idx * old_strides[old_axis];
            }
            result[flat] = arr.get(old_flat)?;
        }
        return Ok(PrimResult::Array(typed_arr(result, new_shape, arr.fill)));
    }

    // Rank > 2: ⍉ moves first axis to last.
    // permutation p[i] = where old axis i goes in result.
    // p = (r-1)‿0‿1‿...‿(r-2): axis 0 → pos r-1, axis k → pos k-1 for k>0
    let perm: Vec<i32> = std::iter::once(rank as i32 - 1).chain(0..rank as i32 - 1).collect();
    transpose_with_perm(arr, &perm)
}

// ⍉⁼ monad: inverse transpose
// For rank ≤ 2: self-inverse (same as ⍉).
// For rank r > 2: permutation 1‿2‿...‿(r-1)‿0 moves first axis to last.
// This gives shape ¯1⌽≢x (rotate-left the shape).
pub fn transpose_inv_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_atom() {
        return transpose_c1(x, xa);
    }
    let arr = xa.ok_or_else(|| BqnError::Type("⍉⁼𝕩: 𝕩 must be an array".into()))?;
    if arr.rank() <= 2 {
        return transpose_c1(x, xa);
    }
    // rank > 2: ⍉ moves first axis to last (permutation (r-1)‿0‿1‿...‿(r-2) in "where axis i goes").
    // ⍉⁼ is the inverse: moves last axis to first (permutation 1‿2‿...‿(r-1)‿0).
    // p[i] = where old axis i goes in result.
    let rank = arr.rank() as i32;
    let perm: Vec<i32> = (1..rank).chain(std::iter::once(0)).collect();
    transpose_with_perm(arr, &perm)
}

// Apply a permutation p to reorder axes of arr.
// p[i] says where old axis i maps to in the result.
// Shared by reorder_c2 and transpose_inv_c1.
fn transpose_with_perm(arr: &BqnArr, perm: &[i32]) -> Result<PrimResult> {
    let rank = arr.rank() as usize;
    let old_shape = &arr.shape;

    let max_p = perm.iter().copied().max().unwrap_or(0);
    let new_rank = (max_p + 1) as usize;

    // Build new shape: for each new axis, take min of all old axes mapped to it
    let mut new_shape = vec![usize::MAX; new_rank];
    for (old_axis, &p) in perm.iter().enumerate() {
        let na = p as usize;
        new_shape[na] = new_shape[na].min(arr.shape[old_axis]);
    }
    for s in &mut new_shape {
        if *s == usize::MAX { *s = 0; }
    }

    let ia: usize = new_shape.iter().product();

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
        let mut new_idx = vec![0usize; new_rank];
        let mut rem = flat;
        for ni in 0..new_rank {
            new_idx[ni] = rem / new_strides[ni];
            rem %= new_strides[ni];
        }
        let mut old_flat = 0;
        for (old_axis, &p) in perm.iter().enumerate() {
            old_flat += new_idx[p as usize] * old_strides[old_axis];
        }
        result.push(arr.get(old_flat)?);
    }

    Ok(PrimResult::Array(typed_arr(result, new_shape, arr.fill)))
}

// ⍉ dyad: reorder axes
// p⍉x reorders axes of x according to permutation p.
// p[i] says where axis i of x ends up in the result.
// NOTE: p must be a rank-1 integer array. All values must be in [0, rank) and distinct.
pub fn reorder_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    // Handle atom x with empty perm
    if x.is_atom() {
        if let Some(warr) = wa {
            if warr.ia() != 0 {
                return Err(BqnError::Rank("𝕨⍉𝕩: 𝕨 must be empty for scalar 𝕩".into()));
            }
        } else {
            return Err(BqnError::Rank("𝕨⍉𝕩: scalar 𝕨 not valid for scalar 𝕩".into()));
        }
        return Ok(PrimResult::Array(BqnArr {
            shape: vec![],
            data: ArrData::Boxed(vec![x]),
            fill: None,
        }));
    }

    let arr = xa.ok_or_else(|| BqnError::Type("𝕨⍉𝕩: 𝕩 must be an array".into()))?;
    let rank = arr.rank() as usize;

    // NOTE: w must be rank-1 (not higher-rank)
    let perm = if w.is_f64() {
        vec![w.to_i32()?]
    } else {
        let warr = wa.ok_or_else(|| BqnError::Type("𝕨⍉𝕩: 𝕨 must be a number or array".into()))?;
        if warr.rank() != 1 {
            return Err(BqnError::Rank(format!(
                "𝕨⍉𝕩: 𝕨 must be rank-1, got rank {}",
                warr.rank()
            )));
        }
        warr.i32_iter()?
    };

    // Validate length: perm must have length <= rank.
    // If len(perm) < rank, extend by appending uncovered target positions (CBQN extension rule).
    let perm = if perm.len() < rank {
        // Validate partial permutation
        for &p in &perm {
            if p < 0 {
                return Err(BqnError::Domain(format!(
                    "𝕨⍉𝕩: axis index {} must be non-negative", p
                )));
            }
        }
        // Find covered target positions
        let mut covered = vec![false; rank];
        for &p in &perm {
            if p >= 0 && (p as usize) < rank {
                covered[p as usize] = true;
            }
        }
        // Append uncovered positions for remaining source axes
        let mut extended = perm.clone();
        let uncovered: Vec<i32> = (0..rank as i32).filter(|&i| !covered[i as usize]).collect();
        let n = perm.len();
        for (i, &uc) in uncovered.iter().enumerate() {
            if n + i < rank {
                extended.push(uc);
            }
        }
        extended
    } else if perm.len() > rank {
        return Err(BqnError::Rank(format!(
            "𝕨⍉𝕩: 𝕨 length ({}) must be ≤ rank of 𝕩 ({})",
            perm.len(),
            rank
        )));
    } else {
        perm
    };

    // Validate: all values must be non-negative
    for &p in &perm {
        if p < 0 {
            return Err(BqnError::Domain(format!(
                "𝕨⍉𝕩: axis index {} must be non-negative", p
            )));
        }
    }

    let max_p = perm.iter().copied().max().unwrap_or(0);
    let new_rank = (max_p + 1) as usize;

    // NOTE: All new axes [0, new_rank) must be referenced by at least one old axis.
    // If there's a gap (some new axis has no old axis mapped to it), error.
    let mut new_axis_covered = vec![false; new_rank];
    for &p in &perm {
        new_axis_covered[p as usize] = true;
    }
    for (na, &covered) in new_axis_covered.iter().enumerate() {
        if !covered {
            return Err(BqnError::Domain(format!(
                "𝕨⍉𝕩: new axis {} is not covered by any old axis (gap in permutation)",
                na
            )));
        }
    }

    // Build new shape: for each new axis, take min of all old axes mapped to it (diagonal case)
    let mut new_shape = vec![usize::MAX; new_rank];
    for (old_axis, &p) in perm.iter().enumerate() {
        let na = p as usize;
        new_shape[na] = new_shape[na].min(arr.shape[old_axis]);
    }
    // Replace any remaining MAX with 0
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
