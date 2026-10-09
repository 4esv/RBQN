@group(0) @binding(0) var<storage, read> input_a: array<f32>;
@group(0) @binding(1) var<storage, read> input_b: array<f32>;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(256)
fn add_f32(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let idx = id.x + id.y * nwg.x * 256u; // 2D grid, see dispatch::workgroup_grid
    if (idx < arrayLength(&output)) {
        output[idx] = input_a[idx] + input_b[idx];
    }
}

@compute @workgroup_size(256)
fn sub_f32(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let idx = id.x + id.y * nwg.x * 256u; // 2D grid, see dispatch::workgroup_grid
    if (idx < arrayLength(&output)) {
        output[idx] = input_a[idx] - input_b[idx];
    }
}

@compute @workgroup_size(256)
fn mul_f32(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let idx = id.x + id.y * nwg.x * 256u; // 2D grid, see dispatch::workgroup_grid
    if (idx < arrayLength(&output)) {
        output[idx] = input_a[idx] * input_b[idx];
    }
}

@compute @workgroup_size(256)
fn div_f32(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let idx = id.x + id.y * nwg.x * 256u; // 2D grid, see dispatch::workgroup_grid
    if (idx < arrayLength(&output)) {
        output[idx] = input_a[idx] / input_b[idx];
    }
}
