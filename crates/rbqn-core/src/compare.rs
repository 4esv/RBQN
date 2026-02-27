use crate::array::BqnArr;
use crate::value::B;

pub fn atom_equal(w: B, x: B) -> bool {
    if w.is_f64() && x.is_f64() { return w.o2f() == x.o2f(); }
    if w.is_c32() && x.is_c32() { return w.0 as u32 == x.0 as u32; }
    // NOTE: For functions and modifiers, use structural equality (registered by rbqn-vm)
    if (w.is_fun() || w.is_md1() || w.is_md2()) && (x.is_fun() || x.is_md1() || x.is_md2()) {
        return derived_equal(w, x, 0);
    }
    w.atom_equal(x)
}

/// Check structural equality of two derived function/modifier objects.
/// Called from deep_equal when both w and x are functions or modifiers.
/// Returns true if they are structurally identical (same kind + same operands).
/// Depth limit prevents infinite recursion in circular structures.
pub fn derived_equal(w: B, x: B, depth: u32) -> bool {
    if depth > 16 { return false; }
    // Same identity is always equal
    if w.0 == x.0 { return true; }
    // Must both be functions or both be modifiers of the same kind
    let w_is_fun = w.is_fun();
    let x_is_fun = x.is_fun();
    let w_is_md1 = w.is_md1();
    let x_is_md1 = x.is_md1();
    let w_is_md2 = w.is_md2();
    let x_is_md2 = x.is_md2();
    if (w_is_fun != x_is_fun) || (w_is_md1 != x_is_md1) || (w_is_md2 != x_is_md2) {
        return false;
    }
    if !w_is_fun && !w_is_md1 && !w_is_md2 {
        return false;
    }
    // Try to compare derived objects via the rbqn-vm DERIVED_STORE
    // We use a callback approach to avoid circular dep: the VM registers a comparer.
    DERIVED_EQUAL_FN.with(|f| {
        if let Some(ref cmp) = *f.borrow() {
            cmp(w, x, depth)
        } else {
            // No VM registered: fall back to identity comparison
            w.0 == x.0
        }
    })
}

// Thread-local function for derived equality comparison (set by rbqn-vm)
std::thread_local! {
    static DERIVED_EQUAL_FN: std::cell::RefCell<Option<Box<dyn Fn(B, B, u32) -> bool>>> =
        std::cell::RefCell::new(None);
}

/// Register a derived equality comparison function (called by rbqn-vm at startup).
pub fn register_derived_equal_fn(f: impl Fn(B, B, u32) -> bool + 'static) {
    DERIVED_EQUAL_FN.with(|cell| {
        *cell.borrow_mut() = Some(Box::new(f));
    });
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
    } else if (w.is_fun() || w.is_md1() || w.is_md2()) && (x.is_fun() || x.is_md1() || x.is_md2()) {
        // Functions and modifiers: compare structurally (structural equality for ≡)
        derived_equal(w, x, 0)
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
    // BQN comparison: atoms promoted to 1-element arrays.
    // Compare element-by-element in ravel order, shorter < longer if prefix matches.
    // After ravel comparison, rank breaks ties, then shape dimensions.
    let w_is_sortable = w.is_f64() || w.is_c32() || w.is_arr();
    let x_is_sortable = x.is_f64() || x.is_c32() || x.is_arr();
    if w_is_sortable && x_is_sortable {
        let wa = if w.is_arr() { crate::get_arr(w) } else { None };
        let xa = if x.is_arr() { crate::get_arr(x) } else { None };
        let w_elems: usize = wa.as_ref().map_or(1, |a| a.ia());
        let x_elems: usize = xa.as_ref().map_or(1, |a| a.ia());
        let n = w_elems.min(x_elems);
        for i in 0..n {
            let wv = wa.as_ref().map_or(Ok(w), |a| a.get(i));
            let xv = xa.as_ref().map_or(Ok(x), |a| a.get(i));
            if let (Ok(wv), Ok(xv)) = (wv, xv) {
                let c = compare(wv, xv);
                if c != 0 { return c; }
            }
        }
        if w_elems < x_elems { return -1; }
        if w_elems > x_elems { return 1; }
        // Same ravel: lower rank is less
        let w_rank = wa.as_ref().map_or(0u8, |a| a.rank());
        let x_rank = xa.as_ref().map_or(0u8, |a| a.rank());
        if w_rank != x_rank {
            return if w_rank < x_rank { -1 } else { 1 };
        }
        // Same rank and ravel: compare shapes
        if let (Some(wa), Some(xa)) = (&wa, &xa) {
            for i in 0..wa.shape.len() {
                if wa.shape[i] != xa.shape[i] {
                    return if wa.shape[i] < xa.shape[i] { -1 } else { 1 };
                }
            }
        }
        return 0;
    }
    0
}
