// NOTE: Benchmarked on Apple Silicon (M-series) 2026-02-28.
// GPU dispatch overhead (Metal) is ~1.5ms regardless of array size.
// CPU is faster for all sizes tested (50K-500K).
// Estimated crossover from linear extrapolation:
//   reduce:  ~40M elements (CPU 22µs at 500K → 1.76ms at 40M)
//   arith:   ~14M elements (CPU 54µs at 500K → 1.51ms at 14M)
//   scan:    ~5M elements  (CPU 182µs at 500K → 1.82ms at 5M)
//   sort:    unknown (not benchmarked at large sizes)
// Thresholds set conservatively at ~2x estimated crossover.
const GPU_THRESHOLD_ARITH: usize = 30_000_000;
const GPU_THRESHOLD_REDUCE: usize = 80_000_000;
const GPU_THRESHOLD_SCAN: usize = 10_000_000;
const GPU_THRESHOLD_SORT: usize = 100_000_000;

pub fn should_use_gpu(op: &str, len: usize) -> bool {
    len >= threshold_for(op)
}

fn threshold_for(op: &str) -> usize {
    match op {
        "sort" => GPU_THRESHOLD_SORT,
        "scan" | "reduce" => {
            if op == "scan" {
                GPU_THRESHOLD_SCAN
            } else {
                GPU_THRESHOLD_REDUCE
            }
        }
        _ => GPU_THRESHOLD_ARITH,
    }
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
