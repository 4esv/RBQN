@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<storage, read_write> block_sums: array<f32>;

var<workgroup> temp: array<f32, 512>;

@compute @workgroup_size(256)
fn scan_add_f32(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
) {
    let n = arrayLength(&input);
    let idx = gid.x;
    let local_idx = lid.x;

    // Load into shared memory (Blelloch scan uses 2x elements per workgroup)
    let ai = local_idx;
    let bi = local_idx + 256u;

    let a_idx = wid.x * 512u + ai;
    let b_idx = wid.x * 512u + bi;

    if (a_idx < n) { temp[ai] = input[a_idx]; } else { temp[ai] = 0.0; }
    if (b_idx < n) { temp[bi] = input[b_idx]; } else { temp[bi] = 0.0; }

    // Up-sweep (reduce)
    var offset = 1u;
    for (var d = 256u; d > 0u; d = d >> 1u) {
        workgroupBarrier();
        if (local_idx < d) {
            let ai2 = offset * (2u * local_idx + 1u) - 1u;
            let bi2 = offset * (2u * local_idx + 2u) - 1u;
            temp[bi2] = temp[bi2] + temp[ai2];
        }
        offset = offset << 1u;
    }

    // Store block sum and clear last element
    if (local_idx == 0u) {
        block_sums[wid.x] = temp[511];
        temp[511] = 0.0;
    }

    // Down-sweep
    for (var d = 1u; d <= 256u; d = d << 1u) {
        offset = offset >> 1u;
        workgroupBarrier();
        if (local_idx < d) {
            let ai2 = offset * (2u * local_idx + 1u) - 1u;
            let bi2 = offset * (2u * local_idx + 2u) - 1u;
            let t = temp[ai2];
            temp[ai2] = temp[bi2];
            temp[bi2] = temp[bi2] + t;
        }
    }
    workgroupBarrier();

    // Write exclusive scan results
    if (a_idx < n) { output[a_idx] = temp[ai]; }
    if (b_idx < n) { output[b_idx] = temp[bi]; }
}

@group(0) @binding(0) var<storage, read> scan_data: array<f32>;
@group(0) @binding(1) var<storage, read_write> scan_output: array<f32>;
@group(0) @binding(2) var<storage, read> prefix_sums: array<f32>;

@compute @workgroup_size(256)
fn propagate_f32(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
) {
    let idx = gid.x;
    if (idx < arrayLength(&scan_output)) {
        scan_output[idx] = scan_data[idx] + prefix_sums[wid.x];
    }
}
