use rbqn_core::*;
use crate::dispatch::PrimResult;

// ! monad: assert
// NOTE: CBQN semantics (from sysfn.c asrt_c1):
//   ! 1      → succeeds, returns 1
//   ! 0      → fails with "Assertion error"  (x is a non-1 float)
//   ! x      → throws x directly when x is not a float
// The ⟨msg, cond⟩ form used in CBQN's compiler is casrt_c2 (dyadic assert during
// compilation), not user-level !. User-level ! just throws the whole value if not 1.
pub fn assert_c1(x: B, _xa: Option<&BqnArr>) -> Result<PrimResult> {
    if x.is_f64() {
        let v = x.o2f();
        if v == 1.0 {
            return Ok(PrimResult::Scalar(x));
        }
        return Err(BqnError::Assert("Assertion error".to_string()));
    }
    // For non-float x: throw the value directly (CBQN: thr(x))
    // Format the thrown value as a string for the error message.
    let msg = format_assert_msg(x);
    Err(BqnError::Assert(msg))
}

/// Format a B value as an assertion message string.
fn format_assert_msg(x: B) -> String {
    if x.is_f64() {
        return format!("{}", x.o2f());
    }
    if x.is_c32()
        && let Some(ch) = char::from_u32(x.0 as u32) {
            return ch.to_string();
        }
    if let Some(arr) = rbqn_core::get_arr(x) {
        // Try to decode as a char array (string)
        if let Ok(chars) = arr.c32_iter() {
            let s: String = chars.iter().filter_map(|&c| char::from_u32(c)).collect();
            if !s.is_empty() {
                return s;
            }
        }
        // Try to decode as boxed array of strings (nested message)
        if let rbqn_core::array::ArrData::Boxed(ref v) = arr.data {
            // Compiler errors are ⟨position, message⟩; CBQN shows only the message.
            if v.len() == 2 && rbqn_core::get_arr(v[0]).is_some_and(|p| p.f64_iter().is_ok()) {
                return format_assert_msg(v[1]);
            }
            let parts: Vec<String> = v.iter().map(|&b| format_assert_msg(b)).collect();
            return parts.join(": ");
        }
        // Numeric array — format as numbers
        if let Ok(f64s) = arr.f64_iter() {
            let nums: Vec<String> = f64s.iter().map(|&n| format!("{}", n)).collect();
            return format!("[{}]", nums.join(", "));
        }
        return format!("Assertion failed (array ia={})", arr.ia());
    }
    "Assertion failed".to_string()
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
            "Assertion failed".to_string()
        }
    } else {
        "Assertion failed".to_string()
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
