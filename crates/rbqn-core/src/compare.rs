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
    // BQN array ordering per spec:
    // 1. Compare elements at corresponding indices (suffix correspondence) in ravel order
    // 2. If one lacks a corresponding index, it is smaller
    // 3. If all match, higher rank is larger, then compare shape from leading axis
    // 4. Atoms: atom is promoted by enclosing; if enclosed atom matches array, atom is smaller

    // Both atoms: direct comparison
    if w.is_f64() && x.is_f64() {
        let wf = w.o2f();
        let xf = x.o2f();
        return if wf < xf { -1 } else if wf > xf { 1 } else { 0 };
    }
    if w.is_c32() && x.is_c32() {
        let wc = w.0 as u32;
        let xc = x.0 as u32;
        return if wc < xc { -1 } else if wc > xc { 1 } else { 0 };
    }
    // numbers < characters
    if w.is_f64() && x.is_c32() { return -1; }
    if w.is_c32() && x.is_f64() { return 1; }

    let w_is_sortable = w.is_f64() || w.is_c32() || w.is_arr();
    let x_is_sortable = x.is_f64() || x.is_c32() || x.is_arr();
    if w_is_sortable && x_is_sortable {
        return compare_values(w, x);
    }
    0
}

/// Compare two sortable BQN values using the BQN spec array ordering.
/// Atoms are promoted to rank-0 arrays (enclosed). If an enclosed atom matches
/// an array, the atom is considered smaller.
fn compare_values(w: B, x: B) -> i32 {
    let w_is_atom = !w.is_arr();
    let x_is_atom = !x.is_arr();

    // Both atoms (different types, e.g. number vs array-as-atom shouldn't happen here
    // since compare() handles same-type atoms above)
    if w_is_atom && x_is_atom {
        return 0; // same type handled above
    }

    // Get shapes: atoms get shape [] (rank 0, 1 element)
    let w_shape: Vec<usize>;
    let x_shape: Vec<usize>;
    let w_rank: usize;
    let x_rank: usize;

    if w_is_atom {
        w_shape = vec![];
        w_rank = 0;
    } else {
        let wa = crate::get_arr(w).unwrap();
        w_shape = wa.shape.clone();
        w_rank = wa.rank() as usize;
    }
    if x_is_atom {
        x_shape = vec![];
        x_rank = 0;
    } else {
        let xa = crate::get_arr(x).unwrap();
        x_shape = xa.shape.clone();
        x_rank = xa.rank() as usize;
    }

    // Pad shapes to same rank with leading 1s
    let max_rank = w_rank.max(x_rank);
    let w_padded: Vec<usize> = {
        let mut s = vec![1usize; max_rank.saturating_sub(w_rank)];
        s.extend_from_slice(&w_shape);
        s
    };
    let x_padded: Vec<usize> = {
        let mut s = vec![1usize; max_rank.saturating_sub(x_rank)];
        s.extend_from_slice(&x_shape);
        s
    };

    // Combined shape for iteration: max of each axis
    let combined: Vec<usize> = (0..max_rank)
        .map(|i| w_padded[i].max(x_padded[i]))
        .collect();

    let total: usize = combined.iter().product::<usize>().max(1);

    // Iterate in ravel order through the combined shape
    // For rank 0, total is 1 and we just compare the single elements
    if max_rank == 0 {
        // Both effectively rank 0
        let wv = if w_is_atom { w } else {
            crate::get_arr(w).and_then(|a| a.get(0).ok()).unwrap_or(B::SENTINEL)
        };
        let xv = if x_is_atom { x } else {
            crate::get_arr(x).and_then(|a| a.get(0).ok()).unwrap_or(B::SENTINEL)
        };
        let c = compare(wv, xv);
        if c != 0 { return c; }
        // Same content: atom < enclosed atom
        if w_is_atom && !x_is_atom { return -1; }
        if !w_is_atom && x_is_atom { return 1; }
        return 0;
    }

    // Compute strides for padded shapes
    let w_strides = compute_strides(&w_padded);
    let x_strides = compute_strides(&x_padded);
    let combined_strides = compute_strides(&combined);

    for flat in 0..total {
        // Decode multi-index from combined shape
        let mut idx = vec![0usize; max_rank];
        let mut rem = flat;
        for k in 0..max_rank {
            idx[k] = rem / combined_strides[k];
            rem %= combined_strides[k];
        }

        // Check if this index is in range for w and x
        let w_in = idx.iter().enumerate().all(|(k, &i)| i < w_padded[k]);
        let x_in = idx.iter().enumerate().all(|(k, &i)| i < x_padded[k]);

        if !w_in && !x_in { continue; } // shouldn't happen
        if !w_in { return -1; } // w lacks this index, w is smaller
        if !x_in { return 1; }  // x lacks this index, x is smaller

        // Both in range: compute flat index for each and compare elements
        let w_flat = idx.iter().enumerate().map(|(k, &i)| i * w_strides[k]).sum::<usize>();
        let x_flat = idx.iter().enumerate().map(|(k, &i)| i * x_strides[k]).sum::<usize>();

        let wv = if w_is_atom { w } else {
            crate::get_arr(w).and_then(|a| a.get(w_flat).ok()).unwrap_or(B::SENTINEL)
        };
        let xv = if x_is_atom { x } else {
            crate::get_arr(x).and_then(|a| a.get(x_flat).ok()).unwrap_or(B::SENTINEL)
        };

        let c = compare(wv, xv);
        if c != 0 { return c; }
    }

    // All elements match. Compare by rank (higher rank = larger), then shape from leading axis.
    if w_rank != x_rank {
        return if w_rank < x_rank { -1 } else { 1 };
    }
    // Same rank: compare shape from leading axis
    for k in 0..w_rank {
        if w_shape[k] != x_shape[k] {
            return if w_shape[k] < x_shape[k] { -1 } else { 1 };
        }
    }

    // Identical shape and content: atom < array with same content
    if w_is_atom && !x_is_atom { return -1; }
    if !w_is_atom && x_is_atom { return 1; }
    0
}

fn compute_strides(shape: &[usize]) -> Vec<usize> {
    let n = shape.len();
    if n == 0 { return vec![]; }
    let mut s = vec![1usize; n];
    for k in (0..n.saturating_sub(1)).rev() {
        s[k] = s[k + 1] * shape[k + 1];
    }
    s
}
