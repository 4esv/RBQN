@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(256)
fn exp_f32(@builtin(global_invocation_id) id: vec3<u32>) {
    let idx = id.x;
    if (idx < arrayLength(&output)) {
        output[idx] = exp(input[idx]);
    }
}

@compute @workgroup_size(256)
fn sqrt_f32(@builtin(global_invocation_id) id: vec3<u32>) {
    let idx = id.x;
    if (idx < arrayLength(&output)) {
        output[idx] = sqrt(input[idx]);
    }
}

@compute @workgroup_size(256)
fn neg_f32(@builtin(global_invocation_id) id: vec3<u32>) {
    let idx = id.x;
    if (idx < arrayLength(&output)) {
        output[idx] = -input[idx];
    }
}

@compute @workgroup_size(256)
fn abs_f32(@builtin(global_invocation_id) id: vec3<u32>) {
    let idx = id.x;
    if (idx < arrayLength(&output)) {
        output[idx] = abs(input[idx]);
    }
}
