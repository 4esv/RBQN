use std::sync::OnceLock;
use rbqn_core::*;
use rbqn_core::array;
use crate::dispatch::PrimResult;

// NOTE: GPU grade hook — set by rbqn crate at startup via register_gpu_grade.
// Returns permutation indices (like grade_up/grade_down), or None for CPU fallback.
type GpuGradeFn = fn(arr: &BqnArr, ascending: bool) -> Option<BqnArr>;
static GPU_GRADE_HOOK: OnceLock<GpuGradeFn> = OnceLock::new();

pub fn register_gpu_grade(f: GpuGradeFn) {
    let _ = GPU_GRADE_HOOK.set(f);
}

// NOTE: GPU sort hook — set by rbqn crate at startup via register_gpu_sort.
// Returns sorted values (like sort_up/sort_down), or None for CPU fallback.
type GpuSortFn = fn(arr: &BqnArr, ascending: bool) -> Option<BqnArr>;
static GPU_SORT_HOOK: OnceLock<GpuSortFn> = OnceLock::new();

pub fn register_gpu_sort(f: GpuSortFn) {
    let _ = GPU_SORT_HOOK.set(f);
}

/// Recursively check if a B value contains any non-data (function/modifier) values.
/// Returns true if the value or any nested array element is a function/modifier.
fn contains_non_data(v: B) -> bool {
    rbqn_core::stack::guard();
    if v.is_f64() || v.is_c32() { return false; }
    if v.is_fun() || v.is_md1() || v.is_md2() { return true; }
    if v.is_arr()
        && let Some(arr) = get_arr(v) {
            for i in 0..arr.ia() {
                if let Ok(elem) = arr.get(i)
                    && contains_non_data(elem) { return true; }
            }
        }
    false
}

// PERF: fast paths for rank-1 typed arrays. Keys are mapped to an order-
// preserving unsigned integer; grade uses counting sort for small ranges and
// (key, index) pairs otherwise, which is stable because the index breaks ties.
fn bit_get(v: &[u64], i: usize) -> bool { (v[i / 64] >> (i % 64)) & 1 == 1 }

#[inline]
fn i32_key(x: i32) -> u32 { (x as u32) ^ 0x8000_0000 }

#[inline]
fn f64_key(x: f64) -> u64 {
    let b = (x + 0.0).to_bits(); // NOTE: −0+0 = +0, matching partial_cmp equality.
    if b >> 63 == 1 { !b } else { b | (1 << 63) }
}

/// Stable counting-sort scatter: `cnt` holds exclusive prefix sums per bucket.
#[inline(always)]
fn scatter<T: Copy>(vals: &[T], bucket: impl Fn(T) -> usize, cnt: &mut [u32], out: &mut [i32]) {
    debug_assert_eq!(vals.len(), out.len());
    let o = out.as_mut_ptr();
    for (i, &x) in vals.iter().enumerate() {
        let b = bucket(x);
        // SAFETY: b < cnt.len() (bucket is in range by construction of cnt), and
        // the prefix sums of an exact histogram place every write in 0..n.
        unsafe {
            let c = cnt.get_unchecked_mut(b);
            *o.add(*c as usize) = i as i32;
            *c += 1;
        }
    }
}

/// Stable grade of small-width data (I8/C8/Bit as u8 index, I16/C16 as u16):
/// fixed-size table, no min/max pass. `idx` must map order-preservingly into 0..K.
/// PERF: the input is split into 4 contiguous chunks, each with its own histogram
/// and cursor table, and the 4 chunks are processed interleaved. With few distinct
/// keys a single table serialises on store-to-load forwarding of the same counter;
/// 4 independent tables keep 4 chains in flight. Chunk c's cursors start after
/// all earlier chunks' entries for the same bucket, so the result stays stable.
fn grade_small<T: Copy, const K: usize>(vals: &[T], idx: impl Fn(T) -> usize, ascending: bool) -> Vec<i32> {
    const C: usize = 4;
    let n = vals.len();
    if n == 0 { return Vec::new(); }
    let ordered = if ascending { vals.windows(2).all(|w| idx(w[0]) <= idx(w[1])) }
                  else { vals.windows(2).all(|w| idx(w[0]) >= idx(w[1])) };
    if ordered { return (0..n as i32).collect(); }
    let q = n / C;
    let mut cnt = vec![[0u32; C]; K];
    {
        let ch: [&[T]; C] = std::array::from_fn(|c| &vals[c * q..(c + 1) * q]);
        for j in 0..q {
            for (c, s) in ch.iter().enumerate() {
                // SAFETY: j < q = s.len(); idx & (K-1) < K = cnt.len().
                unsafe { cnt.get_unchecked_mut(idx(*s.get_unchecked(j)) & (K - 1))[c] += 1; }
            }
        }
        for &x in &vals[C * q..] { cnt[idx(x) & (K - 1)][C - 1] += 1; }
    }
    let mut sum = 0u32;
    let mut pre = |row: &mut [u32; C]| for c in row.iter_mut() { let t = *c; *c = sum; sum += t; };
    if ascending { cnt.iter_mut().for_each(&mut pre); } else { cnt.iter_mut().rev().for_each(&mut pre); }
    let mut out: Vec<i32> = Vec::with_capacity(n);
    let o = out.as_mut_ptr();
    // SAFETY: the histogram is exact, so the cursors place every index 0..n into
    // a distinct slot 0..n; all n slots are written before set_len.
    unsafe {
        for j in 0..q {
            for c in 0..C {
                let i = c * q + j;
                let k = cnt.get_unchecked_mut(idx(*vals.get_unchecked(i)) & (K - 1));
                *o.add(k[c] as usize) = i as i32;
                k[c] += 1;
            }
        }
        for i in C * q..n {
            let k = cnt.get_unchecked_mut(idx(*vals.get_unchecked(i)) & (K - 1));
            *o.add(k[C - 1] as usize) = i as i32;
            k[C - 1] += 1;
        }
        out.set_len(n);
    }
    out
}

/// Stable grade of values mapped to order-preserving u32 keys.
/// Descending keeps equal keys in original order (BQN semantics).
fn grade_keyed<T: Copy>(vals: &[T], key: impl Fn(T) -> u32, ascending: bool) -> Vec<i32> {
    let n = vals.len();
    if n == 0 { return Vec::new(); }
    let (mut lo, mut hi) = (u32::MAX, 0u32);
    for &x in vals { let k = key(x); lo = lo.min(k); hi = hi.max(k); }
    // PERF: already-ordered input (e.g. ↕n) is the identity permutation.
    let ordered = if ascending { vals.windows(2).all(|w| key(w[0]) <= key(w[1])) }
                  else { vals.windows(2).all(|w| key(w[0]) >= key(w[1])) };
    if ordered { return (0..n as i32).collect(); }
    let mut out = vec![0i32; n];
    let range = (hi - lo) as usize + 1;
    if range <= n.max(1 << 16) {
        let bucket = |k: u32| if ascending { (k - lo) as usize } else { (hi - k) as usize };
        let mut cnt = vec![0u32; range];
        for &x in vals { cnt[bucket(key(x))] += 1; }
        let mut sum = 0u32;
        for c in cnt.iter_mut() { let t = *c; *c = sum; sum += t; }
        scatter(vals, |x| bucket(key(x)), &mut cnt, &mut out);
    } else {
        let mut pairs: Vec<u64> = vals.iter().enumerate()
            .map(|(i, &x)| {
                let k = if ascending { key(x) } else { !key(x) };
                ((k as u64) << 32) | i as u64
            })
            .collect();
        pairs.sort_unstable();
        for (o, p) in out.iter_mut().zip(pairs) { *o = p as u32 as i32; }
    }
    out
}

fn grade_u64(keys: &[u64], ascending: bool) -> Vec<i32> {
    let mut pairs: Vec<(u64, u32)> = keys.iter().enumerate()
        .map(|(i, &k)| (if ascending { k } else { !k }, i as u32))
        .collect();
    pairs.sort_unstable();
    pairs.into_iter().map(|(_, i)| i as i32).collect()
}

/// Returns None when the fast path does not apply (rank ≠ 1, boxed, NaN present).
fn fast_grade(arr: &BqnArr, ascending: bool) -> Option<Vec<i32>> {
    if arr.rank() != 1 { return None; }
    let n = arr.ia();
    Some(match &arr.data {
        ArrData::Bit(v) => {
            let bits: Vec<u8> = (0..n).map(|i| bit_get(v, i) as u8).collect();
            grade_small::<u8, 2>(&bits, |x| x as usize, ascending)
        }
        ArrData::I8(v) => grade_small::<i8, 256>(v, |x| (x as u8 ^ 0x80) as usize, ascending),
        ArrData::I16(v) => grade_small::<i16, 65536>(v, |x| (x as u16 ^ 0x8000) as usize, ascending),
        ArrData::C8(v) => grade_small::<u8, 256>(v, |x| x as usize, ascending),
        ArrData::I32(v) => grade_keyed(v, i32_key, ascending),
        ArrData::C16(v) => grade_keyed(v, |x| x as u32, ascending),
        ArrData::C32(v) => grade_keyed(v, |x| x, ascending),
        ArrData::F64(v) => {
            if v.iter().any(|x| x.is_nan()) { return None; }
            grade_u64(&v.iter().map(|&x| f64_key(x)).collect::<Vec<_>>(), ascending)
        }
        ArrData::Boxed(_) => return None,
    })
}

/// Sort a slice of integer-like values (given an order-preserving key) in place.
/// Small key ranges use histogram expansion; otherwise sort_unstable (values
/// with equal keys are identical, so stability is unobservable).
fn sort_ints<T: Copy + Ord>(v: &mut [T], key: impl Fn(T) -> i64, from: impl Fn(i64) -> T, ascending: bool) {
    let n = v.len();
    if n == 0 { return; }
    let (mut lo, mut hi) = (i64::MAX, i64::MIN);
    for &x in v.iter() { let k = key(x); lo = lo.min(k); hi = hi.max(k); }
    let range = (hi - lo) as usize + 1;
    if range <= n.max(1 << 16) {
        let mut cnt = vec![0usize; range];
        for &x in v.iter() { cnt[(key(x) - lo) as usize] += 1; }
        let mut pos = 0;
        let mut emit = |b: usize, c: usize, v: &mut [T]| {
            let val = from(lo + b as i64);
            v[pos..pos + c].fill(val);
            pos += c;
        };
        if ascending {
            for (b, &c) in cnt[..range].iter().enumerate() { if c > 0 { emit(b, c, v); } }
        } else {
            for (b, &c) in cnt[..range].iter().enumerate().rev() { if c > 0 { emit(b, c, v); } }
        }
    } else if ascending {
        v.sort_unstable();
    } else {
        v.sort_unstable_by(|a, b| b.cmp(a));
    }
}

/// Sort small-width data via a fixed K-entry histogram (4 interleaved tables to
/// avoid a serial counter chain), writing the output directly with no input copy.
/// `idx` maps order-preservingly into 0..K and `val` inverts it.
fn sort_small<T: Copy, const K: usize>(v: &[T], idx: impl Fn(T) -> usize, val: impl Fn(usize) -> T, ascending: bool) -> Vec<T> {
    let mut cnt = vec![[0usize; 4]; K];
    let (ch, rest) = v.as_chunks::<4>();
    for w in ch {
        for (c, &x) in w.iter().enumerate() {
            // SAFETY: idx & (K-1) < K = cnt.len().
            unsafe { cnt.get_unchecked_mut(idx(x) & (K - 1))[c] += 1; }
        }
    }
    for &x in rest { cnt[idx(x) & (K - 1)][0] += 1; }
    let mut out = Vec::with_capacity(v.len());
    let mut emit = |b: usize| {
        let c: usize = cnt[b].iter().sum();
        if c > 0 { out.extend(std::iter::repeat_n(val(b), c)); }
    };
    if ascending { (0..K).for_each(&mut emit); } else { (0..K).rev().for_each(&mut emit); }
    out
}

fn fast_sort(arr: &BqnArr, ascending: bool) -> Option<BqnArr> {
    if arr.rank() != 1 { return None; }
    let n = arr.ia();
    let data = match &arr.data {
        ArrData::Bit(v) => {
            let ones = (0..n).filter(|&i| bit_get(v, i)).count();
            let mut w = vec![0u64; n.div_ceil(64)];
            let range = if ascending { n - ones..n } else { 0..ones };
            for i in range { w[i / 64] |= 1 << (i % 64); }
            ArrData::Bit(w)
        }
        ArrData::I8(v) => ArrData::I8(sort_small::<i8, 256>(v, |x| (x as u8 ^ 0x80) as usize, |b| (b as u8 ^ 0x80) as i8, ascending)),
        ArrData::I16(v) => { let mut v = v.clone(); sort_ints(&mut v, |x| x as i64, |k| k as i16, ascending); ArrData::I16(v) }
        ArrData::I32(v) => { let mut v = v.clone(); sort_ints(&mut v, |x| x as i64, |k| k as i32, ascending); ArrData::I32(v) }
        ArrData::C8(v) => ArrData::C8(sort_small::<u8, 256>(v, |x| x as usize, |b| b as u8, ascending)),
        ArrData::C16(v) => { let mut v = v.clone(); sort_ints(&mut v, |x| x as i64, |k| k as u16, ascending); ArrData::C16(v) }
        ArrData::C32(v) => { let mut v = v.clone(); sort_ints(&mut v, |x| x as i64, |k| k as u32, ascending); ArrData::C32(v) }
        ArrData::F64(v) => {
            if v.iter().any(|x| x.is_nan()) { return None; }
            let mut v = v.clone();
            // NOTE: stable so −0 and +0 keep original relative order, as before.
            if ascending { v.sort_by_key(|&x| f64_key(x)); } else { v.sort_by_key(|&x| std::cmp::Reverse(f64_key(x))); }
            ArrData::F64(v)
        }
        ArrData::Boxed(_) => return None,
    };
    Some(BqnArr { shape: arr.shape.clone(), data, fill: arr.fill })
}

fn grade(arr: &BqnArr, ascending: bool) -> Result<Vec<i32>> {
    if let Some(idx) = fast_grade(arr, ascending) { return Ok(idx); }
    // NOTE: BQN grade operates on the first axis (rows for 2D arrays).
    // For rank-1 arrays, each element is a cell. For rank-N, each cell is a
    // row-major sub-array. The result length equals arr.shape[0] (nrows).

    // Validate: elements must be sortable (numbers, chars, or arrays thereof).
    // Functions and modifiers are not sortable.
    if arr.el_type() == ElType::B {
        // Boxed array: check each element
        for i in 0..arr.ia() {
            let v = arr.get(i)?;
            if !v.is_f64() && !v.is_c32() && !v.is_arr() {
                return Err(BqnError::Type(
                    "⍋/⍒𝕩: elements must be numbers, characters, or arrays".into()
                ));
            }
        }
    }

    let nrows = if arr.rank() == 0 { 1 } else { arr.shape[0] };
    let cell_size: usize = if arr.rank() <= 1 { 1 } else { arr.shape[1..].iter().product() };
    let mut indices: Vec<i32> = (0..nrows as i32).collect();

    if cell_size == 1 && arr.el_type().is_num() {
        // Fast path: rank-1 numeric array — compare scalars directly
        let vals = arr.f64_iter()?;
        indices.sort_by(|&a, &b| {
            let va = vals[a as usize];
            let vb = vals[b as usize];
            if ascending {
                va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                vb.partial_cmp(&va).unwrap_or(std::cmp::Ordering::Equal)
            }
        });
    } else if cell_size == 1 {
        // rank-1 non-numeric: compare elements
        let mut vals = Vec::with_capacity(nrows);
        for i in 0..nrows {
            vals.push(arr.get(i)?);
        }
        indices.sort_by(|&a, &b| {
            let va = vals[a as usize];
            let vb = vals[b as usize];
            let cmp = compare::compare(va, vb);
            if ascending { cmp.cmp(&0) } else { 0.cmp(&cmp) }
        });
    } else {
        // Higher-rank: compare rows lexicographically, cell by cell
        indices.sort_by(|&row_a, &row_b| {
            let base_a = row_a as usize * cell_size;
            let base_b = row_b as usize * cell_size;
            for col in 0..cell_size {
                let va = arr.get(base_a + col).unwrap_or(B::SENTINEL);
                let vb = arr.get(base_b + col).unwrap_or(B::SENTINEL);
                let cmp = compare::compare(va, vb);
                if cmp != 0 {
                    return if ascending { cmp.cmp(&0) } else { 0.cmp(&cmp) };
                }
            }
            std::cmp::Ordering::Equal
        });
    }

    Ok(indices)
}

/// Apply a row permutation to an array.
/// `indices` contains row indices (from grade). For rank-1, each "row" is one element.
/// For higher-rank, each "row" is a cell of `cell_size` elements along the first axis.
fn apply_row_permutation(arr: &BqnArr, indices: &[i32]) -> Result<BqnArr> {
    let cell_size: usize = if arr.rank() <= 1 { 1 } else { arr.shape[1..].iter().product() };
    let ia = arr.ia();
    let mut result = Vec::with_capacity(ia);
    for &row_idx in indices {
        let base = row_idx as usize * cell_size;
        for col in 0..cell_size {
            result.push(arr.get(base + col)?);
        }
    }
    Ok(array::typed_arr_from_b_vec(result, arr.shape.clone(), arr.fill))
}

// ⍋ monad: grade up
pub fn grade_up_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("⍋𝕩: 𝕩 must be an array".into()))?;
    // GPU dispatch for rank-1 numeric arrays above threshold
    if let Some(hook) = GPU_GRADE_HOOK.get()
        && let Some(result) = hook(arr, true) {
            return Ok(PrimResult::Array(result));
        }
    let indices = grade(arr, true)?;
    let mut out = BqnArr::new_vec_i32(indices);
    out.fill = Some(B::m_i32(0));
    Ok(PrimResult::Array(out))
}

// ⍒ monad: grade down
pub fn grade_down_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("⍒𝕩: 𝕩 must be an array".into()))?;
    // GPU dispatch for rank-1 numeric arrays above threshold
    if let Some(hook) = GPU_GRADE_HOOK.get()
        && let Some(result) = hook(arr, false) {
            return Ok(PrimResult::Array(result));
        }
    let indices = grade(arr, false)?;
    let mut out = BqnArr::new_vec_i32(indices);
    out.fill = Some(B::m_i32(0));
    Ok(PrimResult::Array(out))
}

// ∧ monad: sort up (ascending) — returns sorted values, NOT the grade
pub fn sort_up_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("∧𝕩: 𝕩 must be an array".into()))?;
    // GPU dispatch for rank-1 numeric arrays above threshold
    if let Some(hook) = GPU_SORT_HOOK.get()
        && let Some(result) = hook(arr, true) {
            return Ok(PrimResult::Array(result));
        }
    if let Some(out) = fast_sort(arr, true) { return Ok(PrimResult::Array(out)); }
    let indices = grade(arr, true)?;
    let out = apply_row_permutation(arr, &indices)?;
    Ok(PrimResult::Array(out))
}

// ∨ monad: sort down (descending) — returns sorted values, NOT the grade
pub fn sort_down_c1(_x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let arr = xa.ok_or_else(|| BqnError::Type("∨𝕩: 𝕩 must be an array".into()))?;
    // GPU dispatch for rank-1 numeric arrays above threshold
    if let Some(hook) = GPU_SORT_HOOK.get()
        && let Some(result) = hook(arr, false) {
            return Ok(PrimResult::Array(result));
        }
    if let Some(out) = fast_sort(arr, false) { return Ok(PrimResult::Array(out)); }
    let indices = grade(arr, false)?;
    let out = apply_row_permutation(arr, &indices)?;
    Ok(PrimResult::Array(out))
}

// ⍋ dyad: bins (ascending)
// NOTE: 𝕨 and 𝕩 must be compatible sorted arrays. Elements must be comparable (numbers, chars, arrays).
pub fn bins_up_c2(_w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⍋𝕩: 𝕨 must be an array".into()))?;
    // Handle atom x: result is rank-0
    if xa.is_none() {
        let w_lead = if warr.rank() > 0 { warr.shape[0] } else { 1 };
        let w_cell_size: usize = if warr.rank() > 1 { warr.shape[1..].iter().product() } else { 1 };
        if w_cell_size != 1 {
            return Err(BqnError::Rank("𝕨⍋𝕩: atom 𝕩 incompatible with non-scalar 𝕨 cells".into()));
        }
        let mut lo = 0usize;
        let mut hi = w_lead;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let wv = warr.get(mid)?;
            if compare::compare(wv, x) <= 0 { lo = mid + 1; } else { hi = mid; }
        }
        return Ok(PrimResult::Array(BqnArr {
            shape: vec![],
            data: array::ArrData::I32(vec![lo as i32]),
            fill: Some(B::m_i32(0)),
        }));
    }
    let xarr = xa.unwrap();

    // Validate: elements must be comparable (no functions/modifiers, even nested)
    if warr.el_type() == ElType::B {
        for i in 0..warr.ia() {
            let v = warr.get(i)?;
            if contains_non_data(v) {
                return Err(BqnError::Type("𝕨⍋𝕩: elements must be numbers, characters, or arrays".into()));
            }
        }
    }
    // Validate x elements recursively
    if xarr.el_type() == ElType::B {
        for i in 0..xarr.ia() {
            let v = xarr.get(i)?;
            if contains_non_data(v) {
                return Err(BqnError::Type("𝕨⍋𝕩: 𝕩 elements must be numbers, characters, or arrays".into()));
            }
        }
    }

    // Validate rank compatibility: x.rank must be >= w.rank-1, and x.shape must end with w.shape[1..]
    let wr = warr.rank() as usize;
    let xr = xarr.rank() as usize;
    let w_cell_shape = if wr > 0 { &warr.shape[1..] } else { &[] as &[usize] };
    let w_cell_rank = w_cell_shape.len();
    if xr < w_cell_rank {
        return Err(BqnError::Rank(format!(
            "𝕨⍋𝕩: 𝕩 rank {} too low for 𝕨 cell rank {} (need rank ≥ {})",
            xr, w_cell_rank, w_cell_rank
        )));
    }
    // x's trailing shape must match w's cell shape
    let x_tail = &xarr.shape[xr - w_cell_rank..];
    if x_tail != w_cell_shape {
        return Err(BqnError::Shape(format!(
            "𝕨⍋𝕩: 𝕩 trailing shape {:?} doesn't match 𝕨 cell shape {:?}",
            x_tail, w_cell_shape
        )));
    }

    let w_lead = if wr > 0 { warr.shape[0] } else { 1 };
    let w_cell_size: usize = w_cell_shape.iter().product::<usize>().max(1);
    let x_lead_dims = xr - w_cell_rank;
    let x_lead_shape = &xarr.shape[..x_lead_dims];
    let x_lead_count: usize = x_lead_shape.iter().product::<usize>().max(1);

    // w_leq_x: returns true if w cell j <= x cell at offset (ascending bins condition)
    let w_leq_x = |x_offset: usize, w_j: usize| -> bool {
        for k in 0..w_cell_size {
            let wv = warr.get(w_j * w_cell_size + k).unwrap_or(B::SENTINEL);
            let xv = xarr.get(x_offset + k).unwrap_or(B::SENTINEL);
            let c = compare::compare(wv, xv);
            if c < 0 { return true; }   // w < x → w <= x
            if c > 0 { return false; }  // w > x → not w <= x
        }
        true // equal → w <= x
    };

    let mut result = Vec::with_capacity(x_lead_count);
    for i in 0..x_lead_count {
        let x_off = i * w_cell_size;
        // Binary search: find first w_j where w_cell[w_j] > x_cell
        let mut lo = 0usize;
        let mut hi = w_lead;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if w_leq_x(x_off, mid) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        result.push(lo as i32);
    }

    if x_lead_dims == 0 {
        // Result is rank-0
        return Ok(PrimResult::Array(BqnArr {
            shape: vec![],
            data: array::ArrData::I32(result),
            fill: Some(B::m_i32(0)),
        }));
    }
    let mut out = BqnArr::new_vec_i32(result);
    out.shape = x_lead_shape.to_vec();
    out.fill = Some(B::m_i32(0));
    Ok(PrimResult::Array(out))
}

// ⍒ dyad: bins (descending)
pub fn bins_down_c2(_w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let warr = wa.ok_or_else(|| BqnError::Type("𝕨⍒𝕩: 𝕨 must be an array".into()))?;
    // Handle atom x: result is rank-0
    if xa.is_none() {
        let w_lead = if warr.rank() > 0 { warr.shape[0] } else { 1 };
        let w_cell_size: usize = if warr.rank() > 1 { warr.shape[1..].iter().product() } else { 1 };
        if w_cell_size != 1 {
            return Err(BqnError::Rank("𝕨⍒𝕩: atom 𝕩 incompatible with non-scalar 𝕨 cells".into()));
        }
        let mut lo = 0usize;
        let mut hi = w_lead;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let wv = warr.get(mid)?;
            if compare::compare(wv, x) >= 0 { lo = mid + 1; } else { hi = mid; }
        }
        return Ok(PrimResult::Array(BqnArr {
            shape: vec![],
            data: array::ArrData::I32(vec![lo as i32]),
            fill: Some(B::m_i32(0)),
        }));
    }
    let xarr = xa.unwrap();

    // Validate x elements recursively for non-data values
    if xarr.el_type() == ElType::B {
        for i in 0..xarr.ia() {
            let v = xarr.get(i)?;
            if contains_non_data(v) {
                return Err(BqnError::Type("𝕨⍒𝕩: 𝕩 elements must be numbers, characters, or arrays".into()));
            }
        }
    }

    let wr = warr.rank() as usize;
    let xr = xarr.rank() as usize;
    let w_cell_shape = if wr > 0 { &warr.shape[1..] } else { &[] as &[usize] };
    let w_cell_rank = w_cell_shape.len();
    if xr < w_cell_rank {
        return Err(BqnError::Rank(format!(
            "𝕨⍒𝕩: 𝕩 rank {} too low for 𝕨 cell rank {}",
            xr, w_cell_rank
        )));
    }
    let x_tail = &xarr.shape[xr - w_cell_rank..];
    if x_tail != w_cell_shape {
        return Err(BqnError::Shape(format!(
            "𝕨⍒𝕩: 𝕩 trailing shape {:?} doesn't match 𝕨 cell shape {:?}",
            x_tail, w_cell_shape
        )));
    }

    let w_lead = if wr > 0 { warr.shape[0] } else { 1 };
    let w_cell_size: usize = w_cell_shape.iter().product::<usize>().max(1);
    let x_lead_dims = xr - w_cell_rank;
    let x_lead_shape = &xarr.shape[..x_lead_dims];
    let x_lead_count: usize = x_lead_shape.iter().product::<usize>().max(1);

    // Validate w elements as well
    if warr.el_type() == ElType::B {
        for i in 0..warr.ia() {
            let v = warr.get(i)?;
            if contains_non_data(v) {
                return Err(BqnError::Type("𝕨⍒𝕩: 𝕨 elements must be numbers, characters, or arrays".into()));
            }
        }
    }

    // Compare descending: w[j] >= x means "keep going"
    let compare_cell_desc = |x_offset: usize, w_j: usize| -> bool {
        for k in 0..w_cell_size {
            let xv = xarr.get(x_offset + k).unwrap_or(B::SENTINEL);
            let wv = warr.get(w_j * w_cell_size + k).unwrap_or(B::SENTINEL);
            let c = compare::compare(wv, xv);
            if c > 0 { return true; }   // w > x → w >= x, keep going
            if c < 0 { return false; }  // w < x → stop
        }
        true // equal → w >= x
    };

    let mut result = Vec::with_capacity(x_lead_count);
    for i in 0..x_lead_count {
        let x_off = i * w_cell_size;
        let mut lo = 0usize;
        let mut hi = w_lead;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if compare_cell_desc(x_off, mid) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        result.push(lo as i32);
    }

    if x_lead_dims == 0 {
        return Ok(PrimResult::Array(BqnArr {
            shape: vec![],
            data: array::ArrData::I32(result),
            fill: Some(B::m_i32(0)),
        }));
    }
    let mut out = BqnArr::new_vec_i32(result);
    out.shape = x_lead_shape.to_vec();
    out.fill = Some(B::m_i32(0));
    Ok(PrimResult::Array(out))
}
