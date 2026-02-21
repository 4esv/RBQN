use crate::value::B;

pub fn fill_for(x: B) -> Option<B> {
    if x.is_f64() {
        Some(B::m_f64(0.0))
    } else if x.is_c32() {
        Some(B::m_c32(b' ' as u32))
    } else {
        None
    }
}
