use rbqn_core::*;
use crate::dispatch::PrimResult;

// ! monad: assert
pub fn assert_c1(x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_f64() {
        let v = x.o2f();
        if v == 1.0 {
            return Ok(PrimResult::Scalar(x));
        }
        return Err(BqnError::Assert(format!("!{v}: Assertion failed")));
    }
    Err(BqnError::Type("!𝕩: 𝕩 must be 0 or 1".into()))
}

// ! dyad: assert with message
pub fn assert_msg_c2(w: B, wa: Option<&BqnArr>, x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_f64() && x.o2f() == 1.0 {
        return Ok(PrimResult::Scalar(B::m_i32(1)));
    }
    // w is the error message (often a string = char array)
    let msg = if w.is_f64() {
        format!("{}", w.o2f())
    } else if let Some(arr) = wa {
        // Try to extract string from char array
        if let Ok(chars) = arr.c32_iter() {
            let s: String = chars.iter().filter_map(|&c| char::from_u32(c)).collect();
            s
        } else {
            format!("Assertion failed")
        }
    } else {
        format!("Assertion failed")
    };
    Err(BqnError::Assert(msg))
}

// •Type system function
pub fn type_fn(x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    let t = if x.is_f64() {
        1 // number
    } else if x.is_c32() {
        2 // character
    } else if x.is_arr() {
        0 // array
    } else if x.is_fun() {
        3 // function
    } else if x.is_md1() {
        4 // 1-modifier
    } else if x.is_md2() {
        5 // 2-modifier
    } else if x.is_nsp() {
        6 // namespace
    } else {
        -1
    };
    Ok(PrimResult::Scalar(B::m_i32(t)))
}

// •FillFn system function (stub)
pub fn fill_fn(x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    if let Some(arr) = xa {
        match arr.fill {
            Some(f) => Ok(PrimResult::Scalar(f)),
            None => Err(BqnError::Domain("•FillFn: No fill defined".into())),
        }
    } else {
        Ok(PrimResult::Scalar(fill::fill_for(x).unwrap_or(B::SENTINEL)))
    }
}
