@group(0) @binding(0) var<storage, read> input_a: array<i32>;
@group(0) @binding(1) var<storage, read> input_b: array<i32>;
@group(0) @binding(2) var<storage, read_write> output: array<i32>;

@compute @workgroup_size(256)
fn add_i32(@builtin(global_invocation_id) id: vec3<u32>) {
    let idx = id.x;
    if (idx < arrayLength(&output)) {
        output[idx] = input_a[idx] + input_b[idx];
    }
}

@compute @workgroup_size(256)
fn sub_i32(@builtin(global_invocation_id) id: vec3<u32>) {
    let idx = id.x;
    if (idx < arrayLength(&output)) {
        output[idx] = input_a[idx] - input_b[idx];
    }
}

@compute @workgroup_size(256)
fn mul_i32(@builtin(global_invocation_id) id: vec3<u32>) {
    let idx = id.x;
    if (idx < arrayLength(&output)) {
        output[idx] = input_a[idx] * input_b[idx];
    }
}

@compute @workgroup_size(256)
fn div_i32(@builtin(global_invocation_id) id: vec3<u32>) {
    let idx = id.x;
    if (idx < arrayLength(&output)) {
        output[idx] = input_a[idx] / input_b[idx];
    }
}
