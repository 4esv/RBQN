use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};

use rbqn_core::array::{ArrData, BqnArr, squeeze_num};
use rbqn_core::B;
use rbqn_gpu::buffer::{ElementKind, GpuBuffer, upload_i32, upload_f32, download_i32, download_f32};
use rbqn_gpu::context::GpuContext;
use rbqn_gpu::fusion::{FusedOp, FusionBuilder};
use rbqn_gpu::pipeline::PipelineCache;

// NOTE: Singleton GPU runtime — initialized lazily on first qualifying dispatch.
pub struct GpuRuntime {
    pub ctx: GpuContext,
    pub cache: Mutex<PipelineCache>,
}

static GPU_RUNTIME: OnceLock<Option<GpuRuntime>> = OnceLock::new();
static GPU_DISABLED: AtomicBool = AtomicBool::new(false);
static GPU_DEBUG: AtomicBool = AtomicBool::new(false);

/// Record CLI GPU settings. Called from main.rs after CLI parse.
/// The device itself is created lazily by `get()` on the first dispatch that
/// passes its size threshold, so startup never pays for adapter/device setup.
pub fn init(no_gpu: bool) {
    if std::env::var("RBQN_GPU_DEBUG").is_ok() {
        GPU_DEBUG.store(true, Ordering::Relaxed);
    }
    if no_gpu {
        GPU_DISABLED.store(true, Ordering::Relaxed);
    }
}

/// Get the GPU runtime, creating it on first call. None when disabled
/// (`--no-gpu`) or when no adapter is available.
/// NOTE: Callers must apply their size threshold before calling this.
pub fn get() -> Option<&'static GpuRuntime> {
    if GPU_DISABLED.load(Ordering::Relaxed) {
        return None;
    }
    GPU_RUNTIME
        .get_or_init(|| {
            if debug_enabled() {
                eprintln!("[gpu] initializing device on thread {:?}", std::thread::current().name());
            }
            let ctx = pollster::block_on(GpuContext::new())?;
            let cache = Mutex::new(PipelineCache::new(ctx.device.clone()));
            Some(GpuRuntime { ctx, cache })
        })
        .as_ref()
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
        // Threshold and safety checks (before get(): it initializes the device)
        if !should_dispatch("arith", w_arr.ia()) { return None; }
        if !gpu_safe_arr(w_arr) || !gpu_safe_arr(x_arr) { return None; }
        if w_arr.shape != x_arr.shape { return None; }
        let gpu = get()?;

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
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        gpu_fused_arith_inner(ops, a, b)
    })).unwrap_or_else(|_| {
        if debug_enabled() {
            eprintln!("[gpu] fused_arith GPU error — falling back to CPU");
        }
        None
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
    if !should_dispatch("reduce", arr.ia()) {
        return None;
    }
    if !gpu_safe_arr(arr) {
        return None;
    }
    let gpu = get()?;
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
    if !should_dispatch("scan", arr.ia()) {
        return None;
    }
    if !gpu_safe_arr(arr) {
        return None;
    }
    let gpu = get()?;
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
        let xa = rbqn_vm::vm::get_arr(x)?;
        if xa.rank() != 1 { return None; }
        let n = xa.ia();

        // NOTE: Dispatch threshold — softmax GPU overhead only pays off for larger arrays.
        if n < 256 { return None; }
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
    })).unwrap_or_else(|_| {
        if debug_enabled() {
            eprintln!("[gpu] sort GPU error — falling back to CPU");
        }
        None
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
