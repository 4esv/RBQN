use std::sync::OnceLock;
use rbqn_core::*;
use crate::dispatch::PrimResult;

// NOTE: GPU dispatch hook — set by rbqn crate at startup via register_gpu_arith.
// Using a function pointer in a OnceLock avoids a direct dependency on rbqn (which depends on rbqn-prim).
// Takes `B` (not `&BqnArr`) so pending device values are consumed without a readback.
type GpuArithFn = fn(op: &str, w: B, x: B) -> Option<B>;
static GPU_ARITH_HOOK: OnceLock<GpuArithFn> = OnceLock::new();

pub fn register_gpu_arith(f: GpuArithFn) {
    let _ = GPU_ARITH_HOOK.set(f);
}

/// GPU arithmetic for `+ - × ⌊ ⌈` (array-array or number-array), called by the VM before it forces
/// the operands with `get_arr`. None means take the CPU path.
#[inline]
pub fn try_gpu_arith(glyph: &str, w: B, x: B) -> Option<B> {
    // NOTE: bit tests only: atom-atom and char/boxed-atom operands never reach the hook.
    let (wa, xa) = (w.is_arr(), x.is_arr());
    if !(wa && xa || wa && x.is_num() || xa && w.is_num()) {
        return None;
    }
    let op = gpu_op_name(glyph)?;
    GPU_ARITH_HOOK.get().and_then(|h| h(op, w, x))
}

// NOTE: GPU fused dispatch hook — set by rbqn crate at startup via register_gpu_fused.
// Allows callers to submit a sequence of elementwise ops as a single GPU kernel dispatch.
// True expression-level fusion (auto-detecting `2×a+b`) requires VM-level lookahead and
// is a future optimization. This hook provides the explicit API for that future work.
type GpuFusedFn = fn(ops: &[(&str, Option<f64>)], a: &BqnArr, b: Option<&BqnArr>) -> Option<BqnArr>;
static GPU_FUSED_HOOK: OnceLock<GpuFusedFn> = OnceLock::new();

pub fn register_gpu_fused(f: GpuFusedFn) {
    let _ = GPU_FUSED_HOOK.set(f);
}

/// Explicit fused arithmetic dispatch: apply a sequence of elementwise ops as one GPU kernel.
///
/// `ops` is a slice of `(op_name, optional_scalar)` tuples. Supported op names:
/// - "add", "sub", "mul", "div" — binary ops applied to `a` and `b`
/// - "scalar_add", "scalar_mul" — scalar ops applied element-wise (scalar in `Option<f64>`)
///
/// Returns None if GPU is unavailable, threshold not met, or the FusionBuilder fails.
/// Falls back to CPU arithmetic in that case. This is intentional — the caller must still
/// handle the None case via the normal CPU path.
///
/// # Future work
/// VM-level expression analysis can detect fuseable patterns like `2×a+b` and call this
/// directly, amortizing upload/download overhead across multiple ops.
pub fn try_fused_arith(
    ops: &[(&str, Option<f64>)],
    a: &BqnArr,
    b: Option<&BqnArr>,
) -> Option<BqnArr> {
    GPU_FUSED_HOOK.get().and_then(|f| f(ops, a, b))
}

/// Map BQN operator names to GPU kernel names (only for supported numeric ops).
fn gpu_op_name(name: &str) -> Option<&'static str> {
    match name {
        "+" => Some("add"),
        "-" => Some("sub"),
        "×" => Some("mul"),
        "÷" => Some("div"),
        "⌊" => Some("min"),
        "⌈" => Some("max"),
        _ => None,
    }
}

/// Check that `short` is a prefix of `long`. Returns true if every element
/// of `short` equals the corresponding leading element of `long`.
fn is_shape_prefix(short: &[usize], long: &[usize]) -> bool {
    short.len() <= long.len() && short.iter().zip(long.iter()).all(|(a, b)| a == b)
}

/// Recursively apply a scalar dyadic function element-wise through Boxed arrays.
/// This handles depth>1 pervasion: e.g. `3 + ⟨1, 2, ⟨3, 4⟩⟩ → ⟨4, 5, ⟨6, 7⟩⟩`.
fn pervasive_boxed_scalar_arr<F: Fn(f64, f64) -> f64 + Copy>(
    w: B,
    xa_arr: &BqnArr,
    scalar_fn: F,
    name: &str,
) -> Result<PrimResult> {
    // NOTE: If w is a rank-0 box broadcasting over xa_arr, extract its content
    // so the result elements are unwrapped, matching BQN prefix broadcasting semantics.
    let (w_eff, w_eff_arr_opt) = if let Some(wa) = get_arr(w) {
        if wa.rank() == 0 && wa.ia() > 0 {
            let content = wa.get(0)?;
            let content_arr = get_arr(content);
            (content, content_arr)
        } else {
            (w, Some(wa))
        }
    } else {
        (w, None)
    };
    let n = xa_arr.ia();
    let mut results: Vec<B> = Vec::with_capacity(n);
    for i in 0..n {
        let xi = xa_arr.get(i)?;
        let xi_arr = get_arr(xi);
        let r = pervasive_dyad(w_eff, w_eff_arr_opt.as_deref(), xi, xi_arr.as_deref(), scalar_fn, name)?;
        results.push(prim_result_to_b(r));
    }
    let result_fill = pervasive_fill(&results, xa_arr.fill);
    let out = array::typed_arr_from_b_vec(results, xa_arr.shape.clone(), result_fill);
    Ok(PrimResult::Array(out))
}

fn pervasive_boxed_arr_scalar<F: Fn(f64, f64) -> f64 + Copy>(
    wa_arr: &BqnArr,
    x: B,
    scalar_fn: F,
    name: &str,
) -> Result<PrimResult> {
    // NOTE: If x is a rank-0 box (BQN scalar broadcast), extract its content so the
    // result elements are unwrapped arrays, matching BQN prefix broadcasting semantics.
    // e.g. 1‿0 × <↕3 → ⟨↕3, 0‿0‿0⟩ (elements are plain arrays, not rank-0 boxes)
    if let Some(xa_arr) = get_arr(x)
        && xa_arr.rank() == 0 && xa_arr.ia() > 0 {
            let x_content = xa_arr.get(0)?;
            let x_content_arr = get_arr(x_content);
            let n = wa_arr.ia();
            let mut results: Vec<B> = Vec::with_capacity(n);
            for i in 0..n {
                let wi = wa_arr.get(i)?;
                let wi_arr = get_arr(wi);
                let r = pervasive_dyad(wi, wi_arr.as_deref(), x_content, x_content_arr.as_deref(), scalar_fn, name)?;
                results.push(prim_result_to_b(r));
            }
            let result_fill = pervasive_fill(&results, wa_arr.fill);
            let out = array::typed_arr_from_b_vec(results, wa_arr.shape.clone(), result_fill);
            return Ok(PrimResult::Array(out));
        }
    let n = wa_arr.ia();
    let mut results: Vec<B> = Vec::with_capacity(n);
    for i in 0..n {
        let wi = wa_arr.get(i)?;
        let wi_arr = get_arr(wi);
        let r = pervasive_dyad(wi, wi_arr.as_deref(), x, None, scalar_fn, name)?;
        results.push(prim_result_to_b(r));
    }
    let result_fill = pervasive_fill(&results, wa_arr.fill);
    let out = array::typed_arr_from_b_vec(results, wa_arr.shape.clone(), result_fill);
    Ok(PrimResult::Array(out))
}

fn pervasive_boxed_arr_arr<F: Fn(f64, f64) -> f64 + Copy>(
    wa_arr: &BqnArr,
    xa_arr: &BqnArr,
    scalar_fn: F,
    name: &str,
) -> Result<PrimResult> {
    if wa_arr.shape != xa_arr.shape {
        return Err(BqnError::Shape(format!(
            "𝕨{name}𝕩: Expected equal shape prefix ({:?} ≡ ≢𝕨, {:?} ≡ ≢𝕩)",
            wa_arr.shape, xa_arr.shape
        )));
    }
    let n = wa_arr.ia();
    let mut results: Vec<B> = Vec::with_capacity(n);
    for i in 0..n {
        let wi = wa_arr.get(i)?;
        let xi = xa_arr.get(i)?;
        let wi_arr = get_arr(wi);
        let xi_arr = get_arr(xi);
        let r = pervasive_dyad(wi, wi_arr.as_deref(), xi, xi_arr.as_deref(), scalar_fn, name)?;
        results.push(prim_result_to_b(r));
    }
    // NOTE: Compute fill from result elements (not wa_arr.fill) since the result
    // may have different structure than either input (e.g., scalar+array → array).
    let result_fill = pervasive_fill(&results, wa_arr.fill);
    let out = array::typed_arr_from_b_vec(results, wa_arr.shape.clone(), result_fill);
    Ok(PrimResult::Array(out))
}

fn prim_result_to_b(r: PrimResult) -> B {
    match r {
        PrimResult::Scalar(b) => b,
        PrimResult::Array(a) => tag_arr(a),
    }
}

/// Compute fill for a pervasive result: prototype of first element, or fallback fill.
#[inline]
fn pervasive_fill(results: &[B], fallback: Option<B>) -> Option<B> {
    if !results.is_empty() {
        Some(crate::structural::prototype_of(results[0]))
    } else {
        fallback
    }
}

fn pervasive_dyad<F: Fn(f64, f64) -> f64 + Copy>(
    w: B,
    wa: Option<&BqnArr>,
    x: B,
    xa: Option<&BqnArr>,
    scalar_fn: F,
    name: &str,
) -> Result<PrimResult> {
    match (wa, xa) {
        // scalar-scalar (or scalar-box: pervasion enters boxes)
        (None, None) => {
            // If either side is a box (rank-0 array), open it and recurse.
            if x.is_arr() {
                let xa_arr = get_arr(x);
                return pervasive_dyad(w, None, x, xa_arr.as_deref(), scalar_fn, name);
            }
            if w.is_arr() {
                let wa_arr = get_arr(w);
                return pervasive_dyad(w, wa_arr.as_deref(), x, None, scalar_fn, name);
            }
            let wf = w.to_f64().map_err(|_| BqnError::Type(format!("𝕨{name}𝕩: Unexpected argument types")))?;
            let xf = x.to_f64().map_err(|_| BqnError::Type(format!("𝕨{name}𝕩: Unexpected argument types")))?;
            Ok(PrimResult::Scalar(B::m_f64(scalar_fn(wf, xf))))
        }
        // scalar-array
        (None, Some(xa_arr)) => {
            if let Ok(wf) = w.to_f64()
                && let Some(out) = typed_scalar_arr(name, wf, xa_arr, true) {
                    return Ok(PrimResult::Array(out));
                }
            if let Ok(wf) = w.to_f64()
                && let Ok(xvals) = num_view(xa_arr) {
                    let result: Vec<f64> = xvals.iter().map(|&xv| scalar_fn(wf, xv)).collect();
                    let mut out = BqnArr::new_vec_f64(result);
                    out.shape = xa_arr.shape.clone();
                    return Ok(PrimResult::Array(array::squeeze_num(out)));
                }
            // Boxed fallback: recurse element-wise
            pervasive_boxed_scalar_arr(w, xa_arr, scalar_fn, name)
        }
        // array-scalar
        (Some(wa_arr), None) => {
            if let Ok(xf) = x.to_f64()
                && let Some(out) = typed_scalar_arr(name, xf, wa_arr, false) {
                    return Ok(PrimResult::Array(out));
                }
            if let Ok(xf) = x.to_f64()
                && let Ok(wvals) = num_view(wa_arr) {
                    let result: Vec<f64> = wvals.iter().map(|&wv| scalar_fn(wv, xf)).collect();
                    let mut out = BqnArr::new_vec_f64(result);
                    out.shape = wa_arr.shape.clone();
                    return Ok(PrimResult::Array(array::squeeze_num(out)));
                }
            // Boxed fallback: recurse element-wise
            pervasive_boxed_arr_scalar(wa_arr, x, scalar_fn, name)
        }
        // array-array: leading axis agreement (prefix broadcasting)
        (Some(wa_arr), Some(xa_arr)) => {
            // NOTE: rank-0 arrays act as scalars in BQN pervasion.
            // When one side is rank-0 (shape=[]), extract its single element and use as scalar.
            // NOTE: rank-0 arrays broadcast as scalars in BQN pervasion.
            // Treat the rank-0 value itself (not its contents) as the repeating element.
            if wa_arr.rank() == 0 {
                return pervasive_dyad(w, None, x, Some(xa_arr), scalar_fn, name);
            }
            if xa_arr.rank() == 0 {
                return pervasive_dyad(w, Some(wa_arr), x, None, scalar_fn, name);
            }
            // NOTE: GPU dispatch happens earlier, in try_gpu_arith (called from derive::c2).
            // Try fast numeric path first
            if let (Ok(wvals), Ok(xvals)) = (num_view(wa_arr), num_view(xa_arr)) {
                if wa_arr.shape == xa_arr.shape {
                    let result: Vec<f64> = wvals
                        .iter()
                        .zip(xvals.iter())
                        .map(|(&wv, &xv)| scalar_fn(wv, xv))
                        .collect();
                    let mut out = BqnArr::new_vec_f64(result);
                    out.shape = wa_arr.shape.clone();
                    return Ok(PrimResult::Array(array::squeeze_num(out)));
                } else if is_shape_prefix(&wa_arr.shape, &xa_arr.shape) {
                    // Each w element pairs with a contiguous block of x (leading axis agreement).
                    let rep = (xa_arr.ia() / wa_arr.ia().max(1)).max(1);
                    let result: Vec<f64> = xvals
                        .iter()
                        .enumerate()
                        .map(|(i, &xv)| scalar_fn(wvals[i / rep], xv))
                        .collect();
                    let mut out = BqnArr::new_vec_f64(result);
                    out.shape = xa_arr.shape.clone();
                    return Ok(PrimResult::Array(array::squeeze_num(out)));
                } else if is_shape_prefix(&xa_arr.shape, &wa_arr.shape) {
                    let rep = (wa_arr.ia() / xa_arr.ia().max(1)).max(1);
                    let result: Vec<f64> = wvals
                        .iter()
                        .enumerate()
                        .map(|(i, &wv)| scalar_fn(wv, xvals[i / rep]))
                        .collect();
                    let mut out = BqnArr::new_vec_f64(result);
                    out.shape = wa_arr.shape.clone();
                    return Ok(PrimResult::Array(array::squeeze_num(out)));
                } else {
                    return Err(BqnError::Shape(format!(
                        "𝕨{name}𝕩: Expected equal shape prefix ({:?} ≡ ≢𝕨, {:?} ≡ ≢𝕩)",
                        wa_arr.shape, xa_arr.shape
                    )));
                }
            }
            // Boxed fallback: recurse element-wise
            pervasive_boxed_arr_arr(wa_arr, xa_arr, scalar_fn, name)
        }
    }
}

/// Numeric contents as f64, borrowing when the array already stores f64.
fn num_view(a: &BqnArr) -> Result<std::borrow::Cow<'_, [f64]>> {
    match &a.data {
        array::ArrData::F64(v) => Ok(std::borrow::Cow::Borrowed(v)),
        _ => a.f64_iter().map(std::borrow::Cow::Owned),
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

    // Boxed/nested arrays: recurse element-wise, preserving char arithmetic
    if wk == 'o' || xk == 'o' {
        return pervasive_mixed_boxed(w, wa, x, xa, add_c2);
    }

    // char + char is a type error in BQN
    Err(BqnError::Type("𝕨+𝕩: Unexpected argument types".into()))
}

/// Generic pervasive handler for mixed/Boxed arrays with char-aware operations.
/// `op_fn` should be the top-level function (e.g., add_c2) that handles all type combinations.
fn pervasive_mixed_boxed(
    w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>,
    op_fn: fn(B, Option<&BqnArr>, B, Option<&BqnArr>) -> Result<PrimResult>,
) -> Result<PrimResult> {
    fn to_b(r: PrimResult) -> B { match r { PrimResult::Scalar(b) => b, PrimResult::Array(a) => tag_arr(a) } }

    match (wa, xa) {
        (Some(wa_a), Some(xa_a)) => {
            // NOTE: rank-0 arrays in arithmetic: the rank-0 side is "opened" and its content
            // is broadcast against each element of the rank-n side.
            // e.g. [0,1]+<[0,1] = ⟨0+[0,1], 1+[0,1]⟩ = ⟨[0,1],[1,2]⟩ (plain arrays, no re-boxing)
            if wa_a.rank() == 0 {
                // Extract the content of the rank-0 box and broadcast it
                let inner = wa_a.get(0).unwrap_or(B::SENTINEL);
                let inner_arr = get_arr(inner);
                let n = xa_a.ia();
                let mut results = Vec::with_capacity(n);
                for i in 0..n {
                    let xi = xa_a.get(i).unwrap_or(B::SENTINEL);
                    let xi_a = get_arr(xi);
                    results.push(to_b(op_fn(inner, inner_arr.as_deref(), xi, xi_a.as_deref())?));
                }
                let result_fill = results.first().copied().map(crate::structural::prototype_of);
                return Ok(PrimResult::Array(array::typed_arr_from_b_vec(results, xa_a.shape.clone(), result_fill)));
            }
            if xa_a.rank() == 0 {
                // Extract the content of the rank-0 box and broadcast it against wa_a elements
                let inner = xa_a.get(0).unwrap_or(B::SENTINEL);
                let inner_arr = get_arr(inner);
                let n = wa_a.ia();
                let mut results = Vec::with_capacity(n);
                for i in 0..n {
                    let wi = wa_a.get(i).unwrap_or(B::SENTINEL);
                    let wi_a = get_arr(wi);
                    results.push(to_b(op_fn(wi, wi_a.as_deref(), inner, inner_arr.as_deref())?));
                }
                let result_fill = results.first().copied().map(crate::structural::prototype_of);
                return Ok(PrimResult::Array(array::typed_arr_from_b_vec(results, wa_a.shape.clone(), result_fill)));
            }
            if wa_a.shape != xa_a.shape {
                return Err(BqnError::Shape(format!(
                    "shape mismatch ({:?} vs {:?})", wa_a.shape, xa_a.shape
                )));
            }
            let n = wa_a.ia();
            let mut results = Vec::with_capacity(n);
            for i in 0..n {
                let wi = wa_a.get(i)?;
                let xi = xa_a.get(i)?;
                let wi_a = get_arr(wi);
                let xi_a = get_arr(xi);
                results.push(to_b(op_fn(wi, wi_a.as_deref(), xi, xi_a.as_deref())?));
            }
            let result_fill = results.first().copied().map(crate::structural::prototype_of);
            Ok(PrimResult::Array(array::typed_arr_from_b_vec(results, wa_a.shape.clone(), result_fill)))
        }
        (None, Some(xa_a)) => {
            let n = xa_a.ia();
            let mut results = Vec::with_capacity(n);
            let w_a = get_arr(w);
            for i in 0..n {
                let xi = xa_a.get(i)?;
                let xi_a = get_arr(xi);
                results.push(to_b(op_fn(w, w_a.as_deref(), xi, xi_a.as_deref())?));
            }
            let result_fill = results.first().copied().map(crate::structural::prototype_of);
            Ok(PrimResult::Array(array::typed_arr_from_b_vec(results, xa_a.shape.clone(), result_fill)))
        }
        (Some(wa_a), None) => {
            let n = wa_a.ia();
            let mut results = Vec::with_capacity(n);
            let x_a = get_arr(x);
            for i in 0..n {
                let wi = wa_a.get(i)?;
                let wi_a = get_arr(wi);
                results.push(to_b(op_fn(wi, wi_a.as_deref(), x, x_a.as_deref())?));
            }
            let result_fill = results.first().copied().map(crate::structural::prototype_of);
            Ok(PrimResult::Array(array::typed_arr_from_b_vec(results, wa_a.shape.clone(), result_fill)))
        }
        (None, None) => {
            let wa2 = get_arr(w);
            let xa2 = get_arr(x);
            if wa2.is_some() || xa2.is_some() {
                return pervasive_mixed_boxed(w, wa2.as_deref(), x, xa2.as_deref(), op_fn);
            }
            Err(BqnError::Type("Unexpected argument types".into()))
        }
    }
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
        // array char + array num (with prefix agreement)
        (Some(ca_arr), Some(na_arr)) => {
            let chars = ca_arr.c32_iter()?;
            let nums = na_arr.f64_iter()?;
            if ca_arr.shape == na_arr.shape {
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
            } else if is_shape_prefix(&ca_arr.shape, &na_arr.shape) {
                let rep = (na_arr.ia() / ca_arr.ia().max(1)).max(1);
                let result: std::result::Result<Vec<u32>, _> = nums.iter().enumerate().map(|(i, &n)| {
                    let r = chars[i / rep] as i64 + n as i64;
                    if r < 0 || r > value::CHR_MAX as i64 {
                        Err(BqnError::Domain("𝕨+𝕩: Invalid character".into()))
                    } else { Ok(r as u32) }
                }).collect();
                let mut out = BqnArr::new_vec_c32(result?);
                out.shape = na_arr.shape.clone();
                Ok(PrimResult::Array(out))
            } else if is_shape_prefix(&na_arr.shape, &ca_arr.shape) {
                let rep = (ca_arr.ia() / na_arr.ia().max(1)).max(1);
                let result: std::result::Result<Vec<u32>, _> = chars.iter().enumerate().map(|(i, &c)| {
                    let r = c as i64 + nums[i / rep] as i64;
                    if r < 0 || r > value::CHR_MAX as i64 {
                        Err(BqnError::Domain("𝕨+𝕩: Invalid character".into()))
                    } else { Ok(r as u32) }
                }).collect();
                let mut out = BqnArr::new_vec_c32(result?);
                out.shape = ca_arr.shape.clone();
                Ok(PrimResult::Array(out))
            } else {
                Err(BqnError::Shape(format!(
                    "𝕨+𝕩: Expected equal shape prefix ({:?} ≡ ≢𝕨, {:?} ≡ ≢𝕩)",
                    if w_is_char { &ca_arr.shape } else { &na_arr.shape },
                    if w_is_char { &na_arr.shape } else { &ca_arr.shape },
                )))
            }
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
            let nums = num_view(xa_arr)?;
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
            let chars = wa_arr.c32_iter()?;
            let nums = num_view(xa_arr)?;
            if wa_arr.shape == xa_arr.shape {
                let result: std::result::Result<Vec<u32>, _> = chars.iter().zip(nums.iter()).map(|(&c, &n)| {
                    let r = c as i64 - n as i64;
                    if r < 0 || r > value::CHR_MAX as i64 {
                        Err(BqnError::Domain("𝕨-𝕩: Invalid character".into()))
                    } else { Ok(r as u32) }
                }).collect();
                let mut out = BqnArr::new_vec_c32(result?);
                out.shape = wa_arr.shape.clone();
                Ok(PrimResult::Array(out))
            } else if is_shape_prefix(&wa_arr.shape, &xa_arr.shape) {
                // Each w element pairs with a contiguous block of x (leading axis agreement).
                    let rep = (xa_arr.ia() / wa_arr.ia().max(1)).max(1);
                let result: std::result::Result<Vec<u32>, _> = nums.iter().enumerate().map(|(i, &n)| {
                    let r = chars[i / rep] as i64 - n as i64;
                    if r < 0 || r > value::CHR_MAX as i64 {
                        Err(BqnError::Domain("𝕨-𝕩: Invalid character".into()))
                    } else { Ok(r as u32) }
                }).collect();
                let mut out = BqnArr::new_vec_c32(result?);
                out.shape = xa_arr.shape.clone();
                Ok(PrimResult::Array(out))
            } else if is_shape_prefix(&xa_arr.shape, &wa_arr.shape) {
                let rep = (wa_arr.ia() / xa_arr.ia().max(1)).max(1);
                let result: std::result::Result<Vec<u32>, _> = chars.iter().enumerate().map(|(i, &c)| {
                    let r = c as i64 - nums[i / rep] as i64;
                    if r < 0 || r > value::CHR_MAX as i64 {
                        Err(BqnError::Domain("𝕨-𝕩: Invalid character".into()))
                    } else { Ok(r as u32) }
                }).collect();
                let mut out = BqnArr::new_vec_c32(result?);
                out.shape = wa_arr.shape.clone();
                Ok(PrimResult::Array(out))
            } else {
                Err(BqnError::Shape(format!(
                    "𝕨-𝕩: Expected equal shape prefix ({:?} ≡ ≢𝕨, {:?} ≡ ≢𝕩)",
                    wa_arr.shape, xa_arr.shape
                )))
            }
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
            let wchars = wa_arr.c32_iter()?;
            let xchars = xa_arr.c32_iter()?;
            if wa_arr.shape == xa_arr.shape {
                let result: Vec<f64> = wchars.iter().zip(xchars.iter())
                    .map(|(&wc, &xc)| (wc as i64 - xc as i64) as f64).collect();
                let mut out = BqnArr::new_vec_f64(result);
                out.shape = wa_arr.shape.clone();
                Ok(PrimResult::Array(array::squeeze_num(out)))
            } else if is_shape_prefix(&wa_arr.shape, &xa_arr.shape) {
                // Each w element pairs with a contiguous block of x (leading axis agreement).
                    let rep = (xa_arr.ia() / wa_arr.ia().max(1)).max(1);
                let result: Vec<f64> = xchars.iter().enumerate()
                    .map(|(i, &xc)| (wchars[i / rep] as i64 - xc as i64) as f64).collect();
                let mut out = BqnArr::new_vec_f64(result);
                out.shape = xa_arr.shape.clone();
                Ok(PrimResult::Array(array::squeeze_num(out)))
            } else if is_shape_prefix(&xa_arr.shape, &wa_arr.shape) {
                let rep = (wa_arr.ia() / xa_arr.ia().max(1)).max(1);
                let result: Vec<f64> = wchars.iter().enumerate()
                    .map(|(i, &wc)| (wc as i64 - xchars[i / rep] as i64) as f64).collect();
                let mut out = BqnArr::new_vec_f64(result);
                out.shape = wa_arr.shape.clone();
                Ok(PrimResult::Array(array::squeeze_num(out)))
            } else {
                Err(BqnError::Shape(format!(
                    "𝕨-𝕩: Expected equal shape prefix ({:?} ≡ ≢𝕨, {:?} ≡ ≢𝕩)",
                    wa_arr.shape, xa_arr.shape
                )))
            }
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

    // Boxed/nested arrays: recurse element-wise, preserving char arithmetic
    if wk == 'o' || xk == 'o' {
        return pervasive_mixed_boxed(w, wa, x, xa, sub_c2);
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
    // NOTE: BQN dyadic ¬ is w¬x = 1+w-x. With char args:
    // char¬char → num: 1 + (w_codepoint - x_codepoint)
    // char¬num → char: char shifted by (1-x), i.e. codepoint + 1 - x
    let wk = char_num_class(w, wa);
    let xk = char_num_class(x, xa);

    if wk == 'c' && xk == 'c' {
        // char¬char = 1 + (w_cp - x_cp) → num
        let sub = sub_c2(w, wa, x, xa)?;
        return match sub {
            PrimResult::Scalar(b) => Ok(PrimResult::Scalar(B::m_f64(b.o2f() + 1.0))),
            PrimResult::Array(arr) => {
                let ia = arr.ia();
                let mut vals = Vec::with_capacity(ia);
                for i in 0..ia {
                    vals.push(arr.get(i)?.o2f() + 1.0);
                }
                let mut out = BqnArr::new_vec_f64(vals);
                out.shape = arr.shape.clone();
                Ok(PrimResult::Array(array::squeeze_num(out)))
            }
        };
    }

    if wk == 'c' && xk == 'n' {
        // char¬num: w_cp + 1 - x = char shifted by (1-x)
        // = char_add(w, 1.0 - x)
        return not_char_num(w, wa, x, xa);
    }

    if w.is_f64() && x.is_f64() {
        return Ok(PrimResult::Scalar(B::m_f64(1.0 + w.o2f() - x.o2f())));
    }
    pervasive_dyad(w, wa, x, xa, |a, b| 1.0 + a - b, "¬")
}

/// Handle char¬num: result is char shifted by (1 - x_num).
fn not_char_num(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    match (wa, xa) {
        (None, None) => {
            let c = w.o2c()?;
            let n = x.to_f64()?;
            let r = c as i64 + 1 - n as i64;
            if r < 0 || r > value::CHR_MAX as i64 {
                return Err(BqnError::Domain("𝕨¬𝕩: Invalid character result".into()));
            }
            Ok(PrimResult::Scalar(B::m_c32(r as u32)))
        }
        (None, Some(xa_arr)) => {
            let c = w.o2c()? as i64;
            let nums = num_view(xa_arr)?;
            let result: std::result::Result<Vec<u32>, _> = nums.iter().map(|&n| {
                let r = c + 1 - n as i64;
                if r < 0 || r > value::CHR_MAX as i64 {
                    Err(BqnError::Domain("𝕨¬𝕩: Invalid character result".into()))
                } else { Ok(r as u32) }
            }).collect();
            let mut out = BqnArr::new_vec_c32(result?);
            out.shape = xa_arr.shape.clone();
            Ok(PrimResult::Array(out))
        }
        (Some(wa_arr), None) => {
            let n = x.to_f64()? as i64;
            let chars = wa_arr.c32_iter()?;
            let result: std::result::Result<Vec<u32>, _> = chars.iter().map(|&c| {
                let r = c as i64 + 1 - n;
                if r < 0 || r > value::CHR_MAX as i64 {
                    Err(BqnError::Domain("𝕨¬𝕩: Invalid character result".into()))
                } else { Ok(r as u32) }
            }).collect();
            let mut out = BqnArr::new_vec_c32(result?);
            out.shape = wa_arr.shape.clone();
            Ok(PrimResult::Array(out))
        }
        (Some(wa_arr), Some(xa_arr)) => {
            let chars = wa_arr.c32_iter()?;
            let nums = num_view(xa_arr)?;
            if wa_arr.shape == xa_arr.shape {
                let result: std::result::Result<Vec<u32>, _> = chars.iter().zip(nums.iter()).map(|(&c, &n)| {
                    let r = c as i64 + 1 - n as i64;
                    if r < 0 || r > value::CHR_MAX as i64 {
                        Err(BqnError::Domain("𝕨¬𝕩: Invalid character result".into()))
                    } else { Ok(r as u32) }
                }).collect();
                let mut out = BqnArr::new_vec_c32(result?);
                out.shape = wa_arr.shape.clone();
                Ok(PrimResult::Array(out))
            } else {
                Err(BqnError::Shape(format!(
                    "𝕨¬𝕩: Shape mismatch ({:?} vs {:?})", wa_arr.shape, xa_arr.shape
                )))
            }
        }
    }
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

// ---------------------------------------------------------------------------
// Typed scalar-array fast path
// ---------------------------------------------------------------------------

/// Integer-typed array contents (Bit/I8/I16/I32) as i32, borrowing for I32.
fn int_view(a: &BqnArr) -> Option<std::borrow::Cow<'_, [i32]>> {
    use std::borrow::Cow;
    Some(match &a.data {
        array::ArrData::I32(v) => Cow::Borrowed(&v[..]),
        array::ArrData::I16(v) => Cow::Owned(v.iter().map(|&x| x as i32).collect()),
        array::ArrData::I8(v) => Cow::Owned(v.iter().map(|&x| x as i32).collect()),
        array::ArrData::Bit(w) => {
            let n = a.ia();
            Cow::Owned((0..n).map(|i| ((w[i / 64] >> (i % 64)) & 1) as i32).collect())
        }
        _ => return None,
    })
}

/// Pack a predicate over a slice into a Bit array's words.
fn pack_bits<F: Fn(i32) -> bool>(v: &[i32], f: F) -> Vec<u64> {
    let mut words = vec![0u64; v.len().div_ceil(64)];
    for (w, chunk) in words.iter_mut().zip(v.chunks(64)) {
        let mut acc = 0u64;
        for (j, &x) in chunk.iter().enumerate() {
            acc |= (f(x) as u64) << j;
        }
        *w = acc;
    }
    words
}

/// Build the narrowest integer array for `vals`, matching `squeeze_num`.
fn narrow_i32(vals: Vec<i32>, shape: Vec<usize>) -> BqnArr {
    let (mut lo, mut hi) = (i32::MAX, i32::MIN);
    for &v in &vals {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    let data = if lo >= 0 && hi <= 1 {
        array::ArrData::Bit(pack_bits(&vals, |x| x == 1))
    } else if lo >= i8::MIN as i32 && hi <= i8::MAX as i32 {
        array::ArrData::I8(vals.iter().map(|&v| v as i8).collect())
    } else if lo >= i16::MIN as i32 && hi <= i16::MAX as i32 {
        array::ArrData::I16(vals.iter().map(|&v| v as i16).collect())
    } else {
        array::ArrData::I32(vals)
    };
    BqnArr { shape, data, fill: Some(B::m_f64(0.0)) }
}

/// Map with an i64 kernel; None if any result leaves the i32 range.
#[inline]
fn map_int<F: Fn(i32) -> i64>(v: &[i32], f: F) -> Option<Vec<i32>> {
    let mut bad = false;
    let out: Vec<i32> = v
        .iter()
        .map(|&x| {
            let r = f(x);
            bad |= r != r as i32 as i64;
            r as i32
        })
        .collect();
    if bad { None } else { Some(out) }
}

#[inline(always)]
fn floored_mod(x: i32, m: i32) -> i32 {
    // m == 0 is excluded by the caller (CBQN gives NaN); result takes the sign of m.
    let r = x.wrapping_rem(m);
    if r != 0 && ((r < 0) != (m < 0)) { r + m } else { r }
}

/// Typed scalar-vs-array pervasive op for integer arrays. `scalar_left` says
/// whether the scalar is the left argument. Returns None when the case is not
/// covered (non-integer array, non-integer scalar for arithmetic, ÷, overflow),
/// in which case the caller uses the generic f64 path.
pub(crate) fn typed_scalar_arr(name: &str, s: f64, arr: &BqnArr, scalar_left: bool) -> Option<BqnArr> {
    if arr.ia() == 0 || !s.is_finite() {
        return None;
    }
    let v = int_view(arr)?;
    let shape = arr.shape.clone();
    // Comparisons: any numeric scalar, result is a Bit array.
    macro_rules! cmp {
        ($op:tt) => {{
            let words = if scalar_left {
                pack_bits(&v, |x| s $op (x as f64))
            } else {
                pack_bits(&v, |x| (x as f64) $op s)
            };
            return Some(BqnArr { shape, data: array::ArrData::Bit(words), fill: Some(B::m_f64(0.0)) });
        }};
    }
    match name {
        "=" => cmp!(==),
        "≠" => cmp!(!=),
        "<" => cmp!(<),
        ">" => cmp!(>),
        "≤" => cmp!(<=),
        "≥" => cmp!(>=),
        _ => {}
    }
    if s.fract() != 0.0 || s.abs() > i32::MAX as f64 {
        return None;
    }
    let si = s as i32;
    let sl = si as i64;
    let out = match (name, scalar_left) {
        ("+", _) => map_int(&v, |x| x as i64 + sl)?,
        ("-", true) => map_int(&v, |x| sl - x as i64)?,
        ("-", false) => map_int(&v, |x| x as i64 - sl)?,
        ("×", _) | ("∧", _) => map_int(&v, |x| x as i64 * sl)?,
        ("∨", _) => map_int(&v, |x| x as i64 + sl - x as i64 * sl)?,
        ("⌊", _) => v.iter().map(|&x| x.min(si)).collect(),
        ("⌈", _) => v.iter().map(|&x| x.max(si)).collect(),
        // s|x
        ("|", true) => {
            if si == 0 {
                return None;
            }
            if si > 0 && (si & (si - 1)) == 0 {
                let mask = si - 1;
                v.iter().map(|&x| x & mask).collect()
            } else {
                v.iter().map(|&x| floored_mod(x, si)).collect()
            }
        }
        // x|s
        ("|", false) => {
            if v.contains(&0) {
                return None;
            }
            v.iter().map(|&m| floored_mod(si, m)).collect()
        }
        _ => return None,
    };
    Some(narrow_i32(out, shape))
}
