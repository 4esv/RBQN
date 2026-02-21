use rbqn_core::*;
use crate::dispatch::PrimResult;

fn pervasive_dyad(
    w: B,
    wa: Option<&BqnArr>,
    x: B,
    xa: Option<&BqnArr>,
    scalar_fn: fn(f64, f64) -> f64,
    name: &str,
) -> Result<PrimResult> {
    match (wa, xa) {
        // scalar-scalar
        (None, None) => {
            let wf = w.to_f64().map_err(|_| BqnError::Type(format!("𝕨{name}𝕩: Unexpected argument types")))?;
            let xf = x.to_f64().map_err(|_| BqnError::Type(format!("𝕨{name}𝕩: Unexpected argument types")))?;
            Ok(PrimResult::Scalar(B::m_f64(scalar_fn(wf, xf))))
        }
        // scalar-array
        (None, Some(xa_arr)) => {
            let wf = w.to_f64().map_err(|_| BqnError::Type(format!("𝕨{name}𝕩: Unexpected argument types")))?;
            let xvals = xa_arr.f64_iter()?;
            let result: Vec<f64> = xvals.iter().map(|&xv| scalar_fn(wf, xv)).collect();
            let mut out = BqnArr::new_vec_f64(result);
            out.shape = xa_arr.shape.clone();
            Ok(PrimResult::Array(array::squeeze_num(out)))
        }
        // array-scalar
        (Some(wa_arr), None) => {
            let xf = x.to_f64().map_err(|_| BqnError::Type(format!("𝕨{name}𝕩: Unexpected argument types")))?;
            let wvals = wa_arr.f64_iter()?;
            let result: Vec<f64> = wvals.iter().map(|&wv| scalar_fn(wv, xf)).collect();
            let mut out = BqnArr::new_vec_f64(result);
            out.shape = wa_arr.shape.clone();
            Ok(PrimResult::Array(array::squeeze_num(out)))
        }
        // array-array
        (Some(wa_arr), Some(xa_arr)) => {
            if wa_arr.shape != xa_arr.shape {
                return Err(BqnError::Shape(format!(
                    "𝕨{name}𝕩: Expected equal shape prefix ({:?} ≡ ≢𝕨, {:?} ≡ ≢𝕩)",
                    wa_arr.shape, xa_arr.shape
                )));
            }
            let wvals = wa_arr.f64_iter()?;
            let xvals = xa_arr.f64_iter()?;
            let result: Vec<f64> = wvals
                .iter()
                .zip(xvals.iter())
                .map(|(&wv, &xv)| scalar_fn(wv, xv))
                .collect();
            let mut out = BqnArr::new_vec_f64(result);
            out.shape = wa_arr.shape.clone();
            Ok(PrimResult::Array(array::squeeze_num(out)))
        }
    }
}

fn pfmod(a: f64, b: f64) -> f64 {
    let r = a % b;
    if (a < 0.0) != (b < 0.0) && r != 0.0 {
        r + b
    } else {
        r
    }
}

// + dyad: add
pub fn add_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    // char+num and num+char
    if w.is_c32() && x.is_f64() {
        let wc = w.o2c()? as i64;
        let xi = x.to_f64()? as i64;
        let r = (wc + xi) as u64;
        if r > value::CHR_MAX as u64 {
            return Err(BqnError::Domain("𝕨+𝕩: Invalid character".into()));
        }
        return Ok(PrimResult::Scalar(B::m_c32(r as u32)));
    }
    if w.is_f64() && x.is_c32() {
        let wi = w.to_f64()? as i64;
        let xc = x.o2c()? as i64;
        let r = (wi + xc) as u64;
        if r > value::CHR_MAX as u64 {
            return Err(BqnError::Domain("𝕨+𝕩: Invalid character".into()));
        }
        return Ok(PrimResult::Scalar(B::m_c32(r as u32)));
    }
    if w.is_f64() && x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64(w.o2f() + x.o2f())));
    }
    pervasive_dyad(w, wa, x, xa, |a, b| a + b, "+")
}

// - dyad: subtract
pub fn sub_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    // char-num → char, char-char → num
    if w.is_c32() && x.is_f64() {
        let wc = w.o2c()? as i64;
        let xi = x.to_f64()? as i64;
        let r = (wc - xi) as u64;
        if r > value::CHR_MAX as u64 {
            return Err(BqnError::Domain("𝕨-𝕩: Invalid character".into()));
        }
        return Ok(PrimResult::Scalar(B::m_c32(r as u32)));
    }
    if w.is_c32() && x.is_c32() {
        let wc = w.o2c()? as i32;
        let xc = x.o2c()? as i32;
        return Ok(PrimResult::Scalar(B::m_f64((wc - xc) as f64)));
    }
    if w.is_f64() && x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64(w.o2f() - x.o2f())));
    }
    pervasive_dyad(w, wa, x, xa, |a, b| a - b, "-")
}

// × dyad: multiply
pub fn mul_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if w.is_f64() && x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64(w.o2f() * x.o2f())));
    }
    pervasive_dyad(w, wa, x, xa, |a, b| a * b, "×")
}

// ÷ dyad: divide
pub fn div_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if w.is_f64() && x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64(w.o2f() / (x.o2f() + 0.0))));
    }
    pervasive_dyad(w, wa, x, xa, |a, b| a / (b + 0.0), "÷")
}

// ⋆ dyad: power
pub fn pow_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if w.is_f64() && x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64((w.o2f() + 0.0).powf(x.o2f()))));
    }
    pervasive_dyad(w, wa, x, xa, |a, b| (a + 0.0).powf(b), "⋆")
}

// √ dyad: w-th root of x
pub fn root_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if w.is_f64() && x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64((x.o2f() + 0.0).powf(1.0 / (0.0 + w.o2f())))));
    }
    pervasive_dyad(w, wa, x, xa, |a, b| (b + 0.0).powf(1.0 / (0.0 + a)), "√")
}

// ⌊ dyad: minimum
pub fn floor_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if w.is_f64() && x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64(w.o2f().min(x.o2f()))));
    }
    pervasive_dyad(w, wa, x, xa, f64::min, "⌊")
}

// ⌈ dyad: maximum
pub fn ceil_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if w.is_f64() && x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64(w.o2f().max(x.o2f()))));
    }
    pervasive_dyad(w, wa, x, xa, f64::max, "⌈")
}

// | dyad: modulus
pub fn stile_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if w.is_f64() && x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64(pfmod(x.o2f(), w.o2f()))));
    }
    pervasive_dyad(w, wa, x, xa, |a, b| pfmod(b, a), "|")
}

// ¬ dyad: span (1+w-x)
pub fn not_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if w.is_f64() && x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64(1.0 + w.o2f() - x.o2f())));
    }
    pervasive_dyad(w, wa, x, xa, |a, b| 1.0 + a - b, "¬")
}

// ∧ dyad: and (w×x)
pub fn and_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if w.is_f64() && x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64(w.o2f() * x.o2f())));
    }
    pervasive_dyad(w, wa, x, xa, |a, b| a * b, "∧")
}

// ∨ dyad: or (w+x-w×x)
pub fn or_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if w.is_f64() && x.is_f64() {
        let wf = w.o2f();
        let xf = x.o2f();
        return Ok(PrimResult::Scalar(B::m_f64(wf + xf - wf * xf)));
    }
    pervasive_dyad(w, wa, x, xa, |a, b| a + b - a * b, "∨")
}

// ⋆⁼ dyad: logarithm (log_w(x))
pub fn log_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if w.is_f64() && x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64(x.o2f().ln() / w.o2f().ln())));
    }
    pervasive_dyad(w, wa, x, xa, |a, b| b.ln() / a.ln(), "⋆⁼")
}
