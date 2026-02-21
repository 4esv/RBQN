use crate::value::{B, bi_noFill, m_c32, m_f64};

pub fn fill_get(x: B) -> B {
    if x.is_f64() {
        m_f64(0.0)
    } else if x.is_c32() {
        m_c32(' ' as u32)
    } else {
        bi_noFill
    }
}

pub fn has_fill(x: B) -> bool {
    !x.no_fill()
}
