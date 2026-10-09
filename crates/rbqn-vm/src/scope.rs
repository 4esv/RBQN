use std::sync::Arc;

use rbqn_core::B;

use crate::block::Body;
use crate::namespace::get_ns;

#[derive(Debug)]
pub struct ScopeExt {
    pub var_am: u16,
    pub vars: std::cell::RefCell<Vec<B>>,
}

/// Scope variable storage: up to INLINE_VARS slots live inside the Scope
/// itself (one allocation per call instead of two); larger scopes use a Vec.
#[derive(Debug, Clone)]
pub enum Vars {
    Inline([B; INLINE_VARS], u8),
    Heap(Vec<B>),
}

pub const INLINE_VARS: usize = 8;

impl Vars {
    #[inline]
    pub fn new(var_am: usize, init: &[B]) -> Vars {
        let init = &init[..init.len().min(var_am)];
        if var_am <= INLINE_VARS {
            let mut a = [B::NO_VAR; INLINE_VARS];
            a[..init.len()].copy_from_slice(init);
            Vars::Inline(a, var_am as u8)
        } else {
            let mut v = Vec::with_capacity(var_am);
            v.extend_from_slice(init);
            v.resize(var_am, B::NO_VAR);
            Vars::Heap(v)
        }
    }
}

impl std::ops::Deref for Vars {
    type Target = [B];
    #[inline]
    fn deref(&self) -> &[B] {
        match self {
            Vars::Inline(a, n) => &a[..*n as usize],
            Vars::Heap(v) => v,
        }
    }
}

impl std::ops::DerefMut for Vars {
    #[inline]
    fn deref_mut(&mut self) -> &mut [B] {
        match self {
            Vars::Inline(a, n) => &mut a[..*n as usize],
            Vars::Heap(v) => v,
        }
    }
}

/// Non-owning handle to a scope in the active chain (`current_sc` and its `psc`
/// parents). Valid while the owning `Rc` chain is alive; `eval_bc` holds
/// `current_sc` for the whole lifetime of the `pscs` vector that contains these.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct ScRef(*const Scope);

impl ScRef {
    #[inline(always)]
    pub fn new(rc: &std::rc::Rc<Scope>) -> ScRef { ScRef(std::rc::Rc::as_ptr(rc)) }
    /// Take a new strong reference (for closures that capture this scope).
    #[inline]
    pub fn to_rc(self) -> std::rc::Rc<Scope> {
        // SAFETY: the pointer came from a live Rc (see type docs).
        unsafe {
            std::rc::Rc::increment_strong_count(self.0);
            std::rc::Rc::from_raw(self.0)
        }
    }
}

impl std::ops::Deref for ScRef {
    type Target = Scope;
    #[inline(always)]
    fn deref(&self) -> &Scope { unsafe { &*self.0 } }
}

/// Per-thread interpreter state, kept in one thread-local so a block call
/// pays for the TLS lookup once per phase instead of once per field.
/// ManuallyDrop: no destructor registration (never torn down; leaks at exit).
pub struct Tls {
    /// Current block-evaluation nesting depth.
    pub depth: std::cell::Cell<u32>,
    /// Recycled uniquely-owned scopes.
    pub scopes: std::cell::RefCell<Vec<std::rc::Rc<Scope>>>,
    /// Recycled operand stacks.
    pub stacks: std::cell::RefCell<Vec<Vec<B>>>,
    /// Recycled scope-chain vectors.
    pub pscs: std::cell::RefCell<Vec<Vec<ScRef>>>,
}

std::thread_local! {
    pub static TLS: std::mem::ManuallyDrop<Tls> = const {
        std::mem::ManuallyDrop::new(Tls {
            depth: std::cell::Cell::new(0),
            scopes: std::cell::RefCell::new(Vec::new()),
            stacks: std::cell::RefCell::new(Vec::new()),
            pscs: std::cell::RefCell::new(Vec::new()),
        })
    };
}

#[derive(Debug)]
pub struct Scope {
    pub psc: Option<std::rc::Rc<Scope>>,
    pub body: Arc<Body>,
    pub var_am: u16,
    pub ext: Option<ScopeExt>,
    pub vars: std::cell::RefCell<Vars>,
}

impl Scope {
    pub fn new(body: Arc<Body>, psc: Option<std::rc::Rc<Scope>>, var_am: u16, init_vars: &[B]) -> Self {
        let vars = Vars::new(var_am as usize, init_vars);
        Scope {
            psc,
            body,
            var_am,
            ext: None,
            vars: std::cell::RefCell::new(vars),
        }
    }

    /// Build an `Rc<Scope>`, reusing a recycled allocation when one is available.
    #[inline]
    pub fn new_rc(body: &Arc<Body>, psc: std::rc::Rc<Scope>, var_am: u16, init_vars: &[B]) -> std::rc::Rc<Scope> {
        let recycled = TLS
            .try_with(|t| t.scopes.try_borrow_mut().ok().and_then(|mut f| f.pop()))
            .ok()
            .flatten();
        if let Some(mut rc) = recycled
            && let Some(s) = std::rc::Rc::get_mut(&mut rc)
        {
            s.psc = Some(psc);
            if !Arc::ptr_eq(&s.body, body) {
                s.body = body.clone();
            }
            s.var_am = var_am;
            *s.vars.get_mut() = Vars::new(var_am as usize, init_vars);
            return rc;
        }
        std::rc::Rc::new(Scope::new(body.clone(), Some(psc), var_am, init_vars))
    }

    /// Return a scope to the free list if nothing else holds it (no closure,
    /// namespace or pscs entry captured it).
    #[inline]
    pub fn recycle(mut rc: std::rc::Rc<Scope>) {
        if let Some(s) = std::rc::Rc::get_mut(&mut rc) {
            let parent = s.psc.take();
            s.ext = None;
            let _ = TLS.try_with(|t| {
                if let Ok(mut f) = t.scopes.try_borrow_mut()
                    && f.len() < 64
                {
                    f.push(rc);
                }
            });
            drop(parent);
        }
    }

    /// Read a variable at the given position.
    pub fn var_get(&self, pos: usize) -> B {
        self.vars.borrow_mut()[pos]
    }

    /// Write a variable at the given position.
    pub fn var_set(&self, pos: usize, val: B) {
        self.vars.borrow_mut()[pos] = val;
    }
}

pub fn v_check_bad_read(x: B) -> bool {
    (x.0 >> 47) == (((rbqn_core::TAG_TAG as u64) << 1) | 1)
}

pub fn v_check_bad_write(x: B) -> bool {
    (x.0 >> 46) == (((rbqn_core::TAG_TAG as u64) << 2) | 3)
}

pub fn v_tag_error_pub(x: B, write: bool) -> ! {
    v_tag_error(x, write);
}

fn v_tag_error(x: B, write: bool) -> ! {
    let act = if write { "Assignment: Attempting to modify" } else { "Attempting to read" };
    if x.0 == B::NO_VAR.0 {
        rbqn_core::error::throw(format!("{} variable which is not yet defined", act));
    }
    if x.0 == B::OPT_OUT.0 {
        rbqn_core::error::throw(format!(
            "{} variable which isn't available due to incomplete or aborted F\u{21A9}",
            act
        ));
    }
    rbqn_core::error::throw("Unexpected v_tagError argument");
}

pub fn v_get(pscs: &[ScRef], s: B, chk: bool) -> B {
    if s.is_var() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        let sc = &pscs[d];
        let r = sc.var_get(p);
        if chk && v_check_bad_read(r) {
            v_tag_error(r, false);
        }
        r
    } else if s.is_ext() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        let sc = &pscs[d];
        if let Some(ref ext) = sc.ext {
            let r = ext.vars.borrow_mut()[p];
            if chk && v_check_bad_read(r) {
                v_tag_error(r, false);
            }
            r
        } else {
            rbqn_core::error::throw("v_get: no scope extension for EXT ref");
        }
    } else if s.is_arr() {
        // Array destructuring: read each variable reference in the array
        let s_arr = rbqn_core::get_arr(s)
            .unwrap_or_else(|| rbqn_core::error::throw("v_get: invalid array target"));
        let len = s_arr.ia();
        let mut results = Vec::with_capacity(len);
        for i in 0..len {
            let si = s_arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e));
            results.push(v_get(pscs, si, chk));
        }
        rbqn_core::tag_arr(rbqn_core::array::typed_arr_from_b_vec(results, vec![len], None))
    } else {
        rbqn_core::error::throw("v_get: non-var access not yet implemented");
    }
}

pub fn v_set(pscs: &[ScRef], s: B, x: B, upd: bool, chk: bool) {
    if s.is_var() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        let sc = &pscs[d];

        if upd {
            let prev = sc.var_get(p);
            if chk && v_check_bad_write(prev) {
                v_tag_error(prev, true);
            }
        }
        sc.var_set(p, x);
    } else if s.is_alias() {
        // Namespace field alias (ALIM result): extract field by GID from namespace x,
        // then assign to the variable encoded in the alias.
        let gid = s.alias_gid();
        let d = s.alias_depth() as usize;
        let p = s.alias_pos() as usize;
        let field_val = if x.is_nsp() {
            let ns = get_ns(x);
            ns.get_by_gid(gid)
                .unwrap_or_else(|| rbqn_core::error::throw(
                    format!("Namespace does not have field '{}' (destructuring)", crate::namespace::gid2str(gid))
                ))
        } else {
            rbqn_core::error::throw("Alias assignment: source is not a namespace")
        };
        let sc = &pscs[d];
        if upd {
            let prev = sc.var_get(p);
            if chk && v_check_bad_write(prev) {
                v_tag_error(prev, true);
            }
        }
        sc.var_set(p, field_val);
    } else if s.is_ext() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        let sc = &pscs[d];
        if let Some(ref ext) = sc.ext {
            if upd {
                let prev = ext.vars.borrow_mut()[p];
                if chk && v_check_bad_write(prev) {
                    v_tag_error(prev, true);
                }
            }
            ext.vars.borrow_mut()[p] = x;
        } else {
            rbqn_core::error::throw("v_set: no scope extension for EXT ref");
        }
    } else if s.0 == B::SENTINEL.0 {
        // assigning to Nothing: ignore
    } else if rbqn_core::is_arr_merge(s) {
        // Merge-destructuring ([...] syntax): split x along its first axis (major cells)
        v_merge(pscs, s, x, upd, chk);
    } else if s.is_arr() {
        // List destructuring: ⟨a,b⟩←val or ⟨a,b⟩←ns
        let s_arr = rbqn_core::get_arr(s)
            .unwrap_or_else(|| rbqn_core::error::throw("v_set: invalid array target"));
        let s_len = s_arr.ia();

        if x.is_nsp() {
            // Namespace destructuring: ⟨a,b⟩←ns
            // Each element in s is a VAR ref (field name = variable name) or ALIAS (explicit rename).
            let ns = get_ns(x);
            for i in 0..s_len {
                let si = s_arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e));
                if si.is_var() {
                    // Plain VAR ref: use the variable's own name as the field name
                    let d = si.v_depth() as usize;
                    let p = si.v_pos() as usize;
                    let sc = if d < pscs.len() { &pscs[d] } else {
                        rbqn_core::error::throw("v_set: depth out of bounds for namespace destructuring");
                    };
                    let gid = if p < sc.body.all_var_gids.len() {
                        sc.body.all_var_gids[p]
                    } else {
                        -1
                    };
                    if gid < 0 {
                        rbqn_core::error::throw(
                            "Namespace destructuring: cannot infer field name for variable slot"
                        );
                    }
                    let field_val = ns.get_by_gid(gid)
                        .unwrap_or_else(|| rbqn_core::error::throw(
                            format!("Namespace does not have field '{}' (destructuring)",
                                crate::namespace::gid2str(gid))
                        ));
                    if upd {
                        let prev = sc.var_get(p);
                        if chk && v_check_bad_write(prev) {
                            v_tag_error(prev, true);
                        }
                    }
                    sc.var_set(p, field_val);
                } else if si.is_alias() {
                    // ALIAS ref: explicit field rename ⟨local_var⇐field_name⟩←ns
                    let gid = si.alias_gid();
                    let d = si.alias_depth() as usize;
                    let p = si.alias_pos() as usize;
                    let field_val = ns.get_by_gid(gid)
                        .unwrap_or_else(|| rbqn_core::error::throw(
                            format!("Namespace does not have field '{}' (destructuring alias)",
                                crate::namespace::gid2str(gid))
                        ));
                    let sc = if d < pscs.len() { &pscs[d] } else {
                        rbqn_core::error::throw("v_set alias: depth out of bounds");
                    };
                    if upd {
                        let prev = sc.var_get(p);
                        if chk && v_check_bad_write(prev) {
                            v_tag_error(prev, true);
                        }
                    }
                    sc.var_set(p, field_val);
                } else {
                    // Nested target (list inside list, etc.) — recursively handle
                    v_set(pscs, si, x, upd, chk);
                }
            }
        } else {
            let x_arr = rbqn_core::get_arr(x)
                .unwrap_or_else(|| rbqn_core::error::throw("v_set: expected array value for destructuring"));
            let x_len = x_arr.ia();
            if s_len != x_len {
                rbqn_core::error::throw(format!(
                    "Assignment: Mismatched shape for spread assignment ({} targets vs {} values)",
                    s_len, x_len
                ));
            }
            for i in 0..s_len {
                let si = s_arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e));
                let xi = x_arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e));
                v_set(pscs, si, xi, upd, chk);
            }
        }
    } else {
        rbqn_core::error::throw("v_set: complex assignment not yet implemented");
    }
}

/// Merge-destructuring: split x along its first axis and assign to each target in s.
/// This implements CBQN's v_merge for `[a⋄b]←val` syntax (ARMM targets).
fn v_merge(pscs: &[ScRef], s: B, x: B, upd: bool, chk: bool) {
    let s_arr = rbqn_core::get_arr(s)
        .unwrap_or_else(|| rbqn_core::error::throw("v_merge: invalid merge target"));
    let s_len = s_arr.ia();

    if !x.is_arr() {
        let op = if upd { '\u{21A9}' } else { '\u{2190}' };
        rbqn_core::error::throw(format!("[...]{}x: x cannot have rank 0", op));
    }
    let x_arr = rbqn_core::get_arr(x)
        .unwrap_or_else(|| rbqn_core::error::throw("v_merge: invalid array value"));

    if x_arr.rank() == 0 {
        let op = if upd { '\u{21A9}' } else { '\u{2190}' };
        rbqn_core::error::throw(format!("[...]{}x: x cannot have rank 0", op));
    }

    let first_axis = x_arr.shape[0];
    if first_axis != s_len {
        let op = if upd { '\u{21A9}' } else { '\u{2190}' };
        rbqn_core::error::throw(format!(
            "[...]{}x: Target length & leading axis of x didn't match ({} vs {})",
            op, s_len, first_axis
        ));
    }

    if s_len == 0 {
        return;
    }

    if x_arr.rank() == 1 {
        // Rank 1: each element becomes a unit (rank-0) array like CBQN's m_unit (<x)
        for i in 0..s_len {
            let si = s_arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e));
            let xi = x_arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e));
            // Wrap in rank-0 array (shape []) — equivalent to <x
            let unit = rbqn_core::tag_arr(rbqn_core::array::typed_arr_from_b_vec(
                vec![xi], vec![], x_arr.fill,
            ));
            v_set(pscs, si, unit, upd, chk);
        }
    } else {
        // Rank > 1: split into major cells (toCells)
        // Cell shape is shape[1..]
        let cell_shape: Vec<usize> = x_arr.shape[1..].to_vec();
        let cell_size: usize = cell_shape.iter().product();
        for i in 0..s_len {
            let si = s_arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e));
            // Extract cell i: elements from i*cell_size to (i+1)*cell_size
            let start = i * cell_size;
            let mut cell_data = Vec::with_capacity(cell_size);
            for j in 0..cell_size {
                cell_data.push(x_arr.get(start + j)
                    .unwrap_or_else(|e| rbqn_core::error::throw_bqn(e)));
            }
            let cell = rbqn_core::tag_arr(rbqn_core::array::typed_arr_from_b_vec(
                cell_data, cell_shape.clone(), x_arr.fill,
            ));
            v_set(pscs, si, cell, upd, chk);
        }
    }
}

pub fn v_seth(pscs: &[ScRef], s: B, x: B) -> bool {
    if s.is_var() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        pscs[d].var_set(p, x);
        true
    } else if s.is_ext() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        let sc = &pscs[d];
        if let Some(ref ext) = sc.ext {
            ext.vars.borrow_mut()[p] = x;
            true
        } else {
            false
        }
    } else if s.0 == B::SENTINEL.0 {
        true
    } else if rbqn_core::is_arr_merge(s) {
        // Merge-destructuring header match: split x along first axis
        v_merge_seth(pscs, s, x)
    } else if s.is_alias() {
        // ALIAS_TAG in header: ⟨al⇐a⟩: — extract field by GID from namespace x,
        // then bind to local variable at (depth, pos) encoded in the alias.
        let gid = s.alias_gid();
        let d = s.alias_depth() as usize;
        let p = s.alias_pos() as usize;
        if x.is_nsp() {
            let ns = get_ns(x);
            match ns.get_by_gid(gid) {
                Some(field_val) => {
                    if d < pscs.len() {
                        pscs[d].var_set(p, field_val);
                        true
                    } else {
                        false
                    }
                }
                None => false, // field not found → header doesn't match
            }
        } else {
            false
        }
    } else if s.is_arr() {
        let s_arr = match rbqn_core::get_arr(s) {
            Some(a) => a,
            None => return false,
        };
        let s_len = s_arr.ia();

        // NOTE: when x is a namespace, try namespace destructuring in header
        if x.is_nsp() {
            let ns = get_ns(x);
            for i in 0..s_len {
                let si = match s_arr.get(i) { Ok(v) => v, Err(_) => return false };
                if si.is_var() {
                    // Plain VAR ref: use the variable's own GID as field name
                    let dep = si.v_depth() as usize;
                    let pos = si.v_pos() as usize;
                    if dep >= pscs.len() { return false; }
                    let gid = if pos < pscs[dep].body.all_var_gids.len() {
                        pscs[dep].body.all_var_gids[pos]
                    } else {
                        -1
                    };
                    if gid < 0 { return false; }
                    match ns.get_by_gid(gid) {
                        Some(field_val) => pscs[dep].var_set(pos, field_val),
                        None => return false, // required field missing → header fails
                    }
                } else if si.is_alias() {
                    // ALIAS: ⟨local⇐field⟩ — extract field by GID, bind to local var
                    let field_gid = si.alias_gid();
                    let dep = si.alias_depth() as usize;
                    let pos = si.alias_pos() as usize;
                    if dep >= pscs.len() { return false; }
                    match ns.get_by_gid(field_gid) {
                        Some(field_val) => pscs[dep].var_set(pos, field_val),
                        None => return false, // required field missing → header fails
                    }
                } else {
                    // Other pattern elements (literals, nested arrays) — fall through → no match
                    return false;
                }
            }
            return true;
        }

        let x_arr = match rbqn_core::get_arr(x) {
            Some(a) => a,
            None => return false,
        };
        let x_len = x_arr.ia();
        if s_len != x_len {
            return false;
        }
        for i in 0..s_len {
            let si = match s_arr.get(i) { Ok(v) => v, Err(_) => return false };
            let xi = match x_arr.get(i) { Ok(v) => v, Err(_) => return false };
            if !v_seth(pscs, si, xi) {
                return false;
            }
        }
        true
    } else if s.is_f64() && x.is_f64() {
        // Literal number matching: {2: 4; 𝕩} type headers
        s.o2f() == x.o2f()
    } else if s.is_c32() && x.is_c32() {
        // Literal character matching
        s.0 == x.0
    } else if s.is_f64() || s.is_c32() {
        // Literal vs non-matching type
        false
    } else if (s.is_fun() || s.is_md1() || s.is_md2()) && s.0 == x.0 {
        // Identity match for functions/modifiers
        true
    } else {
        false
    }
}

/// Merge-destructuring for header match (v_seth variant).
fn v_merge_seth(pscs: &[ScRef], s: B, x: B) -> bool {
    let s_arr = match rbqn_core::get_arr(s) {
        Some(a) => a,
        None => return false,
    };
    let s_len = s_arr.ia();

    if !x.is_arr() {
        return false;
    }
    let x_arr = match rbqn_core::get_arr(x) {
        Some(a) => a,
        None => return false,
    };
    if x_arr.rank() == 0 {
        return false;
    }
    if x_arr.shape[0] != s_len {
        return false;
    }
    if s_len == 0 {
        return true;
    }

    if x_arr.rank() == 1 {
        for i in 0..s_len {
            let si = match s_arr.get(i) { Ok(v) => v, Err(_) => return false };
            let xi = match x_arr.get(i) { Ok(v) => v, Err(_) => return false };
            let unit = rbqn_core::tag_arr(rbqn_core::array::typed_arr_from_b_vec(
                vec![xi], vec![], x_arr.fill,
            ));
            if !v_seth(pscs, si, unit) {
                return false;
            }
        }
    } else {
        let cell_shape: Vec<usize> = x_arr.shape[1..].to_vec();
        let cell_size: usize = cell_shape.iter().product();
        for i in 0..s_len {
            let si = match s_arr.get(i) { Ok(v) => v, Err(_) => return false };
            let start = i * cell_size;
            let mut cell_data = Vec::with_capacity(cell_size);
            for j in 0..cell_size {
                match x_arr.get(start + j) {
                    Ok(v) => cell_data.push(v),
                    Err(_) => return false,
                }
            }
            let cell = rbqn_core::tag_arr(rbqn_core::array::typed_arr_from_b_vec(
                cell_data, cell_shape.clone(), x_arr.fill,
            ));
            if !v_seth(pscs, si, cell) {
                return false;
            }
        }
    }
    true
}

pub fn v_get_move(pscs: &[ScRef], s: B, chk: bool) -> B {
    if s.is_var() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        let sc = &pscs[d];
        let r = sc.var_get(p);
        if chk && v_check_bad_read(r) {
            v_tag_error(r, false);
        }
        sc.var_set(p, B::OPT_OUT);
        r
    } else if s.is_ext() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        let sc = &pscs[d];
        if let Some(ref ext) = sc.ext {
            let r = ext.vars.borrow_mut()[p];
            if chk && v_check_bad_read(r) {
                v_tag_error(r, false);
            }
            ext.vars.borrow_mut()[p] = B::OPT_OUT;
            r
        } else {
            rbqn_core::error::throw("v_get_move: no scope extension for EXT ref");
        }
    } else if s.is_arr() {
        // Array destructuring: read+move each variable reference in the array
        let s_arr = rbqn_core::get_arr(s)
            .unwrap_or_else(|| rbqn_core::error::throw("v_get_move: invalid array target"));
        let len = s_arr.ia();
        let mut results = Vec::with_capacity(len);
        for i in 0..len {
            let si = s_arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e));
            results.push(v_get_move(pscs, si, chk));
        }
        rbqn_core::tag_arr(rbqn_core::array::typed_arr_from_b_vec(results, vec![len], None))
    } else {
        rbqn_core::error::throw("v_get_move: non-var access not yet implemented");
    }
}
