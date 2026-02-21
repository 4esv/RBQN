use std::sync::Arc;

use rbqn_core::B;

use crate::block::Body;

#[derive(Debug)]
pub struct ScopeExt {
    pub var_am: u16,
    pub vars: Vec<B>,
}

#[derive(Debug)]
pub struct Scope {
    pub psc: Option<Arc<Scope>>,
    pub body: Arc<Body>,
    pub var_am: u16,
    pub ext: Option<ScopeExt>,
    pub vars: Vec<B>,
}

impl Scope {
    pub fn new(body: Arc<Body>, psc: Option<Arc<Scope>>, var_am: u16, init_vars: &[B]) -> Self {
        let mut vars = Vec::with_capacity(var_am as usize);
        vars.extend_from_slice(init_vars);
        vars.resize(var_am as usize, B::NO_VAR);
        Scope {
            psc,
            body,
            var_am,
            ext: None,
            vars,
        }
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

pub fn v_get(pscs: &[Arc<Scope>], s: B, chk: bool) -> B {
    if s.is_var() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        let sc = &pscs[d];
        let r = sc.vars[p];
        if chk && v_check_bad_read(r) {
            v_tag_error(r, false);
        }
        r
    } else if s.is_ext() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        let sc = &pscs[d];
        if let Some(ref ext) = sc.ext {
            let r = ext.vars[p];
            if chk && v_check_bad_read(r) {
                v_tag_error(r, false);
            }
            r
        } else {
            rbqn_core::error::throw("v_get: no scope extension for EXT ref");
        }
    } else {
        rbqn_core::error::throw("v_get: non-var access not yet implemented");
    }
}

pub fn v_set(pscs: &mut [Arc<Scope>], s: B, x: B, upd: bool, chk: bool) {
    if s.is_var() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        let sc = Arc::make_mut(&mut pscs[d]);
        if upd {
            let prev = sc.vars[p];
            if chk && v_check_bad_write(prev) {
                v_tag_error(prev, true);
            }
        }
        sc.vars[p] = x;
    } else if s.is_ext() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        let sc = Arc::make_mut(&mut pscs[d]);
        if let Some(ref mut ext) = sc.ext {
            if upd {
                let prev = ext.vars[p];
                if chk && v_check_bad_write(prev) {
                    v_tag_error(prev, true);
                }
            }
            ext.vars[p] = x;
        } else {
            rbqn_core::error::throw("v_set: no scope extension for EXT ref");
        }
    } else if s.0 == B::SENTINEL.0 {
        // assigning to Nothing: ignore
    } else if s.is_arr() {
        // Array destructuring: assign each element of x to corresponding target in s
        let s_arr = rbqn_core::get_arr(s)
            .unwrap_or_else(|| rbqn_core::error::throw("v_set: invalid array target"));
        let x_arr = rbqn_core::get_arr(x)
            .unwrap_or_else(|| rbqn_core::error::throw("v_set: expected array value for destructuring"));
        let s_len = s_arr.ia();
        let x_len = x_arr.ia();
        if s_len != x_len {
            rbqn_core::error::throw("v_set: destructuring length mismatch");
        }
        for i in 0..s_len {
            let si = s_arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()));
            let xi = x_arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw(e.to_string()));
            v_set(pscs, si, xi, upd, chk);
        }
    } else {
        rbqn_core::error::throw("v_set: complex assignment not yet implemented");
    }
}

pub fn v_seth(pscs: &mut [Arc<Scope>], s: B, x: B) -> bool {
    if s.is_var() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        let sc = Arc::make_mut(&mut pscs[d]);
        sc.vars[p] = x;
        true
    } else if s.is_ext() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        let sc = Arc::make_mut(&mut pscs[d]);
        if let Some(ref mut ext) = sc.ext {
            ext.vars[p] = x;
            true
        } else {
            false
        }
    } else if s.0 == B::SENTINEL.0 {
        true
    } else if s.is_arr() {
        let s_arr = match rbqn_core::get_arr(s) {
            Some(a) => a,
            None => return false,
        };
        let x_arr = match rbqn_core::get_arr(x) {
            Some(a) => a,
            None => return false,
        };
        let s_len = s_arr.ia();
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
    } else {
        false
    }
}

pub fn v_get_move(pscs: &mut [Arc<Scope>], s: B, chk: bool) -> B {
    if s.is_var() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        let sc = Arc::make_mut(&mut pscs[d]);
        let r = sc.vars[p];
        if chk && v_check_bad_read(r) {
            v_tag_error(r, false);
        }
        sc.vars[p] = B::OPT_OUT;
        r
    } else if s.is_ext() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        let sc = Arc::make_mut(&mut pscs[d]);
        if let Some(ref mut ext) = sc.ext {
            let r = ext.vars[p];
            if chk && v_check_bad_read(r) {
                v_tag_error(r, false);
            }
            ext.vars[p] = B::OPT_OUT;
            r
        } else {
            rbqn_core::error::throw("v_get_move: no scope extension for EXT ref");
        }
    } else {
        rbqn_core::error::throw("v_get_move: non-var access not yet implemented");
    }
}
