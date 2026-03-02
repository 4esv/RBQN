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

pub const WORKGROUP_SIZE: u32 = 256;
