use std::fmt;

use crate::value::B;

impl fmt::Display for B {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_f64() {
            let v = self.to_f64();
            if v == (v as i64 as f64) && v.abs() < 1e15 {
                write!(f, "{}", v as i64)
            } else {
                write!(f, "{}", v)
            }
        } else if self.is_c32() {
            match char::from_u32(self.u as u32) {
                Some(c) => write!(f, "'{}'", c),
                None => write!(f, "@+{}", self.u as u32),
            }
        } else if self.q_n() {
            write!(f, "\u{00B7}") // middle dot for Nothing
        } else {
            write!(f, "<value 0x{:016x}>", self.u)
        }
    }
}
