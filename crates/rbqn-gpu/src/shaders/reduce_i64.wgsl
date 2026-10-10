// Template: @IN@ is i64 or i32 (widening first pass); @COMB@ / @ID@ define the
// monoid over i64. Each workgroup reduces 1024 elements (4 per thread, strided
// so loads coalesce). Two-level tree, no inter-workgroup communication.
@group(0) @binding(0) var<storage, read> input: array<@IN@>;
@group(0) @binding(1) var<storage, read_write> output: array<i64>;

var<workgroup> shared_data: array<i64, 256>;

fn comb(a: i64, b: i64) -> i64 {
    return @COMB@;
}

@compute @workgroup_size(256)
fn main(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let wg = wid.y * nwg.x + wid.x; // linear workgroup index on the 2D grid
    let t = lid.x;
    let n = arrayLength(&input);
    let base = wg * 1024u + t;

    var acc: i64 = @ID@;
    for (var k = 0u; k < 4u; k = k + 1u) {
        let idx = base + k * 256u;
        if (idx < n) {
            acc = comb(acc, i64(input[idx]));
        }
    }
    shared_data[t] = acc;
    workgroupBarrier();

    for (var stride = 128u; stride > 0u; stride = stride >> 1u) {
        if (t < stride) {
            shared_data[t] = comb(shared_data[t], shared_data[t + stride]);
        }
        workgroupBarrier();
    }

    // Spare workgroups on the last grid row must not write.
    if (t == 0u && wg < arrayLength(&output)) {
        output[wg] = shared_data[0];
    }
}
