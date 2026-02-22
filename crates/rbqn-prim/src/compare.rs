use rbqn_core::*;
use crate::dispatch::PrimResult;

fn is_shape_prefix(short: &[usize], long: &[usize]) -> bool {
    short.len() <= long.len() && short.iter().zip(long.iter()).all(|(a, b)| a == b)
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
