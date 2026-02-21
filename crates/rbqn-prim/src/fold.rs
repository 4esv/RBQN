use rbqn_core::*;
use crate::dispatch::PrimResult;

pub type ScalarDyadFn = fn(f64, f64) -> f64;

pub fn fold_numeric(arr: &BqnArr, f: ScalarDyadFn, identity: Option<f64>) -> Result<PrimResult> {
    let vals = arr.f64_iter()?;
    if vals.is_empty() {
        return match identity {
            Some(id) => Ok(PrimResult::Scalar(B::m_f64(id))),
            None => Err(BqnError::Domain("´: 𝕩 is empty with no identity".into())),
        };
    }
    let mut acc = vals[vals.len() - 1];
    for &v in vals[..vals.len() - 1].iter().rev() {
        acc = f(v, acc);
    }
    Ok(PrimResult::Scalar(B::m_f64(acc)))
}

pub fn scan_numeric(arr: &BqnArr, f: ScalarDyadFn) -> Result<PrimResult> {
    let vals = arr.f64_iter()?;
    if vals.is_empty() {
        return Ok(PrimResult::Array(BqnArr::new_vec_f64(vec![])));
    }
    let mut result = Vec::with_capacity(vals.len());
    result.push(vals[0]);
    for &v in &vals[1..] {
        let prev = *result.last().unwrap();
        result.push(f(prev, v));
    }
    let mut out = BqnArr::new_vec_f64(result);
    out.shape = arr.shape.clone();
    Ok(PrimResult::Array(array::squeeze_num(out)))
}

pub fn insert_numeric(arr: &BqnArr, f: ScalarDyadFn, identity: Option<f64>) -> Result<PrimResult> {
    if arr.rank() < 2 {
        return fold_numeric(arr, f, identity);
    }
    Err(error::throw_nyi("˝: rank>1 insert not yet implemented"))
}
