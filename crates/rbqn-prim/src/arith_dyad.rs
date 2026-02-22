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

/// Helper: shift a character code point by a number, returning a char B value.
fn char_add(c: u32, n: f64) -> Result<B> {
    let r = c as i64 + n as i64;
    if r < 0 || r > value::CHR_MAX as i64 {
        return Err(BqnError::Domain("𝕨+𝕩: Invalid character".into()));
    }
    Ok(B::m_c32(r as u32))
}

/// Classify whether w/x (with optional array backing) is char or num.
/// Returns ('c', 'n', or 'o' for other) for each side.
fn char_num_class(v: B, va: Option<&BqnArr>) -> char {
    if v.is_c32() { return 'c'; }
    if v.is_f64() { return 'n'; }
    if let Some(a) = va {
        if a.is_char_arr() { return 'c'; }
        if a.is_num_arr() { return 'n'; }
    }
    'o'
}

// + dyad: add
pub fn add_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let wk = char_num_class(w, wa);
    let xk = char_num_class(x, xa);

    // num + num: standard path
    if wk == 'n' && xk == 'n' {
        if w.is_f64() && x.is_f64() {
            return Ok(PrimResult::Scalar(B::m_f64(w.o2f() + x.o2f())));
        }
        return pervasive_dyad(w, wa, x, xa, |a, b| a + b, "+");
    }

    // char + num → char  or  num + char → char
    if (wk == 'c' && xk == 'n') || (wk == 'n' && xk == 'c') {
        return pervasive_char_add(w, wa, x, xa, wk == 'c');
    }

    // char + char is a type error in BQN
    Err(BqnError::Type("𝕨+𝕩: Unexpected argument types".into()))
}

/// Pervasive char+num (or num+char) → char for all scalar/array combos.
/// `w_is_char`: true means w is the char side, false means x is the char side.
fn pervasive_char_add(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>, w_is_char: bool) -> Result<PrimResult> {
    let (cv, ca, nv, na) = if w_is_char { (w, wa, x, xa) } else { (x, xa, w, wa) };

    match (ca, na) {
        // scalar char + scalar num
        (None, None) => {
            char_add(cv.o2c()?, nv.to_f64()?)
                .map(PrimResult::Scalar)
        }
        // scalar char + array num
        (None, Some(na_arr)) => {
            let c = cv.o2c()?;
            let nums = na_arr.f64_iter()?;
            let result: std::result::Result<Vec<u32>, _> = nums.iter().map(|&n| {
                let r = c as i64 + n as i64;
                if r < 0 || r > value::CHR_MAX as i64 {
                    Err(BqnError::Domain("𝕨+𝕩: Invalid character".into()))
                } else {
                    Ok(r as u32)
                }
            }).collect();
            let mut out = BqnArr::new_vec_c32(result?);
            out.shape = na_arr.shape.clone();
            Ok(PrimResult::Array(out))
        }
        // array char + scalar num
        (Some(ca_arr), None) => {
            let n = nv.to_f64()?;
            let chars = ca_arr.c32_iter()?;
            let result: std::result::Result<Vec<u32>, _> = chars.iter().map(|&c| {
                let r = c as i64 + n as i64;
                if r < 0 || r > value::CHR_MAX as i64 {
                    Err(BqnError::Domain("𝕨+𝕩: Invalid character".into()))
                } else {
                    Ok(r as u32)
                }
            }).collect();
            let mut out = BqnArr::new_vec_c32(result?);
            out.shape = ca_arr.shape.clone();
            Ok(PrimResult::Array(out))
        }
        // array char + array num
        (Some(ca_arr), Some(na_arr)) => {
            if ca_arr.shape != na_arr.shape {
                return Err(BqnError::Shape(format!(
                    "𝕨+𝕩: Expected equal shape prefix ({:?} ≡ ≢𝕨, {:?} ≡ ≢𝕩)",
                    if w_is_char { &ca_arr.shape } else { &na_arr.shape },
                    if w_is_char { &na_arr.shape } else { &ca_arr.shape },
                )));
            }
            let chars = ca_arr.c32_iter()?;
            let nums = na_arr.f64_iter()?;
            let result: std::result::Result<Vec<u32>, _> = chars.iter().zip(nums.iter()).map(|(&c, &n)| {
                let r = c as i64 + n as i64;
                if r < 0 || r > value::CHR_MAX as i64 {
                    Err(BqnError::Domain("𝕨+𝕩: Invalid character".into()))
                } else {
                    Ok(r as u32)
                }
            }).collect();
            let mut out = BqnArr::new_vec_c32(result?);
            out.shape = ca_arr.shape.clone();
            Ok(PrimResult::Array(out))
        }
    }
}

/// Helper for char-num → char subtraction result.
fn char_sub(c: u32, n: f64) -> Result<B> {
    let r = c as i64 - n as i64;
    if r < 0 || r > value::CHR_MAX as i64 {
        return Err(BqnError::Domain("𝕨-𝕩: Invalid character".into()));
    }
    Ok(B::m_c32(r as u32))
}

/// Pervasive char-num → char for all scalar/array combos.
fn pervasive_char_sub_num(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    match (wa, xa) {
        (None, None) => {
            char_sub(w.o2c()?, x.to_f64()?).map(PrimResult::Scalar)
        }
        (None, Some(xa_arr)) => {
            let c = w.o2c()?;
            let nums = xa_arr.f64_iter()?;
            let result: std::result::Result<Vec<u32>, _> = nums.iter().map(|&n| {
                let r = c as i64 - n as i64;
                if r < 0 || r > value::CHR_MAX as i64 {
                    Err(BqnError::Domain("𝕨-𝕩: Invalid character".into()))
                } else { Ok(r as u32) }
            }).collect();
            let mut out = BqnArr::new_vec_c32(result?);
            out.shape = xa_arr.shape.clone();
            Ok(PrimResult::Array(out))
        }
        (Some(wa_arr), None) => {
            let n = x.to_f64()?;
            let chars = wa_arr.c32_iter()?;
            let result: std::result::Result<Vec<u32>, _> = chars.iter().map(|&c| {
                let r = c as i64 - n as i64;
                if r < 0 || r > value::CHR_MAX as i64 {
                    Err(BqnError::Domain("𝕨-𝕩: Invalid character".into()))
                } else { Ok(r as u32) }
            }).collect();
            let mut out = BqnArr::new_vec_c32(result?);
            out.shape = wa_arr.shape.clone();
            Ok(PrimResult::Array(out))
        }
        (Some(wa_arr), Some(xa_arr)) => {
            if wa_arr.shape != xa_arr.shape {
                return Err(BqnError::Shape(format!(
                    "𝕨-𝕩: Expected equal shape prefix ({:?} ≡ ≢𝕨, {:?} ≡ ≢𝕩)",
                    wa_arr.shape, xa_arr.shape
                )));
            }
            let chars = wa_arr.c32_iter()?;
            let nums = xa_arr.f64_iter()?;
            let result: std::result::Result<Vec<u32>, _> = chars.iter().zip(nums.iter()).map(|(&c, &n)| {
                let r = c as i64 - n as i64;
                if r < 0 || r > value::CHR_MAX as i64 {
                    Err(BqnError::Domain("𝕨-𝕩: Invalid character".into()))
                } else { Ok(r as u32) }
            }).collect();
            let mut out = BqnArr::new_vec_c32(result?);
            out.shape = wa_arr.shape.clone();
            Ok(PrimResult::Array(out))
        }
    }
}

/// Pervasive char-char → number for all scalar/array combos.
fn pervasive_char_sub_char(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    match (wa, xa) {
        (None, None) => {
            let wc = w.o2c()? as i32;
            let xc = x.o2c()? as i32;
            Ok(PrimResult::Scalar(B::m_f64((wc - xc) as f64)))
        }
        (None, Some(xa_arr)) => {
            let wc = w.o2c()? as i64;
            let xchars = xa_arr.c32_iter()?;
            let result: Vec<f64> = xchars.iter().map(|&xc| (wc - xc as i64) as f64).collect();
            let mut out = BqnArr::new_vec_f64(result);
            out.shape = xa_arr.shape.clone();
            Ok(PrimResult::Array(array::squeeze_num(out)))
        }
        (Some(wa_arr), None) => {
            let xc = x.o2c()? as i64;
            let wchars = wa_arr.c32_iter()?;
            let result: Vec<f64> = wchars.iter().map(|&wc| (wc as i64 - xc) as f64).collect();
            let mut out = BqnArr::new_vec_f64(result);
            out.shape = wa_arr.shape.clone();
            Ok(PrimResult::Array(array::squeeze_num(out)))
        }
        (Some(wa_arr), Some(xa_arr)) => {
            if wa_arr.shape != xa_arr.shape {
                return Err(BqnError::Shape(format!(
                    "𝕨-𝕩: Expected equal shape prefix ({:?} ≡ ≢𝕨, {:?} ≡ ≢𝕩)",
                    wa_arr.shape, xa_arr.shape
                )));
            }
            let wchars = wa_arr.c32_iter()?;
            let xchars = xa_arr.c32_iter()?;
            let result: Vec<f64> = wchars.iter().zip(xchars.iter())
                .map(|(&wc, &xc)| (wc as i64 - xc as i64) as f64).collect();
            let mut out = BqnArr::new_vec_f64(result);
            out.shape = wa_arr.shape.clone();
            Ok(PrimResult::Array(array::squeeze_num(out)))
        }
    }
}

// - dyad: subtract
pub fn sub_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let wk = char_num_class(w, wa);
    let xk = char_num_class(x, xa);

    // num - num: standard path
    if wk == 'n' && xk == 'n' {
        if w.is_f64() && x.is_f64() {
            return Ok(PrimResult::Scalar(B::m_f64(w.o2f() - x.o2f())));
        }
        return pervasive_dyad(w, wa, x, xa, |a, b| a - b, "-");
    }

    // char - num → char
    if wk == 'c' && xk == 'n' {
        return pervasive_char_sub_num(w, wa, x, xa);
    }

    // char - char → num
    if wk == 'c' && xk == 'c' {
        return pervasive_char_sub_char(w, wa, x, xa);
    }

    // num - char is a type error in BQN
    Err(BqnError::Type("𝕨-𝕩: Unexpected argument types".into()))
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
