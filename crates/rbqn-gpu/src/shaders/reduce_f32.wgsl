@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;

var<workgroup> shared_data: array<f32, 256>;

@compute @workgroup_size(256)
fn reduce_add_f32(
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
        shared_data[local_idx] = 0.0;
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
fn reduce_mul_f32(
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
        shared_data[local_idx] = 1.0;
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
fn reduce_min_f32(
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
        shared_data[local_idx] = 3.40282347e+38;
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
fn reduce_max_f32(
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
        shared_data[local_idx] = -3.40282347e+38;
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
