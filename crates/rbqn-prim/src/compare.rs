use rbqn_core::*;
use crate::dispatch::PrimResult;

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
            if wa_arr.shape != xa_arr.shape {
                return Err(BqnError::Shape(format!(
                    "𝕨{name}𝕩: Expected equal shape prefix ({:?} ≡ ≢𝕨, {:?} ≡ ≢𝕩)",
                    wa_arr.shape, xa_arr.shape
                )));
            }
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
