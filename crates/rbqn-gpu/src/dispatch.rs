//! Dispatch decisions (Step 6). Elementwise ops build a lazy node for arrays
//! of at least `FLOOR` elements (building is free); whether the tree then runs
//! on the GPU or the host is decided where work happens (fold, scan,
//! materialize) by comparing `gpu_ms` with `cpu_ms`. Both read a per-machine `Model` that
//! `calibrate` measures at first device use (#167); the method that validates it is
//! bench/wow/gpu.bqn (one process per run, median of 3 in-process times, `RBQN_GPU=off|force`).

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

// The cost model's constants are per machine: `crate::calibrate` measures them at first device
// use (or reads them from the cache keyed by adapter and driver), see `Model`. Per element and
// byte in ns, fixed in ms. Nothing here is a fitted constant.
/// The measured constants of one machine (#167). Field docs say which measurement they come from
/// (`calibrate::derive`); the cost functions below are the only readers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Model {
    /// Device creation on the init thread (ms); the part not done yet at a decision is exposed.
    pub gpu_init_ms: f64,
    /// Submit + wait + scalar readback + compile of a new tree shape (ms).
    pub gpu_fixed_ms: f64,
    /// Fused map+reduce, one op, input read, output written.
    pub gpu_map_ns: f64,
    /// Each further fused op.
    pub gpu_op_ns: f64,
    /// Host to device, per byte.
    pub gpu_up_ns_per_b: f64,
    /// Device to host, per byte.
    pub gpu_down_ns_per_b: f64,
    /// Scan kernel on top of upload and download.
    pub gpu_scan_ns: f64,
    /// GPU sort per element, upload and download included.
    pub gpu_sort_ns: f64,
    /// GPU per multiply-add (the transfers are charged separately, per element).
    pub gpu_matmul_ns_per_mac: f64,
    /// Host evaluator: one op over a block.
    pub cpu_op_ns: f64,
    /// Host evaluator: reading one leaf plus the fold.
    pub cpu_leaf_ns: f64,
    /// Writing a materialized result + squeeze.
    pub cpu_out_ns: f64,
    /// Typed fold of a host array.
    pub cpu_fold_ns: f64,
    /// Typed `+` scan of a host array.
    pub cpu_scan_ns: f64,
    /// Host radix sort per element.
    pub cpu_sort_ns: f64,
    /// Host per multiply-add on one thread.
    pub cpu_matmul_ns_per_mac: f64,
    /// Host multicore speedup exponent: `threads ^ exp`.
    pub cpu_matmul_thread_exp: f64,
}

/// Field names in a fixed order, for the cache file and the debug print.
pub const MODEL_FIELDS: [&str; 17] = [
    "gpu_init_ms", "gpu_fixed_ms", "gpu_map_ns", "gpu_op_ns", "gpu_up_ns_per_b", "gpu_down_ns_per_b",
    "gpu_scan_ns", "gpu_sort_ns", "gpu_matmul_ns_per_mac", "cpu_op_ns", "cpu_leaf_ns", "cpu_out_ns",
    "cpu_fold_ns", "cpu_scan_ns", "cpu_sort_ns", "cpu_matmul_ns_per_mac", "cpu_matmul_thread_exp",
];

impl Model {
    pub fn values(&self) -> [f64; 17] {
        [
            self.gpu_init_ms, self.gpu_fixed_ms, self.gpu_map_ns, self.gpu_op_ns, self.gpu_up_ns_per_b,
            self.gpu_down_ns_per_b, self.gpu_scan_ns, self.gpu_sort_ns, self.gpu_matmul_ns_per_mac,
            self.cpu_op_ns, self.cpu_leaf_ns, self.cpu_out_ns, self.cpu_fold_ns, self.cpu_scan_ns,
            self.cpu_sort_ns, self.cpu_matmul_ns_per_mac, self.cpu_matmul_thread_exp,
        ]
    }
    pub fn from_values(v: [f64; 17]) -> Model {
        Model {
            gpu_init_ms: v[0], gpu_fixed_ms: v[1], gpu_map_ns: v[2], gpu_op_ns: v[3], gpu_up_ns_per_b: v[4],
            gpu_down_ns_per_b: v[5], gpu_scan_ns: v[6], gpu_sort_ns: v[7], gpu_matmul_ns_per_mac: v[8],
            cpu_op_ns: v[9], cpu_leaf_ns: v[10], cpu_out_ns: v[11], cpu_fold_ns: v[12], cpu_scan_ns: v[13],
            cpu_sort_ns: v[14], cpu_matmul_ns_per_mac: v[15], cpu_matmul_thread_exp: v[16],
        }
    }
}

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

pub fn gpu_ms(m: &Model, k: Kind, w: &Work, exposed_init_ms: f64) -> f64 {
    let n = w.n as f64;
    let map = if w.ops == 0 && matches!(k, Kind::Scan) {
        0.0
    } else {
        n * (m.gpu_map_ns + m.gpu_op_ns * w.ops.saturating_sub(1) as f64)
    };
    let mut ns = map + w.up_bytes as f64 * m.gpu_up_ns_per_b;
    match k {
        // NOTE: a scan result is assumed read back (a following fold refuses
        // anything over 2^63 and most consumers are host primitives); W4
        // `⌈´ +` a` measured 28 ms forced vs 24 ms off.
        Kind::Scan => ns += n * (m.gpu_scan_ns + w.out_elem_bytes as f64 * m.gpu_down_ns_per_b),
        Kind::Materialize => ns += n * w.out_elem_bytes as f64 * m.gpu_down_ns_per_b,
        _ => {}
    }
    exposed_init_ms + m.gpu_fixed_ms + ns * 1e-6
}

/// Modelled GPU time of an i32 sort of `n` keys.
pub fn sort_gpu_ms(m: &Model, n: usize, exposed_init_ms: f64) -> f64 {
    exposed_init_ms + m.gpu_fixed_ms + n as f64 * m.gpu_sort_ns * 1e-6
}

/// Modelled host time of an i32 sort of `n` keys.
pub fn sort_cpu_ms(m: &Model, n: usize) -> f64 {
    n as f64 * m.cpu_sort_ns * 1e-6
}

/// Modelled GPU time of an m×k by k×n matmul.
pub fn matmul_gpu_ms(c: &Model, m: usize, k: usize, n: usize, exposed_init_ms: f64) -> f64 {
    let (m, k, n) = (m as f64, k as f64, n as f64);
    // Operands go up and the result comes back as f32; the multiply-adds are the rest.
    let transfer_ns = (m * k + k * n) * 4.0 * c.gpu_up_ns_per_b + m * n * 4.0 * c.gpu_down_ns_per_b;
    exposed_init_ms + c.gpu_fixed_ms + (m * k * n * c.gpu_matmul_ns_per_mac + transfer_ns) * 1e-6
}

/// Modelled host time of an m×k by k×n matmul on `threads` threads.
pub fn matmul_cpu_ms(c: &Model, m: usize, k: usize, n: usize, threads: usize) -> f64 {
    let ns = c.cpu_matmul_ns_per_mac / (threads.max(1) as f64).powf(c.cpu_matmul_thread_exp);
    (m as f64) * (k as f64) * (n as f64) * ns * 1e-6
}

pub fn cpu_ms(m: &Model, k: Kind, w: &Work) -> f64 {
    let n = w.n as f64;
    let mut ns = w.dev_bytes as f64 * m.gpu_down_ns_per_b;
    let eval = n * (m.cpu_op_ns * w.ops as f64 + m.cpu_leaf_ns * w.leaves as f64);
    ns += match k {
        Kind::FoldPlain => n * m.cpu_fold_ns,
        Kind::Fold => eval,
        Kind::Materialize | Kind::Map => eval + n * m.cpu_out_ns,
        // The typed scan reads a host array; a tree is materialized first.
        Kind::Scan => n * m.cpu_scan_ns + if w.ops > 0 { eval + n * m.cpu_out_ns } else { 0.0 },
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
