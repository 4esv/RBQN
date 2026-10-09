//! Dispatch decisions (Step 6). Elementwise ops build a lazy node for arrays
//! of at least `FLOOR` elements (building is free); whether the tree then runs
//! on the GPU or the host is decided where work happens (fold, scan,
//! materialize) by comparing `gpu_ms` with `cpu_ms`. Constants are fitted by
//! bench/thresholds/fit.py on an M3 Pro (wgpu 24 / Metal); the run they come
//! from is bench/thresholds/fit.json.

/// Smallest array (elements) a hook turns into a lazy node in default mode.
pub const FLOOR: usize = 65_536;
/// Sort/grade. Measured force / off (ms): 1e5 8.9/7.8, 3e5 15.2/13.4,
/// 1e6 25.0/31.1, 3e6 33.1/57.0, 1e7 65.4/232.2. Force never sorts below 1M on
/// the GPU (small-sort HACK in gpu_runtime), so the 1e5/3e5 force rows are the
/// CPU plus init; the crossover is in (3e5, 1e6], × 1.5 = 1.5M. At 1M the
/// README row `+´ (⊢ ⍋⊸⊏ ⊢) 1000000⥊3‿1‿2` went 17 → 30 ms on the GPU (CPU grade
/// of 3 distinct values is cheap), so 1M itself stays on the CPU.
pub const SORT_MIN: usize = 1_500_000;

// GPU (fit.json, 2026-10-09, M3 Pro). Per element / byte in ns, fixed in ms.
/// Submit + wait + scalar readback + compile of a new tree shape (fold intercept).
pub const GPU_FIXED_MS: f64 = 0.7;
/// Fused map+reduce, one op, output written (fold1 slope 0.141).
pub const GPU_MAP_NS: f64 = 0.14;
/// Each further fused op ((fold4 - fold1) / 3 measured -0.001..0.013: noise).
pub const GPU_OP_NS: f64 = 0.01;
pub const GPU_UP_NS_PER_B: f64 = 0.12;
pub const GPU_DOWN_NS_PER_B: f64 = 0.15;
/// i64 scan + its extra passes (scan - fused fold slope 0.356, intercept 1.46 ms).
pub const GPU_SCAN_NS: f64 = 0.36;
pub const GPU_SCAN_FIXED_MS: f64 = 1.5;
/// Device creation on the init thread (8.1-8.6 ms over 32 runs, one 15 ms
/// outlier); the part not done yet when a decision is made is exposed.
pub const GPU_INIT_MS: f64 = 8.3;

// CPU, per element in ns.
/// Host evaluator: one op over a block (h_fold4 - h_fold0 = 0.048/op).
pub const CPU_OP_NS: f64 = 0.05;
/// Host evaluator: reading one leaf (array or iota) plus the fold (h_fold0 0.150).
pub const CPU_LEAF_NS: f64 = 0.15;
/// Writing a materialized result + squeeze (h_mat1 0.454 - op - leaf).
pub const CPU_OUT_NS: f64 = 0.25;
/// Typed fold of a host array (c_fold 0.072).
pub const CPU_FOLD_NS: f64 = 0.07;
/// Typed `+` scan of a host array (c_scan 1.551).
pub const CPU_SCAN_NS: f64 = 1.55;

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
            assert!(x >= 1 && x <= MAX_WORKGROUPS_PER_DIM, "x={x} for {groups}");
            assert!(y >= 1 && y <= MAX_WORKGROUPS_PER_DIM, "y={y} for {groups}");
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
