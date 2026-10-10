@group(0) @binding(0) var<storage, read> input_a: array<i32>;
@group(0) @binding(1) var<uniform> scalar_b: i32;
@group(0) @binding(2) var<storage, read_write> output: array<i32>;

@compute @workgroup_size(256)
fn scalar_add_i32(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let idx = id.x + id.y * nwg.x * 256u; // 2D grid, see dispatch::workgroup_grid
    if (idx < arrayLength(&output)) {
        output[idx] = input_a[idx] + scalar_b;
    }
}

@compute @workgroup_size(256)
fn scalar_sub_i32(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let idx = id.x + id.y * nwg.x * 256u; // 2D grid, see dispatch::workgroup_grid
    if (idx < arrayLength(&output)) {
        output[idx] = input_a[idx] - scalar_b;
    }
}

@compute @workgroup_size(256)
fn scalar_mul_i32(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let idx = id.x + id.y * nwg.x * 256u; // 2D grid, see dispatch::workgroup_grid
    if (idx < arrayLength(&output)) {
        output[idx] = input_a[idx] * scalar_b;
    }
}

@compute @workgroup_size(256)
fn scalar_div_i32(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let idx = id.x + id.y * nwg.x * 256u; // 2D grid, see dispatch::workgroup_grid
    if (idx < arrayLength(&output)) {
        output[idx] = input_a[idx] / scalar_b;
    }
}

@compute @workgroup_size(256)
fn scalar_rsub_i32(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let idx = id.x + id.y * nwg.x * 256u; // 2D grid, see dispatch::workgroup_grid
    if (idx < arrayLength(&output)) {
        output[idx] = scalar_b - input_a[idx];
    }
}

@compute @workgroup_size(256)
fn scalar_min_i32(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let idx = id.x + id.y * nwg.x * 256u; // 2D grid, see dispatch::workgroup_grid
    if (idx < arrayLength(&output)) {
        output[idx] = min(input_a[idx], scalar_b);
    }
}

@compute @workgroup_size(256)
fn scalar_max_i32(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let idx = id.x + id.y * nwg.x * 256u; // 2D grid, see dispatch::workgroup_grid
    if (idx < arrayLength(&output)) {
        output[idx] = max(input_a[idx], scalar_b);
    }
}
