// Per-digit histogram: count occurrences of each radix digit
@group(0) @binding(0) var<storage, read> keys_in: array<u32>;
@group(0) @binding(1) var<storage, read_write> histograms: array<atomic<u32>>;
@group(0) @binding(2) var<uniform> params: vec2<u32>; // (shift, n)

@compute @workgroup_size(256)
fn histogram(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    let n = params.y;
    if (idx >= n) { return; }

    let shift = params.x;
    let digit = (keys_in[idx] >> shift) & 0xFu;
    atomicAdd(&histograms[digit], 1u);
}

// Scatter keys based on prefix-summed histograms
@group(0) @binding(0) var<storage, read> scatter_keys_in: array<u32>;
@group(0) @binding(1) var<storage, read_write> scatter_keys_out: array<u32>;
@group(0) @binding(2) var<storage, read_write> offsets: array<atomic<u32>>;
@group(0) @binding(3) var<uniform> scatter_params: vec2<u32>;

@compute @workgroup_size(256)
fn scatter(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    let n = scatter_params.y;
    if (idx >= n) { return; }

    let shift = scatter_params.x;
    let digit = (scatter_keys_in[idx] >> shift) & 0xFu;
    let dest = atomicAdd(&offsets[digit], 1u);
    scatter_keys_out[dest] = scatter_keys_in[idx];
}
