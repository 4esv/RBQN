use rbqn_core::*;
use crate::dispatch::PrimResult;

fn is_shape_prefix(short: &[usize], long: &[usize]) -> bool {
    short.len() <= long.len() && short.iter().zip(long.iter()).all(|(a, b)| a == b)
}

/// Recursive helper for pervasive comparison on a single B value pair.
fn cmp_pervasive_b(w: B, x: B, scalar_fn: fn(B, B) -> i32, name: &str) -> Result<B> {
    let wa = get_arr(w);
    let xa = get_arr(x);
    match (wa, xa) {
        (None, None) => Ok(B::m_i32(scalar_fn(w, x))),
        (None, Some(xa_arr)) => {
            let n = xa_arr.ia();
            let mut results = Vec::with_capacity(n);
            for i in 0..n {
                let xv = xa_arr.get(i)?;
                results.push(cmp_pervasive_b(w, xv, scalar_fn, name)?);
            }
            let result_fill = results.first().copied().map(crate::structural::prototype_of);
            let mut out = rbqn_core::array::typed_arr_from_b_vec(results, xa_arr.shape.clone(), result_fill);
            Ok(tag_arr(out))
        }
        (Some(wa_arr), None) => {
            let n = wa_arr.ia();
            let mut results = Vec::with_capacity(n);
            for i in 0..n {
                let wv = wa_arr.get(i)?;
                results.push(cmp_pervasive_b(wv, x, scalar_fn, name)?);
            }
            let result_fill = results.first().copied().map(crate::structural::prototype_of);
            let mut out = rbqn_core::array::typed_arr_from_b_vec(results, wa_arr.shape.clone(), result_fill);
            Ok(tag_arr(out))
        }
        (Some(wa_arr), Some(xa_arr)) => {
            if wa_arr.shape != xa_arr.shape {
                return Err(BqnError::Shape(format!(
                    "𝕨{name}𝕩: shape mismatch ({:?} vs {:?})", wa_arr.shape, xa_arr.shape
                )));
            }
            let n = wa_arr.ia();
            let mut results = Vec::with_capacity(n);
            for i in 0..n {
                let wv = wa_arr.get(i)?;
                let xv = xa_arr.get(i)?;
                results.push(cmp_pervasive_b(wv, xv, scalar_fn, name)?);
            }
            let result_fill = results.first().copied().map(crate::structural::prototype_of);
            let out = rbqn_core::array::typed_arr_from_b_vec(results, wa_arr.shape.clone(), result_fill);
            Ok(tag_arr(out))
        }
    }
}

/// Handle comparison of Boxed (nested) arrays element-wise.
fn cmp_pervasive_boxed(
    wa_arr: &BqnArr,
    xa_arr: &BqnArr,
    scalar_fn: fn(B, B) -> i32,
    name: &str,
) -> Result<PrimResult> {
    if wa_arr.shape != xa_arr.shape {
        return Err(BqnError::Shape(format!(
            "𝕨{name}𝕩: shape mismatch ({:?} vs {:?})", wa_arr.shape, xa_arr.shape
        )));
    }
    let n = wa_arr.ia();
    let mut results: Vec<B> = Vec::with_capacity(n);
    for i in 0..n {
        let wv = wa_arr.get(i)?;
        let xv = xa_arr.get(i)?;
        results.push(cmp_pervasive_b(wv, xv, scalar_fn, name)?);
    }
    let result_fill = results.first().copied().map(crate::structural::prototype_of);
    let out = rbqn_core::array::typed_arr_from_b_vec(results, wa_arr.shape.clone(), result_fill);
    Ok(PrimResult::Array(out))
}

fn cmp_pervasive(
    w: B,
    wa: Option<&BqnArr>,
    x: B,
    xa: Option<&BqnArr>,
    scalar_fn: fn(B, B) -> i32,
    name: &str,
) -> Result<PrimResult> {
    match (wa, xa) {
        (None, None) => Ok(PrimResult::Scalar(B::m_i32(scalar_fn(w, x)))),
        (None, Some(xa_arr)) => {
            let ia = xa_arr.ia();
            let mut result = Vec::with_capacity(ia);
            for i in 0..ia {
                let xv = xa_arr.get(i)?;
                result.push(scalar_fn(w, xv) as i32);
            }
            let mut out = BqnArr::new_vec_i32(result);
            out.shape = xa_arr.shape.clone();
            Ok(PrimResult::Array(out))
        }
        (Some(wa_arr), None) => {
            let ia = wa_arr.ia();
            let mut result = Vec::with_capacity(ia);
            for i in 0..ia {
                let wv = wa_arr.get(i)?;
                result.push(scalar_fn(wv, x) as i32);
            }
            let mut out = BqnArr::new_vec_i32(result);
            out.shape = wa_arr.shape.clone();
            Ok(PrimResult::Array(out))
        }
        (Some(wa_arr), Some(xa_arr)) => {
            // NOTE: Boxed arrays require recursive pervasion, similar to arithmetic.
            if wa_arr.el_type() == ElType::B || xa_arr.el_type() == ElType::B {
                return cmp_pervasive_boxed(wa_arr, xa_arr, scalar_fn, name);
            }
            if wa_arr.shape == xa_arr.shape {
                // Fast path: identical shapes
                let ia = wa_arr.ia();
                let mut result = Vec::with_capacity(ia);
                for i in 0..ia {
                    let wv = wa_arr.get(i)?;
                    let xv = xa_arr.get(i)?;
                    result.push(scalar_fn(wv, xv) as i32);
                }
                let mut out = BqnArr::new_vec_i32(result);
                out.shape = wa_arr.shape.clone();
                Ok(PrimResult::Array(out))
            } else if is_shape_prefix(&wa_arr.shape, &xa_arr.shape) {
                // 𝕨 shorter, broadcast across leading axes of 𝕩
                let w_ia = wa_arr.ia().max(1);
                let x_ia = xa_arr.ia();
                let mut result = Vec::with_capacity(x_ia);
                for i in 0..x_ia {
                    let wv = wa_arr.get(i % w_ia)?;
                    let xv = xa_arr.get(i)?;
                    result.push(scalar_fn(wv, xv) as i32);
                }
                let mut out = BqnArr::new_vec_i32(result);
                out.shape = xa_arr.shape.clone();
                Ok(PrimResult::Array(out))
            } else if is_shape_prefix(&xa_arr.shape, &wa_arr.shape) {
                // 𝕩 shorter, broadcast across leading axes of 𝕨
                let w_ia = wa_arr.ia();
                let x_ia = xa_arr.ia().max(1);
                let mut result = Vec::with_capacity(w_ia);
                for i in 0..w_ia {
                    let wv = wa_arr.get(i)?;
                    let xv = xa_arr.get(i % x_ia)?;
                    result.push(scalar_fn(wv, xv) as i32);
                }
                let mut out = BqnArr::new_vec_i32(result);
                out.shape = wa_arr.shape.clone();
                Ok(PrimResult::Array(out))
            } else {
                return Err(BqnError::Shape(format!(
                    "𝕨{name}𝕩: Expected equal shape prefix ({:?} ≡ ≢𝕨, {:?} ≡ ≢𝕩)",
                    wa_arr.shape, xa_arr.shape
                )));
            }
        }
    }
}

fn scalar_le(w: B, x: B) -> i32 {
    if w.is_f64() && x.is_f64() {
        return (w.o2f() <= x.o2f()) as i32;
    }
    if w.is_c32() && x.is_c32() {
        return (w.0 as u32 <= x.0 as u32) as i32;
    }
    // num < char by convention
    if w.is_f64() && x.is_c32() {
        return 1;
    }
    if w.is_c32() && x.is_f64() {
        return 0;
    }
    (compare::compare(w, x) <= 0) as i32
}

fn scalar_ge(w: B, x: B) -> i32 {
    scalar_le(x, w)
}

fn scalar_lt(w: B, x: B) -> i32 {
    if w.is_f64() && x.is_f64() {
        return (w.o2f() < x.o2f()) as i32;
    }
    if w.is_c32() && x.is_c32() {
        return ((w.0 as u32) < (x.0 as u32)) as i32;
    }
    if w.is_f64() && x.is_c32() {
        return 1;
    }
    if w.is_c32() && x.is_f64() {
        return 0;
    }
    (compare::compare(w, x) < 0) as i32
}

fn scalar_gt(w: B, x: B) -> i32 {
    scalar_lt(x, w)
}

fn scalar_eq(w: B, x: B) -> i32 {
    if w.is_f64() && x.is_f64() {
        return (w.o2f() == x.o2f()) as i32;
    }
    if w.is_c32() && x.is_c32() {
        return (w.0 as u32 == x.0 as u32) as i32;
    }
    w.atom_equal(x) as i32
}

fn scalar_ne(w: B, x: B) -> i32 {
    1 - scalar_eq(w, x)
}

// ≤ dyad
pub fn le_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    cmp_pervasive(w, wa, x, xa, scalar_le, "≤")
}

// ≥ dyad
pub fn ge_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    cmp_pervasive(w, wa, x, xa, scalar_ge, "≥")
}

// < dyad
pub fn lt_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    cmp_pervasive(w, wa, x, xa, scalar_lt, "<")
}

// > dyad
pub fn gt_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    cmp_pervasive(w, wa, x, xa, scalar_gt, ">")
}

// = dyad
pub fn eq_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    cmp_pervasive(w, wa, x, xa, scalar_eq, "=")
}

// ≠ dyad
pub fn ne_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    cmp_pervasive(w, wa, x, xa, scalar_ne, "≠")
}

// ≡ dyad: match (depth-recursive equality)
pub fn feq_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let result = match (wa, xa) {
        (Some(wa_arr), Some(xa_arr)) => {
            compare::equal(w, x, Some(wa_arr), Some(xa_arr))
        }
        (None, None) => w.atom_equal(x),
        _ => false,
    };
    Ok(PrimResult::Scalar(B::m_i32(result as i32)))
}

// ≢ dyad: not-match
pub fn fne_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let result = match (wa, xa) {
        (Some(wa_arr), Some(xa_arr)) => {
            !compare::equal(w, x, Some(wa_arr), Some(xa_arr))
        }
        (None, None) => !w.atom_equal(x),
        _ => true,
    };
    Ok(PrimResult::Scalar(B::m_i32(result as i32)))
}
