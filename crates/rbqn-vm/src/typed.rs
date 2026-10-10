//! Typed fast paths for 1-modifiers applied to native arithmetic primitives.
//! Each path applies the same scalar f64 operation in the same order as the
//! generic c2-per-element loop, so results are bit-identical; it only skips
//! the boxing and dispatch.

use rbqn_core::{ArrData, BqnArr, B};
use rbqn_prim::arith_dyad::{nan_max, nan_min};

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

/// Integer-typed (I8/I16/I32/Bit) arrays only.
fn is_int_arr(arr: &BqnArr) -> bool {
    matches!(arr.data, ArrData::I8(_) | ArrData::I16(_) | ArrData::I32(_) | ArrData::Bit(_))
}

/// Largest magnitude at which every integer is exactly representable in f64.
const EXACT: i64 = 1 << 53;

/// Number of set bits among the first n bits of a packed Bit array.
fn bit_count(v: &[u64], n: usize) -> i64 {
    let full = n / 64;
    let mut c: i64 = v[..full].iter().map(|w| w.count_ones() as i64).sum();
    if !n.is_multiple_of(64) {
        c += (v[full] & ((1u64 << (n % 64)) - 1)).count_ones() as i64;
    }
    c
}

/// Exact i64 fold for + × ⌊ ⌈ over integer arrays. All of these are
/// associative and commutative on integers, so order does not matter.
/// None when not applicable (non-integer init, × leaving the exactly
/// representable range) so the caller falls back to the f64 chain.
fn int_fold(idx: usize, arr: &BqnArr, init: Option<f64>) -> Option<f64> {
    if !is_int_arr(arr) || !matches!(idx, ADD | MUL | MIN | MAX) {
        return None;
    }
    let n = arr.ia();
    let init = match init {
        Some(a) if a.fract() == 0.0 && a.abs() < EXACT as f64 => Some(a as i64),
        Some(_) => return None,
        None => None,
    };
    let r: i64 = match (&arr.data, idx) {
        (ArrData::I32(v), ADD) => v.iter().map(|&a| a as i64).sum::<i64>() + init.unwrap_or(0),
        (ArrData::I16(v), ADD) => v.iter().map(|&a| a as i64).sum::<i64>() + init.unwrap_or(0),
        (ArrData::I8(v), ADD) => v.iter().map(|&a| a as i64).sum::<i64>() + init.unwrap_or(0),
        (ArrData::Bit(v), ADD) => bit_count(v, n) + init.unwrap_or(0),
        (ArrData::Bit(v), MAX) => {
            let c = (bit_count(v, n) > 0) as i64;
            init.map_or(c, |i| i.max(c))
        }
        (ArrData::Bit(v), MIN) => {
            let c = (bit_count(v, n) == n as i64) as i64;
            init.map_or(c, |i| i.min(c))
        }
        (ArrData::Bit(v), MUL) => {
            let c = (bit_count(v, n) == n as i64) as i64;
            init.map_or(c, |i| i * c)
        }
        (ArrData::I32(v), MAX) => v.iter().copied().max().map(|a| a as i64).into_iter().chain(init).max()?,
        (ArrData::I16(v), MAX) => v.iter().copied().max().map(|a| a as i64).into_iter().chain(init).max()?,
        (ArrData::I8(v), MAX) => v.iter().copied().max().map(|a| a as i64).into_iter().chain(init).max()?,
        (ArrData::I32(v), MIN) => v.iter().copied().min().map(|a| a as i64).into_iter().chain(init).min()?,
        (ArrData::I16(v), MIN) => v.iter().copied().min().map(|a| a as i64).into_iter().chain(init).min()?,
        (ArrData::I8(v), MIN) => v.iter().copied().min().map(|a| a as i64).into_iter().chain(init).min()?,
        (_, MUL) => {
            // Every partial product must stay exactly representable in f64 so
            // the result equals the f64 chain; otherwise fall back.
            let mut acc = init.unwrap_or(1);
            let ok = match &arr.data {
                ArrData::I32(v) => v.iter().all(|&a| mul_exact(&mut acc, a as i64)),
                ArrData::I16(v) => v.iter().all(|&a| mul_exact(&mut acc, a as i64)),
                ArrData::I8(v) => v.iter().all(|&a| mul_exact(&mut acc, a as i64)),
                _ => false,
            };
            if !ok {
                return None;
            }
            acc
        }
        _ => return None,
    };
    if r.abs() > EXACT && idx != ADD {
        return None;
    }
    Some(r as f64)
}

#[inline]
fn mul_exact(acc: &mut i64, a: i64) -> bool {
    match acc.checked_mul(a) {
        Some(p) if p.abs() <= EXACT => {
            *acc = p;
            true
        }
        _ => false,
    }
}

/// Same observable result as squeeze_num on the f64 values.
fn squeeze_i64(v: Vec<i64>) -> BqnArr {
    let (mn, mx) = v.iter().fold((i64::MAX, i64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
    if mn >= 0 && mx <= 1 {
        let mut words = vec![0u64; v.len().div_ceil(64)];
        for (i, &x) in v.iter().enumerate() {
            words[i / 64] |= (x as u64) << (i % 64);
        }
        BqnArr { shape: vec![v.len()], data: ArrData::Bit(words), fill: None }
    } else if mn >= i8::MIN as i64 && mx <= i8::MAX as i64 {
        BqnArr { shape: vec![v.len()], data: ArrData::I8(v.iter().map(|&x| x as i8).collect()), fill: None }
    } else if mn >= i16::MIN as i64 && mx <= i16::MAX as i64 {
        BqnArr { shape: vec![v.len()], data: ArrData::I16(v.iter().map(|&x| x as i16).collect()), fill: None }
    } else if mn >= i32::MIN as i64 && mx <= i32::MAX as i64 {
        BqnArr::new_vec_i32(v.iter().map(|&x| x as i32).collect())
    } else {
        BqnArr::new_vec_f64(v.iter().map(|&x| x as f64).collect())
    }
}

/// Exact i64 scan for + ⌊ ⌈ over integer arrays (n ≥ 1).
fn int_scan(idx: usize, arr: &BqnArr) -> Option<BqnArr> {
    if !is_int_arr(arr) || !matches!(idx, ADD | MIN | MAX) {
        return None;
    }
    let n = arr.ia();
    let mut out: Vec<i64> = Vec::with_capacity(n);
    macro_rules! run {
        ($it:expr) => {{
            let mut it = $it;
            let mut acc: i64 = it.next()?;
            out.push(acc);
            match idx {
                ADD => for a in it { acc += a; out.push(acc); },
                MIN => for a in it { acc = acc.min(a); out.push(acc); },
                _ => for a in it { acc = acc.max(a); out.push(acc); },
            }
        }};
    }
    match &arr.data {
        ArrData::I32(v) => run!(v.iter().map(|&a| a as i64)),
        ArrData::I16(v) => run!(v.iter().map(|&a| a as i64)),
        ArrData::I8(v) => run!(v.iter().map(|&a| a as i64)),
        ArrData::Bit(v) => run!((0..n).map(|i| ((v[i / 64] >> (i % 64)) & 1) as i64)),
        _ => return None,
    }
    Some(squeeze_i64(out))
}

/// F´ (or 𝕨 F´) on a numeric list with F one of + × ⌊ ⌈ ∧ ∨.
/// Returns None when the fast path does not apply.
pub fn fold(f: B, arr: &BqnArr, init: Option<f64>) -> Option<B> {
    if arr.rank() != 1 || (init.is_none() && arr.ia() == 0) {
        return None;
    }
    let idx = native_fn_idx(f)?;
    if let Some(r) = int_fold(idx, arr, init) {
        return Some(B::m_f64(r));
    }
    let r = match idx {
        ADD => fold_with(arr, init, |a, b| a + b),
        MUL | AND => fold_with(arr, init, |a, b| a * b),
        MIN => fold_with(arr, init, nan_min),
        MAX => fold_with(arr, init, nan_max),
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
    let idx = native_fn_idx(f)?;
    if let Some(mut out) = int_scan(idx, arr) {
        out.fill = arr.fill;
        return Some(out);
    }
    let r = match idx {
        ADD => scan_with(arr, |a, b| a + b),
        MUL | AND => scan_with(arr, |a, b| a * b),
        MIN => scan_with(arr, nan_min),
        MAX => scan_with(arr, nan_max),
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
    if !matches!(idx, 0 | 1 | 2 | 3 | 6 | 7 | 8 | 12..=17) {
        return None;
    }
    let mut shape = warr.shape.clone();
    shape.extend_from_slice(&xarr.shape);
    if let Some(mut r) = int_table(idx, warr, xarr) {
        r.shape = shape;
        return Some(r);
    }
    if idx >= 12 {
        return None;
    }
    let w = f64_view(warr)?;
    let x = f64_view(xarr)?;
    let r = match idx {
        0 => table_with(&w, &x, |a, b| a + b),
        1 => table_with(&w, &x, |a, b| a - b),
        2 => table_with(&w, &x, |a, b| a * b),
        3 => table_with(&w, &x, |a, b| a / (b + 0.0)),
        6 => table_with(&w, &x, nan_min),
        7 => table_with(&w, &x, nan_max),
        _ => table_with(&w, &x, |a, b| pfmod(b, a)),
    };
    let mut out = rbqn_core::array::squeeze_num(BqnArr::new_vec_f64(r));
    out.shape = shape;
    out.fill = Some(B::m_i32(0));
    Some(out)
}

/// Collect `f(a, b)` for every (a, b) in w x x, row-major. Inner loop is over a
/// native slice so it vectorizes.
fn tab<T, F: Fn(i32, i32) -> T>(w: &[i32], x: &[i32], f: F) -> Vec<T> {
    let mut out = Vec::with_capacity(w.len() * x.len());
    for &a in w {
        out.extend(x.iter().map(|&b| f(a, b)));
    }
    out
}

/// Pack 0/1 bytes into a Bit word vector.
fn pack_bytes(v: &[u8]) -> Vec<u64> {
    let mut words = vec![0u64; v.len().div_ceil(64)];
    for (k, c) in v.chunks(64).enumerate() {
        words[k] = c.iter().enumerate().fold(0u64, |acc, (i, &b)| acc | ((b as u64) << i));
    }
    words
}

/// Comparison table packed straight into Bit words, one row buffer at a time.
fn cmp_table<F: Fn(i32, i32) -> bool>(w: &[i32], x: &[i32], f: F) -> Vec<u64> {
    let total = w.len() * x.len();
    let mut words = vec![0u64; total.div_ceil(64)];
    let mut buf = vec![0u8; x.len()];
    let mut pos = 0usize;
    for &a in w {
        buf.iter_mut().zip(x).for_each(|(o, &b)| *o = f(a, b) as u8);
        for c in buf.chunks(64) {
            let v = c.iter().enumerate().fold(0u64, |acc, (i, &b)| acc | ((b as u64) << i));
            let sh = pos % 64;
            words[pos / 64] |= v << sh;
            if sh != 0 && sh + c.len() > 64 {
                words[pos / 64 + 1] |= v >> (64 - sh);
            }
            pos += c.len();
        }
    }
    words
}

/// Integer-only table for + - × ⌊ ⌈ and < > ≠ = ≤ ≥ on Bit/I8/I16/I32 operands
/// (shape not set). The arithmetic result range is known from the operand
/// ranges (exact corners), so the output is written directly in the smallest
/// type; comparisons are packed to Bit. None on non-integer input, other ops,
/// or a range outside i32 (f64 path then handles it exactly).
fn int_table(idx: usize, warr: &BqnArr, xarr: &BqnArr) -> Option<BqnArr> {
    let is_int = |a: &BqnArr| matches!(a.data, ArrData::I8(_) | ArrData::I16(_) | ArrData::I32(_) | ArrData::Bit(_));
    if !is_int(warr) || !is_int(xarr) {
        return None;
    }
    let w = warr.i32_iter().ok()?;
    let x = xarr.i32_iter().ok()?;
    let fill = Some(B::m_i32(0));
    let mk = |data| Some(BqnArr { shape: vec![], data, fill });
    if w.is_empty() || x.is_empty() {
        return mk(if idx >= 12 { ArrData::Bit(vec![]) } else { ArrData::I32(vec![]) });
    }
    if idx >= 12 {
        let r = match idx {
            12 => cmp_table(&w, &x, |a, b| a < b),
            13 => cmp_table(&w, &x, |a, b| a > b),
            14 => cmp_table(&w, &x, |a, b| a != b),
            15 => cmp_table(&w, &x, |a, b| a == b),
            16 => cmp_table(&w, &x, |a, b| a <= b),
            _ => cmp_table(&w, &x, |a, b| a >= b),
        };
        return mk(ArrData::Bit(r));
    }
    let mm = |v: &[i32]| v.iter().fold((i32::MAX, i32::MIN), |(l, h), &e| (l.min(e), h.max(e)));
    let ((wl, wh), (xl, xh)) = (mm(&w), mm(&x));
    let (wl, wh, xl, xh) = (wl as i64, wh as i64, xl as i64, xh as i64);
    let (lo, hi) = match idx {
        0 => (wl + xl, wh + xh),
        1 => (wl - xh, wh - xl),
        2 => {
            let p = [wl * xl, wl * xh, wh * xl, wh * xh];
            (*p.iter().min()?, *p.iter().max()?)
        }
        6 => (wl.min(xl), wh.min(xh)),
        7 => (wl.max(xl), wh.max(xh)),
        _ => return None,
    };
    if lo < i32::MIN as i64 || hi > i32::MAX as i64 {
        return None;
    }
    macro_rules! go {
        ($t:ty) => {
            match idx {
                0 => tab(&w, &x, |a, b| a.wrapping_add(b) as $t),
                1 => tab(&w, &x, |a, b| a.wrapping_sub(b) as $t),
                2 => tab(&w, &x, |a, b| a.wrapping_mul(b) as $t),
                6 => tab(&w, &x, |a, b| a.min(b) as $t),
                _ => tab(&w, &x, |a, b| a.max(b) as $t),
            }
        };
    }
    if lo >= 0 && hi <= 1 {
        let r: Vec<u8> = go!(u8);
        mk(ArrData::Bit(pack_bytes(&r)))
    } else if lo >= i8::MIN as i64 && hi <= i8::MAX as i64 {
        mk(ArrData::I8(go!(i8)))
    } else if lo >= i16::MIN as i64 && hi <= i16::MAX as i64 {
        mk(ArrData::I16(go!(i16)))
    } else {
        mk(ArrData::I32(go!(i32)))
    }
}
