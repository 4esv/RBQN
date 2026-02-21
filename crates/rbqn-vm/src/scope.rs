use std::sync::Arc;

use rbqn_core::value::{B, bi_N, bi_noVar, bi_optOut};

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
        vars.resize(var_am as usize, bi_noVar);
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
    (x.u >> 47) == (((rbqn_core::value::TAG_TAG as u64) << 1) | 1)
}

pub fn v_check_bad_write(x: B) -> bool {
    (x.u >> 46) == (((rbqn_core::value::TAG_TAG as u64) << 2) | 3)
}

fn v_tag_error(x: B, write: bool) -> ! {
    let act = if write { "Assignment: Attempting to modify" } else { "Attempting to read" };
    if x.u == bi_noVar.u {
        rbqn_core::error::throw(format!("{} variable which is not yet defined", act));
    }
    if x.u == bi_optOut.u {
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
    } else {
        rbqn_core::error::throw("v_get: non-var access not yet implemented");
    }
}

pub fn v_set(pscs: &mut [Arc<Scope>], s: B, x: B, upd: bool, _chk: bool) {
    if s.is_var() {
        let d = s.v_depth() as usize;
        let p = s.v_pos() as usize;
        let sc = Arc::make_mut(&mut pscs[d]);
        if upd {
            let prev = sc.vars[p];
            if _chk && v_check_bad_write(prev) {
                v_tag_error(prev, true);
            }
        }
        sc.vars[p] = x;
    } else if s.u == bi_N.u {
        // assigning to Nothing: ignore
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
    } else if s.u == bi_N.u {
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
        sc.vars[p] = bi_optOut;
        r
    } else {
        rbqn_core::error::throw("v_get_move: non-var access not yet implemented");
    }
}
