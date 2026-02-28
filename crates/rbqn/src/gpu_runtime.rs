use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};

use rbqn_core::array::{ArrData, BqnArr, squeeze_num};
use rbqn_core::B;
use rbqn_gpu::buffer::{ElementKind, GpuBuffer, upload_i32, upload_f32, download_i32, download_f32};
use rbqn_gpu::context::GpuContext;
use rbqn_gpu::pipeline::PipelineCache;

// NOTE: Singleton GPU runtime — initialized once after CLI parse.
pub struct GpuRuntime {
    pub ctx: GpuContext,
    pub cache: Mutex<PipelineCache>,
}

static GPU_RUNTIME: OnceLock<Option<GpuRuntime>> = OnceLock::new();
static GPU_DISABLED: AtomicBool = AtomicBool::new(false);
static GPU_DEBUG: AtomicBool = AtomicBool::new(false);

/// Initialize GPU runtime. Called from main.rs after CLI parse.
/// If `no_gpu` is true, stores None and disables GPU dispatch.
/// If GPU adapter is unavailable, stores None silently.
pub fn init(no_gpu: bool) {
    let debug = std::env::var("RBQN_GPU_DEBUG").is_ok();
    if debug {
        GPU_DEBUG.store(true, Ordering::Relaxed);
    }

    if no_gpu {
        GPU_DISABLED.store(true, Ordering::Relaxed);
        GPU_RUNTIME.get_or_init(|| None);
        return;
    }

    let ctx_opt = pollster::block_on(GpuContext::new());
    let runtime_opt = ctx_opt.map(|ctx| {
        let cache = Mutex::new(PipelineCache::new(ctx.device.clone()));
        GpuRuntime { ctx, cache }
    });

    GPU_RUNTIME.get_or_init(|| runtime_opt);
}

/// Get the GPU runtime if available.
pub fn get() -> Option<&'static GpuRuntime> {
    GPU_RUNTIME.get()?.as_ref()
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
    let device = &gpu.ctx.device;
    let queue = &gpu.ctx.queue;

    let data_i32: Vec<i32> = match &arr.data {
        ArrData::I32(v) => v.clone(),
        ArrData::I8(v) => v.iter().map(|&x| x as i32).collect(),
        ArrData::I16(v) => v.iter().map(|&x| x as i32).collect(),
        ArrData::Bit(v) => {
            let ia = arr.ia();
            (0..ia).map(|i| ((v[i / 64] >> (i % 64)) & 1) as i32).collect()
        }
        ArrData::F64(v) => v.iter().map(|&x| x as i32).collect(),
        ArrData::C8(_) | ArrData::C16(_) | ArrData::C32(_) | ArrData::Boxed(_) => return None,
    };

    Some(upload_i32(device, queue, &data_i32))
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
    rbqn_gpu::dispatch::should_use_gpu(op, len)
}

/// GPU-accelerated binary arithmetic: w_arr op x_arr → result.
/// Returns None on any error (CPU fallback).
/// Registered as GPU_ARITH_HOOK in rbqn-prim at startup.
pub fn gpu_arith_binary(op: &str, w_arr: &BqnArr, x_arr: &BqnArr) -> Option<BqnArr> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let gpu = get()?;

        // Threshold and safety checks
        if !should_dispatch("arith", w_arr.ia()) { return None; }
        if !gpu_safe_arr(w_arr) || !gpu_safe_arr(x_arr) { return None; }
        if w_arr.shape != x_arr.shape { return None; }

        let device = &gpu.ctx.device;
        let queue = &gpu.ctx.queue;

        // Upload both arrays as i32
        let w_buf = arr_to_gpu_i32(gpu, w_arr)?;
        let x_buf = arr_to_gpu_i32(gpu, x_arr)?;
        let out_buf = GpuBuffer::storage(device, rbqn_gpu::buffer::ElementKind::I32, w_arr.ia());

        {
            let mut cache = gpu.cache.lock().ok()?;
            rbqn_gpu::kernels::arith::arith_binary(
                device,
                queue,
                &mut cache,
                op,
                &w_buf,
                &x_buf,
                &out_buf,
            );
        }

        let result = gpu_i32_to_arr(gpu, &out_buf, w_arr.shape.clone(), w_arr.fill);
        log_dispatch("arith", w_arr.ia(), "i32");
        Some(result)
    })).unwrap_or_else(|_| {
        if debug_enabled() {
            eprintln!("[gpu] arith GPU error — falling back to CPU");
        }
        None
    })
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

/// GPU fold (reduce) for large rank-1 numeric arrays.
/// Supports +, x, floor, ceil (prim_idx 0, 2, 6, 7).
/// Returns None for unsupported ops or if GPU dispatch is unavailable.
pub fn gpu_fold(f: B, arr: &BqnArr) -> Option<B> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| gpu_fold_inner(f, arr)))
        .unwrap_or_else(|_| {
            if debug_enabled() {
                eprintln!("[gpu] fold dispatch panicked — CPU fallback");
            }
            None
        })
}

fn gpu_fold_inner(f: B, arr: &BqnArr) -> Option<B> {
    let gpu = get()?;
    if !should_dispatch("reduce", arr.ia()) {
        return None;
    }
    if !gpu_safe_arr(arr) {
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
    let buf = arr_to_gpu_i32(gpu, arr)?;
    let result_buf = {
        let mut cache = gpu.cache.lock().unwrap_or_else(|e| e.into_inner());
        rbqn_gpu::kernels::reduce::reduce(&gpu.ctx.device, &gpu.ctx.queue, &mut cache, op, &buf)
    };
    let result_data =
        pollster::block_on(download_i32(&gpu.ctx.device, &gpu.ctx.queue, &result_buf));
    if result_data.is_empty() {
        return None;
    }
    log_dispatch(&format!("reduce_{op}"), arr.ia(), "i32");
    Some(B::m_f64(result_data[0] as f64))
}

/// GPU scan (inclusive prefix sum) for large rank-1 numeric arrays.
/// Only supports + (prim_idx 0) — the scan kernel implements prefix add.
/// Returns None for unsupported ops or if GPU dispatch is unavailable.
pub fn gpu_scan(f: B, arr: &BqnArr) -> Option<B> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| gpu_scan_inner(f, arr)))
        .unwrap_or_else(|_| {
            if debug_enabled() {
                eprintln!("[gpu] scan dispatch panicked — CPU fallback");
            }
            None
        })
}

fn gpu_scan_inner(f: B, arr: &BqnArr) -> Option<B> {
    let gpu = get()?;
    if !should_dispatch("scan", arr.ia()) {
        return None;
    }
    if !gpu_safe_arr(arr) {
        return None;
    }
    if arr.rank() != 1 {
        return None;
    }
    // NOTE: Only + (prim_idx 0) supported for scan (GPU kernel implements prefix add)
    match prim_idx_of(f)? {
        0 => {}
        _ => return None,
    }
    let buf = arr_to_gpu_i32(gpu, arr)?;
    let result_buf = {
        let mut cache = gpu.cache.lock().unwrap_or_else(|e| e.into_inner());
        rbqn_gpu::kernels::scan::inclusive_scan(&gpu.ctx.device, &gpu.ctx.queue, &mut cache, &buf)
    };
    let result_data =
        pollster::block_on(download_i32(&gpu.ctx.device, &gpu.ctx.queue, &result_buf));
    if result_data.len() != arr.ia() {
        return None;
    }
    log_dispatch("scan_add", arr.ia(), "i32");
    // Build result array and squeeze to smallest integer type
    let f64_data: Vec<f64> = result_data.iter().map(|&x| x as f64).collect();
    let f64_arr = BqnArr {
        shape: arr.shape.clone(),
        data: ArrData::F64(f64_data),
        fill: arr.fill,
    };
    Some(rbqn_vm::vm::tag_arr(squeeze_num(f64_arr)))
}

/// GPU-accelerated grade (⍋/⍒): returns permutation indices that sort the array.
/// Returns None on any error (CPU fallback).
/// Registered as GPU_GRADE_HOOK in rbqn-prim at startup.
pub fn gpu_grade(arr: &BqnArr, ascending: bool) -> Option<BqnArr> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let gpu = get()?;

        // Only dispatch rank-1 numeric arrays above threshold
        if arr.rank() != 1 { return None; }
        if !should_dispatch("sort", arr.ia()) { return None; }
        if !gpu_safe_arr(arr) { return None; }

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
    })).unwrap_or_else(|_| {
        if debug_enabled() {
            eprintln!("[gpu] grade GPU error — falling back to CPU");
        }
        None
    })
}

/// GPU-accelerated matrix multiply: w (M×K) •math.MatMul x (K×N) → result (M×N).
/// Returns None when GPU unavailable or data is invalid (CPU fallback).
/// Registered as GPU_MATMUL_HOOK in rbqn-vm::derive at startup.
pub fn gpu_matmul(w: B, x: B) -> Option<B> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let gpu = get()?;

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
        if m * k + k * n < 50_000 { return None; }

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
    })).unwrap_or_else(|_| {
        if debug_enabled() {
            eprintln!("[gpu] matmul GPU error — falling back to CPU");
        }
        None
    })
}

/// GPU-accelerated softmax: •math.Softmax x → probability distribution.
/// Returns None when GPU unavailable or data is invalid (CPU fallback).
/// Registered as GPU_SOFTMAX_HOOK in rbqn-vm::derive at startup.
pub fn gpu_softmax(x: B) -> Option<B> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let gpu = get()?;

        let xa = rbqn_vm::vm::get_arr(x)?;
        if xa.rank() != 1 { return None; }
        let n = xa.ia();

        // NOTE: Dispatch threshold — softmax GPU overhead only pays off for larger arrays.
        if n < 256 { return None; }

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
    })).unwrap_or_else(|_| {
        if debug_enabled() {
            eprintln!("[gpu] softmax GPU error — falling back to CPU");
        }
        None
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
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let gpu = get()?;

        // Only dispatch rank-1 numeric arrays above threshold
        if arr.rank() != 1 { return None; }
        if !should_dispatch("sort", arr.ia()) { return None; }
        if !gpu_safe_arr(arr) { return None; }

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
    })).unwrap_or_else(|_| {
        if debug_enabled() {
            eprintln!("[gpu] sort GPU error — falling back to CPU");
        }
        None
    })
}
