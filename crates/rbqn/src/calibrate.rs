//! Cost-model calibration (#167): time small GPU and host workloads at two sizes each and
//! hand the timings to `rbqn_gpu::calibrate::derive`. Runs once per process at first device
//! use when no cached model matches the adapter (or under `RBQN_GPU_CALIBRATE=1`); the point
//! is a fraction of a second once per machine, not precision: every quantity is the median of
//! a few runs at two sizes, so the fit sees the machine's steady cost and not its load spikes.
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use rbqn_core::B;
use rbqn_core::array::{BqnArr, squeeze_i32};
use rbqn_gpu::buffer::{ElementKind, GpuBuffer, download_i32, upload_i32};
use rbqn_gpu::calibrate::{Pair, Samples, derive};
use rbqn_gpu::dispatch::Model;
use rbqn_gpu::expr::{Expr, Op};

use crate::gpu_host::{self, HostLeaf};
use crate::gpu_runtime::{GpuRuntime, matmul_f32};

/// Element counts of the elementwise, fold, scan and sort timings.
const N1: usize = 1 << 14;
const N2: usize = 1 << 17;
/// Element counts of the host timings that stream through memory (fold, evaluate, write a
/// result): large enough that the data leaves the caches and the result is a fresh allocation.
const C1: usize = 1 << 19;
const C2: usize = 1 << 22;
/// Element counts of the sorts (both sides).
const S1: usize = 1 << 17;
const S2: usize = 1 << 19;
/// Plain typed folds stay below the parallel threshold (`par::INT_FOLD_MIN`, 2^20), as the
/// runtime's decision sizes mostly do not; a pool wake-up would otherwise dominate the fit.
const F1: usize = 1 << 17;
const F2: usize = 1 << 19;
/// Timed runs per point (after one untimed warm-up); the median is kept.
const REPS: usize = 3;
/// Runs for the cheap device timings whose differences give upload, download and map: the
/// median of more runs keeps a preempted run from becoming a constant.
const GPU_REPS: usize = 5;

fn ms_of(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

/// Median of `reps` timed runs after one warm-up run. The median, as the bench uses: the
/// minimum of a few short runs finds quiet slices a longer run on a shared host never gets.
fn best_of(reps: usize, mut f: impl FnMut()) -> f64 {
    f();
    let mut v: Vec<f64> = (0..reps).map(|_| { let t = Instant::now(); f(); ms_of(t) }).collect();
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn best(f: impl FnMut()) -> f64 {
    best_of(REPS, f)
}

fn pair(x1: usize, x2: usize, f: impl FnMut(usize)) -> Pair {
    pair_of(REPS, x1, x2, f)
}

/// Two sizes timed with `reps` runs each. The cheap host timings take more runs: a loaded
/// host preempts a short run only now and then, and the median drops those.
fn pair_of(reps: usize, x1: usize, x2: usize, mut f: impl FnMut(usize)) -> Pair {
    Pair { x1: x1 as f64, ms1: best_of(reps, || f(x1)), x2: x2 as f64, ms2: best_of(reps, || f(x2)) }
}

/// Small values, so every sum and scan stays far inside i32.
fn small(n: usize) -> Vec<i32> {
    (0..n).map(|i| (i % 4) as i32 + 1).collect()
}

/// Pseudo-random keys below 2^31 (xorshift), the range the sort bench uses.
fn keys(n: usize) -> Vec<i32> {
    let mut s = 0x9E37_79B9_7F4A_7C15u64;
    (0..n).map(|_| { s ^= s << 13; s ^= s >> 7; s ^= s << 17; (s >> 33) as i32 }).collect()
}

fn bin(op: Op, l: &Rc<Expr>, r: &Rc<Expr>) -> Rc<Expr> {
    Rc::new(Expr::Bin(op, l.clone(), r.clone()))
}

/// Fold of a tree on the device: the fused map+reduce kernel, one more reduce, a scalar readback.
fn gpu_fold(rt: &GpuRuntime, e: &Rc<Expr>, n: usize, out: &GpuBuffer) {
    let (dev, q) = (&rt.ctx.device, &rt.ctx.queue);
    let res = {
        let mut cache = rt.cache.lock().unwrap_or_else(|e| e.into_inner());
        let r = rbqn_gpu::expr::run(dev, q, &mut cache, e, n, out, Some(("add", ElementKind::I32)));
        let parts = r.partials.expect("reduce partials");
        rbqn_gpu::kernels::reduce::reduce(dev, q, &mut cache, "add", &parts)
    };
    std::hint::black_box(pollster::block_on(download_i32(dev, q, &res)));
}

fn gpu_samples(rt: &GpuRuntime) -> Samples {
    let (dev, q) = (&rt.ctx.device, &rt.ctx.queue);
    let host = small(N2);
    let leaf = |n: usize| Rc::new(Expr::Leaf(Arc::new(upload_i32(dev, q, &host[..n]))));
    let one = Rc::new(Expr::Scalar(1));
    let two = Rc::new(Expr::Scalar(2));
    let tree1 = |x: &Rc<Expr>| bin(Op::Add, x, &one);
    let tree3 = |x: &Rc<Expr>| bin(Op::Add, &bin(Op::Mul, x, x), &bin(Op::Mul, &two, x));
    // NOTE: output buffers are allocated once and reused, as the runtime's pool does; a fresh
    // buffer costs allocation plus a lazy zero-fill that no steady-state dispatch pays.
    let outs = [GpuBuffer::storage(dev, ElementKind::I32, N1), GpuBuffer::storage(dev, ElementKind::I32, N2)];
    let out = |n: usize| &outs[usize::from(n == N2)];

    // Cold cost of a new tree shape: one throwaway fold wakes the device and warms the shared
    // kernels, then the first run of an unseen shape minus its warm time is the compile.
    {
        let x = leaf(N1);
        gpu_fold(rt, &tree3(&x), N1, out(N1));
    }
    let x1 = leaf(N1);
    // NOTE: two unseen shapes; the cheaper compile is kept so one slow compile (a loaded host)
    // does not become every decision's fixed cost.
    let tree1b = |x: &Rc<Expr>| bin(Op::Sub, x, &one);
    let t = Instant::now();
    gpu_fold(rt, &tree1b(&x1), N1, out(N1));
    let cold_b = ms_of(t);
    let t = Instant::now();
    gpu_fold(rt, &tree1(&x1), N1, out(N1));
    let cold = ms_of(t);
    let warm1 = best(|| gpu_fold(rt, &tree1(&x1), N1, out(N1)));
    let cold = (cold - warm1).min(cold_b - warm1) + warm1;

    let resident = |tree: &dyn Fn(&Rc<Expr>) -> Rc<Expr>| {
        let (xa, xb) = (leaf(N1), leaf(N2));
        Pair {
            x1: N1 as f64, ms1: best_of(GPU_REPS, || gpu_fold(rt, &tree(&xa), N1, out(N1))),
            x2: N2 as f64, ms2: best_of(GPU_REPS, || gpu_fold(rt, &tree(&xb), N2, out(N2))),
        }
    };
    let gpu_fold1 = resident(&tree1);
    let gpu_fold3 = resident(&tree3);
    let gpu_up_fold1 = pair_of(GPU_REPS, N1, N2, |n| {
        let x = Rc::new(Expr::Leaf(Arc::new(upload_i32(dev, q, &host[..n]))));
        gpu_fold(rt, &tree1(&x), n, out(n));
    });
    let gpu_map_down = pair_of(GPU_REPS, N1, N2, |n| {
        let x = leaf(n);
        let o = out(n);
        {
            let mut cache = rt.cache.lock().unwrap_or_else(|e| e.into_inner());
            rbqn_gpu::expr::run(dev, q, &mut cache, &tree1(&x), n, o, None);
        }
        std::hint::black_box(pollster::block_on(download_i32(dev, q, o)));
    });
    let gpu_scan_all = pair_of(GPU_REPS, N1, N2, |n| {
        let b = upload_i32(dev, q, &host[..n]);
        let o = out(n);
        {
            let mut cache = rt.cache.lock().unwrap_or_else(|e| e.into_inner());
            rbqn_gpu::kernels::scan::inclusive_scan_into(dev, q, &mut cache, &b, o);
        }
        std::hint::black_box(pollster::block_on(download_i32(dev, q, o)));
    });
    // NOTE: sort sizes sit higher: a radix sort of 16k keys does not fill the device and the
    // host sort's passes leave the caches only around 1e5.
    let ks = keys(S2);
    let gpu_sort = pair(S1, S2, |n| {
        let b = upload_i32(dev, q, &ks[..n]);
        let s = {
            let mut cache = rt.cache.lock().unwrap_or_else(|e| e.into_inner());
            rbqn_gpu::kernels::sort::sort_i32_dir(dev, q, &mut cache, &b, false)
        };
        std::hint::black_box(pollster::block_on(download_i32(dev, q, &s)));
    });
    let mats: Vec<f64> = (0..256 * 256).map(|i| (i % 10) as f64).collect();
    let gpu_matmul = pair(128 * 128 * 128, 256 * 256 * 256, |macs| {
        let d = (macs as f64).cbrt().round() as usize;
        std::hint::black_box(matmul_f32(rt, &mats[..d * d], &mats[..d * d], d, d, d));
    });
    Samples {
        gpu_init_ms: rbqn_gpu::stats::DEVICE_INIT_US.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e3,
        gpu_cold_extra_ms: cold - warm1,
        gpu_fold1,
        gpu_fold3,
        gpu_up_fold1,
        gpu_map_down,
        gpu_scan_all,
        gpu_sort,
        gpu_matmul,
        // NOTE: the thread speedup is the same matmul sizes run as the runtime runs them.
        ..{
            let threads = rbqn_prim::par::threads();
            let mut c = rbqn_prim::par::serially(cpu_samples);
            if threads > 1 {
                c.cpu_mm_par = mm_par();
                c.threads = threads;
            }
            c
        }
    }
}

/// Host matmul at two sizes with the pool as currently set (serial inside `par::serially`).
fn mm_pair() -> Pair {
    let mm = |d: usize| -> Vec<i32> { (0..d * d).map(|i| (i % 10) as i32).collect() };
    let (m192, m288) = (mm(192), mm(288));
    let run = |m: &Vec<i32>, d: usize| {
        let w = rbqn_prim::matmul::Mat::I32(m);
        std::hint::black_box(rbqn_prim::matmul::matmul(w, w, d, d, d, false));
    };
    Pair { x1: 192f64.powi(3), ms1: best_of(3, || run(&m192, 192)), x2: 288f64.powi(3), ms2: best_of(3, || run(&m288, 288)) }
}

/// Host matmul as the runtime runs it, at one size big enough to keep every thread busy
/// (below ~300 the tasks are too small and the speedup reads as 1): a line through the origin.
fn mm_par() -> Pair {
    let d = 448;
    let m: Vec<i32> = (0..d * d).map(|i| (i % 10) as i32).collect();
    let w = rbqn_prim::matmul::Mat::I32(&m);
    let ms = best_of(3, || {
        std::hint::black_box(rbqn_prim::matmul::matmul(w, w, d, d, d, false));
    });
    Pair { x1: 0.0, ms1: 0.0, x2: (d * d * d) as f64, ms2: ms }
}

/// Host timings with every parallel path off (the per-element costs of the serial kernels; a
/// pool wake-up on a loaded host would dominate them), GPU fields left for `gpu_samples`.
fn cpu_samples() -> Samples {
    let data = small(C2);
    // NOTE: trees are built outside the timers; compiling one is part of what a decision costs.
    let leaves: Vec<Rc<Expr>> = [C1, C2]
        .iter()
        .map(|&n| {
            let arr = Arc::new(BqnArr::new_vec_i32(data[..n].to_vec()));
            Rc::new(Expr::Host(Rc::new(HostLeaf { arr, key: 0, bound: 4.0 })))
        })
        .collect();
    let idx = |n: usize| usize::from(n == C2);
    let plus1: Vec<Rc<Expr>> = leaves.iter().map(|l| bin(Op::Add, l, &Rc::new(Expr::Scalar(1)))).collect();
    let no_dev = &mut |_: &Arc<GpuBuffer>| -> gpu_host::Source { unreachable!("no device leaves") };
    let cpu_leaf = pair_of(5, C1, C2, |n| {
        std::hint::black_box(gpu_host::compile(&leaves[idx(n)], no_dev).fold(n, "add", false));
    });
    let cpu_fold1 = pair_of(5, C1, C2, |n| {
        std::hint::black_box(gpu_host::compile(&plus1[idx(n)], no_dev).fold(n, "add", false));
    });
    let cpu_mat1 = pair(C1, C2, |n| {
        std::hint::black_box(squeeze_i32(gpu_host::compile(&plus1[idx(n)], no_dev).eval_i32(n)));
    });
    let arrs: Vec<BqnArr> = [F1, F2].iter().map(|&n| BqnArr::new_vec_i32(data[..n].to_vec())).collect();
    let by_len = |n: usize| &arrs[usize::from(n == F2)];
    let cpu_fold = pair_of(5, F1, F2, |n| {
        std::hint::black_box(rbqn_vm::typed::fold_idx(rbqn_vm::typed::ADD, by_len(n), None));
    });
    // NOTE: the scan result is an i64 vector squeezed afterwards, so it too is timed where it
    // leaves the caches.
    let scan_arrs: Vec<BqnArr> = [C1, C2 / 2].iter().map(|&n| BqnArr::new_vec_i32(data[..n].to_vec())).collect();
    let cpu_scan = pair(C1, C2 / 2, |n| {
        std::hint::black_box(rbqn_vm::typed::scan_idx(rbqn_vm::typed::ADD, &scan_arrs[usize::from(n != C1)]));
    });
    let ks = keys(S2);
    let karrs: Vec<BqnArr> = [S1, S2].iter().map(|&n| BqnArr::new_vec_i32(ks[..n].to_vec())).collect();
    let cpu_sort = pair(S1, S2, |n| {
        std::hint::black_box(rbqn_prim::sort::sort_up_c1(B::m_i32(0), Some(&karrs[usize::from(n == S2)])).ok());
    });
    let cpu_mm_serial = mm_pair();
    let z = Pair { x1: 1.0, ms1: 0.0, x2: 2.0, ms2: 0.0 };
    Samples {
        gpu_init_ms: 0.0,
        gpu_cold_extra_ms: 0.0,
        gpu_fold1: z, gpu_fold3: z, gpu_up_fold1: z, gpu_map_down: z, gpu_scan_all: z, gpu_sort: z, gpu_matmul: z,
        cpu_fold, cpu_leaf, cpu_fold1, cpu_mat1, cpu_scan, cpu_sort, cpu_mm_serial, cpu_mm_par: cpu_mm_serial, threads: 1,
    }
}

/// Measure this machine. Returns the model and how long it took (ms).
pub fn measure(rt: &GpuRuntime) -> (Model, f64) {
    let t = Instant::now();
    let m = derive(&gpu_samples(rt));
    (m, ms_of(t))
}
