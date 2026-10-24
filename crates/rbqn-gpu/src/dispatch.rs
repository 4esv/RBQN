//! Dispatch decisions (Step 6). Elementwise ops build a lazy node for arrays
//! of at least `FLOOR` elements (building is free); whether the tree then runs
//! on the GPU or the host is decided where work happens (fold, scan,
//! materialize) by comparing `gpu_ms` with `cpu_ms`. The constants below were
//! refitted in #146 from bench/wow/gpu.bqn measurements on the machine's GPU
//! (the earlier M3 Pro fit is bench/thresholds/fit.json); the method is in the comment above them.

/// Smallest array (elements) a hook turns into a lazy node in default mode.
pub const FLOOR: usize = 65_536;
/// Sort/grade of wide-range keys (the CPU counting sort handles the rest, see
/// gpu_runtime::sort_on_gpu; i32 keys are then decided by `sort_gpu_ms` / `sort_cpu_ms`, which
/// say CPU on the Radeon). Measured force / off (ms over the same program
/// without the sort, 2026-10-09, M3 Pro, keys `(2⋆31)|48271×↕n`): 1e6 19/11, 2e6
/// 22/21, 3e6 29/36, 1e7 60/122. Force has device init started at entry;
/// default mode starts it at the first large array, so ~5 ms more is exposed
/// at the sort and the gate sits above the 2-3e6 crossover.
pub const SORT_MIN: usize = 4_000_000;

// Constants (#146). Method: bench/wow/gpu.bqn, one process per run, median of 3 in-process times
// ("t="), `RBQN_GPU=off` for the CPU side and `RBQN_GPU=force` for the GPU side, at 1e6/1e7/1e8
// elements (matmul 1000/2000/4096). Slopes between 1e7 and 1e8 give the per-element terms,
// the 1e6 run the fixed cost. On the AMD Radeon Pro 5300M (Metal), 2026-10-24, load ~4:
//   mode `fold`   +´x            gpu 5.8/56.5/544 ms   cpu 0.14/1.1/10.2 ms  = 4 up + map
//   mode `fold1`  +´x+1          gpu 9.3/68.6/707 ms   cpu 0.48/4.7/46 ms    = 4 up + map + 1 op
//   mode `fold2`  +´x+y          gpu  -/134/1252 ms    cpu  -/6.7/75 ms      fold2 - fold1 = one more upload
//   pipeline      +´(x×x)+2×x    gpu 8.9/67.8/744 ms   cpu 0.77/6.4/74 ms    ops=3
//   mode `map`    (x+1)[¯1]      gpu 10.6/105/1094 ms  cpu 2.4/31/304 ms     map - fold1 = download
//   mode `scan`   +`x [¯1]       gpu 11.0/106/1090 ms  cpu 6.1/74/708 ms
// The fit before this (bench/thresholds/fit.json, M3 Pro) put the GPU 5-20x below these
// measurements on this device (upload 0.12 ns/B against 1.4 here) and the host evaluator
// 2-10x too fast (output write 0.25 ns against 2.4), so the model sent work to the GPU that
// loses by 10x. Predictions with the constants below are within 1.3x of every row above.
// Per element / byte in ns, fixed in ms.
/// Submit + wait + scalar readback + compile of a new tree shape (fold1 1e6 intercept).
pub const GPU_FIXED_MS: f64 = 2.0;
/// Fused map+reduce, one op, input read, output written (fold1 slope 7.1 - 4 up).
pub const GPU_MAP_NS: f64 = 1.5;
/// Each further fused op ((pipeline 7.5 - fold1 7.1) / 2).
pub const GPU_OP_NS: f64 = 0.2;
/// Host to device (fold2 - fold1 = 5.45 ns per 4 B element).
pub const GPU_UP_NS_PER_B: f64 = 1.4;
/// Device to host ((map - fold1) / 4 B).
pub const GPU_DOWN_NS_PER_B: f64 = 1.0;
/// Scan kernel on top of upload and download (scan slope 10.9 - 4 up - 4 down).
pub const GPU_SCAN_NS: f64 = 1.6;
pub const GPU_SCAN_FIXED_MS: f64 = 0.0;
/// Device creation on the init thread (8.1-8.6 ms over 32 runs on the M3 Pro; the Radeon's
/// `init=` summary reads 14.7-19.4 ms, under 2.3x, so the M3 figure is kept); the part not
/// done yet when a decision is made is exposed.
pub const GPU_INIT_MS: f64 = 8.3;

// CPU, per element in ns.
/// Host evaluator: one op over a block (fold1 0.46 vs pipeline 0.74 per element over 2 ops).
pub const CPU_OP_NS: f64 = 0.14;
/// Host evaluator: reading one leaf plus the fold (fold1 0.46 - one op).
pub const CPU_LEAF_NS: f64 = 0.32;
/// Writing a materialized result + squeeze (map 3.0 ns - op - leaf).
pub const CPU_OUT_NS: f64 = 2.4;
/// Typed fold of a host array (fold 0.10-0.14; the M3 figure 0.07 is within 2x and kept).
pub const CPU_FOLD_NS: f64 = 0.07;
/// Typed `+` scan of a host array (scan 6.1/7.4/7.1 ns).
pub const CPU_SCAN_NS: f64 = 7.0;

// Sort of i32 keys (`∧`, mode `sort`, 1e6/1e7/1e8 random values below 2⋆31, same method).
// gpu 34.6/290/3380 ms (force: upload, 24 dispatches, download), cpu 18.2/209/2458 ms (radix).
// The GPU never wins here, so the default path keeps the host sort.
/// GPU sort per element, upload and download included (3380 ms / 1e8).
pub const GPU_SORT_NS: f64 = 34.0;
/// Host radix sort per element (2458 ms / 1e8 at the top, 18 ns at 1e6).
pub const CPU_SORT_NS: f64 = 22.0;

// Matmul (`•math.MatMul`, mode `matmul`, entries below 10 so the f32 kernel is exact).
// gpu 68/543/4673 ms for 1000/2000/4096 = 0.067-0.068 ns per multiply-add at every size.
// cpu (blocked packed kernel of #166, `RBQN_THREADS=1`, `RBQN_GPU=off`): 1000/2000/4096 took
// 102/864/8807 ms = 0.102/0.108/0.128 ns per multiply-add (the step up is the packed right operand
// leaving L2). With t threads the same runs took 55/786/5199 ms at 2, 31/297/3183 ms at 4 and
// 28/190/2202 ms at 12, a speedup of about t^0.8 (1.7/3.0/7.3 predicted for 2/4/12); the machine
// was shared (load 6-9 on 12 cores), so this under-states an idle host.
// The old naive loop was 1.46/4.45/10.8 ns per multiply-add; against it the GPU always won, now it
// only wins where the host has few threads.
/// GPU per multiply-add, transfers and the f64/f32 conversions included.
pub const GPU_MATMUL_NS_PER_MAC: f64 = 0.067;
/// Host per multiply-add on one thread (the 4096² figure; smaller sizes are up to 20% faster).
pub const CPU_MATMUL_NS_PER_MAC: f64 = 0.128;
/// Host multicore speedup exponent: `threads ^ 0.8`.
pub const CPU_MATMUL_THREAD_EXP: f64 = 0.8;

/// What a decision point is about to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Fold of a tree (fused map+reduce on GPU, fused block fold on host).
    Fold,
    /// Fold of a plain host array (CPU side is the typed fold).
    FoldPlain,
    /// Scan of a tree or array.
    Scan,
    /// Evaluate a tree to a resident buffer (no download).
    Map,
    /// Evaluate a tree for a host consumer (download on the GPU path).
    Materialize,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Fold | Kind::FoldPlain => "fold",
            Kind::Scan => "scan",
            Kind::Map => "map",
            Kind::Materialize => "materialize",
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Work {
    pub n: usize,
    /// Binary nodes in the tree (0 for a bare leaf).
    pub ops: usize,
    /// Leaves read per element (host, device, iota).
    pub leaves: usize,
    /// Bytes of host leaves the GPU path must upload.
    pub up_bytes: usize,
    /// Bytes of device leaves the CPU path must download.
    pub dev_bytes: usize,
    /// Bytes per output element (4 or 8).
    pub out_elem_bytes: usize,
}

pub fn gpu_ms(k: Kind, w: &Work, exposed_init_ms: f64) -> f64 {
    let n = w.n as f64;
    let map = if w.ops == 0 && matches!(k, Kind::Scan) {
        0.0
    } else {
        n * (GPU_MAP_NS + GPU_OP_NS * w.ops.saturating_sub(1) as f64)
    };
    let mut ns = map + w.up_bytes as f64 * GPU_UP_NS_PER_B;
    match k {
        // NOTE: a scan result is assumed read back (a following fold refuses
        // anything over 2^63 and most consumers are host primitives); W4
        // `⌈´ +` a` measured 28 ms forced vs 24 ms off.
        Kind::Scan => ns += n * (GPU_SCAN_NS + w.out_elem_bytes as f64 * GPU_DOWN_NS_PER_B) + GPU_SCAN_FIXED_MS * 1e6,
        Kind::Materialize => ns += n * w.out_elem_bytes as f64 * GPU_DOWN_NS_PER_B,
        _ => {}
    }
    exposed_init_ms + GPU_FIXED_MS + ns * 1e-6
}

/// Modelled GPU time of an i32 sort of `n` keys.
pub fn sort_gpu_ms(n: usize, exposed_init_ms: f64) -> f64 {
    exposed_init_ms + GPU_FIXED_MS + n as f64 * GPU_SORT_NS * 1e-6
}

/// Modelled host time of an i32 sort of `n` keys.
pub fn sort_cpu_ms(n: usize) -> f64 {
    n as f64 * CPU_SORT_NS * 1e-6
}

/// Modelled GPU time of an m×k by k×n matmul.
pub fn matmul_gpu_ms(m: usize, k: usize, n: usize, exposed_init_ms: f64) -> f64 {
    exposed_init_ms + GPU_FIXED_MS + (m as f64) * (k as f64) * (n as f64) * GPU_MATMUL_NS_PER_MAC * 1e-6
}

/// Modelled host time of an m×k by k×n matmul on `threads` threads.
pub fn matmul_cpu_ms(m: usize, k: usize, n: usize, threads: usize) -> f64 {
    let ns = CPU_MATMUL_NS_PER_MAC / (threads.max(1) as f64).powf(CPU_MATMUL_THREAD_EXP);
    (m as f64) * (k as f64) * (n as f64) * ns * 1e-6
}

pub fn cpu_ms(k: Kind, w: &Work) -> f64 {
    let n = w.n as f64;
    let mut ns = w.dev_bytes as f64 * GPU_DOWN_NS_PER_B;
    let eval = n * (CPU_OP_NS * w.ops as f64 + CPU_LEAF_NS * w.leaves as f64);
    ns += match k {
        Kind::FoldPlain => n * CPU_FOLD_NS,
        Kind::Fold => eval,
        Kind::Materialize | Kind::Map => eval + n * CPU_OUT_NS,
        // The typed scan reads a host array; a tree is materialized first.
        Kind::Scan => n * CPU_SCAN_NS + if w.ops > 0 { eval + n * CPU_OUT_NS } else { 0.0 },
    };
    ns * 1e-6
}

/// Per-op gate applied when a hook is first called: `FLOOR` for elementwise,
/// fold and scan (the cost model decides later), `SORT_MIN` for sort/grade.
pub fn should_use_gpu(op: &str, len: usize) -> bool {
    len >= if op == "sort" { SORT_MIN } else { FLOOR }
}

pub fn workgroup_count(len: usize, workgroup_size: u32) -> u32 {
    (len as u32).div_ceil(workgroup_size)
}

/// Per-dimension workgroup cap (WebGPU default and Metal's limit).
pub const MAX_WORKGROUPS_PER_DIM: u32 = 65535;

/// Split `groups` linear workgroups into an (x, y) grid that respects
/// `MAX_WORKGROUPS_PER_DIM`. Shaders recover the linear workgroup index as
/// `wid.y * num_workgroups.x + wid.x`.
/// BUG(fixed, #10): a 1D grid failed validation above 65535 groups, i.e. for
/// every array over 16.7M elements, so arith/reduce/sort never ran on the GPU.
pub fn workgroup_grid(groups: u32) -> (u32, u32) {
    if groups <= MAX_WORKGROUPS_PER_DIM {
        (groups.max(1), 1)
    } else {
        (MAX_WORKGROUPS_PER_DIM, groups.div_ceil(MAX_WORKGROUPS_PER_DIM))
    }
}

/// (x, y) workgroup grid covering `len` elements at `workgroup_size` each.
pub fn grid_for(len: usize, workgroup_size: u32) -> (u32, u32) {
    workgroup_grid(workgroup_count(len, workgroup_size))
}

pub const WORKGROUP_SIZE: u32 = 256;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_covers_all_groups_within_cap() {
        for groups in [0u32, 1, 255, 65535, 65536, 131070, 390625, 1 << 22] {
            let (x, y) = workgroup_grid(groups);
            assert!((1..=MAX_WORKGROUPS_PER_DIM).contains(&x), "x={x} for {groups}");
            assert!((1..=MAX_WORKGROUPS_PER_DIM).contains(&y), "y={y} for {groups}");
            let covered = x as u64 * y as u64;
            assert!(covered >= groups as u64, "{groups} groups, grid {x}x{y}");
            // Never a whole spare row: y is the minimum that covers.
            let spare = covered - (groups as u64).max(1);
            assert!(spare < x as u64, "{groups} groups, grid {x}x{y} has a spare row");
        }
    }

    #[test]
    fn grid_for_100m_elements_is_2d() {
        let (x, y) = grid_for(100_000_000, WORKGROUP_SIZE);
        assert_eq!((x, y), (65535, 6));
    }
}
