use crate::eltype::ElType;
use crate::value::B;

pub fn squeeze_type(x: B) -> ElType {
    if !x.is_f64() {
        if x.is_c32() {
            let c = x.o2c_unchecked();
            if c < 256 {
                return ElType::C8;
            }
            if c < 65536 {
                return ElType::C16;
            }
            return ElType::C32;
        }
        return ElType::B;
    }
    let f = x.to_f64();
    if f == 0.0 || f == 1.0 {
        return ElType::Bit;
    }
    if f == (f as i32 as i8 as f64) {
        return ElType::I8;
    }
    if f == (f as i32 as i16 as f64) {
        return ElType::I16;
    }
    if f == (f as i32 as f64) {
        return ElType::I32;
    }
    ElType::F64
}

pub fn widen_type(a: ElType, b: ElType) -> ElType {
    if a >= b { a } else { b }
}
