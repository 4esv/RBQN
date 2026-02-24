use crate::array::BqnArr;
use crate::value::B;

pub fn atom_equal(w: B, x: B) -> bool {
    w.atom_equal(x)
}

/// Deep structural equality (BQN ≡). Compares arrays recursively.
pub fn deep_equal(w: B, x: B) -> bool {
    if w.is_f64() && x.is_f64() {
        return w.o2f() == x.o2f();
    }
    if w.is_c32() && x.is_c32() {
        return w.0 as u32 == x.0 as u32;
    }
    if w.is_arr() && x.is_arr() {
        let wa = crate::get_arr(w);
        let xa = crate::get_arr(x);
        match (wa, xa) {
            (Some(wa), Some(xa)) => {
                if wa.shape != xa.shape {
                    return false;
                }
                let ia = wa.ia();
                for i in 0..ia {
                    let wv = wa.get(i).unwrap_or(B::SENTINEL);
                    let xv = xa.get(i).unwrap_or(B::SENTINEL);
                    if !deep_equal(wv, xv) {
                        return false;
                    }
                }
                true
            }
            _ => false,
        }
    } else if !w.is_f64() && !w.is_c32() && !x.is_f64() && !x.is_c32() {
        // Non-numeric, non-char, non-array: compare by identity (functions, modifiers, etc.)
        w.0 == x.0
    } else {
        false
    }
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
                // NOTE: Use deep_equal for recursive array comparison
                if !deep_equal(wv, xv) {
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
