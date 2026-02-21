@group(0) @binding(0) var<storage, read> data: array<u32>;
@group(0) @binding(1) var<storage, read> indices: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;

@compute @workgroup_size(256)
fn gather_u32(@builtin(global_invocation_id) id: vec3<u32>) {
    let idx = id.x;
    if (idx < arrayLength(&output)) {
        output[idx] = data[indices[idx]];
    }
}
