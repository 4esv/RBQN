//! Typed fast paths for 1-modifiers applied to native arithmetic primitives.
//! Each path applies the same scalar f64 operation in the same order as the
//! generic c2-per-element loop, so results are bit-identical; it only skips
//! the boxing and dispatch.

use rbqn_core::{ArrData, BqnArr, B};

// Runtime primitive indices (see rbqn_prim::get_runtime).
const ADD: usize = 0;
const MUL: usize = 2;
const MIN: usize = 6;
const MAX: usize = 7;
const AND: usize = 10;
const OR: usize = 11;

/// Runtime index of `f` if it is a native primitive function.
pub fn native_fn_idx(f: B) -> Option<usize> {
    if !f.is_fun() {
        return None;
    }
    let id = (f.0 & 0xFFFFFFFFFFFF) >> 3;
    match crate::derive::get_derived(id).kind {
        crate::derive::DerivedKind::NativeFn { prim_idx } => Some(prim_idx),
        _ => None,
    }
}

/// Element i of a numeric array as f64, or None for non-numeric data.
#[inline]
fn bit_at(v: &[u64], i: usize) -> f64 {
    ((v[i / 64] >> (i % 64)) & 1) as f64
}

/// Right-to-left fold `x0 op (x1 op (... op (xn-1 op init)))` over a numeric array.
/// With `init` None the last element seeds the accumulator. Requires ia ≥ 1 when init is None.
fn fold_with<F: Fn(f64, f64) -> f64>(arr: &BqnArr, init: Option<f64>, op: F) -> Option<f64> {
    let n = arr.ia();
    let (mut acc, end) = match init {
        Some(a) => (a, n),
        None => (get_num(arr, n - 1)?, n - 1),
    };
    match &arr.data {
        ArrData::F64(v) => for &a in v[..end].iter().rev() { acc = op(a, acc); },
        ArrData::I32(v) => for &a in v[..end].iter().rev() { acc = op(a as f64, acc); },
        ArrData::I16(v) => for &a in v[..end].iter().rev() { acc = op(a as f64, acc); },
        ArrData::I8(v) => for &a in v[..end].iter().rev() { acc = op(a as f64, acc); },
        ArrData::Bit(v) => for i in (0..end).rev() { acc = op(bit_at(v, i), acc); },
        _ => return None,
    }
    Some(acc)
}

#[inline]
fn get_num(arr: &BqnArr, i: usize) -> Option<f64> {
    Some(match &arr.data {
        ArrData::F64(v) => v[i],
        ArrData::I32(v) => v[i] as f64,
        ArrData::I16(v) => v[i] as f64,
        ArrData::I8(v) => v[i] as f64,
        ArrData::Bit(v) => bit_at(v, i),
        _ => return None,
    })
}

/// F´ (or 𝕨 F´) on a numeric list with F one of + × ⌊ ⌈ ∧ ∨.
/// Returns None when the fast path does not apply.
pub fn fold(f: B, arr: &BqnArr, init: Option<f64>) -> Option<B> {
    if arr.rank() != 1 || (init.is_none() && arr.ia() == 0) {
        return None;
    }
    let r = match native_fn_idx(f)? {
        ADD => fold_with(arr, init, |a, b| a + b),
        MUL | AND => fold_with(arr, init, |a, b| a * b),
        MIN => fold_with(arr, init, f64::min),
        MAX => fold_with(arr, init, f64::max),
        OR => fold_with(arr, init, |a, b| a + b - a * b),
        _ => None,
    }?;
    Some(B::m_f64(r))
}

/// Left-to-right scan `r0 = x0, ri = r(i-1) op xi` over a numeric list.
fn scan_with<F: Fn(f64, f64) -> f64>(arr: &BqnArr, op: F) -> Option<Vec<f64>> {
    let n = arr.ia();
    let mut out = Vec::with_capacity(n);
    let mut acc = get_num(arr, 0)?;
    out.push(acc);
    macro_rules! run {
        ($it:expr) => { for a in $it { acc = op(acc, a); out.push(acc); } };
    }
    match &arr.data {
        ArrData::F64(v) => run!(v[1..].iter().copied()),
        ArrData::I32(v) => run!(v[1..].iter().map(|&a| a as f64)),
        ArrData::I16(v) => run!(v[1..].iter().map(|&a| a as f64)),
        ArrData::I8(v) => run!(v[1..].iter().map(|&a| a as f64)),
        ArrData::Bit(v) => run!((1..n).map(|i| bit_at(v, i))),
        _ => return None,
    }
    Some(out)
}

/// F` on a non-empty numeric list with F one of + × ⌊ ⌈ ∧ ∨. Result is squeezed.
pub fn scan(f: B, arr: &BqnArr) -> Option<BqnArr> {
    if arr.rank() != 1 || arr.ia() == 0 {
        return None;
    }
    let r = match native_fn_idx(f)? {
        ADD => scan_with(arr, |a, b| a + b),
        MUL | AND => scan_with(arr, |a, b| a * b),
        MIN => scan_with(arr, f64::min),
        MAX => scan_with(arr, f64::max),
        OR => scan_with(arr, |a, b| a + b - a * b),
        _ => None,
    }?;
    let mut out = rbqn_core::array::squeeze_num(BqnArr::new_vec_f64(r));
    out.fill = arr.fill;
    Some(out)
}

/// Numeric contents as f64 (borrowed when already F64), or None for chars/boxed.
fn f64_view(arr: &BqnArr) -> Option<std::borrow::Cow<'_, [f64]>> {
    use std::borrow::Cow;
    Some(match &arr.data {
        ArrData::F64(v) => Cow::Borrowed(v.as_slice()),
        ArrData::I32(v) => Cow::Owned(v.iter().map(|&a| a as f64).collect()),
        ArrData::I16(v) => Cow::Owned(v.iter().map(|&a| a as f64).collect()),
        ArrData::I8(v) => Cow::Owned(v.iter().map(|&a| a as f64).collect()),
        ArrData::Bit(v) => Cow::Owned((0..arr.ia()).map(|i| bit_at(v, i)).collect()),
        _ => return None,
    })
}

/// Same as rbqn_prim's ⊣|⊢ helper: floored modulus with the sign of b.
fn pfmod(a: f64, b: f64) -> f64 {
    let r = a % b;
    if (a < 0.0) != (b < 0.0) && r != 0.0 { r + b } else { r }
}

fn table_with<F: Fn(f64, f64) -> f64>(w: &[f64], x: &[f64], op: F) -> Vec<f64> {
    let mut out = Vec::with_capacity(w.len() * x.len());
    for &a in w {
        out.extend(x.iter().map(|&b| op(a, b)));
    }
    out
}

/// 𝕨 F⌜ 𝕩 on numeric arrays with F one of + - × ÷ ⌊ ⌈ |, using the same scalar
/// ops as the primitives. Result shape is 𝕨's shape then 𝕩's, squeezed.
pub fn table(f: B, warr: &BqnArr, xarr: &BqnArr) -> Option<BqnArr> {
    let idx = native_fn_idx(f)?;
    if !matches!(idx, 0 | 1 | 2 | 3 | 6 | 7 | 8) {
        return None;
    }
    let mut shape = warr.shape.clone();
    shape.extend_from_slice(&xarr.shape);
    if let Some(r) = int_table(idx, warr, xarr) {
        return Some(BqnArr { shape, data: ArrData::I32(r), fill: Some(B::m_i32(0)) });
    }
    let w = f64_view(warr)?;
    let x = f64_view(xarr)?;
    let r = match idx {
        0 => table_with(&w, &x, |a, b| a + b),
        1 => table_with(&w, &x, |a, b| a - b),
        2 => table_with(&w, &x, |a, b| a * b),
        3 => table_with(&w, &x, |a, b| a / (b + 0.0)),
        6 => table_with(&w, &x, f64::min),
        7 => table_with(&w, &x, f64::max),
        _ => table_with(&w, &x, |a, b| pfmod(b, a)),
    };
    let mut out = rbqn_core::array::squeeze_num(BqnArr::new_vec_f64(r));
    out.shape = shape;
    out.fill = Some(B::m_i32(0));
    Some(out)
}

/// Integer-only table for + - × ⌊ ⌈ when every result fits in i32 (exact, so it
/// equals the f64 result). None on non-integer input, other ops, or overflow.
fn int_table(idx: usize, warr: &BqnArr, xarr: &BqnArr) -> Option<Vec<i32>> {
    let is_int = |a: &BqnArr| matches!(a.data, ArrData::I8(_) | ArrData::I16(_) | ArrData::I32(_) | ArrData::Bit(_));
    if !is_int(warr) || !is_int(xarr) {
        return None;
    }
    let w = warr.i32_iter().ok()?;
    let x = xarr.i32_iter().ok()?;
    let mut out: Vec<i32> = Vec::with_capacity(w.len() * x.len());
    macro_rules! run {
        ($op:expr) => {{
            let op = $op;
            for &a in &w {
                let a = a as i64;
                for &b in &x {
                    out.push(i32::try_from(op(a, b as i64)).ok()?);
                }
            }
        }};
    }
    match idx {
        0 => run!(|a: i64, b: i64| a + b),
        1 => run!(|a: i64, b: i64| a - b),
        2 => run!(|a: i64, b: i64| a * b),
        6 => run!(|a: i64, b: i64| a.min(b)),
        7 => run!(|a: i64, b: i64| a.max(b)),
        _ => return None,
    }
    Some(out)
}
