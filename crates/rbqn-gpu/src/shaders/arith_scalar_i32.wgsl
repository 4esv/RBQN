@group(0) @binding(0) var<storage, read> input_a: array<i32>;
@group(0) @binding(1) var<uniform> scalar_b: i32;
@group(0) @binding(2) var<storage, read_write> output: array<i32>;

@compute @workgroup_size(256)
fn scalar_add_i32(@builtin(global_invocation_id) id: vec3<u32>) {
    let idx = id.x;
    if (idx < arrayLength(&output)) {
        output[idx] = input_a[idx] + scalar_b;
    }
}

@compute @workgroup_size(256)
fn scalar_sub_i32(@builtin(global_invocation_id) id: vec3<u32>) {
    let idx = id.x;
    if (idx < arrayLength(&output)) {
        output[idx] = input_a[idx] - scalar_b;
    }
}

@compute @workgroup_size(256)
fn scalar_mul_i32(@builtin(global_invocation_id) id: vec3<u32>) {
    let idx = id.x;
    if (idx < arrayLength(&output)) {
        output[idx] = input_a[idx] * scalar_b;
    }
}

@compute @workgroup_size(256)
fn scalar_div_i32(@builtin(global_invocation_id) id: vec3<u32>) {
    let idx = id.x;
    if (idx < arrayLength(&output)) {
        output[idx] = input_a[idx] / scalar_b;
    }
}
