use rbqn_core::*;
use crate::dispatch::PrimResult;

/// Apply `f` to every number of a numeric array, reading the typed data in
/// place (no f64 copy of the input), then squeeze the result.
fn map_num(arr: &BqnArr, f: fn(f64) -> f64) -> BqnArr {
    let result: Vec<f64> = match &arr.data {
        ArrData::F64(v) => v.iter().map(|&x| f(x)).collect(),
        ArrData::I32(v) => v.iter().map(|&x| f(x as f64)).collect(),
        ArrData::I16(v) => v.iter().map(|&x| f(x as f64)).collect(),
        ArrData::I8(v) => v.iter().map(|&x| f(x as f64)).collect(),
        ArrData::Bit(v) => (0..arr.ia()).map(|i| f(((v[i / 64] >> (i % 64)) & 1) as f64)).collect(),
        _ => unreachable!("map_num on non-numeric data"),
    };
    let mut out = BqnArr::new_vec_f64(result);
    out.shape = arr.shape.clone();
    array::squeeze_num(out)
}

/// ⌊/⌈ of an F64 array written straight to i32 when every result fits
/// (one 4-byte pass instead of an 8-byte result plus the squeeze scan);
/// None when some value is out of i32 range or not finite.
fn round_to_i32(v: &[f64], f: fn(f64) -> f64) -> Option<Vec<i32>> {
    let mut out = Vec::with_capacity(v.len());
    for &x in v {
        let r = f(x);
        if !(r >= i32::MIN as f64 && r <= i32::MAX as f64) {
            return None;
        }
        out.push(r as i32);
    }
    Some(out)
}

fn round_c1(x: B, xa: Option<&BqnArr>, f: fn(f64) -> f64, name: &str) -> Result<PrimResult> {
    if x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64(f(x.o2f()))));
    }
    let arr = xa.ok_or_else(|| BqnError::Type(format!("{name}𝕩: 𝕩 must be a number or array")))?;
    if arr.el_type().is_int() {
        return Ok(PrimResult::Array(arr.clone()));
    }
    if let ArrData::F64(v) = &arr.data
        && let Some(r) = round_to_i32(v, f)
    {
        let mut out = BqnArr { shape: arr.shape.clone(), data: array::squeeze_i32(r), fill: None };
        out.fill = arr.fill;
        return Ok(PrimResult::Array(out));
    }
    pervasive_monad(x, xa, f, name)
}

/// Negate an integer array in its own type; the one value whose negation
/// does not fit (the type's MIN) sends the array through the f64 path.
fn negate_int(arr: &BqnArr) -> Option<ArrData> {
    Some(match &arr.data {
        ArrData::I8(v) => ArrData::I8(v.iter().map(|&x| x.checked_neg()).collect::<Option<Vec<_>>>()?),
        ArrData::I16(v) => ArrData::I16(v.iter().map(|&x| x.checked_neg()).collect::<Option<Vec<_>>>()?),
        ArrData::I32(v) => ArrData::I32(v.iter().map(|&x| x.checked_neg()).collect::<Option<Vec<_>>>()?),
        _ => return None,
    })
}

// NOTE: Recursive helper for pervasive monadic application on a single B value.
// Handles scalars, flat numeric arrays, and nested Boxed arrays.
fn pervasive_monad_b(x: B, scalar_fn: fn(f64) -> f64, name: &str) -> Result<B> {
    if x.is_f64() {
        return Ok(B::m_f64(scalar_fn(x.o2f())));
    }
    let arr = get_arr(x)
        .ok_or_else(|| BqnError::Type(format!("{name}𝕩: 𝕩 must be a number or array")))?;
    if arr.el_type().is_num() {
        return Ok(tag_arr(map_num(&arr, scalar_fn)));
    }
    // NOTE: Boxed array — recurse into each element (BQN pervasion)
    if let ArrData::Boxed(ref elems) = arr.data {
        let results: Result<Vec<B>> = elems
            .iter()
            .map(|&elem| pervasive_monad_b(elem, scalar_fn, name))
            .collect();
        let mut out = BqnArr::new_vec_b(results?);
        out.shape = arr.shape.clone();
        out.fill = arr.fill;
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
        return Ok(PrimResult::Array(map_num(arr, scalar_fn)));
    }
    // NOTE: Boxed array — recurse into each element (BQN pervasion)
    if let ArrData::Boxed(ref elems) = arr.data {
        let results: Result<Vec<B>> = elems
            .iter()
            .map(|&elem| pervasive_monad_b(elem, scalar_fn, name))
            .collect();
        let mut out = BqnArr::new_vec_b(results?);
        out.shape = arr.shape.clone();
        // NOTE: Preserve fill from source array (pervasion doesn't change fill structure)
        out.fill = arr.fill;
        return Ok(PrimResult::Array(out));
    }
    Err(BqnError::Type(format!("{name}𝕩: 𝕩 contained non-number")))
}

/// Scalar number fast path for monadic primitive index `op` (0..=9: `+-×÷⋆√⌊⌈|¬`).
/// Mirrors the scalar closures of the `*_c1` functions below.
#[inline(always)]
pub fn scalar_monad(op: u8, x: f64) -> f64 {
    match op {
        0 => x,
        1 => -x,
        2 => if x > 0.0 { 1.0 } else if x == 0.0 { 0.0 } else { -1.0 },
        3 => 1.0 / (x + 0.0),
        4 => x.exp(),
        5 => x.sqrt(),
        6 => x.floor(),
        7 => x.ceil(),
        8 => x.abs(),
        _ => 1.0 - x,
    }
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
        out.fill = arr.fill;
        return Ok(PrimResult::Array(out));
    }
    Err(BqnError::Type("+𝕩: 𝕩 must consist of numbers".into()))
}

// - monad: negate
pub fn sub_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if let Some(arr) = xa
        && let Some(data) = negate_int(arr)
    {
        return Ok(PrimResult::Array(BqnArr { shape: arr.shape.clone(), data, fill: arr.fill }));
    }
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
    round_c1(x, xa, f64::floor, "⌊")
}

// ⌈ monad: ceiling
pub fn ceil_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    round_c1(x, xa, f64::ceil, "⌈")
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

// √⁼ monad: square (x^2)
pub fn square_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    pervasive_monad(x, xa, |v| v * v, "√⁼")
}

// +˜⁼ monad: halve (x÷2)
pub fn halve_c1(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    pervasive_monad(x, xa, |v| v / 2.0, "+˜⁼")
}
