use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};

use rbqn_core::array::{ArrData, BqnArr, squeeze_num};
use rbqn_core::B;
use rbqn_gpu::buffer::{GpuBuffer, upload_i32, download_i32};
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
