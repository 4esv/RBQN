use crate::array::BqnArr;
use crate::value::B;

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
