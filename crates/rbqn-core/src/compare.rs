use crate::value::B;

pub fn equal(w: B, x: B) -> bool {
    if w.is_f64() && x.is_f64() {
        return w.to_f64() == x.to_f64();
    }
    w.u == x.u
}

pub fn eequal(w: B, x: B) -> bool {
    if w.is_f64() && x.is_f64() {
        let wf = w.to_f64();
        let xf = x.to_f64();
        return wf == xf || (wf.is_nan() && xf.is_nan());
    }
    if w.is_c32() && x.is_c32() {
        return w.o2c_unchecked() == x.o2c_unchecked();
    }
    w.u == x.u
}
