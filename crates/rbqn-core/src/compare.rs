use crate::array::BqnArr;
use crate::value::B;

pub fn atom_equal(w: B, x: B) -> bool {
    w.atom_equal(x)
}

pub fn equal(w: B, x: B, warr: Option<&BqnArr>, xarr: Option<&BqnArr>) -> bool {
    if w.is_f64() && x.is_f64() {
        return w.o2f() == x.o2f();
    }
    if w.is_c32() && x.is_c32() {
        return w.0 as u32 == x.0 as u32;
    }
    match (warr, xarr) {
        (Some(wa), Some(xa)) => {
            if wa.shape != xa.shape {
                return false;
            }
            let ia = wa.ia();
            for i in 0..ia {
                let wv = wa.get(i).unwrap();
                let xv = xa.get(i).unwrap();
                if !atom_equal(wv, xv) {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
}

pub fn compare(w: B, x: B) -> i32 {
    if w.is_f64() && x.is_f64() {
        let wf = w.o2f();
        let xf = x.o2f();
        return if wf < xf {
            -1
        } else if wf > xf {
            1
        } else {
            0
        };
    }
    if w.is_c32() && x.is_c32() {
        let wc = w.0 as u32;
        let xc = x.0 as u32;
        return if wc < xc {
            -1
        } else if wc > xc {
            1
        } else {
            0
        };
    }
    // numbers < characters
    if w.is_f64() && x.is_c32() {
        return -1;
    }
    if w.is_c32() && x.is_f64() {
        return 1;
    }
    0
}
