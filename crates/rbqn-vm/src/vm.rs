use std::sync::Arc;

use rbqn_core::B;
use rbqn_core::array::BqnArr;

use crate::block::{Block, Body, eval_fun_block, m_md1_block, m_md2_block};
use crate::bytecode::Op;
use crate::derive::{c1, c2, m_fork, m_atop, m1_d, m2_d, m_md2_partial_l, m_md2_partial_r};
use crate::namespace::{self, NS, get_ns, store_ns};
use crate::scope::{Scope, v_get, v_set, v_seth, v_check_bad_read};

pub fn exec_block(bl: &Block, body: Arc<Body>, psc: std::rc::Rc<Scope>) -> B {
    let var_am = body.var_am;
    let sc = std::rc::Rc::new(Scope::new(body.clone(), Some(psc), var_am, &[]));
    eval_bc(&body, sc, bl)
}

pub fn exec_block_with_args(bl: &Block, body: Arc<Body>, psc: std::rc::Rc<Scope>, args: &[B]) -> B {
    let var_am = body.var_am.max(args.len() as u16);
    let sc = std::rc::Rc::new(Scope::new(body.clone(), Some(psc), var_am, args));
    eval_bc(&body, sc, bl)
}

/// Fill `pscs` with the scope chain starting at `sc`, up to `max_psc` entries.
fn build_pscs(pscs: &mut Vec<std::rc::Rc<Scope>>, sc: &std::rc::Rc<Scope>, max_psc: u16) {
    pscs.clear();
    if max_psc > 0 {
        pscs.push(sc.clone());
        let mut current = sc.clone();
        for _ in 1..max_psc {
            match &current.psc {
                Some(p) => {
                    pscs.push(p.clone());
                    current = p.clone();
                }
                None => break,
            }
        }
    }
}

type Pool<T> = std::cell::RefCell<Vec<Vec<T>>>;

std::thread_local! {
    static STACK_POOL: Pool<B> = const { std::cell::RefCell::new(Vec::new()) };
    static PSCS_POOL: Pool<std::rc::Rc<Scope>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A Vec borrowed from a thread-local free list and returned (cleared) on drop,
/// so each block call reuses buffers instead of allocating a stack and a scope
/// chain. Pool access uses try_with/try_borrow_mut, so it never panics; on any
/// failure it just allocates or frees normally.
struct PoolVec<T: 'static> {
    v: Vec<T>,
    pool: &'static std::thread::LocalKey<Pool<T>>,
}

impl<T: 'static> PoolVec<T> {
    fn take(pool: &'static std::thread::LocalKey<Pool<T>>, cap: usize) -> Self {
        let mut v = pool
            .try_with(|p| p.try_borrow_mut().ok().and_then(|mut p| p.pop()))
            .ok()
            .flatten()
            .unwrap_or_default();
        v.reserve(cap);
        PoolVec { v, pool }
    }
}

impl<T: 'static> std::ops::Deref for PoolVec<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> { &self.v }
}

impl<T: 'static> std::ops::DerefMut for PoolVec<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> { &mut self.v }
}

impl<T: 'static> Drop for PoolVec<T> {
    fn drop(&mut self) {
        let mut v = std::mem::take(&mut self.v);
        // Drop elements before touching the pool (dropping a scope may run other drops).
        v.clear();
        if v.capacity() <= 4096 {
            let _ = self.pool.try_with(|p| {
                if let Ok(mut p) = p.try_borrow_mut()
                    && p.len() < 1024 {
                        p.push(v);
                    }
            });
        }
    }
}

/// Helper: unpack an immediate variable reference from a u64.
/// The encoding is: low 32 bits = position, high 32 bits = depth.
fn unpack_var_ref(packed: u64) -> (usize, usize) {
    let pos = packed as u32 as usize;
    let depth = (packed >> 32) as u32 as usize;
    (depth, pos)
}

/// Resolve a SYSV system value by index.
fn sysv_lookup(idx: u32) -> B {
    // System values used by the bootstrap compiler:
    //  0: Type    1: Decompose   4: Glyph   5: PrimInd   7: Fill/FillFn
    //  8: setInvReg  9: setInvSwap  10: nativeInvReg  11: nativeInvSwap
    // 22: GroupLen  23: GroupOrd
    // 30: •BQN  31: •ReBQN  32: •Out  33: •Show  34: •Fmt  35: •Repr  36: •Exit
    // 37: •args  38: •path  39: •name  40: •wdpath  41: •state
    // 100: system value name resolver
    match idx {
        0 | 1 | 4 | 5 | 7 | 8 | 9 | 10 | 11 | 22 | 23 => crate::derive::m_sys_fn(idx),
        // NOTE: these system values are resolved at call-time (c1), not at lookup time,
        // except for environment values (37-41) which return current values immediately.
        30..=36 => crate::derive::m_sys_fn(idx),
        // Environment values: return the value directly
        37 => crate::derive::dispatch_sys_env(37),
        38 => crate::derive::dispatch_sys_env(38),
        39 => crate::derive::dispatch_sys_env(39),
        40 => crate::derive::dispatch_sys_env(40),
        41 => crate::derive::dispatch_sys_env(41),
        100 => crate::derive::m_sys_fn(100),
        _ => B::SENTINEL,
    }
}

// Thread-local trace buffer for debugging VM crashes
std::thread_local! {
    static VM_TRACE: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Push a formatted line to the VM trace buffer, only when RBQN_PRIM_TRACE is set.
/// The format! runs inside the branch so the hot path pays one bool load.
#[macro_export]
macro_rules! vm_trace {
    ($($arg:tt)*) => {
        if $crate::vm::prim_trace_enabled() {
            $crate::vm::vm_trace_push(format!($($arg)*));
        }
    };
}

pub fn vm_trace_push(msg: String) {
    VM_TRACE.with(|t| {
        let mut buf = t.borrow_mut();
        if buf.len() > 500 { buf.remove(0); }
        buf.push(msg);
    });
}

pub fn vm_trace_dump() -> Vec<String> {
    // NOTE: Temporary debugging function - remove when not needed
    VM_TRACE.with(|t| t.borrow().clone())
}

// NOTE: RBQN_VM_TRACE env-var gate, read once instead of per block execution
static VM_DEBUG_ENABLED: std::sync::LazyLock<bool> =
    std::sync::LazyLock::new(|| std::env::var("RBQN_VM_TRACE").is_ok());

// NOTE: RBQN_PRIM_TRACE env-var gate — checked once at startup, zero cost when unset
static PRIM_TRACE_ENABLED: std::sync::LazyLock<bool> =
    std::sync::LazyLock::new(|| std::env::var("RBQN_PRIM_TRACE").is_ok());

/// Returns true when RBQN_PRIM_TRACE is set in the environment.
pub fn prim_trace_enabled() -> bool {
    *PRIM_TRACE_ENABLED
}

/// Format a B value in a short form suitable for trace lines.
pub fn fmt_b_short(b: B) -> String {
    if b.q_n() {
        return "·".to_string();
    }
    if b.is_f64() {
        let v = b.o2f();
        if v < 0.0 {
            return format!("¯{}", -v);
        }
        return format!("{}", v);
    }
    if b.is_c32() {
        let ch = char::from_u32(b.0 as u32).unwrap_or('?');
        return format!("'{}'", ch);
    }
    if b.is_arr() {
        if let Some(arr) = get_arr(b) {
            let shape_str = arr.shape.iter().map(|s| s.to_string()).collect::<Vec<_>>().join("×");
            let el = format!("{:?}", arr.el_type());
            let ia = arr.ia();
            let n = ia.min(3);
            let mut elems = Vec::with_capacity(n);
            for i in 0..n {
                if let Ok(elem) = arr.get(i) {
                    // depth=1: format scalars only, no recursive arrays
                    elems.push(fmt_b_scalar(elem));
                }
            }
            let first3 = elems.join(",");
            if ia > 3 {
                return format!("arr([{}] {} [{},...])", shape_str, el, first3);
            } else {
                return format!("arr([{}] {} [{}])", shape_str, el, first3);
            }
        }
        return "arr(?)".to_string();
    }
    if b.is_fun() {
        let id = (b.0 & 0xFFFFFFFFFFFF) >> 3;
        if let Some(d) = crate::derive::try_get_derived(id) {
            return match &d.kind {
                crate::derive::DerivedKind::NativeFn { prim_idx } => format!("fun(prim={})", prim_idx),
                crate::derive::DerivedKind::FunBlock => "fun(block)".to_string(),
                crate::derive::DerivedKind::Fork => "fun(fork)".to_string(),
                crate::derive::DerivedKind::Atop => "fun(atop)".to_string(),
                crate::derive::DerivedKind::Md1D => "fun(md1d)".to_string(),
                crate::derive::DerivedKind::Md2D => "fun(md2d)".to_string(),
                crate::derive::DerivedKind::SysFn { sys_idx } => format!("fun(sys={})", sys_idx),
                _ => "fun(?)".to_string(),
            };
        }
        return "fun(?)".to_string();
    }
    if b.is_md() {
        let id = (b.0 & 0xFFFFFFFFFFFF) >> 3;
        if let Some(d) = crate::derive::try_get_derived(id) {
            return match &d.kind {
                crate::derive::DerivedKind::NativeMd1 { prim_idx } => format!("md1(prim={})", prim_idx),
                crate::derive::DerivedKind::NativeMd2 { prim_idx } => format!("md2(prim={})", prim_idx),
                crate::derive::DerivedKind::Md1Block => "md1(block)".to_string(),
                crate::derive::DerivedKind::Md2Block => "md2(block)".to_string(),
                _ => "md(?)".to_string(),
            };
        }
        return "md(?)".to_string();
    }
    format!("{:#018x}", b.0)
}

/// Format a scalar B value without recursing into arrays (used for array element display).
fn fmt_b_scalar(b: B) -> String {
    if b.q_n() { return "·".to_string(); }
    if b.is_f64() {
        let v = b.o2f();
        if v < 0.0 { return format!("¯{}", -v); }
        return format!("{}", v);
    }
    if b.is_c32() {
        let ch = char::from_u32(b.0 as u32).unwrap_or('?');
        return format!("'{}'", ch);
    }
    if b.is_arr() { return "arr(...)".to_string(); }
    if b.is_fun() { return "fun".to_string(); }
    format!("{:#x}", b.0)
}

/// Format a B value with more detail (used for tracing results).
/// Shows full shape, element type, and up to 10 elements.
/// For char arrays, shows string content.
pub fn fmt_b_detail(b: B) -> String {
    if b.q_n() { return "·".to_string(); }
    if b.is_f64() {
        let v = b.o2f();
        if v < 0.0 { return format!("¯{}", -v); }
        return format!("{}", v);
    }
    if b.is_c32() {
        let ch = char::from_u32(b.0 as u32).unwrap_or('?');
        return format!("'{}'", ch);
    }
    if b.is_arr() {
        if let Some(arr) = get_arr(b) {
            let shape_str = arr.shape.iter().map(|s| s.to_string()).collect::<Vec<_>>().join("×");
            let el = format!("{:?}", arr.el_type());
            let ia = arr.ia();
            // For char arrays, show string content
            if arr.el_type().is_chr() {
                let chars: String = (0..ia.min(60)).filter_map(|i| {
                    arr.get(i).ok().and_then(|b| {
                        if b.is_c32() { char::from_u32(b.0 as u32) } else { None }
                    })
                }).collect();
                if ia > 60 {
                    return format!("arr([{}] {} \"{}...\")", shape_str, el, chars);
                } else {
                    return format!("arr([{}] {} \"{}\")", shape_str, el, chars);
                }
            }
            let n = ia.min(10);
            let mut elems = Vec::with_capacity(n);
            for i in 0..n {
                if let Ok(elem) = arr.get(i) {
                    elems.push(fmt_b_scalar(elem));
                }
            }
            let elem_str = elems.join(",");
            if ia > 10 {
                return format!("arr([{}] {} [{},...])", shape_str, el, elem_str);
            } else {
                return format!("arr([{}] {} [{}])", shape_str, el, elem_str);
            }
        }
        return "arr(?)".to_string();
    }
    fmt_b_short(b)
}


pub fn eval_bc(body: &Body, sc: std::rc::Rc<Scope>, bl: &Block) -> B {
    let bc = &bl.bc;
    let mut pc = body.bc_offset;
    let mut stack = PoolVec::take(&STACK_POOL, body.max_stack as usize);
    let mut current_sc = sc.clone();
    let mut pscs = PoolVec::take(&PSCS_POOL, body.max_psc as usize);
    build_pscs(&mut pscs, &current_sc, body.max_psc);

    macro_rules! pop {
        () => {
            stack.pop().unwrap_or_else(|| rbqn_core::error::throw("VM: stack underflow"))
        };
    }
    macro_rules! push {
        ($v:expr) => {
            stack.push($v)
        };
    }
    macro_rules! peek {
        ($n:expr) => {
            stack[stack.len() - $n]
        };
    }
    macro_rules! read_u32 {
        () => {{
            let v = bc[pc] as u32;
            pc += 1;
            v
        }};
    }
    macro_rules! read_u64 {
        () => {{
            let lo = bc[pc] as u32;
            let hi = bc[pc + 1] as u32;
            pc += 2;
            (lo as u64) | ((hi as u64) << 32)
        }};
    }

    // NOTE: Debug flag for targeted tracing during runtime1 bootstrap
    let vm_debug = *VM_DEBUG_ENABLED;

    loop {
        if pc >= bc.len() {
            rbqn_core::error::throw("VM: bytecode overrun");
        }
        let op_val = bc[pc] as u32;
        pc += 1;

        let op = Op::from_u32(op_val);

        if vm_debug {
            let op_name = op.map(|o| format!("{:?}", o)).unwrap_or_else(|| format!("0x{:02x}", op_val));
            let stack_info: String = stack.iter().rev().take(4).enumerate().map(|(i, b)| {
                if b.is_arr() {
                    let ia = get_arr(*b).map_or(-1i64, |a| a.ia() as i64);
                    format!("s[{}]=arr(ia={})", i, ia)
                } else if b.is_f64() {
                    format!("s[{}]={}", i, b.o2f())
                } else if b.is_fun() {
                    format!("s[{}]=fun", i)
                } else {
                    format!("s[{}]={:#x}", i, b.0)
                }
            }).collect::<Vec<_>>().join(" ");
            vm_trace_push(format!("OP pc={} {} stk=[{}]", pc-1, op_name, stack_info));
        }

        match op {
            Some(Op::POPS) => {
                pop!();
            }
            Some(Op::ADDI) => {
                let v = read_u64!();
                push!(B::from_u64(v));
            }
            Some(Op::ADDU) => {
                let v = read_u64!();
                push!(B::from_u64(v));
            }
            Some(Op::PUSH) => {
                let idx = read_u32!() as usize;
                push!(bl.comp.objs[idx]);
            }

            // --- Function calls ---
            Some(Op::FN1C) => {
                let f = pop!();
                let x = pop!();
                push!(c1(f, x));
            }
            Some(Op::FN1O) => {
                let f = pop!();
                let x = pop!();
                if x.q_n() {
                    push!(x);
                } else {
                    push!(c1(f, x));
                }
            }
            Some(Op::FN2C) => {
                let w = pop!();
                let f = pop!();
                let x = pop!();
                // NOTE: Debug trace for FN2C with empty array arguments
                if vm_debug && x.is_arr()
                    && let Some(xa) = get_arr(x)
                        && xa.ia() == 0 && w.is_f64() {
                            let f_tag = (f.0 >> 48) as u16;
                            vm_trace_push(format!(
                                "FN2C w={} f_tag={:#06x} x=EMPTY_ARR shape={:?} bc_pc={}",
                                w.o2f(), f_tag, xa.shape, pc
                            ));
                            if f.is_fun() {
                                let id = (f.0 & 0xFFFFFFFFFFFF) >> 3;
                                let d = crate::derive::get_derived(id);
                                vm_trace_push(format!("  f_kind={:?}", d.kind));
                            }
                        }
                push!(c2(f, w, x));
            }
            Some(Op::FN2O) => {
                let w = pop!();
                let f = pop!();
                let x = pop!();
                if x.q_n() {
                    push!(x);
                } else if w.q_n() {
                    push!(c1(f, x));
                } else {
                    push!(c2(f, w, x));
                }
            }

            // --- Inline function call variants ---
            Some(Op::FN1Ci) => {
                let f = B::from_u64(read_u64!());
                let x = pop!();
                push!(c1(f, x));
            }
            Some(Op::FN1Oi) => {
                let f = B::from_u64(read_u64!());
                let x = pop!();
                if x.q_n() {
                    push!(x);
                } else {
                    push!(c1(f, x));
                }
            }
            Some(Op::FN2Ci) => {
                let f = B::from_u64(read_u64!());
                let w = pop!();
                let x = pop!();
                push!(c2(f, w, x));
            }
            Some(Op::FN2Oi) => {
                // FIX: FN2Oi has TWO u64 immediates — monadic fn ptr and dyadic fn ptr.
                // CBQN: bL_m[FN2Oi]=5 (1 opcode + 2*u64 = 5 u32 words).
                let f_mono = B::from_u64(read_u64!());
                let f_dy = B::from_u64(read_u64!());
                let w = pop!();
                let x = pop!();
                if x.q_n() {
                    push!(x);
                } else if w.q_n() {
                    push!(c1(f_mono, x));
                } else {
                    push!(c2(f_dy, w, x));
                }
            }

            // --- List/array construction ---
            Some(Op::LSTO) | Some(Op::LSTM) => {
                let sz = read_u32!() as usize;
                if sz == 0 {
                    push!(tag_arr(BqnArr::empty_harr()));
                } else {
                    let mut elems = vec![B::SENTINEL; sz];
                    for i in 0..sz {
                        elems[sz - i - 1] = pop!();
                    }
                    // NOTE: try to produce typed array (numeric/char) when homogeneous
                    push!(b_vec_to_arr(elems));
                }
            }
            Some(Op::ARMO) => {
                let sz = read_u32!() as usize;
                let mut elems = vec![B::SENTINEL; sz];
                for i in 0..sz {
                    elems[sz - i - 1] = pop!();
                }
                // FIX: ARMO is "array merge observable" — equivalent to > on the
                // collected list. It merges elements (all same-shape arrays) into
                // a single array with one extra leading dimension.
                push!(bqn_merge(elems));
            }
            Some(Op::ARMM) => {
                let sz = read_u32!() as usize;
                let mut elems = vec![B::SENTINEL; sz];
                for i in 0..sz {
                    elems[sz - i - 1] = pop!();
                }
                // NOTE: ARMM uses tag_arr_merge to mark as merge-destructuring target.
                // v_set checks this to split by major cells instead of flat elements.
                push!(rbqn_core::tag_arr_merge(BqnArr::from_b_vec(elems)));
            }

            // --- Block definitions ---
            Some(Op::DFND0) => {
                let bl_data = read_u64!();
                let bl_idx = bl_data as usize;
                if bl_idx < bl.blocks.len() {
                    let child_bl = bl.blocks[bl_idx].clone();
                    let psc = if !pscs.is_empty() { pscs[0].clone() } else { current_sc.clone() };
                    push!(eval_fun_block(child_bl, psc));
                } else {
                    rbqn_core::error::throw("DFND0: block index out of bounds");
                }
            }
            Some(Op::DFND1) => {
                let bl_data = read_u64!();
                let bl_idx = bl_data as usize;
                if bl_idx < bl.blocks.len() {
                    let child_bl = bl.blocks[bl_idx].clone();
                    let psc = if !pscs.is_empty() { pscs[0].clone() } else { current_sc.clone() };
                    push!(m_md1_block(child_bl, psc));
                } else {
                    rbqn_core::error::throw("DFND1: block index out of bounds");
                }
            }
            Some(Op::DFND2) => {
                let bl_data = read_u64!();
                let bl_idx = bl_data as usize;
                if bl_idx < bl.blocks.len() {
                    let child_bl = bl.blocks[bl_idx].clone();
                    let psc = if !pscs.is_empty() { pscs[0].clone() } else { current_sc.clone() };
                    push!(m_md2_block(child_bl, psc));
                } else {
                    rbqn_core::error::throw("DFND2: block index out of bounds");
                }
            }

            // --- Modifier application ---
            Some(Op::MD1C) => {
                let f = pop!();
                let m = pop!();
                push!(m1_d(m, f));
            }
            Some(Op::MD2C) => {
                let f = pop!();
                let m = pop!();
                let g = pop!();
                push!(m2_d(m, f, g));
            }
            Some(Op::MD2L) => {
                // Partial 2-modifier: has left operand, needs right
                let f = pop!();
                let m2 = pop!();
                push!(m_md2_partial_l(m2, f));
            }
            Some(Op::MD2R) => {
                // Partial 2-modifier: has right operand, needs left
                let g = pop!();
                let m2 = pop!();
                push!(m_md2_partial_r(m2, g));
            }

            // --- Trains ---
            Some(Op::TR2D) => {
                let g = pop!();
                let h = pop!();
                push!(m_atop(g, h));
            }
            Some(Op::TR3D) => {
                let f = pop!();
                let g = pop!();
                let h = pop!();
                push!(m_fork(f, g, h));
            }
            Some(Op::TR3O) => {
                let f = pop!();
                let g = pop!();
                let h = pop!();
                if f.q_n() {
                    push!(m_atop(g, h));
                } else {
                    push!(m_fork(f, g, h));
                }
            }

            // --- Variable access ---
            Some(Op::VARO) => {
                let d = read_u32!();
                let p = read_u32!();
                let val = pscs[d as usize].var_get(p as usize);
                if vm_debug && val.is_arr()
                    && let Some(a) = get_arr(val)
                        && a.ia() == 0 {
                            vm_trace_push(format!("  VARO d={} p={} → EMPTY_ARR shape={:?}", d, p, a.shape));
                        }
                if v_check_bad_read(val) {
                    rbqn_core::error::throw("Attempting to read variable which is not yet defined");
                }
                push!(val);
            }
            Some(Op::VARM) => {
                let d = read_u32!();
                let p = read_u32!();
                push!(rbqn_core::tagu64((d as u64) << 32 | p as u64, rbqn_core::VAR_TAG));
            }
            Some(Op::VARU) => {
                let d = read_u32!();
                let p = read_u32!();
                let val = pscs[d as usize].var_get(p as usize);
                push!(val);
                pscs[d as usize].var_set(p as usize, B::OPT_OUT);
            }
            Some(Op::EXTO) => {
                let d = read_u32!();
                let p = read_u32!();
                if let Some(ref ext) = pscs[d as usize].ext {
                    let val = ext.vars.borrow_mut()[p as usize];
                    if v_check_bad_read(val) {
                        rbqn_core::error::throw("Attempting to read ext variable which is not yet defined");
                    }
                    push!(val);
                } else {
                    rbqn_core::error::throw("EXTO: no scope extension");
                }
            }
            Some(Op::EXTM) => {
                let d = read_u32!();
                let p = read_u32!();
                push!(rbqn_core::tagu64((d as u64) << 32 | p as u64, rbqn_core::EXT_TAG));
            }
            Some(Op::EXTU) => {
                let d = read_u32!();
                let p = read_u32!();
                if let Some(ref ext) = pscs[d as usize].ext {
                    let val = ext.vars.borrow_mut()[p as usize];
                    push!(val);
                } else {
                    rbqn_core::error::throw("EXTU: no scope extension");
                }
                if let Some(ref ext) = pscs[d as usize].ext {
                    ext.vars.borrow_mut()[p as usize] = B::OPT_OUT;
                }
            }

            // --- Dynamic variables ---
            Some(Op::DYNO) => {
                let idx = read_u32!();
                push!(sysv_lookup(idx));
            }
            Some(Op::DYNM) => {
                let _idx = read_u32!();
                // Push a mutable reference placeholder — for now treat as read-only
                push!(B::SENTINEL);
            }

            // --- Assignment opcodes ---
            Some(Op::SETN) => {
                let s = pop!();
                let x = pop!();
                v_set(&pscs, s, x, false, true);
                push!(x);
            }
            Some(Op::SETU) => {
                let s = pop!();
                let x = pop!();
                v_set(&pscs, s, x, true, true);
                push!(x);
            }
            Some(Op::SETM) => {
                let s = pop!();
                let f = pop!();
                let x = pop!();
                let w = v_get(&pscs, s, true);
                let r = c2(f, w, x);
                v_set(&pscs, s, r, true, false);
                push!(r);
            }
            Some(Op::SETC) => {
                let s = pop!();
                let f = pop!();
                let x = v_get(&pscs, s, true);
                let r = c1(f, x);
                v_set(&pscs, s, r, true, false);
                push!(r);
            }

            // --- Immediate set variants (variable ref encoded as immediate u64) ---
            Some(Op::SETNi) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let x = pop!();
                pscs[d].var_set(p, x);
                push!(x);
            }
            Some(Op::SETUi) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let x = pop!();
                let prev = pscs[d].var_get(p);
                if crate::scope::v_check_bad_write(prev) {
                    crate::scope::v_tag_error_pub(prev, true);
                }
                pscs[d].var_set(p, x);
                push!(x);
            }
            Some(Op::SETMi) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let f = pop!();
                let x = pop!();
                let w = pscs[d].var_get(p);
                let r = c2(f, w, x);
                pscs[d].var_set(p, r);
                push!(r);
            }
            Some(Op::SETCi) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let f = pop!();
                let x = pscs[d].var_get(p);
                let r = c1(f, x);
                pscs[d].var_set(p, r);
                push!(r);
            }

            // --- Void set variants (like SETNi but don't push result) ---
            Some(Op::SETNv) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let x = pop!();
                pscs[d].var_set(p, x);
            }
            Some(Op::SETUv) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let x = pop!();
                let prev = pscs[d].var_get(p);
                if crate::scope::v_check_bad_write(prev) {
                    crate::scope::v_tag_error_pub(prev, true);
                }
                pscs[d].var_set(p, x);
            }
            Some(Op::SETMv) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let f = pop!();
                let x = pop!();
                let w = pscs[d].var_get(p);
                let r = c2(f, w, x);
                pscs[d].var_set(p, r);
            }
            Some(Op::SETCv) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let f = pop!();
                let x = pscs[d].var_get(p);
                let r = c1(f, x);
                pscs[d].var_set(p, r);
            }

            // --- Header match (iterative retry instead of recursive exec_block) ---
            Some(Op::SETH1) => {
                let s = pop!();
                let x = pop!();
                let next_body_idx = read_u64!() as usize;
                if !v_seth(&pscs, s, x) {
                    let next_body = bl.bodies[next_body_idx].clone();
                    if !next_body.exists {
                        rbqn_core::error::throw("No matching header");
                    }
                    // NOTE: Preserve original args when retrying — needed for modifier blocks
                    // where 𝕣 (var[0]) and 𝔽 (operand) must be available in the next body.
                    let arg_count = crate::block::arg_count(bl.ty, bl.imm) as usize;
                    let args: Vec<B> = if arg_count > 0 {
                        let vars = current_sc.vars.borrow_mut();
                        (0..arg_count).map(|i| vars.get(i).copied().unwrap_or(B::SENTINEL)).collect()
                    } else {
                        vec![]
                    };
                    let parent = current_sc.psc.clone().unwrap_or(current_sc.clone());
                    let var_am = next_body.var_am.max(args.len() as u16);
                    let new_sc = std::rc::Rc::new(Scope::new(next_body.clone(), Some(parent), var_am, &args));
                    current_sc = new_sc;
                    build_pscs(&mut pscs, &current_sc, next_body.max_psc);
                    pc = next_body.bc_offset;
                    stack.clear();
                    continue;
                }
            }
            Some(Op::SETH2) => {
                let s = pop!();
                let x = pop!();
                let mono_idx = read_u64!() as usize;
                let dy_idx = read_u64!() as usize;
                if !v_seth(&pscs, s, x) {
                    let vars = current_sc.vars.borrow_mut();
                    let is_dyadic = vars.get(2).is_some_and(|b| !b.q_n());
                    let next_idx = if is_dyadic { dy_idx } else { mono_idx };
                    let next_body = bl.bodies[next_idx].clone();
                    if !next_body.exists {
                        rbqn_core::error::throw("No matching header");
                    }
                    let arg_count = crate::block::arg_count(bl.ty, bl.imm) as usize;
                    let args: Vec<B> = (0..arg_count).map(|i| {
                        vars.get(i).copied().unwrap_or(B::SENTINEL)
                    }).collect();
                    drop(vars);
                    // Iterative retry: reset VM state for the new body
                    let parent = current_sc.psc.clone().unwrap_or(current_sc.clone());
                    let var_am = next_body.var_am.max(args.len() as u16);
                    let new_sc = std::rc::Rc::new(Scope::new(next_body.clone(), Some(parent), var_am, &args));
                    current_sc = new_sc;
                    build_pscs(&mut pscs, &current_sc, next_body.max_psc);
                    pc = next_body.bc_offset;
                    stack.clear();
                    continue;
                }
            }

            // --- Predicates (iterative retry instead of recursive exec_block) ---
            Some(Op::PRED1) => {
                let x = pop!();
                let next_body_idx = read_u64!() as usize;
                // NOTE: BQN predicates require a boolean (0 or 1), not arbitrary numbers
                if x.is_f64() {
                    let v = x.o2f();
                    if v != 0.0 && v != 1.0 {
                        rbqn_core::error::throw("Expected boolean");
                    }
                }
                if !x.o2b() {
                    let next_body = bl.bodies[next_body_idx].clone();
                    if !next_body.exists {
                        rbqn_core::error::throw("No matching predicate");
                    }
                    // Iterative retry: preserve original args for modifier blocks
                    let arg_count = crate::block::arg_count(bl.ty, bl.imm) as usize;
                    let args: Vec<B> = if arg_count > 0 {
                        let vars = current_sc.vars.borrow_mut();
                        (0..arg_count).map(|i| vars.get(i).copied().unwrap_or(B::SENTINEL)).collect()
                    } else {
                        vec![]
                    };
                    let parent = current_sc.psc.clone().unwrap_or(current_sc.clone());
                    let var_am = next_body.var_am.max(args.len() as u16);
                    let new_sc = std::rc::Rc::new(Scope::new(next_body.clone(), Some(parent), var_am, &args));
                    current_sc = new_sc;
                    build_pscs(&mut pscs, &current_sc, next_body.max_psc);
                    pc = next_body.bc_offset;
                    stack.clear();
                    continue;
                }
            }
            Some(Op::PRED2) => {
                let x = pop!();
                let mono_idx = read_u64!() as usize;
                let dy_idx = read_u64!() as usize;
                // NOTE: BQN predicates require a boolean (0 or 1), not arbitrary numbers
                if x.is_f64() {
                    let v = x.o2f();
                    if v != 0.0 && v != 1.0 {
                        rbqn_core::error::throw("Expected boolean");
                    }
                }
                if !x.o2b() {
                    let vars = current_sc.vars.borrow_mut();
                    let is_dyadic = vars.get(2).is_some_and(|b| !b.q_n());
                    let next_idx = if is_dyadic { dy_idx } else { mono_idx };
                    let next_body = bl.bodies[next_idx].clone();
                    if !next_body.exists {
                        rbqn_core::error::throw("No matching predicate");
                    }
                    let arg_count = crate::block::arg_count(bl.ty, bl.imm) as usize;
                    let args: Vec<B> = (0..arg_count).map(|i| {
                        vars.get(i).copied().unwrap_or(B::SENTINEL)
                    }).collect();
                    drop(vars);
                    // Iterative retry
                    let parent = current_sc.psc.clone().unwrap_or(current_sc.clone());
                    let var_am = next_body.var_am.max(args.len() as u16);
                    let new_sc = std::rc::Rc::new(Scope::new(next_body.clone(), Some(parent), var_am, &args));
                    current_sc = new_sc;
                    build_pscs(&mut pscs, &current_sc, next_body.max_psc);
                    pc = next_body.bc_offset;
                    stack.clear();
                    continue;
                }
            }

            // --- Namespace field access ---
            Some(Op::FLDG) => {
                let ns_val = pop!();
                let gid = read_u32!() as i32;
                if !ns_val.is_nsp() {
                    rbqn_core::error::throw("Trying to read a field from non-namespace");
                }
                let ns = get_ns(ns_val);
                match ns.get_by_gid(gid) {
                    Some(v) => push!(v),
                    None => rbqn_core::error::throw(
                        format!("Namespace does not have field '{}'", namespace::gid2str(gid))
                    ),
                }
            }
            Some(Op::FLDO) => {
                let ns_val = pop!();
                let gid = read_u32!() as i32;
                if !ns_val.is_nsp() {
                    rbqn_core::error::throw("Trying to read a field from non-namespace");
                }
                let ns = get_ns(ns_val);
                match ns.get_by_gid(gid) {
                    Some(v) => push!(v),
                    None => push!(B::SENTINEL), // optional: Nothing if not found
                }
            }
            Some(Op::FLDM) => {
                let ns_val = pop!();
                let gid = read_u32!() as i32;
                if !ns_val.is_nsp() {
                    rbqn_core::error::throw("Trying to read a field from non-namespace");
                }
                let ns = get_ns(ns_val);
                match ns.get_by_gid(gid) {
                    Some(v) => push!(v),
                    None => rbqn_core::error::throw(
                        format!("Namespace does not have field '{}' for modification", namespace::gid2str(gid))
                    ),
                }
            }

            Some(Op::ALIM) => {
                // Build a field-alias value that stores both the variable reference
                // (depth+pos from the VARM result) and the GID for namespace extraction.
                // ALIAS_TAG encoding: bits 47:32 = GID, bits 31:16 = depth, bits 15:0 = pos
                let o = pop!();
                let gid = read_u32!() as i32;
                // o is a VAR-tagged value: bits 47:32 = depth, bits 31:0 = pos
                // For alias, depth fits in 16 bits (typically 0-15); pos also small.
                let depth16 = o.v_depth() as u16;
                let pos16 = (o.v_pos() & 0xFFFF) as u16;
                let alias_payload = ((gid as u64 & 0xFFFF) << 32)
                    | ((depth16 as u64) << 16)
                    | (pos16 as u64);
                push!(rbqn_core::tagu64(alias_payload, rbqn_core::ALIAS_TAG));
            }

            Some(Op::CHKV) => {
                if peek!(1).q_n() {
                    rbqn_core::error::throw("Unexpected Nothing (\u{00B7})");
                }
            }

            Some(Op::VFYM) => {
                // Verify mutable: check that top of stack is a valid mutable reference.
                // For now, just pass through (the value stays on the stack).
                // VFYM consumes 1, produces 1 => net 0.
                // It pops and re-pushes, or just peeks. stack_diff=0, consumed=1 suggests pop+push.
                let v = pop!();
                push!(v);
            }

            Some(Op::FAIL) => {
                rbqn_core::error::throw("This block cannot be called with these arguments");
            }

            // --- System values ---
            Some(Op::SYSV) => {
                let idx = read_u32!();
                push!(sysv_lookup(idx));
            }

            // --- Return opcodes ---
            Some(Op::RETD) => {
                // Build namespace from scope exports and return it
                if let Some(ref ns_desc) = body.ns_desc {
                    // Pop the unused stack value if present
                    if !stack.is_empty() {
                        pop!();
                    }
                    return store_ns(NS {
                        desc: ns_desc.clone(),
                        sc: if !pscs.is_empty() { pscs[0].clone() } else { current_sc.clone() },
                    });
                }
                // No namespace descriptor: just return top of stack or SENTINEL
                if stack.is_empty() {
                    return B::SENTINEL;
                }
                return pop!();
            }
            Some(Op::RETN) => {
                return pop!();
            }

            _ => {
                rbqn_core::error::throw(format!("VM: unhandled opcode 0x{:02x}", op_val));
            }
        }
    }
}

/// Convert a Vec<B> to a typed B array value.
/// Produces a numeric array if all elements are f64 scalars,
/// a character array if all are c32, otherwise keeps Boxed.
/// This matches CBQN's behavior where list literals are typed when homogeneous.
pub fn b_vec_to_arr(elems: Vec<B>) -> B {
    if elems.is_empty() {
        return tag_arr(BqnArr::empty_harr());
    }
    if elems.iter().all(|b| b.is_f64()) {
        let vals: Vec<f64> = elems.iter().map(|b| b.o2f()).collect();
        let arr = rbqn_core::array::BqnArr::new_vec_f64(vals);
        return tag_arr(rbqn_core::array::squeeze_num(arr));
    }
    if elems.iter().all(|b| b.is_c32()) {
        let vals: Vec<u32> = elems.iter().map(|b| b.0 as u32).collect();
        let arr = BqnArr::new_vec_c32(vals);
        return tag_arr(arr);
    }
    tag_arr(BqnArr::from_b_vec(elems))
}

/// Merge a list of elements (ARMO opcode). Equivalent to BQN's `>` on the list.
/// Each element should be an array of the same shape. The result has shape
/// `(len(elems)) ∾ inner_shape`. Scalar elements are treated as 0-rank (no inner dims).
fn bqn_merge(elems: Vec<B>) -> B {
    if elems.is_empty() {
        return tag_arr(BqnArr::empty_harr());
    }
    // If all elements are scalars (f64 or c32), just build a typed 1-d array
    if elems.iter().all(|b| b.is_f64()) {
        let vals: Vec<f64> = elems.iter().map(|b| b.o2f()).collect();
        let arr = BqnArr::new_vec_f64(vals);
        return tag_arr(rbqn_core::array::squeeze_num(arr));
    }
    if elems.iter().all(|b| b.is_c32()) {
        let vals: Vec<u32> = elems.iter().map(|b| b.0 as u32).collect();
        return tag_arr(BqnArr::new_vec_c32(vals));
    }
    // If all elements are arrays, merge them
    if elems.iter().all(|b| b.is_arr()) {
        let arrs: Vec<std::sync::Arc<BqnArr>> = elems.iter().filter_map(|b| get_arr(*b)).collect();
        if arrs.len() == elems.len() {
            let inner_shape = &arrs[0].shape;
            let inner_ia: usize = inner_shape.iter().product();
            // Check all elements have the same inner shape
            let same_shape = arrs.iter().all(|a| a.shape == *inner_shape);
            if same_shape {
                // Build result shape: [len, inner_shape...]
                let mut out_shape = vec![elems.len()];
                out_shape.extend_from_slice(inner_shape);
                // Flatten all element data
                let mut flat: Vec<B> = Vec::with_capacity(elems.len() * inner_ia);
                for a in &arrs {
                    for i in 0..inner_ia {
                        flat.push(a.get(i).unwrap_or(B::SENTINEL));
                    }
                }
                let arr = rbqn_core::array::typed_arr_from_b_vec(flat, out_shape, arrs[0].fill);
                return tag_arr(arr);
            }
        }
    }
    // Mixed or non-conforming: fall back to boxed list
    tag_arr(BqnArr::from_b_vec(elems))
}

pub fn tag_arr(arr: BqnArr) -> B {
    rbqn_core::tag_arr(arr)
}

pub fn get_arr(b: B) -> Option<std::sync::Arc<BqnArr>> {
    rbqn_core::get_arr(b)
}

impl Clone for Scope {
    fn clone(&self) -> Self {
        Scope {
            psc: self.psc.clone(),
            body: self.body.clone(),
            var_am: self.var_am,
            ext: self.ext.as_ref().map(|e| crate::scope::ScopeExt {
                var_am: e.var_am,
                vars: std::cell::RefCell::new(e.vars.borrow_mut().clone()),
            }),
            vars: std::cell::RefCell::new(self.vars.borrow_mut().clone()),
        }
    }
}
