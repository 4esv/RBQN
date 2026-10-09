@group(0) @binding(0) var<storage, read> input: array<i32>;
@group(0) @binding(1) var<storage, read_write> output: array<i32>;
@group(0) @binding(2) var<storage, read_write> block_sums: array<i32>;

var<workgroup> temp: array<i32, 512>;

@compute @workgroup_size(256)
fn scan_add_i32(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let n = arrayLength(&input);
    let wg = wid.y * nwg.x + wid.x; // linear workgroup index on the 2D grid
    let local_idx = lid.x;

    let ai = local_idx;
    let bi = local_idx + 256u;

    let a_idx = wg * 512u + ai;
    let b_idx = wg * 512u + bi;

    if (a_idx < n) { temp[ai] = input[a_idx]; } else { temp[ai] = 0; }
    if (b_idx < n) { temp[bi] = input[b_idx]; } else { temp[bi] = 0; }

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

    if (local_idx == 0u) {
        // Spare workgroups on the last grid row must not write block_sums.
        if (wg < arrayLength(&block_sums)) { block_sums[wg] = temp[511]; }
        temp[511] = 0;
    }

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

    if (a_idx < n) { output[a_idx] = temp[ai]; }
    if (b_idx < n) { output[b_idx] = temp[bi]; }
}

@group(0) @binding(0) var<storage, read> scan_data: array<i32>;
@group(0) @binding(1) var<storage, read_write> scan_output: array<i32>;
@group(0) @binding(2) var<storage, read> prefix_sums: array<i32>;

@compute @workgroup_size(256)
fn propagate_i32(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let idx = gid.x + gid.y * nwg.x * 256u; // 2D grid, see dispatch::workgroup_grid
    if (idx < arrayLength(&scan_output)) {
        // NOTE: Each scan block covers 512 elements (ELEMENTS_PER_WORKGROUP).
        // Map the global element index to the correct block_sums entry.
        let block_idx = idx / 512u;
        scan_output[idx] = scan_data[idx] + prefix_sums[block_idx];
    }
}
