const GPU_THRESHOLD: usize = 100_000;

pub fn should_use_gpu(op: &str, len: usize) -> bool {
    len >= threshold_for(op)
}

fn threshold_for(op: &str) -> usize {
    match op {
        "sort" => GPU_THRESHOLD * 5,
        "scan" | "reduce" => GPU_THRESHOLD / 2,
        _ => GPU_THRESHOLD,
    }
}

pub fn workgroup_count(len: usize, workgroup_size: u32) -> u32 {
    ((len as u32) + workgroup_size - 1) / workgroup_size
}

pub const WORKGROUP_SIZE: u32 = 256;
