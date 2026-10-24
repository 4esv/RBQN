use std::sync::Arc;

use rbqn_core::{B, Val};
use rbqn_core::array::BqnArr;

use crate::block::{Block, Body, eval_fun_block, m_md1_block, m_md2_block};
use crate::bytecode::Op;
use crate::derive::{c1, c2, c1_slot, c2_slot, m_fork, m_atop, m1_d, m2_d, m_md2_partial_l, m_md2_partial_r};
use crate::namespace::{self, NS, get_ns, store_ns};
use crate::scope::{ScRef, Scope, TLS, Tls, v_get, v_set, v_seth, v_check_bad_read};

pub fn exec_block(bl: &Block, body: Arc<Body>, psc: std::rc::Rc<Scope>) -> B {
    let var_am = body.var_am;
    let sc = std::rc::Rc::new(Scope::new(body.clone(), Some(psc), var_am, &[]));
    eval_bc(&body, sc, bl)
}

#[inline(never)]
pub fn exec_block_with_args(bl: &Block, body: &Arc<Body>, psc: std::rc::Rc<Scope>, args: &[B]) -> B {
    let var_am = body.var_am.max(args.len() as u16);
    TLS.with(|t| {
        let sc = Scope::new_rc_in(t, body, psc, var_am, args);
        eval_bc_in(t, body, sc, bl)
    })
}

/// Fill `pscs` with the scope chain starting at `sc`, up to `max_psc` entries.
fn build_pscs(pscs: &mut Vec<ScRef>, sc: &std::rc::Rc<Scope>, max_psc: u16) {
    pscs.clear();
    if max_psc > 0 {
        let mut cur: &Scope = sc;
        pscs.push(ScRef::new(sc));
        for _ in 1..max_psc {
            match &cur.psc {
                Some(p) => {
                    pscs.push(ScRef::new(p));
                    cur = p;
                }
                None => break,
            }
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


/// A value-stack slot (#160). `Owned` holds the strong count of an array created
/// by a primitive call and referenced from nowhere else; `Borrowed` is any `B`
/// whose lifetime someone else decides (variables, constants, captures, fields,
/// elements: all still leaked). Dropping an `Owned` slot frees its array.
pub enum Slot {
    Owned(Val),
    Borrowed(B),
}

impl Slot {
    /// Copy view; must not outlive the slot.
    #[inline(always)]
    pub fn b(&self) -> B {
        match self {
            Slot::Owned(v) => v.b(),
            Slot::Borrowed(b) => *b,
        }
    }

    /// Give up ownership without freeing: the value goes to a long-lived location.
    #[inline(always)]
    pub fn leak(self) -> B {
        match self {
            Slot::Owned(v) => v.leak(),
            Slot::Borrowed(b) => b,
        }
    }
}

/// One block evaluation's pooled buffers plus the depth count. Nesting depth is
/// bounded only by the native stack: `stack::guard` in `enter` throws
/// "Stack overflow" before the interpreter thread's stack runs out. Entering takes
/// both vectors from the thread-local pool and bumps the depth in a single TLS
/// access; Drop (also on unwinding from a BQN error) returns them in one more.
struct Frame {
    stack: Vec<Slot>,
    pscs: Vec<ScRef>,
    /// The thread's TLS block, looked up once by the caller of `eval_bc_in`.
    t: *const Tls,
    /// Start pc of the instruction being run, for error-site capture on unwind.
    pc: usize,
    bl: *const Block,
}

impl Frame {
    #[inline]
    fn enter(t: &Tls, max_stack: usize, max_psc: usize) -> Frame {
        rbqn_core::stack::guard();
        t.depth.set(t.depth.get() + 1);
        // SAFETY: `t` is this thread's TLS block, and `Tls` is !Sync (UnsafeCell), so the
        // pool is only reachable from this thread. The exclusive borrow lasts for this
        // one `pop()`, which runs no user code, so no other reference to the pool is live.
        let (mut stack, mut pscs) = unsafe { (*t.frames.get()).pop() }.unwrap_or_default();
        stack.reserve(max_stack);
        pscs.reserve(max_psc);
        Frame { stack, pscs, t, pc: 0, bl: std::ptr::null() }
    }
}

impl Drop for Frame {
    #[inline]
    fn drop(&mut self) {
        let unwinding = std::thread::panicking();
        if !self.bl.is_null() && unwinding {
            // SAFETY: `bl` was set from the `&Block` parameter of the function that owns this
            // Frame as a local (`run_bc` callers), so the block outlives the Frame, whose
            // drop (including unwind) runs before that parameter goes away. It is only
            // non-null after `run_bc` stored it, and never reassigned to a shorter borrow.
            record_error_site(unsafe { &*self.bl }, self.pc);
        }
        let mut stack = std::mem::take(&mut self.stack);
        let mut pscs = std::mem::take(&mut self.pscs);
        if unwinding {
            // WARNING: unwinding from an error: leak what is left instead of freeing
            // (#160 point 5); nothing may read it, but nothing proves that here.
            for slot in stack.drain(..) {
                std::mem::forget(slot);
            }
        }
        // Dropping an `Owned` slot frees its array (a temporary nobody consumed).
        stack.clear();
        pscs.clear();
        // SAFETY: `self.t` came from the `&Tls` handed to `Frame::enter` by the owner of
        // this Frame, on this same thread (Frame holds a raw pointer, so it is !Send).
        // The TLS block is ManuallyDrop and never torn down, so it outlives the Frame.
        let t = unsafe { &*self.t };
        t.depth.set(t.depth.get() - 1);
        // SAFETY: as in Frame::enter: the pool is only reachable from this thread, and
        // `p` is used only for `len`/`push`, which run no user code and cannot re-enter
        // the pool, so it is the sole live reference to it.
        let p = unsafe { &mut *t.frames.get() };
        if p.len() < 1024 && stack.capacity() <= 4096 {
            p.push((stack, pscs));
        }
    }
}

/// Switch `current_sc` to `next_body` for a header/predicate retry, carrying the
/// first `arg_count` vars over. When the scope is not shared (no closure,
/// namespace or pscs entry holds it) it is reset in place; otherwise a fresh
/// Scope is built so captured scopes keep the failed body's variables.
fn retry_scope(
    current_sc: &mut std::rc::Rc<Scope>,
    pscs: &mut Vec<ScRef>,
    next_body: &Arc<Body>,
    arg_count: usize,
) {
    let mut args = [B::SENTINEL; 8];
    {
        let vars = current_sc.vars.borrow();
        for (i, a) in args.iter_mut().enumerate().take(arg_count) {
            *a = vars.get(i).copied().unwrap_or(B::SENTINEL);
        }
    }
    let args = &args[..arg_count.min(8)];
    let var_am = next_body.var_am.max(args.len() as u16);
    // pscs holds clones of current_sc; drop them so the uniqueness check is meaningful
    pscs.clear();
    if current_sc.psc.is_some()
        && let Some(s) = std::rc::Rc::get_mut(current_sc)
        && s.ext.is_none()
    {
        s.body = next_body.clone();
        s.var_am = var_am;
        *s.vars.get_mut() = crate::scope::Vars::new(var_am as usize, args);
        return;
    }
    let parent = current_sc.psc.clone().unwrap_or(current_sc.clone());
    *current_sc = std::rc::Rc::new(Scope::new(next_body.clone(), Some(parent), var_am, args));
}

#[cold]
#[inline(never)]
fn vm_trace_op(stack: &[Slot], pc: usize, op: Option<Op>, op_val: u32) {
    let op_name = op.map(|o| format!("{:?}", o)).unwrap_or_else(|| format!("0x{:02x}", op_val));
    let stack_info: String = stack.iter().rev().take(4).enumerate().map(|(i, sl)| {
    let b = &sl.b();
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
    vm_trace_push(format!("OP pc={} {} stk=[{}]", pc, op_name, stack_info));
}

// NOTE: Debug trace for FN2C with empty array arguments
#[cold]
#[inline(never)]
fn vm_trace_fn2c(w: B, f: B, x: B, pc: usize) {
    if x.is_arr()
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
}

#[cold]
#[inline(never)]
fn vm_trace_varo(val: B, d: u32, p: u32) {
    if val.is_arr()
        && let Some(a) = get_arr(val)
            && a.ia() == 0 {
                vm_trace_push(format!("  VARO d={} p={} → EMPTY_ARR shape={:?}", d, p, a.shape));
            }
}

#[cold]
#[inline(never)]
fn pop_underflow() -> ! {
    rbqn_core::error::throw("VM: stack underflow")
}

#[cold]
#[inline(never)]
fn bad_opcode(op_val: u32) -> ! {
    rbqn_core::error::throw(format!("VM: unhandled opcode 0x{:02x}", op_val))
}

// NOTE: BQN predicates require a boolean (0 or 1), not arbitrary numbers
#[cold]
#[inline(never)]
fn pred_not_bool() -> ! {
    rbqn_core::error::throw("Expected boolean")
}

/// Pop `sz` values into list order and build the array for LSTO/LSTM (mode 0),
/// ARMO (1: merge, like `>`) or ARMM (2: merge-destructuring target).
#[inline(never)]
fn build_list(stack: &mut Vec<Slot>, sz: usize, mode: u8) -> B {
    if mode == 0 && sz == 0 {
        return tag_arr(BqnArr::empty_harr());
    }
    let mut elems = vec![B::SENTINEL; sz];
    for i in 0..sz {
        elems[sz - i - 1] = stack.pop().unwrap_or_else(|| pop_underflow()).leak();
    }
    match mode {
        0 => b_vec_to_arr(elems),
        1 => bqn_merge(elems),
        _ => rbqn_core::tag_arr_merge(BqnArr::from_b_vec(elems)),
    }
}

#[inline(never)]
fn dfnd(kind: u8, bl: &Block, bl_idx: usize, pscs: &[ScRef], current_sc: &std::rc::Rc<Scope>) -> B {
    if bl_idx < bl.blocks.len() {
        let child_bl = bl.blocks[bl_idx].clone();
        let psc = if !pscs.is_empty() { pscs[0].to_rc() } else { current_sc.clone() };
        match kind {
            0 => eval_fun_block(child_bl, psc),
            1 => m_md1_block(child_bl, psc),
            _ => m_md2_block(child_bl, psc),
        }
    } else {
        rbqn_core::error::throw(format!("DFND{}: block index out of bounds", kind))
    }
}

/// EXTO (mode 0, checks for undefined) / EXTU (mode 1, reads then clears).
#[inline(never)]
fn ext_get(pscs: &[ScRef], d: u32, p: u32, mode: u8) -> B {
    if let Some(ref ext) = pscs[d as usize].ext {
        let val = ext.vars.borrow_mut()[p as usize];
        if mode == 0 {
            if v_check_bad_read(val) {
                rbqn_core::error::throw("Attempting to read ext variable which is not yet defined");
            }
        } else {
            ext.vars.borrow_mut()[p as usize] = B::OPT_OUT;
        }
        val
    } else if mode == 0 {
        rbqn_core::error::throw("EXTO: no scope extension")
    } else {
        rbqn_core::error::throw("EXTU: no scope extension")
    }
}

#[inline(never)]
fn pick_idx(current_sc: &std::rc::Rc<Scope>, mono_idx: usize, dy_idx: usize) -> usize {
    let vars = current_sc.vars.borrow_mut();
    let is_dyadic = vars.get(2).is_some_and(|b| !b.q_n());
    if is_dyadic { dy_idx } else { mono_idx }
}

/// Header/predicate retry: switch to body `next_idx`, return its bytecode offset.
#[inline(never)]
fn retry_to(
    bl: &Block,
    next_idx: usize,
    no_match: &str,
    current_sc: &mut std::rc::Rc<Scope>,
    pscs: &mut Vec<ScRef>,
    stack: &mut Vec<Slot>,
) -> usize {
    let next_body = bl.bodies[next_idx].clone();
    if !next_body.exists {
        rbqn_core::error::throw(no_match);
    }
    // NOTE: Preserve original args when retrying; needed for modifier blocks
    // where 𝕣 (var[0]) and 𝔽 (operand) must be available in the next body.
    let arg_count = crate::block::arg_count(bl.ty, bl.imm) as usize;
    retry_scope(current_sc, pscs, &next_body, arg_count);
    build_pscs(pscs, current_sc, next_body.max_psc);
    stack.clear();
    next_body.bc_offset
}

#[inline(never)]
fn fld_get(ns_val: B, gid: i32, mode: u8) -> B {
    if !ns_val.is_nsp() {
        rbqn_core::error::throw("Trying to read a field from non-namespace");
    }
    let ns = get_ns(ns_val);
    match ns.get_by_gid(gid) {
        Some(v) => v,
        None => match mode {
            0 => rbqn_core::error::throw(format!("Namespace does not have field '{}'", namespace::gid2str(gid))),
            1 => B::SENTINEL, // optional: Nothing if not found
            _ => rbqn_core::error::throw(format!(
                "Namespace does not have field '{}' for modification",
                namespace::gid2str(gid)
            )),
        },
    }
}

/// ALIAS_TAG encoding: bits 47:32 = GID, bits 31:16 = depth, bits 15:0 = pos
#[inline(never)]
fn alias_make(o: B, gid: i32) -> B {
    let depth16 = o.v_depth() as u16;
    let pos16 = (o.v_pos() & 0xFFFF) as u16;
    let alias_payload = ((gid as u64 & 0xFFFF) << 32) | ((depth16 as u64) << 16) | (pos16 as u64);
    rbqn_core::tagu64(alias_payload, rbqn_core::ALIAS_TAG)
}

#[inline(never)]
fn ret_d(body: &Body, stack: &mut Vec<Slot>, pscs: &[ScRef], current_sc: &std::rc::Rc<Scope>) -> B {
    if let Some(ref ns_desc) = body.ns_desc {
        if !stack.is_empty() {
            stack.pop();
        }
        return store_ns(NS {
            desc: ns_desc.clone(),
            sc: if !pscs.is_empty() { pscs[0].to_rc() } else { current_sc.clone() },
        });
    }
    stack.pop().map_or(B::SENTINEL, Slot::leak)
}

#[inline(never)]
pub fn eval_bc(body: &Body, sc: std::rc::Rc<Scope>, bl: &Block) -> B {
    TLS.with(|t| eval_bc_in(t, body, sc, bl))
}

#[inline]
fn eval_bc_in(t: &Tls, body: &Body, sc: std::rc::Rc<Scope>, bl: &Block) -> B {
    let mut frame = Frame::enter(t, body.max_stack as usize, body.max_psc as usize);
    run_bc(t, &mut frame, body, sc, bl)
}

/// Call block `bl` (body `body`, parent `psc`) once per element, as `F x` with
/// `𝕊 = f`, appending results to `out`. One TLS lookup, frame and depth bump
/// serve the whole loop instead of one per element.
#[inline(never)]
pub fn exec_block_each(
    bl: &Block,
    body: &Arc<Body>,
    psc: &std::rc::Rc<Scope>,
    f: B,
    elems: impl Iterator<Item = B>,
    out: &mut Vec<B>,
) {
    let var_am = body.var_am.max(3);
    TLS.with(|t| {
        let mut frame = Frame::enter(t, body.max_stack as usize, body.max_psc as usize);
        for x in elems {
            let sc = Scope::new_rc_in(t, body, psc.clone(), var_am, &[f, x, B::SENTINEL]);
            out.push(run_bc(t, &mut frame, body, sc, bl));
        }
    })
}

/// The interpreter loop proper, on a frame owned by the caller.
#[inline(always)]
fn run_bc(t: &Tls, frame: &mut Frame, body: &Body, sc: std::rc::Rc<Scope>, bl: &Block) -> B {
    frame.bl = bl;
    let stack = &mut frame.stack;
    let pscs = &mut frame.pscs;
    stack.clear();
    let bc: &[i32] = &bl.bc;
    let mut pc = body.bc_offset;
    let mut current_sc = sc;
    build_pscs(pscs, &current_sc, body.max_psc);

    // NOTE: `pop!` hands out the slot; `popb!` leaks it into a plain `B` (what every
    // instruction that stores or keeps the value does: #160 point 4). Only the call
    // instructions and POPS look at ownership.
    macro_rules! pop {
        () => {
            stack.pop().unwrap_or_else(|| pop_underflow())
        };
    }
    macro_rules! popb {
        () => {
            pop!().leak()
        };
    }
    macro_rules! push {
        ($v:expr) => {
            stack.push(Slot::Borrowed($v))
        };
    }
    macro_rules! peek {
        ($n:expr) => {
            stack[stack.len() - $n].b()
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
        let Some(&op_raw) = bc.get(pc) else {
            rbqn_core::error::throw("VM: bytecode overrun");
        };
        frame.pc = pc;
        pc += 1;
        let op_val = op_raw as u32;

        let op = Op::from_u32(op_val);

        if vm_debug {
            vm_trace_op(stack, pc - 1, op, op_val);
        }

        match op {
            Some(Op::POPS) => {
                // A discarded temporary: dropping an `Owned` slot frees its array.
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
                let f = popb!();
                let x = pop!();
                stack.push(c1_slot(f, x));
            }
            Some(Op::FN1O) => {
                let f = popb!();
                let x = pop!();
                if x.b().q_n() {
                    stack.push(x);
                } else {
                    stack.push(c1_slot(f, x));
                }
            }
            Some(Op::FN2C) => {
                let w = pop!();
                let f = popb!();
                let x = pop!();
                if vm_debug {
                    vm_trace_fn2c(w.b(), f, x.b(), pc);
                }
                stack.push(c2_slot(f, w, x));
            }
            Some(Op::FN2O) => {
                let w = pop!();
                let f = popb!();
                let x = pop!();
                if x.b().q_n() {
                    stack.push(x);
                } else if w.b().q_n() {
                    stack.push(c1_slot(f, x));
                } else {
                    stack.push(c2_slot(f, w, x));
                }
            }

            // --- Inline function call variants ---
            Some(Op::FN1Ci) => {
                let f = B::from_u64(read_u64!());
                let x = pop!();
                stack.push(c1_slot(f, x));
            }
            Some(Op::FN1Oi) => {
                let f = B::from_u64(read_u64!());
                let x = pop!();
                if x.b().q_n() {
                    stack.push(x);
                } else {
                    stack.push(c1_slot(f, x));
                }
            }
            Some(Op::FN2Ci) => {
                let f = B::from_u64(read_u64!());
                let w = pop!();
                let x = pop!();
                stack.push(c2_slot(f, w, x));
            }
            Some(Op::FN2Oi) => {
                // FIX: FN2Oi has TWO u64 immediates — monadic fn ptr and dyadic fn ptr.
                // CBQN: bL_m[FN2Oi]=5 (1 opcode + 2*u64 = 5 u32 words).
                let f_mono = B::from_u64(read_u64!());
                let f_dy = B::from_u64(read_u64!());
                let w = pop!();
                let x = pop!();
                if x.b().q_n() {
                    stack.push(x);
                } else if w.b().q_n() {
                    stack.push(c1_slot(f_mono, x));
                } else {
                    stack.push(c2_slot(f_dy, w, x));
                }
            }

            // --- List/array construction ---
            Some(Op::LSTO) | Some(Op::LSTM) => {
                let sz = read_u32!() as usize;
                let l = build_list(stack, sz, 0);
                push!(l);
            }
            Some(Op::ARMO) => {
                let sz = read_u32!() as usize;
                let l = build_list(stack, sz, 1);
                push!(l);
            }
            Some(Op::ARMM) => {
                let sz = read_u32!() as usize;
                let l = build_list(stack, sz, 2);
                push!(l);
            }

            // --- Block definitions ---
            Some(Op::DFND0) => {
                let bl_idx = read_u64!() as usize;
                push!(dfnd(0, bl, bl_idx, pscs, &current_sc));
            }
            Some(Op::DFND1) => {
                let bl_idx = read_u64!() as usize;
                push!(dfnd(1, bl, bl_idx, pscs, &current_sc));
            }
            Some(Op::DFND2) => {
                let bl_idx = read_u64!() as usize;
                push!(dfnd(2, bl, bl_idx, pscs, &current_sc));
            }

            // --- Modifier application ---
            Some(Op::MD1C) => {
                let f = popb!();
                let m = popb!();
                push!(m1_d(m, f));
            }
            Some(Op::MD2C) => {
                let f = popb!();
                let m = popb!();
                let g = popb!();
                push!(m2_d(m, f, g));
            }
            Some(Op::MD2L) => {
                // Partial 2-modifier: has left operand, needs right
                let f = popb!();
                let m2 = popb!();
                push!(m_md2_partial_l(m2, f));
            }
            Some(Op::MD2R) => {
                // Partial 2-modifier: has right operand, needs left
                let g = popb!();
                let m2 = popb!();
                push!(m_md2_partial_r(m2, g));
            }

            // --- Trains ---
            Some(Op::TR2D) => {
                let g = popb!();
                let h = popb!();
                push!(m_atop(g, h));
            }
            Some(Op::TR3D) => {
                let f = popb!();
                let g = popb!();
                let h = popb!();
                push!(m_fork(f, g, h));
            }
            Some(Op::TR3O) => {
                let f = popb!();
                let g = popb!();
                let h = popb!();
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
                if vm_debug {
                    vm_trace_varo(val, d, p);
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
                push!(ext_get(pscs, d, p, 0));
            }
            Some(Op::EXTM) => {
                let d = read_u32!();
                let p = read_u32!();
                push!(rbqn_core::tagu64((d as u64) << 32 | p as u64, rbqn_core::EXT_TAG));
            }
            Some(Op::EXTU) => {
                let d = read_u32!();
                let p = read_u32!();
                push!(ext_get(pscs, d, p, 1));
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
                let s = popb!();
                let x = popb!();
                v_set(pscs, s, x, false, true);
                push!(x);
            }
            Some(Op::SETU) => {
                let s = popb!();
                let x = popb!();
                v_set(pscs, s, x, true, true);
                push!(x);
            }
            Some(Op::SETM) => {
                let s = popb!();
                let f = popb!();
                let x = popb!();
                let w = v_get(pscs, s, true);
                let r = c2(f, w, x);
                v_set(pscs, s, r, true, false);
                push!(r);
            }
            Some(Op::SETC) => {
                let s = popb!();
                let f = popb!();
                let x = v_get(pscs, s, true);
                let r = c1(f, x);
                v_set(pscs, s, r, true, false);
                push!(r);
            }

            // --- Immediate set variants (variable ref encoded as immediate u64) ---
            Some(Op::SETNi) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let x = popb!();
                pscs[d].var_set(p, x);
                push!(x);
            }
            Some(Op::SETUi) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let x = popb!();
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
                let f = popb!();
                let x = popb!();
                let w = pscs[d].var_get(p);
                let r = c2(f, w, x);
                pscs[d].var_set(p, r);
                push!(r);
            }
            Some(Op::SETCi) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let f = popb!();
                let x = pscs[d].var_get(p);
                let r = c1(f, x);
                pscs[d].var_set(p, r);
                push!(r);
            }

            // --- Void set variants (like SETNi but don't push result) ---
            Some(Op::SETNv) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let x = popb!();
                pscs[d].var_set(p, x);
            }
            Some(Op::SETUv) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let x = popb!();
                let prev = pscs[d].var_get(p);
                if crate::scope::v_check_bad_write(prev) {
                    crate::scope::v_tag_error_pub(prev, true);
                }
                pscs[d].var_set(p, x);
            }
            Some(Op::SETMv) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let f = popb!();
                let x = popb!();
                let w = pscs[d].var_get(p);
                let r = c2(f, w, x);
                pscs[d].var_set(p, r);
            }
            Some(Op::SETCv) => {
                let packed = read_u64!();
                let (d, p) = unpack_var_ref(packed);
                let f = popb!();
                let x = pscs[d].var_get(p);
                let r = c1(f, x);
                pscs[d].var_set(p, r);
            }

            // --- Header match (iterative retry instead of recursive exec_block) ---
            Some(Op::SETH1) => {
                let s = popb!();
                let x = popb!();
                let next_body_idx = read_u64!() as usize;
                if !v_seth(pscs, s, x) {
                    pc = retry_to(bl, next_body_idx, "No matching header", &mut current_sc, pscs, stack);
                    continue;
                }
            }
            Some(Op::SETH2) => {
                let s = popb!();
                let x = popb!();
                let mono_idx = read_u64!() as usize;
                let dy_idx = read_u64!() as usize;
                if !v_seth(pscs, s, x) {
                    let next_idx = pick_idx(&current_sc, mono_idx, dy_idx);
                    pc = retry_to(bl, next_idx, "No matching header", &mut current_sc, pscs, stack);
                    continue;
                }
            }

            // --- Predicates (iterative retry instead of recursive exec_block) ---
            Some(Op::PRED1) => {
                let x = popb!();
                let next_body_idx = read_u64!() as usize;
                if x.is_f64() {
                    let v = x.o2f();
                    if v != 0.0 && v != 1.0 {
                        pred_not_bool();
                    }
                }
                if !x.o2b() {
                    pc = retry_to(bl, next_body_idx, "No matching predicate", &mut current_sc, pscs, stack);
                    continue;
                }
            }
            Some(Op::PRED2) => {
                let x = popb!();
                let mono_idx = read_u64!() as usize;
                let dy_idx = read_u64!() as usize;
                if x.is_f64() {
                    let v = x.o2f();
                    if v != 0.0 && v != 1.0 {
                        pred_not_bool();
                    }
                }
                if !x.o2b() {
                    let next_idx = pick_idx(&current_sc, mono_idx, dy_idx);
                    pc = retry_to(bl, next_idx, "No matching predicate", &mut current_sc, pscs, stack);
                    continue;
                }
            }

            // --- Namespace field access ---
            Some(Op::FLDG) => {
                let ns_val = popb!();
                let gid = read_u32!() as i32;
                push!(fld_get(ns_val, gid, 0));
            }
            Some(Op::FLDO) => {
                let ns_val = popb!();
                let gid = read_u32!() as i32;
                push!(fld_get(ns_val, gid, 1));
            }
            Some(Op::FLDM) => {
                let ns_val = popb!();
                let gid = read_u32!() as i32;
                push!(fld_get(ns_val, gid, 2));
            }

            Some(Op::ALIM) => {
                let o = popb!();
                let gid = read_u32!() as i32;
                push!(alias_make(o, gid));
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
                let v = popb!();
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
                return ret_d(body, stack, pscs, &current_sc);
            }
            Some(Op::RETN) => {
                let r = popb!();
                Scope::recycle_in(t, current_sc);
                return r;
            }

            _ => {
                bad_opcode(op_val);
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
    // Mixed or non-conforming: defer to `>` so shape mismatches raise its error
    let list = tag_arr(BqnArr::from_b_vec(elems));
    match rbqn_prim::structural::merge_c1(list, get_arr(list).as_deref()) {
        Ok(r) => r.leak(),
        Err(e) => rbqn_core::error::throw_bqn(e),
    }
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


/// Source position of the innermost user-code instruction that raised an error.
pub struct ErrorSite {
    comp: Arc<crate::block::Comp>,
    pc: usize,
    throw_id: u64,
}

thread_local! {
    static ERROR_SITE: std::cell::RefCell<Option<ErrorSite>> = const { std::cell::RefCell::new(None) };
}

/// Called while unwinding: keep the first (innermost) frame whose block has source.
fn record_error_site(bl: &Block, pc: usize) {
    let id = rbqn_core::error::throw_count();
    if !get_arr(bl.comp.indices).is_some_and(|a| a.ia() >= 2) {
        return;
    }
    ERROR_SITE.with(|s| {
        let mut s = s.borrow_mut();
        if s.as_ref().is_none_or(|e| e.throw_id != id) {
            let src_pc = bl.map.get(pc).map_or(pc, |&p| p as usize);
            *s = Some(ErrorSite { comp: bl.comp.clone(), pc: src_pc, throw_id: id });
        }
    });
}

/// Forget any recorded site (call before running a new top-level action).
pub fn clear_error_site() {
    ERROR_SITE.with(|s| *s.borrow_mut() = None);
    COMPILE_SITE.with(|s| *s.borrow_mut() = None);
}

/// Mark the recorded site as belonging to the next error thrown, so a
/// catch-and-rethrow (•Import) keeps the innermost position.
pub fn keep_error_site_for_next_throw() {
    let next = rbqn_core::error::throw_count() + 1;
    ERROR_SITE.with(|s| {
        if let Some(e) = s.borrow_mut().as_mut() {
            e.throw_id = next;
        }
    });
    COMPILE_SITE.with(|s| {
        if let Some(e) = s.borrow_mut().as_mut() {
            e.throw_id = next;
        }
    });
}

/// Source and span of a compiler error, kept until the error is reported.
struct CompileSite {
    name: Option<String>,
    src: Vec<char>,
    start: usize,
    end: usize,
    throw_id: u64,
}

thread_local! {
    static COMPILE_SITE: std::cell::RefCell<Option<CompileSite>> = const { std::cell::RefCell::new(None) };
}

/// Call the BQN compiler on `src`. A compiler error (`!⟨positions, message⟩`)
/// keeps its positions so the report can show the offending line and carets.
pub fn compile_call(compiler: B, args: B, src: &str, name: Option<&str>) -> B {
    let chars: Vec<char> = src.chars().collect();
    let src_b = tag_arr(BqnArr::new_vec_c32(chars.iter().map(|&c| c as u32).collect()));
    rbqn_core::error::take_compile_error_pos();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| crate::derive::c2(compiler, args, src_b))) {
        Ok(r) => r,
        Err(payload) => {
            if let Some(pos) = rbqn_core::error::take_compile_error_pos()
                && let (Some(&start), Some(&last)) = (pos.iter().min(), pos.iter().max())
                && start < chars.len()
            {
                // The compiler points at token starts; extend the last one over its word.
                let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '•';
                let mut end = last.min(chars.len() - 1);
                if is_word(chars[end]) {
                    while end + 1 < chars.len() && is_word(chars[end + 1]) {
                        end += 1;
                    }
                }
                COMPILE_SITE.with(|s| {
                    *s.borrow_mut() = Some(CompileSite {
                        name: name.map(str::to_string),
                        src: chars,
                        start,
                        end,
                        throw_id: rbqn_core::error::throw_count(),
                    })
                });
            }
            std::panic::resume_unwind(payload)
        }
    }
}

/// Take the site of the most recent error as text lines:
/// `<name>:<line>:`, the source line, and a caret line.
pub fn take_error_site_lines(default_name: &str) -> Option<String> {
    let current = rbqn_core::error::throw_count();
    if let Some(cs) = COMPILE_SITE.with(|s| s.borrow_mut().take())
        && cs.throw_id == current
    {
        return render_site(&cs.src, cs.start, cs.end, cs.name.as_deref().unwrap_or(default_name));
    }
    let site = ERROR_SITE.with(|s| s.borrow_mut().take())?;
    if site.throw_id != current {
        return None;
    }
    let comp = &site.comp;
    let ind = get_arr(comp.indices)?;
    let at = |k: usize| -> Option<usize> {
        let a = get_arr(ind.get(k).ok()?)?;
        let v = a.get(site.pc).ok()?.o2f();
        if v >= 0.0 { Some(v as usize) } else { None }
    };
    let (start, end) = (at(0)?, at(1)?);
    let src: Vec<char> = {
        let a = get_arr(comp.src)?;
        (0..a.ia())
            .map(|i| a.get(i).ok().and_then(|b| b.o2c().ok()).and_then(char::from_u32).unwrap_or('?'))
            .collect()
    };
    let name = match get_arr(comp.fullpath) {
        Some(a) if a.ia() > 0 => crate::derive::b_to_string_pub(comp.fullpath),
        _ => default_name.to_string(),
    };
    render_site(&src, start, end, &name)
}

fn render_site(src: &[char], start: usize, end: usize, name: &str) -> Option<String> {
    if start >= src.len() {
        return None;
    }
    let end = end.clamp(start, src.len() - 1);
    let line_start = src[..start].iter().rposition(|&c| c == '\n').map_or(0, |i| i + 1);
    let line_end = src[start..].iter().position(|&c| c == '\n').map_or(src.len(), |i| start + i);
    let line_no = src[..start].iter().filter(|&&c| c == '\n').count() + 1;
    let caret_end = end.min(line_end.saturating_sub(1)).max(start);
    let text: String = src[line_start..line_end].iter().collect();
    let pad: String = src[line_start..start].iter().map(|&c| if c == '\t' { '\t' } else { ' ' }).collect();
    let carets = "^".repeat(caret_end - start + 1);
    Some(format!("{name}:{line_no}:\n  {text}\n  {pad}{carets}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_pool_reuses_buffers_and_tracks_depth() {
        TLS.with(|t| {
            let d0 = t.depth.get();
            {
                let mut f = Frame::enter(t, 8, 2);
                assert_eq!(t.depth.get(), d0 + 1);
                f.stack.push(Slot::Borrowed(B::SENTINEL));
            }
            assert_eq!(t.depth.get(), d0);
            // SAFETY: test thread owns its TLS pool; no other reference is live.
            assert!(!unsafe { (*t.frames.get()).is_empty() });
            let f = Frame::enter(t, 8, 2);
            assert!(f.stack.is_empty(), "recycled stack is cleared");
            drop(f);
            assert_eq!(t.depth.get(), d0);
            // NOTE: the pool is ManuallyDrop TLS by design (never torn down); drain it so
            // Miri's leak check sees nothing left behind by this test thread.
            // SAFETY: test thread owns its TLS pool; no other reference is live.
            drop(unsafe { std::mem::take(&mut *t.frames.get()) });
        });
    }
}
