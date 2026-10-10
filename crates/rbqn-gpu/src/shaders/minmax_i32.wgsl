// (min, max) of an i32 array. Pass 1 (minmax_first) reads values, later passes
// (minmax_pairs) read the interleaved (min, max) pairs from the previous pass.
// Output is interleaved pairs, one per workgroup. 1024 inputs per workgroup.
@group(0) @binding(0) var<storage, read> input: array<i32>;
@group(0) @binding(1) var<storage, read_write> output: array<i32>;

var<workgroup> smin: array<i32, 256>;
var<workgroup> smax: array<i32, 256>;

fn finish(t: u32, wg: u32, lo0: i32, hi0: i32) {
    smin[t] = lo0;
    smax[t] = hi0;
    workgroupBarrier();
    for (var stride = 128u; stride > 0u; stride = stride >> 1u) {
        if (t < stride) {
            smin[t] = min(smin[t], smin[t + stride]);
            smax[t] = max(smax[t], smax[t + stride]);
        }
        workgroupBarrier();
    }
    if (t == 0u && wg * 2u + 1u < arrayLength(&output)) {
        output[wg * 2u] = smin[0];
        output[wg * 2u + 1u] = smax[0];
    }
}

@compute @workgroup_size(256)
fn minmax_first(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let wg = wid.y * nwg.x + wid.x;
    let t = lid.x;
    let n = arrayLength(&input);
    var lo: i32 = 2147483647;
    var hi: i32 = -2147483647 - 1;
    for (var k = 0u; k < 4u; k = k + 1u) {
        let idx = wg * 1024u + t + k * 256u;
        if (idx < n) {
            lo = min(lo, input[idx]);
            hi = max(hi, input[idx]);
        }
    }
    finish(t, wg, lo, hi);
}

@compute @workgroup_size(256)
fn minmax_pairs(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let wg = wid.y * nwg.x + wid.x;
    let t = lid.x;
    let n = arrayLength(&input) / 2u;
    var lo: i32 = 2147483647;
    var hi: i32 = -2147483647 - 1;
    for (var k = 0u; k < 4u; k = k + 1u) {
        let idx = wg * 1024u + t + k * 256u;
        if (idx < n) {
            lo = min(lo, input[idx * 2u]);
            hi = max(hi, input[idx * 2u + 1u]);
        }
    }
    finish(t, wg, lo, hi);
}
