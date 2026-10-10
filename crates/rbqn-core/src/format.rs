use crate::array::BqnArr;
use crate::value::B;

/// Format a finite |f| the way CBQN does: shortest round-trip digits, plain
/// when the decimal exponent is in -4..=14, otherwise `d.ddde¯x` / `d.ddde x`.
pub fn fmt_abs_num(abs: f64) -> String {
    if abs == 0.0 {
        return "0".to_string();
    }
    let sci = format!("{:e}", abs);
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    if (-4..=14).contains(&exp) {
        format!("{}", abs)
    } else if exp < 0 {
        format!("{}e¯{}", mant, -exp)
    } else {
        format!("{}e{}", mant, exp)
    }
}

pub fn fmt_val(x: B, arr: Option<&BqnArr>) -> String {
    if x.is_f64() {
        let f = x.o2f();
        if f == f as i64 as f64 && f.abs() < 1e15 {
            format!("{}", f as i64)
        } else {
            format!("{}", f)
        }
    } else if x.is_c32() {
        let c = x.0 as u32;
        match char::from_u32(c) {
            Some(ch) => format!("'{ch}'"),
            None => format!("@+{c}"),
        }
    } else if let Some(a) = arr {
        let mut s = String::from("⟨");
        let ia = a.ia();
        for i in 0..ia.min(20) {
            if i > 0 {
                s.push_str(", ");
            }
            if let Ok(v) = a.get(i) {
                s.push_str(&fmt_val(v, None));
            }
        }
        if ia > 20 {
            s.push_str(", …");
        }
        s.push('⟩');
        s
    } else {
        format!("B({:#018x})", x.0)
    }
}
