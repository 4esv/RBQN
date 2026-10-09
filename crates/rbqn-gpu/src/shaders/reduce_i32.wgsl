@group(0) @binding(0) var<storage, read> input: array<i32>;
@group(0) @binding(1) var<storage, read_write> output: array<i32>;

var<workgroup> shared_data: array<i32, 256>;

@compute @workgroup_size(256)
fn reduce_add_i32(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let wg = wid.y * nwg.x + wid.x; // linear workgroup index on the 2D grid
    let local_idx = lid.x;
    let idx = wg * 256u + local_idx;

    if (idx < arrayLength(&input)) {
        shared_data[local_idx] = input[idx];
    } else {
        shared_data[local_idx] = 0;
    }
    workgroupBarrier();

    for (var stride = 128u; stride > 0u; stride = stride >> 1u) {
        if (local_idx < stride) {
            shared_data[local_idx] = shared_data[local_idx] + shared_data[local_idx + stride];
        }
        workgroupBarrier();
    }

    // Spare workgroups on the last grid row must not write: an out-of-bounds
    // store is clamped onto the last element on Metal and races with its owner.
    if (local_idx == 0u && wg < arrayLength(&output)) {
        output[wg] = shared_data[0];
    }
}

@compute @workgroup_size(256)
fn reduce_mul_i32(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let wg = wid.y * nwg.x + wid.x; // linear workgroup index on the 2D grid
    let local_idx = lid.x;
    let idx = wg * 256u + local_idx;

    if (idx < arrayLength(&input)) {
        shared_data[local_idx] = input[idx];
    } else {
        shared_data[local_idx] = 1;
    }
    workgroupBarrier();

    for (var stride = 128u; stride > 0u; stride = stride >> 1u) {
        if (local_idx < stride) {
            shared_data[local_idx] = shared_data[local_idx] * shared_data[local_idx + stride];
        }
        workgroupBarrier();
    }

    // Spare workgroups on the last grid row must not write: an out-of-bounds
    // store is clamped onto the last element on Metal and races with its owner.
    if (local_idx == 0u && wg < arrayLength(&output)) {
        output[wg] = shared_data[0];
    }
}

@compute @workgroup_size(256)
fn reduce_min_i32(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let wg = wid.y * nwg.x + wid.x; // linear workgroup index on the 2D grid
    let local_idx = lid.x;
    let idx = wg * 256u + local_idx;

    if (idx < arrayLength(&input)) {
        shared_data[local_idx] = input[idx];
    } else {
        shared_data[local_idx] = 2147483647;
    }
    workgroupBarrier();

    for (var stride = 128u; stride > 0u; stride = stride >> 1u) {
        if (local_idx < stride) {
            shared_data[local_idx] = min(shared_data[local_idx], shared_data[local_idx + stride]);
        }
        workgroupBarrier();
    }

    // Spare workgroups on the last grid row must not write: an out-of-bounds
    // store is clamped onto the last element on Metal and races with its owner.
    if (local_idx == 0u && wg < arrayLength(&output)) {
        output[wg] = shared_data[0];
    }
}

@compute @workgroup_size(256)
fn reduce_max_i32(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let wg = wid.y * nwg.x + wid.x; // linear workgroup index on the 2D grid
    let local_idx = lid.x;
    let idx = wg * 256u + local_idx;

    if (idx < arrayLength(&input)) {
        shared_data[local_idx] = input[idx];
    } else {
        shared_data[local_idx] = -2147483648;
    }
    workgroupBarrier();

    for (var stride = 128u; stride > 0u; stride = stride >> 1u) {
        if (local_idx < stride) {
            shared_data[local_idx] = max(shared_data[local_idx], shared_data[local_idx + stride]);
        }
        workgroupBarrier();
    }

    // Spare workgroups on the last grid row must not write: an out-of-bounds
    // store is clamped onto the last element on Metal and races with its owner.
    if (local_idx == 0u && wg < arrayLength(&output)) {
        output[wg] = shared_data[0];
    }
}
