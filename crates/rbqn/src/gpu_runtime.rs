use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};

use rbqn_core::array::{ArrData, BqnArr, squeeze_num, squeeze_i32};
use rbqn_core::{B, DeviceValue, peek_device, tag_device};
use rbqn_gpu::buffer::{ElementKind, GpuBuffer, upload_i32_with, upload_f32, download_i32, download_i64, download_f32};
use rbqn_gpu::context::GpuContext;
use rbqn_gpu::expr::Expr;
use rbqn_gpu::fusion::{FusedOp, FusionBuilder};
use rbqn_gpu::pipeline::PipelineCache;
use rbqn_gpu::dispatch::{Kind, Work};

use crate::gpu_host::{self, HostLeaf, Source};

// NOTE: Singleton GPU runtime — initialized lazily on first qualifying dispatch.
pub struct GpuRuntime {
    pub ctx: GpuContext,
    pub cache: Mutex<PipelineCache>,
    /// Output buffers of pending values, reused once nothing else holds them.
    pub pool: Mutex<Vec<Arc<GpuBuffer>>>,
}

/// Bytes of free pooled buffers kept around; free ones beyond this are dropped.
const POOL_FREE_BYTES: u64 = 512 << 20;

/// An n-element output buffer for a kernel whose result becomes a pending
/// value. Reuses a pooled buffer of the same kind and length whose only
/// reference is the pool's own (its GpuArr was materialized or dropped, and
/// no kernel input or upload-cache entry holds it). Queue order makes reuse
/// safe against GPU work already submitted on the old contents.
fn pooled_out(gpu: &GpuRuntime, kind: ElementKind, n: usize) -> Arc<GpuBuffer> {
    let mut pool = gpu.pool.lock().unwrap_or_else(|e| e.into_inner());
    let free = |b: &Arc<GpuBuffer>| Arc::strong_count(b) == 1;
    if let Some(b) = pool.iter().find(|b| free(b) && b.element_type() == kind && b.len() == n) {
        return b.clone();
    }
    // Trim free buffers (oldest first) so retained-but-unused memory stays bounded.
    let mut free_bytes: u64 = pool.iter().filter(|b| free(b)).map(|b| b.size()).sum();
    pool.retain(|b| {
        if free_bytes > POOL_FREE_BYTES && free(b) {
            free_bytes -= b.size();
            false
        } else {
            true
        }
    });
    let b = Arc::new(GpuBuffer::storage(&gpu.ctx.device, kind, n));
    pool.push(b.clone());
    b
}

static GPU_RUNTIME: OnceLock<Option<GpuRuntime>> = OnceLock::new();
static GPU_DISABLED: AtomicBool = AtomicBool::new(false);
static GPU_DEBUG: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq, Eq)]
enum GpuMode {
    Default,
    Off,
    Force,
}

/// RBQN_GPU=off|force, read once. Anything else (or unset) keeps the thresholds.
fn gpu_mode() -> GpuMode {
    static MODE: OnceLock<GpuMode> = OnceLock::new();
    *MODE.get_or_init(|| match std::env::var("RBQN_GPU").as_deref() {
        Ok("off") => GpuMode::Off,
        Ok("force") => GpuMode::Force,
        _ => GpuMode::Default,
    })
}

/// One-line GPU counter summary on stderr; no-op unless RBQN_GPU_DEBUG=1.
pub fn print_summary() {
    if !debug_enabled() {
        return;
    }
    let s = rbqn_gpu::stats::snapshot();
    eprintln!(
        "gpu: dispatches={} submits={} readbacks={} up={}B down={}B init={:.1}ms compiles={}",
        s.dispatches, s.submits, s.readbacks, s.bytes_up, s.bytes_down,
        s.device_init_us as f64 / 1000.0, s.pipeline_compiles
    );
}

/// Prints the summary when dropped (end of the interpreter thread, also on unwind).
pub struct SummaryGuard;
impl Drop for SummaryGuard {
    fn drop(&mut self) {
        print_summary();
    }
}

/// Create the context and cache. Pure device work: it never touches the
/// interpreter's thread-local registries.
fn build_runtime() -> Option<GpuRuntime> {
    let t_init = std::time::Instant::now();
    let ctx = pollster::block_on(GpuContext::new())?;
    rbqn_gpu::stats::DEVICE_INIT_US.store(t_init.elapsed().as_micros() as u64, Ordering::Relaxed);
    let cache = Mutex::new(PipelineCache::new(ctx.device.clone()));
    Some(GpuRuntime { ctx, cache, pool: Mutex::new(Vec::new()) })
}

fn build_runtime_logged() -> Option<GpuRuntime> {
    let rt = std::panic::catch_unwind(build_runtime).ok().flatten();
    if rt.is_none() && debug_enabled() {
        eprintln!("[gpu] device init failed; using CPU path");
    }
    rt
}

/// Compile the pipelines in `kernels::precompile_set` with
/// `create_compute_pipeline` only (no warmup dispatches). Each pipeline is
/// compiled with no lock held and inserted under a short lock, so the main
/// thread is never blocked behind the whole set; a pipeline it needs before
/// this gets there is compiled by the main thread itself (and counted).
fn precompile(rt: &GpuRuntime) {
    let d = &rt.ctx.device;
    let mut modules = std::collections::HashMap::new();
    for (key, src) in rbqn_gpu::kernels::precompile_set(rt.ctx.shader_int64) {
        if rt.cache.lock().unwrap_or_else(|e| e.into_inner()).contains(&key) {
            continue;
        }
        let p = rbqn_gpu::pipeline::compile_detached(d, &mut modules, &key, &src);
        rt.cache.lock().unwrap_or_else(|e| e.into_inner()).insert(key, p);
    }
}

/// Record CLI GPU settings. Disabled (`--no-gpu`, RBQN_GPU=off): nothing else.
/// Force mode starts device creation now. Default mode starts it from the
/// first array of `LARGE_ARR_HINT` elements the program creates: creating a
/// Metal device costs ~1 ms of the main thread's time even when nothing is
/// dispatched (driver load contention), which a program like `1` should not pay.
pub fn init(no_gpu: bool) {
    if std::env::var("RBQN_GPU_DEBUG").is_ok() {
        GPU_DEBUG.store(true, Ordering::Relaxed);
    }
    if no_gpu || gpu_mode() == GpuMode::Off {
        GPU_DISABLED.store(true, Ordering::Relaxed);
        return;
    }
    if gpu_mode() == GpuMode::Force {
        start_init_thread();
    } else {
        rbqn_core::arrstore::register_large_arr_hook(start_init_thread);
    }
}

/// Start device creation on a background thread so it overlaps the program.
/// The thread initializes GPU_RUNTIME and then keeps precompiling; `get()`
/// waits only for the device (OnceLock init), never for the precompile.
fn start_init_thread() {
    if INIT_START.set(std::time::Instant::now()).is_err() {
        return;
    }
    let _ = std::thread::Builder::new().name("rbqn-gpu-init".into()).spawn(|| {
        if let Some(rt) = GPU_RUNTIME.get_or_init(build_runtime_logged) {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| precompile(rt)));
        }
    });
}

static INIT_START: OnceLock<std::time::Instant> = OnceLock::new();

/// False when the GPU is disabled or device creation already failed. Never blocks.
fn gpu_possible() -> bool {
    !GPU_DISABLED.load(Ordering::Relaxed) && !matches!(GPU_RUNTIME.get(), Some(None))
}

/// Part of device creation a dispatch issued now would still wait for (ms).
fn exposed_init_ms() -> f64 {
    if GPU_RUNTIME.get().is_some() {
        return 0.0;
    }
    let el = INIT_START.get().map_or(0.0, |t| t.elapsed().as_secs_f64() * 1e3);
    (rbqn_gpu::dispatch::GPU_INIT_MS - el).max(0.0)
}

/// The Step 6 decision: run `w` on the GPU when its modelled cost beats the
/// host's. Force mode always says GPU; `wide` trees need SHADER_INT64.
fn decide(kind: Kind, w: &Work, wide: bool) -> bool {
    if !gpu_possible() {
        return false;
    }
    if wide && let Some(Some(rt)) = GPU_RUNTIME.get() && !rt.ctx.shader_int64 {
        return false;
    }
    let init = exposed_init_ms();
    let g = rbqn_gpu::dispatch::gpu_ms(kind, w, init);
    let c = rbqn_gpu::dispatch::cpu_ms(kind, w);
    let gpu = gpu_mode() == GpuMode::Force || g < c;
    if debug_enabled() {
        eprintln!(
            "[gpu] decide {} n={:e} ops={} gpu={g:.2}ms cpu={c:.2}ms (init {init:.1}ms) → {}",
            kind.name(), w.n as f64, w.ops, if gpu { "gpu" } else { "cpu" }
        );
    }
    gpu
}

/// `get()` after a GPU decision, logging how long the init thread was waited for.
fn get_decided() -> Option<&'static GpuRuntime> {
    if !debug_enabled() || GPU_RUNTIME.get().is_some() {
        return get();
    }
    let t = std::time::Instant::now();
    let r = get();
    let since = INIT_START.get().map_or(0.0, |s| s.elapsed().as_secs_f64() * 1e3);
    eprintln!("[gpu] init wait {:.2}ms ({since:.1}ms after init start)", t.elapsed().as_secs_f64() * 1e3);
    r
}

/// Get the GPU runtime, waiting for device creation on first call. None when
/// disabled (`--no-gpu`) or when no adapter is available.
/// NOTE: Callers must apply their size threshold before calling this.
pub fn get() -> Option<&'static GpuRuntime> {
    if GPU_DISABLED.load(Ordering::Relaxed) {
        return None;
    }
    // If the init thread is mid-build, OnceLock blocks here until it is done;
    // if it has not started yet, this thread builds the device and the init
    // thread finds it set and only precompiles.
    GPU_RUNTIME.get_or_init(build_runtime_logged).as_ref()
}

// Set while a GPU dispatch runs so the interpreter's panic hook (which is
// silent for BQN's panic-based errors) can report GPU panics under
// RBQN_GPU_DEBUG=1. The VM is single-threaded, so a plain flag suffices.
static GPU_IN_DISPATCH: AtomicBool = AtomicBool::new(false);

struct DispatchGuard;
impl DispatchGuard {
    fn enter() -> Self {
        GPU_IN_DISPATCH.store(true, Ordering::Relaxed);
        Self
    }
}
impl Drop for DispatchGuard {
    fn drop(&mut self) {
        GPU_IN_DISPATCH.store(false, Ordering::Relaxed);
    }
}

/// Run a GPU dispatch, turning any panic into a CPU fallback (`None`).
/// The panic itself is reported by `report_panic` from the panic hook.
fn guarded<T>(what: &str, f: impl FnOnce() -> Option<T>) -> Option<T> {
    let _guard = DispatchGuard::enter();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| {
        if debug_enabled() {
            eprintln!("[gpu] {what} dispatch panicked, CPU fallback");
        }
        None
    })
}

/// Called from the interpreter's panic hook. Prints the payload and location
/// of a panic raised inside a GPU dispatch when RBQN_GPU_DEBUG=1, and a
/// backtrace when RUST_BACKTRACE is also set. Silent otherwise, since BQN
/// errors are panic-based and must not print. (Issue #11.)
pub fn report_panic(info: &std::panic::PanicHookInfo<'_>) {
    if !GPU_IN_DISPATCH.load(Ordering::Relaxed) || !debug_enabled() {
        return;
    }
    let payload = info.payload();
    let msg = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("<non-string panic payload>");
    match info.location() {
        Some(l) => eprintln!("[gpu] panic at {}:{}:{}: {msg}", l.file(), l.line(), l.column()),
        None => eprintln!("[gpu] panic: {msg}"),
    }
    if std::env::var_os("RUST_BACKTRACE").is_some_and(|v| v != "0") {
        eprintln!("{}", std::backtrace::Backtrace::force_capture());
    }
}

pub fn debug_enabled() -> bool {
    GPU_DEBUG.load(Ordering::Relaxed)
}

/// Log a GPU dispatch operation to stderr (only when RBQN_GPU_DEBUG=1).
pub fn log_dispatch(op: &str, len: usize, kind: &str) {
    if debug_enabled() {
        eprintln!("[gpu] {op} {len} elements ({kind})");
    }
}

/// Returns true if all values in the slice are safe for i32 GPU compute:
/// finite, integer-valued, and within [-2^24, 2^24].
pub fn gpu_safe_integer(data: &[f64]) -> bool {
    data.iter().all(|&v| {
        v.is_finite() && v.fract() == 0.0 && v.abs() < 16_777_216.0
    })
}

/// Returns true if the array can be safely dispatched to GPU as i32 data.
/// Integer-typed arrays (I8/I16/I32/Bit) are always safe.
/// F64 arrays are safe only when all values pass `gpu_safe_integer`.
/// C8/C16/C32/Boxed are never safe.
pub fn gpu_safe_arr(arr: &BqnArr) -> bool {
    match &arr.data {
        ArrData::Bit(_) | ArrData::I8(_) | ArrData::I16(_) | ArrData::I32(_) => true,
        ArrData::F64(v) => gpu_safe_integer(v),
        ArrData::C8(_) | ArrData::C16(_) | ArrData::C32(_) | ArrData::Boxed(_) => false,
    }
}

/// Transfer BqnArr to a GPU i32 buffer.
/// Caller must have verified `gpu_safe_arr` for F64 arrays.
/// Returns None for unsupported types (chars, boxed).
pub fn arr_to_gpu_i32(gpu: &GpuRuntime, arr: &BqnArr) -> Option<GpuBuffer> {
    arr_to_gpu_i32_bound(gpu, arr).map(|(b, _)| b)
}

/// Max |x| over a slice, folded into the copy loop below.
#[inline]
fn copy_max<T: Copy>(src: &[T], dst: &mut [i32], f: impl Fn(T) -> i32) -> f64 {
    let mut m = 0u32;
    for (d, &x) in dst.iter_mut().zip(src) {
        let v = f(x);
        *d = v;
        m = m.max(v.unsigned_abs());
    }
    m as f64
}

/// `arr_to_gpu_i32` plus the exact max |element| (Bit: 1), computed in the same
/// pass that writes the source into the mapped upload buffer.
pub fn arr_to_gpu_i32_bound(gpu: &GpuRuntime, arr: &BqnArr) -> Option<(GpuBuffer, f64)> {
    let (device, queue) = (&gpu.ctx.device, &gpu.ctx.queue);
    let n = arr.ia();
    Some(match &arr.data {
        ArrData::I32(v) => upload_i32_with(device, queue, n, |d| copy_max(v, d, |x| x)),
        ArrData::I8(v) => upload_i32_with(device, queue, n, |d| copy_max(v, d, |x| x as i32)),
        ArrData::I16(v) => upload_i32_with(device, queue, n, |d| copy_max(v, d, |x| x as i32)),
        // F64 already passed gpu_safe_integer, so the cast is exact.
        ArrData::F64(v) => upload_i32_with(device, queue, n, |d| copy_max(v, d, |x| x as i32)),
        ArrData::Bit(v) => upload_i32_with(device, queue, n, |d| {
            for (chunk, &w) in d.chunks_mut(64).zip(v.iter()) {
                for (j, o) in chunk.iter_mut().enumerate() {
                    *o = ((w >> j) & 1) as i32;
                }
            }
            1.0
        }),
        ArrData::C8(_) | ArrData::C16(_) | ArrData::C32(_) | ArrData::Boxed(_) => return None,
    })
}

/// Download a GPU i32 buffer back to a BqnArr.
/// Uses `squeeze_num` to compact to the smallest integer type.
pub fn gpu_i32_to_arr(gpu: &GpuRuntime, buf: &GpuBuffer, shape: Vec<usize>, fill: Option<B>) -> BqnArr {
    let data = pollster::block_on(download_i32(&gpu.ctx.device, &gpu.ctx.queue, buf));
    let arr = BqnArr {
        shape,
        data: ArrData::I32(data),
        fill,
    };
    // NOTE: Compact to smallest integer type that fits (I8/I16/I32).
    // Converts to F64 first for squeeze_num to work, since squeeze_num
    // only squeezes F64 arrays.
    let f64_data: Vec<f64> = if let ArrData::I32(ref v) = arr.data {
        v.iter().map(|&x| x as f64).collect()
    } else {
        unreachable!()
    };
    let f64_arr = BqnArr {
        shape: arr.shape.clone(),
        data: ArrData::F64(f64_data),
        fill: arr.fill,
    };
    squeeze_num(f64_arr)
}

/// Check whether an operation on `len` elements should use the GPU.
pub fn should_dispatch(op: &str, len: usize) -> bool {
    match gpu_mode() {
        GpuMode::Off => false,
        // HACK: GPU sort/grade return wrong results on small arrays (e.g. `⍋3‿1‿1`
        // gives 0 1 2), and the compiler sorts, so forcing sort to 0 breaks every
        // program. Force lowers its threshold to 1M instead.
        GpuMode::Force => {
            if !rbqn_gpu::stats::program_started() {
                return rbqn_gpu::dispatch::should_use_gpu(op, len);
            }
            len >= if op == "sort" { 1_000_000 } else { 1 }
        }
        GpuMode::Default => rbqn_gpu::dispatch::should_use_gpu(op, len),
    }
}

/// Results whose bound is below this run in i32.
const I32_LIM: f64 = 2_147_483_648.0;
/// Results whose bound is below this run in i64 and convert to f64 exactly;
/// anything larger goes to the CPU (f64 semantics, readback counted).
const EXACT_LIM: f64 = 9_007_199_254_740_992.0;
/// 2^63: an i64 reduce stays exact below this (fold only).
const I64_LIM: f64 = 9_223_372_036_854_775_808.0;

/// A pending integer result living on the GPU (Steps 2/3/5, lazy device values).
/// `bound` is a conservative bound on |element|, so arith/fold/scan pick i32,
/// i64 or CPU without reading the data back; it can be tightened in place by a
/// device min/max pass (`refine_bound`). The value is either a buffer or an
/// unevaluated elementwise tree (`Lazy`), evaluated by one fused kernel when a
/// non-elementwise consumer needs it; the buffer then replaces the tree.
pub struct GpuArr {
    state: RefCell<State>,
    shape: Vec<usize>,
    fill: Option<B>,
    bound: Cell<f64>,
    /// Max bound over every node of the tree (i32 evaluation needs all < 2^31).
    maxb: f64,
    /// Evaluates (or is stored) as i64.
    wide: bool,
    /// A fold/scan already decided this tree runs on the host.
    host_pref: Cell<bool>,
}

enum State {
    Buf(Arc<GpuBuffer>),
    Lazy(Rc<Expr>),
}

impl GpuArr {
    fn new(buf: Arc<GpuBuffer>, shape: Vec<usize>, fill: Option<B>, bound: f64) -> Self {
        let wide = buf.element_type() == ElementKind::I64;
        GpuArr { state: RefCell::new(State::Buf(buf)), shape, fill, bound: Cell::new(bound), maxb: bound, wide, host_pref: Cell::new(false) }
    }
    fn lazy(e: Rc<Expr>, shape: Vec<usize>, fill: Option<B>, bound: f64, maxb: f64, wide: bool) -> Self {
        GpuArr { state: RefCell::new(State::Lazy(e)), shape, fill, bound: Cell::new(bound), maxb, wide, host_pref: Cell::new(false) }
    }
    fn lazy_expr(&self) -> Option<Rc<Expr>> {
        match &*self.state.borrow() {
            State::Lazy(e) => Some(e.clone()),
            State::Buf(_) => None,
        }
    }
    /// Evaluate a lazy tree for a consumer that needs a buffer-or-leaf (tree
    /// over the node limit): GPU into a resident buffer, or the host into a
    /// host leaf, by the cost model.
    fn evaluate(&self) {
        let Some(e) = self.lazy_expr() else { return };
        if matches!(*e, Expr::Host(_)) {
            return;
        }
        let w = tree_work(&e, self.n(), self.out_kind());
        if !self.host_pref.get() && decide(Kind::Map, &w, self.wide)
            && let Some(gpu) = get_decided()
            && guarded("map", || Some(self.force(gpu))).is_some()
        {
            return;
        }
        let arr = Arc::new(BqnArr { shape: self.shape.clone(), data: self.host_eval(&e), fill: self.fill });
        let leaf = HostLeaf { arr, key: 0, bound: self.bound.get() };
        *self.state.borrow_mut() = State::Lazy(Rc::new(Expr::Host(Rc::new(leaf))));
    }
    fn n(&self) -> usize {
        self.shape.iter().product()
    }
    fn buf(&self) -> Option<Arc<GpuBuffer>> {
        match &*self.state.borrow() {
            State::Buf(b) => Some(b.clone()),
            State::Lazy(_) => None,
        }
    }
    fn expr(&self) -> Rc<Expr> {
        match &*self.state.borrow() {
            State::Buf(b) => Rc::new(Expr::Leaf(b.clone())),
            State::Lazy(e) => e.clone(),
        }
    }
    fn out_kind(&self) -> ElementKind {
        if self.wide { ElementKind::I64 } else { ElementKind::I32 }
    }
    /// The device buffer, evaluating a lazy tree with one fused kernel first.
    fn force(&self, gpu: &GpuRuntime) -> Arc<GpuBuffer> {
        let e = match &*self.state.borrow() {
            State::Buf(b) => return b.clone(),
            State::Lazy(e) => e.clone(),
        };
        let n = self.n();
        let e = resolve(gpu, &e);
        let out = pooled_out(gpu, self.out_kind(), n);
        let r = {
            let mut cache = gpu.cache.lock().unwrap_or_else(|e| e.into_inner());
            rbqn_gpu::expr::run(&gpu.ctx.device, &gpu.ctx.queue, &mut cache, &e, n, &out, None)
        };
        log_fused(&r, n, self.wide, "map");
        *self.state.borrow_mut() = State::Buf(out.clone());
        out
    }
    /// Host evaluation of the tree (the CPU side of a decision, and the
    /// fallback when the device path panicked). Host leaves are read in place.
    fn host_eval(&self, e: &Rc<Expr>) -> ArrData {
        let n = self.n();
        let prog = host_prog(e);
        if self.wide {
            i64_to_data(prog.eval_i64(n))
        } else {
            squeeze_i32(prog.eval_i32(n))
        }
    }
}

/// Compile a tree for the host evaluator; device leaves are downloaded.
fn host_prog(e: &Rc<Expr>) -> gpu_host::Prog {
    gpu_host::compile(e, &mut |b| {
        let gpu = get().expect("device leaf without a GPU runtime");
        match b.element_type() {
            ElementKind::I64 => Source::I64(pollster::block_on(download_i64(&gpu.ctx.device, &gpu.ctx.queue, b))),
            _ => Source::I32(pollster::block_on(download_i32(&gpu.ctx.device, &gpu.ctx.queue, b))),
        }
    })
}

/// Replace host leaves with uploaded buffers (cached by `B` bits), keeping
/// shared nodes shared. Called only once the GPU path was chosen.
fn resolve(gpu: &GpuRuntime, e: &Rc<Expr>) -> Rc<Expr> {
    fn go(gpu: &GpuRuntime, e: &Rc<Expr>, memo: &mut Vec<(*const Expr, Rc<Expr>)>) -> Rc<Expr> {
        let p = Rc::as_ptr(e);
        if let Some((_, r)) = memo.iter().find(|(q, _)| *q == p) {
            return r.clone();
        }
        let r = match &**e {
            Expr::Host(h) => {
                let leaf = h.downcast_ref::<HostLeaf>().expect("host leaf type");
                let buf = upload_cached(gpu, leaf.key, &leaf.arr, leaf.bound);
                Rc::new(Expr::Leaf(buf))
            }
            Expr::Bin(op, l, r) => {
                let (nl, nr) = (go(gpu, l, memo), go(gpu, r, memo));
                if Rc::ptr_eq(&nl, l) && Rc::ptr_eq(&nr, r) { e.clone() } else { Rc::new(Expr::Bin(*op, nl, nr)) }
            }
            _ => e.clone(),
        };
        memo.push((p, r.clone()));
        r
    }
    go(gpu, e, &mut Vec::new())
}

/// Element/byte counts of a tree for the cost model.
fn tree_work(e: &Rc<Expr>, n: usize, out: ElementKind) -> Work {
    let mut seen: Vec<*const Expr> = Vec::new();
    let mut w = Work { n, out_elem_bytes: if out == ElementKind::I64 { 8 } else { 4 }, ..Work::default() };
    fn go(e: &Rc<Expr>, seen: &mut Vec<*const Expr>, w: &mut Work, srcs: &mut Vec<*const ()>) {
        let p = Rc::as_ptr(e);
        if seen.contains(&p) {
            return;
        }
        seen.push(p);
        match &**e {
            Expr::Bin(_, l, r) => {
                w.ops += 1;
                go(l, seen, w, srcs);
                go(r, seen, w, srcs);
            }
            Expr::Iota => w.leaves += 1,
            Expr::Scalar(_) => {}
            Expr::Leaf(b) => {
                let k = Arc::as_ptr(b) as *const ();
                if !srcs.contains(&k) {
                    srcs.push(k);
                    w.leaves += 1;
                    w.dev_bytes += b.size() as usize;
                }
            }
            Expr::Host(h) => {
                let k = Rc::as_ptr(h) as *const ();
                if !srcs.contains(&k) {
                    srcs.push(k);
                    w.leaves += 1;
                    let leaf = h.downcast_ref::<HostLeaf>().expect("host leaf type");
                    if !upload_is_cached(leaf.key) {
                        w.up_bytes += 4 * leaf.arr.ia();
                    }
                }
            }
        }
    }
    go(e, &mut seen, &mut w, &mut Vec::new());
    w
}

fn i64_to_data(v: Vec<i64>) -> ArrData {
    // NOTE: every value is within ±2^53 (the guard), so f64 is exact.
    if v.iter().all(|&x| x as i32 as i64 == x) {
        squeeze_i32(v.into_iter().map(|x| x as i32).collect())
    } else {
        ArrData::F64(v.into_iter().map(|x| x as f64).collect())
    }
}

/// `[gpu] fused: <ops>` under RBQN_GPU_DEBUG; the WGSL once per new tree shape at level 2.
fn log_fused(r: &rbqn_gpu::expr::FusedRun, n: usize, wide: bool, mode: &str) {
    if !debug_enabled() {
        return;
    }
    eprintln!("[gpu] fused {mode}: {} {n} elements ({})", r.desc, if wide { "i64" } else { "i32" });
    if let Some(src) = &r.new_source
        && std::env::var("RBQN_GPU_DEBUG").as_deref() == Ok("2")
    {
        eprintln!("[gpu] fused WGSL:\n{src}");
    }
}

impl DeviceValue for GpuArr {
    fn shape(&self) -> &[usize] {
        &self.shape
    }
    fn fill(&self) -> Option<B> {
        self.fill
    }
    fn materialize(&self) -> BqnArr {
        // `↕n` on the host is cheaper than a kernel plus a readback.
        if let State::Lazy(e) = &*self.state.borrow()
            && matches!(**e, Expr::Iota)
        {
            let data = ArrData::I32((0..self.n() as i32).collect());
            return BqnArr { shape: self.shape.clone(), data, fill: self.fill };
        }
        let lazy = self.lazy_expr();
        if let Some(e) = &lazy {
            if let Expr::Host(h) = &**e {
                let leaf = h.downcast_ref::<HostLeaf>().expect("host leaf type");
                return (*leaf.arr).clone();
            }
            let w = tree_work(e, self.n(), self.out_kind());
            if self.host_pref.get() || !decide(Kind::Materialize, &w, self.wide) {
                return BqnArr { shape: self.shape.clone(), data: self.host_eval(e), fill: self.fill };
            }
        }
        let gpu = get_decided().expect("pending GPU value without a GPU runtime");
        let dev = guarded("materialize", || {
            let buf = self.force(gpu);
            Some(match buf.element_type() {
                ElementKind::I64 => i64_to_data(pollster::block_on(download_i64(&gpu.ctx.device, &gpu.ctx.queue, &buf))),
                _ => squeeze_i32(pollster::block_on(download_i32(&gpu.ctx.device, &gpu.ctx.queue, &buf))),
            })
        });
        let data = match (dev, lazy) {
            (Some(d), _) => d,
            (None, Some(e)) => {
                if debug_enabled() {
                    eprintln!("[gpu] fused eval failed, host evaluation");
                }
                self.host_eval(&e)
            }
            (None, None) => panic!("GPU readback failed"),
        };
        BqnArr { shape: self.shape.clone(), data, fill: self.fill }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    /// Keep the evaluated buffer as a cached upload of the host copy, so a
    /// later GPU consumer of the same value does not upload it again.
    fn materialized(&self, bits: u64) {
        if let Some(buf) = self.buf() {
            UPLOAD_CACHE.with(|c| {
                let mut c = c.borrow_mut();
                if c.len() >= UPLOAD_CACHE_LEN {
                    c.remove(0);
                }
                c.push((bits, buf, self.bound.get()));
            });
        }
    }
}

// Recent host->device uploads keyed by the array's B bits (ids are never reused,
// since ARR_STORE never frees), so `a+a` and repeated uses of `a` upload once.
const UPLOAD_CACHE_LEN: usize = 4;
std::thread_local! {
    static UPLOAD_CACHE: RefCell<Vec<(u64, Arc<GpuBuffer>, f64)>> = const { RefCell::new(Vec::new()) };
}

/// Shape of an array operand without forcing a pending device value.
/// Returns the host array too when the operand is not pending.
fn operand_shape(b: B) -> Option<(Vec<usize>, Option<Arc<BqnArr>>)> {
    if !b.is_arr() {
        return None;
    }
    if let Some(d) = peek_device(b) {
        return Some((d.shape().to_vec(), None));
    }
    let a = rbqn_core::get_arr(b)?;
    Some((a.shape.clone(), Some(a)))
}

fn device_arr(d: &Arc<dyn DeviceValue>) -> Option<&GpuArr> {
    d.as_any().downcast_ref::<GpuArr>()
}

/// Device buffer and |element| bound for an operand: the pending buffer itself
/// (a lazy tree is evaluated first), a cached upload, or a fresh upload
/// (bound = exact max |v| from a host pass). None for non-integer-safe host data.
fn operand_buf(gpu: &GpuRuntime, b: B, host: Option<&BqnArr>) -> Option<(Arc<GpuBuffer>, f64)> {
    if let Some(d) = peek_device(b) {
        let g = device_arr(&d)?;
        return Some((g.force(gpu), g.bound.get()));
    }
    upload_operand(gpu, b, host)
}

fn upload_operand(gpu: &GpuRuntime, b: B, host: Option<&BqnArr>) -> Option<(Arc<GpuBuffer>, f64)> {
    if let Some(hit) = UPLOAD_CACHE.with(|c| {
        c.borrow().iter().find(|(k, _, _)| *k == b.0).map(|(_, buf, m)| (buf.clone(), *m))
    }) {
        return Some(hit);
    }
    let arr = host?;
    if !gpu_safe_arr(arr) {
        return None;
    }
    let (buf, bound) = arr_to_gpu_i32_bound(gpu, arr)?;
    let buf = Arc::new(buf);
    UPLOAD_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= UPLOAD_CACHE_LEN {
            c.remove(0);
        }
        c.push((b.0, buf.clone(), bound));
    });
    Some((buf, bound))
}

/// An operand as a tree node: (expr, bound, max node bound, wide).
struct Operand {
    e: Rc<Expr>,
    bound: f64,
    maxb: f64,
    wide: bool,
}

fn upload_is_cached(key: u64) -> bool {
    key != 0 && UPLOAD_CACHE.with(|c| c.borrow().iter().any(|(k, _, _)| *k == key))
}

/// Upload a host leaf (an upload-cache hit when `key` was uploaded recently).
fn upload_cached(gpu: &GpuRuntime, key: u64, arr: &BqnArr, bound: f64) -> Arc<GpuBuffer> {
    if let Some(hit) = UPLOAD_CACHE.with(|c| c.borrow().iter().find(|(k, _, _)| key != 0 && *k == key).map(|(_, b, _)| b.clone())) {
        return hit;
    }
    let (buf, _) = arr_to_gpu_i32_bound(gpu, arr).expect("host leaf is integer-safe");
    let buf = Arc::new(buf);
    if key != 0 {
        UPLOAD_CACHE.with(|c| {
            let mut c = c.borrow_mut();
            if c.len() >= UPLOAD_CACHE_LEN {
                c.remove(0);
            }
            c.push((key, buf.clone(), bound));
        });
    }
    buf
}

// Host leaves of recent host operands keyed by `B` bits, so `a+a` shares one
// leaf (one upload, one read) and the max |v| pass runs once per array.
std::thread_local! {
    static HOST_LEAVES: RefCell<Vec<(u64, Rc<Expr>, f64)>> = const { RefCell::new(Vec::new()) };
}

/// A host array as a tree leaf (no upload): (leaf, exact max |v|).
fn host_leaf(b: B, arr: &Arc<BqnArr>) -> Option<(Rc<Expr>, f64)> {
    if let Some(hit) = HOST_LEAVES.with(|c| c.borrow().iter().find(|(k, _, _)| *k == b.0).map(|(_, e, m)| (e.clone(), *m))) {
        return Some(hit);
    }
    // NOTE: a cached upload already knows the bound.
    let bound = match UPLOAD_CACHE.with(|c| c.borrow().iter().find(|(k, _, _)| *k == b.0).map(|(_, _, m)| *m)) {
        Some(m) => m,
        None => gpu_host::max_abs(arr)?,
    };
    let arc = arr.clone();
    let e = Rc::new(Expr::Host(Rc::new(HostLeaf { arr: arc, key: b.0, bound })));
    HOST_LEAVES.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= UPLOAD_CACHE_LEN {
            c.remove(0);
        }
        c.push((b.0, e.clone(), bound));
    });
    Some((e, bound))
}

/// Pending values contribute their tree (or their buffer as a leaf) without
/// any dispatch; host arrays become host leaves (uploaded only if the GPU
/// path is chosen later).
fn operand_expr(b: B, host: Option<&Arc<BqnArr>>) -> Option<Operand> {
    if let Some(d) = peek_device(b) {
        let g = device_arr(&d)?;
        return Some(Operand { e: g.expr(), bound: g.bound.get(), maxb: g.maxb.max(g.bound.get()), wide: g.wide });
    }
    let (e, bound) = host_leaf(b, host?)?;
    Some(Operand { e, bound, maxb: bound, wide: false })
}

/// Evaluate a lazy operand (GPU buffer or host leaf) so the next tree starts from a leaf.
fn force_operand(b: B) {
    if let Some(d) = peek_device(b)
        && let Some(g) = device_arr(&d)
    {
        g.evaluate();
    }
}

fn over_limit(e: &Rc<Expr>) -> bool {
    let (nodes, leaves) = rbqn_gpu::expr::counts(e);
    // Bindings: leaves + scalars + out + partials.
    let budget = match GPU_RUNTIME.get() {
        Some(Some(gpu)) => (gpu.ctx.device.limits().max_storage_buffers_per_shader_stage as usize).saturating_sub(3),
        _ => usize::MAX,
    };
    nodes > rbqn_gpu::expr::MAX_NODES || leaves > rbqn_gpu::expr::MAX_LEAVES.min(budget)
}

/// Tighten the bound of a pending I32 operand with a device min/max pass
/// (one tiny readback). Returns the new bound, or None when `b` is not a
/// pending I32 buffer (host uploads already carry their exact max |v|; lazy
/// trees are not evaluated for this).
fn refine_bound(b: B) -> Option<f64> {
    let d = peek_device(b)?;
    let g = device_arr(&d)?;
    let buf = g.buf()?;
    if buf.element_type() != ElementKind::I32 || buf.is_empty() {
        return None;
    }
    let gpu = get()?;
    let mm = {
        let mut cache = gpu.cache.lock().unwrap_or_else(|e| e.into_inner());
        rbqn_gpu::kernels::minmax::minmax_i32(&gpu.ctx.device, &gpu.ctx.queue, &mut cache, &buf)
    };
    let v = pollster::block_on(download_i32(&gpu.ctx.device, &gpu.ctx.queue, &mm));
    let nb = (v[0] as f64).abs().max((v[1] as f64).abs());
    log_dispatch("minmax", buf.len(), "i32");
    g.bound.set(g.bound.get().min(nb));
    Some(g.bound.get())
}

fn fill_of(b: B, host: Option<&BqnArr>) -> Option<B> {
    match host {
        Some(a) => a.fill,
        None => peek_device(b).and_then(|d| d.fill()),
    }
}

fn arith_bound(op: &str, wb: f64, xb: f64) -> f64 {
    match op {
        "add" | "sub" => wb + xb,
        _ => wb * xb,
    }
}

/// A lazy node `l op r` (no dispatch). i32 evaluation when every node bound
/// is < 2^31 and no leaf is i64; else i64 (needs SHADER_INT64, else None).
/// When the tree outgrows MAX_NODES / MAX_LEAVES the operands are evaluated
/// to buffers first and the node starts from two leaves.
fn lazy_node(
    op: &str,
    mk: &dyn Fn() -> Option<(Operand, Operand)>,
    operands: &[B],
    shape: Vec<usize>,
    fill: Option<B>,
    bound: f64,
) -> Option<B> {
    let eop = rbqn_gpu::expr::Op::from_name(op)?;
    let (mut l, mut r) = mk()?;
    let mut e = Rc::new(Expr::Bin(eop, l.e.clone(), r.e.clone()));
    if over_limit(&e) {
        for &b in operands {
            force_operand(b);
        }
        (l, r) = mk()?;
        e = Rc::new(Expr::Bin(eop, l.e, r.e));
    }
    let maxb = l.maxb.max(r.maxb).max(bound);
    let wide = l.wide || r.wide || maxb >= I32_LIM;
    if debug_enabled() {
        eprintln!("[gpu] lazy {op} ({})", if wide { "i64" } else { "i32" });
    }
    Some(tag_device(Arc::new(GpuArr::lazy(e, shape, fill, bound, maxb, wide))))
}

/// GPU binary arithmetic on two same-shape integer arrays, given as raw `B`
/// so pending device operands are used in place. Returns a lazy device value
/// (an expression node, no dispatch): i32 when every bound is < 2^31, i64
/// when < 2^53, else None (CPU). `÷` is never taken (non-integer results).
/// Registered as GPU_ARITH_HOOK in rbqn-prim at startup.
pub fn gpu_arith_binary(op: &str, w: B, x: B) -> Option<B> {
    if gpu_mode() == GpuMode::Off || !gpu_possible() { return None; }
    if !w.is_arr() { return gpu_arith_scalar(op, w.o2f(), true, x); }
    if !x.is_arr() { return gpu_arith_scalar(op, x.o2f(), false, w); }
    if !matches!(op, "add" | "sub" | "mul") { return None; }
    guarded("arith", || {
        let (wshape, wh) = operand_shape(w)?;
        let (xshape, xh) = operand_shape(x)?;
        if wshape.is_empty() || wshape != xshape { return None; }
        let n: usize = wshape.iter().product();
        if n == 0 || n >= i32::MAX as usize || !should_dispatch("arith", n) { return None; }
        if let Some(a) = &wh && !gpu_safe_arr(a) { return None; }
        if let Some(a) = &xh && !gpu_safe_arr(a) { return None; }
        let mk = || Some((operand_expr(w, wh.as_ref())?, operand_expr(x, xh.as_ref())?));
        let (lw, lx) = mk()?;
        let mut bound = arith_bound(op, lw.bound, lx.bound);
        if bound >= EXACT_LIM {
            let wb = refine_bound(w).unwrap_or(lw.bound);
            let xb = refine_bound(x).unwrap_or(lx.bound);
            bound = arith_bound(op, wb, xb);
            if bound >= EXACT_LIM { return None; }
        }
        let fill = fill_of(w, wh.as_deref());
        lazy_node(op, &mk, &[w, x], wshape, fill, bound)
    })
}

fn scalar_bound(op: &str, b: f64, s: f64) -> f64 {
    match op {
        "add" | "sub" => b + s.abs(),
        "mul" => b * s.abs(),
        _ => b.max(s.abs()),
    }
}

/// Lazy `s op a` (`scalar_left`) or `a op s` for an integer scalar within i32
/// and an integer array that is pending on the device, or a host array over
/// the arith threshold. Same i32 / i64 / CPU selection as the array-array path.
/// Non-integer scalars, `÷` and non-integer-safe host data return None (CPU).
fn gpu_arith_scalar(op: &str, s: f64, scalar_left: bool, a: B) -> Option<B> {
    if !matches!(op, "add" | "sub" | "mul" | "min" | "max") { return None; }
    if !(s.fract() == 0.0 && s.abs() < I32_LIM) { return None; }
    let si = s as i32;
    guarded("arith_scalar", || {
        let (shape, host) = operand_shape(a)?;
        let n: usize = shape.iter().product();
        if shape.is_empty() || n == 0 || n >= i32::MAX as usize { return None; }
        // Pending values dispatch at any size; host arrays honour the threshold.
        if host.is_some() && !should_dispatch("arith", n) { return None; }
        if let Some(h) = &host && !gpu_safe_arr(h) { return None; }
        let mk = || {
            let o = operand_expr(a, host.as_ref())?;
            let sc = Operand { e: Rc::new(Expr::Scalar(si)), bound: s.abs(), maxb: s.abs(), wide: false };
            Some(if scalar_left { (sc, o) } else { (o, sc) })
        };
        let o = operand_expr(a, host.as_ref())?;
        let mut bound = scalar_bound(op, o.bound, s);
        if bound >= EXACT_LIM {
            let b = refine_bound(a).unwrap_or(o.bound);
            bound = scalar_bound(op, b, s);
            if bound >= EXACT_LIM { return None; }
        }
        let fill = fill_of(a, host.as_deref());
        lazy_node(op, &mk, &[a], shape, fill, bound)
    })
}

/// Device `↕n` for n at the arith threshold (or under force): a lazy iota
/// leaf, never uploaded; a host consumer gets it built on the host.
/// Registered as GPU_IOTA_HOOK in rbqn-prim at startup.
pub fn gpu_iota(n: usize) -> Option<B> {
    if gpu_mode() == GpuMode::Off || GPU_DISABLED.load(Ordering::Relaxed) { return None; }
    if n == 0 || n >= i32::MAX as usize || !should_dispatch("arith", n) || !gpu_possible() { return None; }
    let b = (n - 1) as f64;
    Some(tag_device(Arc::new(GpuArr::lazy(Rc::new(Expr::Iota), vec![n], Some(B::m_i32(0)), b, b, false))))
}

/// Map an op name string to a `FusedOp` variant for the FusionBuilder.
/// Returns None for unrecognised names so callers can fall back to CPU.
fn op_str_to_fused(name: &str, scalar: Option<f64>) -> Option<FusedOp> {
    match name {
        "add" => Some(FusedOp::Add),
        "sub" => Some(FusedOp::Sub),
        "mul" => Some(FusedOp::Mul),
        "div" => Some(FusedOp::Div),
        "scalar_add" => Some(FusedOp::ScalarAdd(scalar? as f32)),
        "scalar_mul" => Some(FusedOp::ScalarMul(scalar? as f32)),
        _ => None,
    }
}

/// GPU-accelerated fused arithmetic: execute a sequence of elementwise ops in one kernel.
///
/// `ops` is a list of `(op_name, optional_scalar)` pairs. Op names: "add", "sub", "mul",
/// "div" (binary, requires `b`), "scalar_add", "scalar_mul" (element-wise, scalar in tuple).
///
/// `a` is the primary array (left operand for binary ops). `b` is the optional secondary
/// array (right operand for binary ops — required when any op in `ops` is a binary op).
///
/// Returns None when GPU is unavailable, threshold is not met, ops list is empty/invalid,
/// or any op name is unrecognised. CPU fallback is the caller's responsibility.
///
/// # WGSL codegen test
///
/// The FusionBuilder generates a single fused WGSL kernel by chaining all ops into one
/// compute shader. For example, `[("add", None), ("scalar_mul", Some(2.0))]` produces
/// a shader that computes `val = (a[idx] + b[idx]) * 2.0` in one dispatch.
///
/// # Future work
///
/// True expression-level auto-fusion (detecting `2×a+b` as a fuseable pattern) requires
/// VM-level lookahead — the BQN evaluator calls `c2` one call at a time with no forward
/// lookahead. Phase 5 delivers the fusion infrastructure and this explicit API. Future
/// phases can add a VM-level fusion pass that calls `try_fused_arith` directly.
pub fn gpu_fused_arith(
    ops: &[(&str, Option<f64>)],
    a: &BqnArr,
    b: Option<&BqnArr>,
) -> Option<BqnArr> {
    guarded("fused_arith", || {
        gpu_fused_arith_inner(ops, a, b)
    })
}

fn gpu_fused_arith_inner(
    ops: &[(&str, Option<f64>)],
    a: &BqnArr,
    b: Option<&BqnArr>,
) -> Option<BqnArr> {
    if ops.is_empty() { return None; }

    if !should_dispatch("arith", a.ia()) { return None; }
    if !gpu_safe_arr(a) { return None; }
    if let Some(b_arr) = b {
        if !gpu_safe_arr(b_arr) { return None; }
        if a.shape != b_arr.shape { return None; }
    }
    let gpu = get()?;

    // Build FusionBuilder from ops list
    let mut builder = FusionBuilder::new();
    for &(name, scalar) in ops {
        let fused_op = op_str_to_fused(name, scalar)?;
        builder.push(fused_op);
    }

    if builder.is_empty() { return None; }

    let device = &gpu.ctx.device;
    let queue = &gpu.ctx.queue;

    let a_buf = arr_to_gpu_i32(gpu, a)?;
    // Upload secondary array if present; propagate None to signal GPU fallback.
    let b_buf: Option<GpuBuffer> = match b {
        Some(b_arr) => Some(arr_to_gpu_i32(gpu, b_arr)?),
        None => None,
    };
    let out_buf = GpuBuffer::storage(device, ElementKind::I32, a.ia());

    {
        let mut cache = gpu.cache.lock().ok()?;
        builder.execute(device, queue, &mut cache, &a_buf, b_buf.as_ref(), &out_buf);
    }

    let result = gpu_i32_to_arr(gpu, &out_buf, a.shape.clone(), a.fill);
    log_dispatch("fused_arith", a.ia(), "i32");
    Some(result)
}

/// Verify that the FusionBuilder generates valid WGSL for a common fused pattern.
/// This is called at startup (in debug builds) to assert correctness of codegen.
/// The generated WGSL for Add+ScalarMul must contain both "val + b_val" and "val * f32".
#[allow(dead_code)]
pub fn verify_fusion_wgsl() {
    let mut builder = FusionBuilder::new();
    builder.push(FusedOp::Add);
    builder.push(FusedOp::ScalarMul(2.0));
    let (source, entry) = builder.generate_wgsl(rbqn_gpu::buffer::ElementKind::I32);
    assert!(source.contains("val + b_val"), "FusionBuilder WGSL missing Add op");
    assert!(source.contains("val * i32(2)"), "FusionBuilder WGSL missing ScalarMul(2.0) for i32");
    assert!(entry.contains("fused_"), "FusionBuilder entry point missing fused_ prefix");
    if debug_enabled() {
        eprintln!("[gpu] fusion WGSL verify: OK (entry={})", entry);
    }
}

/// Extract prim_idx from a B function value, if it's a NativeFn.
fn prim_idx_of(f: B) -> Option<usize> {
    if !f.is_fun() {
        return None;
    }
    let fid = (f.0 & 0xFFFFFFFFFFFF) >> 3;
    let d = rbqn_vm::derive::get_derived(fid);
    if let rbqn_vm::derive::DerivedKind::NativeFn { prim_idx } = d.kind {
        Some(prim_idx)
    } else {
        None
    }
}

/// GPU fold (reduce) for large rank-1 numeric arrays, given as raw `x`.
/// Supports +, x, floor, ceil (prim_idx 0, 2, 6, 7). Returns a number (the
/// result is read back immediately), never a pending value.
/// Returns None for unsupported ops or if GPU dispatch is unavailable.
pub fn gpu_fold(f: B, x: B) -> Option<B> {
    if gpu_mode() == GpuMode::Off { return None; }
    guarded("fold", || gpu_fold_inner(f, x))
}

fn gpu_fold_inner(f: B, x: B) -> Option<B> {
    let (shape, host) = operand_shape(x)?;
    // NOTE: n = 0 stays on the CPU (identity element; the kernels assert n > 0).
    if shape.len() != 1 || shape[0] == 0 { return None; }
    let n = shape[0];
    if !should_dispatch("reduce", n) {
        return None;
    }
    // NOTE: Map prim_idx to GPU reduce op; only supported primitives dispatch
    let op = match prim_idx_of(f)? {
        0 => "add", // +
        2 => "mul", // x
        6 => "min", // floor
        7 => "max", // ceil
        _ => return None,
    };
    if let Some(a) = &host && !gpu_safe_arr(a) { return None; }
    let out_bound = |b: f64| match op {
        "add" => b * n as f64,
        // Products only when every |v| <= 1 (bound stays 1).
        "mul" => if b > 1.0 { f64::INFINITY } else { b },
        _ => b,
    };
    // NOTE: a fold yields one number, so an exact i64 sum rounded once to f64
    // is at least as accurate as the CPU's f64 chain: allow up to 2^63 here
    // (elementwise and scan results stay under 2^53 so later ops see exact f64).
    let fold_lim = if op == "add" { I64_LIM } else { EXACT_LIM };
    let lazy = peek_device(x).filter(|d| device_arr(d).is_some_and(|g| g.buf().is_none()));
    if let Some(d) = &lazy {
        let g = device_arr(d)?;
        let rb = out_bound(g.bound.get());
        if rb >= fold_lim { return None; }
        let e = g.lazy_expr()?;
        let w = tree_work(&e, n, g.out_kind());
        if decide(Kind::Fold, &w, g.wide || rb >= I32_LIM)
            && let Some(gpu) = get_decided()
        {
            return fused_fold(gpu, g, op, n, rb);
        }
        // Host: one fused pass over blocks, nothing materialized or uploaded.
        g.host_pref.set(true);
        let v = host_prog(&e).fold(n, op, g.wide);
        if debug_enabled() {
            eprintln!("[gpu] host fold_{op} {n} elements");
        }
        return Some(B::m_f64(v as f64));
    }
    if host.is_some() {
        let up = if upload_is_cached(x.0) { 0 } else { 4 * n };
        let w = Work { n, leaves: 1, up_bytes: up, out_elem_bytes: 4, ..Work::default() };
        if !decide(Kind::FoldPlain, &w, false) { return None; }
    }
    let gpu = get_decided()?;
    let (buf, mut bound) = operand_buf(gpu, x, host.as_deref())?;
    if out_bound(bound) >= fold_lim {
        bound = refine_bound(x).unwrap_or(bound);
        if out_bound(bound) >= fold_lim { return None; }
    }
    let rb = out_bound(bound);
    let is_i32 = buf.element_type() == ElementKind::I32;
    let (result, kind) = if is_i32 && rb < I32_LIM {
        let result_buf = {
            let mut cache = gpu.cache.lock().unwrap_or_else(|e| e.into_inner());
            rbqn_gpu::kernels::reduce::reduce(&gpu.ctx.device, &gpu.ctx.queue, &mut cache, op, &buf)
        };
        let d = pollster::block_on(download_i32(&gpu.ctx.device, &gpu.ctx.queue, &result_buf));
        (*d.first()? as f64, "i32")
    } else {
        if !gpu.ctx.shader_int64 { return None; }
        let result_buf = {
            let mut cache = gpu.cache.lock().unwrap_or_else(|e| e.into_inner());
            if is_i32 {
                rbqn_gpu::kernels::reduce_i64::reduce_i32_to_i64(&gpu.ctx.device, &gpu.ctx.queue, &mut cache, op, &buf)
            } else {
                rbqn_gpu::kernels::reduce_i64::reduce_i64(&gpu.ctx.device, &gpu.ctx.queue, &mut cache, op, &buf)
            }
        };
        let d = pollster::block_on(download_i64(&gpu.ctx.device, &gpu.ctx.queue, &result_buf));
        (*d.first()? as f64, if is_i32 { "i32→i64" } else { "i64" })
    };
    log_dispatch(&format!("reduce_{op}"), n, kind);
    Some(B::m_f64(result))
}

/// Fold over a lazy tree: one fused kernel evaluates the tree into a buffer
/// (which replaces the tree, so a second use does not re-evaluate) and writes
/// per-workgroup partials; one more reduce pass combines the partials.
fn fused_fold(gpu: &GpuRuntime, g: &GpuArr, op: &str, n: usize, rb: f64) -> Option<B> {
    let acc_wide = g.wide || rb >= I32_LIM;
    if acc_wide && !gpu.ctx.shader_int64 { return None; }
    let acc = if acc_wide { ElementKind::I64 } else { ElementKind::I32 };
    let e = resolve(gpu, &g.expr());
    let out = pooled_out(gpu, g.out_kind(), n);
    let (dev, q) = (&gpu.ctx.device, &gpu.ctx.queue);
    let result_buf = {
        let mut cache = gpu.cache.lock().unwrap_or_else(|e| e.into_inner());
        let r = rbqn_gpu::expr::run(dev, q, &mut cache, &e, n, &out, Some((op, acc)));
        log_fused(&r, n, g.wide, &format!("reduce_{op}"));
        let parts = r.partials?;
        if acc_wide {
            rbqn_gpu::kernels::reduce_i64::reduce_i64(dev, q, &mut cache, op, &parts)
        } else {
            rbqn_gpu::kernels::reduce::reduce(dev, q, &mut cache, op, &parts)
        }
    };
    *g.state.borrow_mut() = State::Buf(out);
    let v = if acc_wide {
        *pollster::block_on(download_i64(dev, q, &result_buf)).first()? as f64
    } else {
        *pollster::block_on(download_i32(dev, q, &result_buf)).first()? as f64
    };
    Some(B::m_f64(v))
}

/// GPU scan (inclusive prefix sum) for large rank-1 numeric arrays, given as raw `x`.
/// Only supports + (prim_idx 0). Returns a pending device value (i32 when every
/// partial sum fits, else i64 while < 2^53, else None for the CPU).
/// Returns None for unsupported ops or if GPU dispatch is unavailable.
pub fn gpu_scan(f: B, x: B) -> Option<B> {
    if gpu_mode() == GpuMode::Off || !x.is_arr() { return None; }
    guarded("scan", || gpu_scan_inner(f, x))
}

fn gpu_scan_inner(f: B, x: B) -> Option<B> {
    let (shape, host) = operand_shape(x)?;
    if shape.len() != 1 || shape[0] == 0 { return None; }
    let n = shape[0];
    if !should_dispatch("scan", n) {
        return None;
    }
    // NOTE: Only + (prim_idx 0) supported for scan (GPU kernel implements prefix add)
    if prim_idx_of(f)? != 0 {
        return None;
    }
    if let Some(a) = &host && !gpu_safe_arr(a) { return None; }
    let dev = peek_device(x);
    let g = dev.as_ref().and_then(device_arr);
    let mut w = match g {
        Some(g) => match g.lazy_expr() {
            Some(e) => tree_work(&e, n, g.out_kind()),
            None => Work { n, leaves: 1, dev_bytes: g.buf()?.size() as usize, ..Work::default() },
        },
        None => Work { n, leaves: 1, up_bytes: if upload_is_cached(x.0) { 0 } else { 4 * n }, ..Work::default() },
    };
    // Scan output width (host input: bound unknown before the upload, assume i64).
    w.out_elem_bytes = match g {
        Some(g) if g.bound.get() * (n as f64) < I32_LIM => 4,
        _ => 8,
    };
    if !decide(Kind::Scan, &w, g.is_some_and(|g| g.wide)) {
        if let Some(g) = g {
            g.host_pref.set(true);
        }
        return None;
    }
    let gpu = get_decided()?;
    let (buf, mut bound) = operand_buf(gpu, x, host.as_deref())?;
    if bound * n as f64 >= EXACT_LIM {
        bound = refine_bound(x).unwrap_or(bound);
        if bound * n as f64 >= EXACT_LIM { return None; }
    }
    let out_bound = bound * n as f64;
    let is_i32 = buf.element_type() == ElementKind::I32;
    let (result_buf, kind) = if is_i32 && out_bound < I32_LIM {
        let mut cache = gpu.cache.lock().unwrap_or_else(|e| e.into_inner());
        let out = pooled_out(gpu, ElementKind::I32, n);
        rbqn_gpu::kernels::scan::inclusive_scan_into(&gpu.ctx.device, &gpu.ctx.queue, &mut cache, &buf, &out);
        (out, "i32")
    } else {
        if !gpu.ctx.shader_int64 { return None; }
        let mut cache = gpu.cache.lock().unwrap_or_else(|e| e.into_inner());
        let out = pooled_out(gpu, ElementKind::I64, n);
        rbqn_gpu::kernels::scan_i64::scan_i64_into(&gpu.ctx.device, &gpu.ctx.queue, &mut cache, "add", &buf, &out);
        (out, if is_i32 { "i32→i64" } else { "i64" })
    };
    if result_buf.len() != n {
        return None;
    }
    log_dispatch("scan_add", n, kind);
    let fill = fill_of(x, host.as_deref());
    Some(tag_device(Arc::new(GpuArr::new(result_buf, shape, fill, out_bound))))
}

/// GPU-accelerated grade (⍋/⍒): returns permutation indices that sort the array.
/// Returns None on any error (CPU fallback).
/// Registered as GPU_GRADE_HOOK in rbqn-prim at startup.
pub fn gpu_grade(arr: &BqnArr, ascending: bool) -> Option<BqnArr> {
    guarded("grade", || {
        // Only dispatch rank-1 numeric arrays above threshold
        if arr.rank() != 1 { return None; }
        if !should_dispatch("sort", arr.ia()) { return None; }
        if !gpu_safe_arr(arr) { return None; }
        let gpu = get()?;

        let device = &gpu.ctx.device;
        let queue = &gpu.ctx.queue;

        let input_buf = arr_to_gpu_i32(gpu, arr)?;

        let mut indices = {
            let mut cache = gpu.cache.lock().ok()?;
            rbqn_gpu::kernels::sort::argsort_i32(device, queue, &mut cache, &input_buf)
        };

        if !ascending {
            indices.reverse();
        }

        let n = arr.ia();
        let mut out = BqnArr::new_vec_i32(indices);
        out.shape = vec![n];
        out.fill = Some(B::m_i32(0));
        log_dispatch("grade", n, "i32");
        Some(out)
    })
}

/// GPU-accelerated matrix multiply: w (M×K) •math.MatMul x (K×N) → result (M×N).
/// Returns None when GPU unavailable or data is invalid (CPU fallback).
/// Registered as GPU_MATMUL_HOOK in rbqn-vm::derive at startup.
pub fn gpu_matmul(w: B, x: B) -> Option<B> {
    guarded("matmul", || {
        let wa = rbqn_vm::vm::get_arr(w)?;
        let xa = rbqn_vm::vm::get_arr(x)?;

        if wa.rank() != 2 || xa.rank() != 2 { return None; }
        let m = wa.shape[0];
        let k = wa.shape[1];
        let kx = xa.shape[0];
        let n = xa.shape[1];
        if k != kx { return None; }

        // NOTE: Threshold check — dispatch to GPU for larger matrices.
        // For small matrices, CPU is faster due to transfer overhead.
        if gpu_mode() != GpuMode::Force && m * k + k * n < 50_000 { return None; }
        let gpu = get()?;

        // Extract f64 data from arrays; skip if char/boxed
        let w_f64: Vec<f64> = arr_to_f64(&wa)?;
        let x_f64: Vec<f64> = arr_to_f64(&xa)?;

        let device = &gpu.ctx.device;
        let queue = &gpu.ctx.queue;

        // Convert to f32 for GPU kernel (matmul kernel only supports f32)
        let w_f32: Vec<f32> = w_f64.iter().map(|&v| v as f32).collect();
        let x_f32: Vec<f32> = x_f64.iter().map(|&v| v as f32).collect();

        let a_buf = upload_f32(device, queue, &w_f32);
        let b_buf = upload_f32(device, queue, &x_f32);
        let out_buf = GpuBuffer::storage(device, ElementKind::F32, m * n);

        {
            let mut cache = gpu.cache.lock().ok()?;
            rbqn_gpu::kernels::matmul::matmul(
                device,
                queue,
                &mut cache,
                &a_buf,
                &b_buf,
                &out_buf,
                m as u32,
                n as u32,
                k as u32,
            );
        }

        let result_f32 = pollster::block_on(download_f32(device, queue, &out_buf));
        let result_f64: Vec<f64> = result_f32.iter().map(|&v| v as f64).collect();

        let out = BqnArr {
            shape: vec![m, n],
            data: ArrData::F64(result_f64),
            fill: Some(B::m_f64(0.0)),
        };

        if debug_enabled() {
            eprintln!("[gpu] matmul {}x{}x{} (f32)", m, k, n);
        }

        Some(rbqn_vm::vm::tag_arr(out))
    })
}

/// GPU-accelerated softmax: •math.Softmax x → probability distribution.
/// Returns None when GPU unavailable or data is invalid (CPU fallback).
/// Registered as GPU_SOFTMAX_HOOK in rbqn-vm::derive at startup.
pub fn gpu_softmax(x: B) -> Option<B> {
    guarded("softmax", || {
        let xa = rbqn_vm::vm::get_arr(x)?;
        if xa.rank() != 1 { return None; }
        let n = xa.ia();

        // NOTE: Dispatch threshold — softmax GPU overhead only pays off for larger arrays.
        if gpu_mode() != GpuMode::Force && n < 256 { return None; }
        let gpu = get()?;

        let x_f64: Vec<f64> = arr_to_f64(&xa)?;
        let x_f32: Vec<f32> = x_f64.iter().map(|&v| v as f32).collect();

        let device = &gpu.ctx.device;
        let queue = &gpu.ctx.queue;

        let input_buf = upload_f32(device, queue, &x_f32);
        let out_buf = GpuBuffer::storage(device, ElementKind::F32, n);

        {
            let mut cache = gpu.cache.lock().ok()?;
            rbqn_gpu::kernels::softmax::softmax(device, queue, &mut cache, &input_buf, &out_buf);
        }

        let result_f32 = pollster::block_on(download_f32(device, queue, &out_buf));
        let result_f64: Vec<f64> = result_f32.iter().map(|&v| v as f64).collect();

        let out = BqnArr {
            shape: vec![n],
            data: ArrData::F64(result_f64),
            fill: Some(B::m_f64(0.0)),
        };

        if debug_enabled() {
            eprintln!("[gpu] softmax {} elements (f32)", n);
        }

        Some(rbqn_vm::vm::tag_arr(out))
    })
}

/// Extract f64 values from a BqnArr. Returns None for char/boxed arrays.
fn arr_to_f64(arr: &BqnArr) -> Option<Vec<f64>> {
    let n = arr.ia();
    match &arr.data {
        ArrData::F64(v) => Some(v.clone()),
        ArrData::I32(v) => Some(v.iter().map(|&x| x as f64).collect()),
        ArrData::I16(v) => Some(v.iter().map(|&x| x as f64).collect()),
        ArrData::I8(v)  => Some(v.iter().map(|&x| x as f64).collect()),
        ArrData::Bit(v) => Some((0..n).map(|i| ((v[i/64] >> (i%64)) & 1) as f64).collect()),
        ArrData::C8(_) | ArrData::C16(_) | ArrData::C32(_) | ArrData::Boxed(_) => None,
    }
}

/// GPU-accelerated monadic sort (∧/∨): returns sorted values.
/// Returns None on any error (CPU fallback).
/// Registered as GPU_SORT_HOOK in rbqn-prim at startup.
pub fn gpu_sort(arr: &BqnArr, ascending: bool) -> Option<BqnArr> {
    guarded("sort", || {
        // Only dispatch rank-1 numeric arrays above threshold
        if arr.rank() != 1 { return None; }
        if !should_dispatch("sort", arr.ia()) { return None; }
        if !gpu_safe_arr(arr) { return None; }
        let gpu = get()?;

        let device = &gpu.ctx.device;
        let queue = &gpu.ctx.queue;

        let input_buf = arr_to_gpu_i32(gpu, arr)?;

        let sorted_buf = {
            let mut cache = gpu.cache.lock().ok()?;
            rbqn_gpu::kernels::sort::sort_i32(device, queue, &mut cache, &input_buf)
        };

        let n = arr.ia();
        let mut result = gpu_i32_to_arr(gpu, &sorted_buf, vec![n], arr.fill);
        if !ascending {
            // Reverse the sorted array for descending order
            if let rbqn_core::array::ArrData::I32(ref mut v) = result.data {
                v.reverse();
            } else {
                // Convert to i32 for reversal if squeezed to smaller type
                let vals: Vec<i32> = (0..result.ia()).filter_map(|i| {
                    result.get(i).ok().and_then(|b| b.to_f64().ok()).map(|f| f as i32)
                }).collect();
                let mut reversed = vals;
                reversed.reverse();
                result.data = rbqn_core::array::ArrData::I32(reversed);
            }
        }
        log_dispatch("sort", n, "i32");
        Some(result)
    })
}

#[cfg(test)]
mod tests {
    use rbqn_gpu::fusion::{FusedOp, FusionBuilder};
    use rbqn_gpu::buffer::ElementKind;

    /// Verify FusionBuilder generates correct WGSL for Add+ScalarMul sequence.
    /// This tests the fusion infrastructure without requiring a GPU device.
    #[test]
    fn fusion_wgsl_add_scalarmul() {
        let mut builder = FusionBuilder::new();
        builder.push(FusedOp::Add);
        builder.push(FusedOp::ScalarMul(2.0));

        let (source, entry) = builder.generate_wgsl(ElementKind::I32);

        // Must declare both input arrays (binary op requires b)
        assert!(source.contains("input_a: array<i32>"), "missing input_a declaration");
        assert!(source.contains("input_b: array<i32>"), "missing input_b declaration");
        assert!(source.contains("output: array<i32>"), "missing output declaration");

        // Must compute Add then ScalarMul in sequence
        assert!(source.contains("val + b_val"), "missing Add op");
        assert!(source.contains("val * i32(2)"), "missing ScalarMul(2.0) for i32");

        // Entry point must follow fused_{n_ops} naming
        assert_eq!(entry, "fused_2", "entry point name mismatch");
    }

    /// Verify FusionBuilder generates correct WGSL for scalar-only ops (no b array).
    #[test]
    fn fusion_wgsl_scalar_only() {
        let mut builder = FusionBuilder::new();
        builder.push(FusedOp::ScalarAdd(1.0));
        builder.push(FusedOp::ScalarMul(3.0));

        let (source, entry) = builder.generate_wgsl(ElementKind::F32);

        // Scalar-only: no b array needed
        assert!(source.contains("input_a: array<f32>"), "missing input_a");
        assert!(!source.contains("input_b"), "unexpected input_b for scalar-only");

        assert!(source.contains("val + f32(1)"), "missing ScalarAdd(1.0)");
        assert!(source.contains("val * f32(3)"), "missing ScalarMul(3.0)");
        assert_eq!(entry, "fused_2");
    }

    /// Verify op_str_to_fused maps all supported op names.
    #[test]
    fn fused_op_name_mapping() {
        use super::op_str_to_fused;
        assert!(op_str_to_fused("add", None).is_some());
        assert!(op_str_to_fused("sub", None).is_some());
        assert!(op_str_to_fused("mul", None).is_some());
        assert!(op_str_to_fused("div", None).is_some());
        assert!(op_str_to_fused("scalar_add", Some(1.0)).is_some());
        assert!(op_str_to_fused("scalar_mul", Some(2.0)).is_some());
        // Scalar ops require the scalar value
        assert!(op_str_to_fused("scalar_mul", None).is_none());
        // Unknown ops return None
        assert!(op_str_to_fused("unknown", None).is_none());
    }
}
