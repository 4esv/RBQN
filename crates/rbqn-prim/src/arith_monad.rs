use rbqn_core::*;
use crate::dispatch::PrimResult;

// NOTE: Recursive helper for pervasive monadic application on a single B value.
// Handles scalars, flat numeric arrays, and nested Boxed arrays.
fn pervasive_monad_b(x: B, scalar_fn: fn(f64) -> f64, name: &str) -> Result<B> {
    if x.is_f64() {
        return Ok(B::m_f64(scalar_fn(x.o2f())));
    }
    let arr = get_arr(x)
        .ok_or_else(|| BqnError::Type(format!("{name}𝕩: 𝕩 must be a number or array")))?;
    if arr.el_type().is_num() {
        let vals = arr.f64_iter()?;
        let result: Vec<f64> = vals.iter().map(|&v| scalar_fn(v)).collect();
        let mut out = BqnArr::new_vec_f64(result);
        out.shape = arr.shape.clone();
        return Ok(tag_arr(array::squeeze_num(out)));
    }
    // NOTE: Boxed array — recurse into each element (BQN pervasion)
    if let ArrData::Boxed(ref elems) = arr.data {
        let results: Result<Vec<B>> = elems
            .iter()
            .map(|&elem| pervasive_monad_b(elem, scalar_fn, name))
            .collect();
        let mut out = BqnArr::new_vec_b(results?);
        out.shape = arr.shape.clone();
        return Ok(tag_arr(out));
    }
    Err(BqnError::Type(format!("{name}𝕩: 𝕩 contained non-number")))
}

fn pervasive_monad(
    x: B,
    xa: Option<&BqnArr>,
    scalar_fn: fn(f64) -> f64,
    name: &str,
) -> Result<PrimResult> {
    if x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64(scalar_fn(x.o2f()))));
    }
    let arr = xa.ok_or_else(|| BqnError::Type(format!("{name}𝕩: 𝕩 must be a number or array")))?;
    if arr.el_type().is_num() {
        let vals = arr.f64_iter()?;
        let result: Vec<f64> = vals.iter().map(|&v| scalar_fn(v)).collect();
        let mut out = BqnArr::new_vec_f64(result);
        out.shape = arr.shape.clone();
        return Ok(PrimResult::Array(array::squeeze_num(out)));
    }
    // NOTE: Boxed array — recurse into each element (BQN pervasion)
    if let ArrData::Boxed(ref elems) = arr.data {
        let results: Result<Vec<B>> = elems
            .iter()
            .map(|&elem| pervasive_monad_b(elem, scalar_fn, name))
            .collect();
        let mut out = BqnArr::new_vec_b(results?);
        out.shape = arr.shape.clone();
        return Ok(PrimResult::Array(out));
    }
    Err(BqnError::Type(format!("{name}𝕩: 𝕩 contained non-number")))
}

// + monad: identity (assert numeric)
pub fn add_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_f64() {
        return Ok(PrimResult::Scalar(x));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("+𝕩: 𝕩 must consist of numbers".into()))?;
    if arr.el_type().is_num() {
        return Ok(PrimResult::Array(arr.clone()));
    }
    // NOTE: Boxed array — recurse to assert all leaves are numeric (BQN pervasion)
    if let ArrData::Boxed(ref elems) = arr.data {
        let results: Result<Vec<B>> = elems
            .iter()
            .map(|&elem| pervasive_monad_b(elem, |v| v, "+"))
            .collect();
        let mut out = BqnArr::new_vec_b(results?);
        out.shape = arr.shape.clone();
        return Ok(PrimResult::Array(out));
    }
    Err(BqnError::Type("+𝕩: 𝕩 must consist of numbers".into()))
}

// - monad: negate
pub fn sub_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    pervasive_monad(x, xa, |v| -v, "-")
}

// × monad: sign
pub fn mul_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    pervasive_monad(
        x,
        xa,
        |v| {
            if v > 0.0 {
                1.0
            } else if v == 0.0 {
                0.0
            } else {
                -1.0
            }
        },
        "×",
    )
}

// ÷ monad: reciprocal (1÷x, with +0 to handle -0)
pub fn div_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    pervasive_monad(x, xa, |v| 1.0 / (v + 0.0), "÷")
}

// ⋆ monad: e^x
pub fn pow_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    pervasive_monad(x, xa, f64::exp, "⋆")
}

// √ monad: square root
pub fn root_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    pervasive_monad(x, xa, f64::sqrt, "√")
}

// ⌊ monad: floor
pub fn floor_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64(x.o2f().floor())));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("⌊𝕩: 𝕩 must be a number or array".into()))?;
    if arr.el_type().is_int() {
        return Ok(PrimResult::Array(arr.clone()));
    }
    pervasive_monad(x, xa, f64::floor, "⌊")
}

// ⌈ monad: ceiling
pub fn ceil_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64(x.o2f().ceil())));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("⌈𝕩: 𝕩 must be a number or array".into()))?;
    if arr.el_type().is_int() {
        return Ok(PrimResult::Array(arr.clone()));
    }
    pervasive_monad(x, xa, f64::ceil, "⌈")
}

// | monad: absolute value
pub fn stile_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64(x.o2f().abs())));
    }
    let arr = xa.ok_or_else(|| BqnError::Type("|𝕩: 𝕩 must be a number or array".into()))?;
    if arr.el_type() == ElType::Bit {
        return Ok(PrimResult::Array(arr.clone()));
    }
    pervasive_monad(x, xa, f64::abs, "|")
}

// ¬ monad: not (1-x)
pub fn not_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    pervasive_monad(x, xa, |v| 1.0 - v, "¬")
}

// ⋆⁼ monad: natural log
pub fn log_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    pervasive_monad(x, xa, f64::ln, "⋆⁼")
}
