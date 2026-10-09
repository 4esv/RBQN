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
