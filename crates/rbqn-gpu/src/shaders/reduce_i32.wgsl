// i32/u32 reduce: each workgroup reduces 1024 elements (4 per thread, strided
// so loads coalesce). Every thread writes shared_data[t] before it is read, so
// workgroup zero-initialization is disabled for this module (pipeline.rs).
@group(0) @binding(0) var<storage, read> input: array<i32>;
@group(0) @binding(1) var<storage, read_write> output: array<i32>;

var<workgroup> shared_data: array<i32, 256>;

@compute @workgroup_size(256)
fn reduce_add_i32(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let wg = wid.y * nwg.x + wid.x; // linear workgroup index on the 2D grid
    let t = lid.x;
    let n = arrayLength(&input);
    let base = wg * 1024u + t;

    var acc: i32 = 0;
    for (var k = 0u; k < 4u; k = k + 1u) {
        let idx = base + k * 256u;
        if (idx < n) {
            acc = acc + input[idx];
        }
    }
    shared_data[t] = acc;
    workgroupBarrier();

    for (var stride = 128u; stride > 0u; stride = stride >> 1u) {
        if (t < stride) {
            shared_data[t] = shared_data[t] + shared_data[t + stride];
        }
        workgroupBarrier();
    }

    // Spare workgroups on the last grid row must not write: an out-of-bounds
    // store is clamped onto the last element on Metal and races with its owner.
    if (t == 0u && wg < arrayLength(&output)) {
        output[wg] = shared_data[0];
    }
}

@compute @workgroup_size(256)
fn reduce_mul_i32(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let wg = wid.y * nwg.x + wid.x; // linear workgroup index on the 2D grid
    let t = lid.x;
    let n = arrayLength(&input);
    let base = wg * 1024u + t;

    var acc: i32 = 1;
    for (var k = 0u; k < 4u; k = k + 1u) {
        let idx = base + k * 256u;
        if (idx < n) {
            acc = acc * input[idx];
        }
    }
    shared_data[t] = acc;
    workgroupBarrier();

    for (var stride = 128u; stride > 0u; stride = stride >> 1u) {
        if (t < stride) {
            shared_data[t] = shared_data[t] * shared_data[t + stride];
        }
        workgroupBarrier();
    }

    // Spare workgroups on the last grid row must not write: an out-of-bounds
    // store is clamped onto the last element on Metal and races with its owner.
    if (t == 0u && wg < arrayLength(&output)) {
        output[wg] = shared_data[0];
    }
}

@compute @workgroup_size(256)
fn reduce_min_i32(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let wg = wid.y * nwg.x + wid.x; // linear workgroup index on the 2D grid
    let t = lid.x;
    let n = arrayLength(&input);
    let base = wg * 1024u + t;

    var acc: i32 = 2147483647;
    for (var k = 0u; k < 4u; k = k + 1u) {
        let idx = base + k * 256u;
        if (idx < n) {
            acc = min(acc, input[idx]);
        }
    }
    shared_data[t] = acc;
    workgroupBarrier();

    for (var stride = 128u; stride > 0u; stride = stride >> 1u) {
        if (t < stride) {
            shared_data[t] = min(shared_data[t], shared_data[t + stride]);
        }
        workgroupBarrier();
    }

    // Spare workgroups on the last grid row must not write: an out-of-bounds
    // store is clamped onto the last element on Metal and races with its owner.
    if (t == 0u && wg < arrayLength(&output)) {
        output[wg] = shared_data[0];
    }
}

@compute @workgroup_size(256)
fn reduce_max_i32(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let wg = wid.y * nwg.x + wid.x; // linear workgroup index on the 2D grid
    let t = lid.x;
    let n = arrayLength(&input);
    let base = wg * 1024u + t;

    var acc: i32 = -2147483648;
    for (var k = 0u; k < 4u; k = k + 1u) {
        let idx = base + k * 256u;
        if (idx < n) {
            acc = max(acc, input[idx]);
        }
    }
    shared_data[t] = acc;
    workgroupBarrier();

    for (var stride = 128u; stride > 0u; stride = stride >> 1u) {
        if (t < stride) {
            shared_data[t] = max(shared_data[t], shared_data[t + stride]);
        }
        workgroupBarrier();
    }

    // Spare workgroups on the last grid row must not write: an out-of-bounds
    // store is clamped onto the last element on Metal and races with its owner.
    if (t == 0u && wg < arrayLength(&output)) {
        output[wg] = shared_data[0];
    }
}
