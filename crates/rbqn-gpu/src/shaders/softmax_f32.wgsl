// NOTE: Single-pass softmax for vectors up to 256 elements (one workgroup).
// Algorithm: exp(x - max(x)) / sum(exp(x - max(x)))
// Uses two shared arrays: one for max-reduction, one for exp values.

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;

var<workgroup> shared_vals: array<f32, 256>;
var<workgroup> shared_exp: array<f32, 256>;

@compute @workgroup_size(256)
fn softmax_f32(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
) {
    let idx = gid.x;
    let local_idx = lid.x;
    let n = arrayLength(&input);

    // Step 1: Load input into shared memory, pad out-of-bounds with -FLT_MAX.
    if (idx < n) {
        shared_vals[local_idx] = input[idx];
    } else {
        shared_vals[local_idx] = -3.40282347e+38;
    }
    workgroupBarrier();

    // Step 2: Parallel tree reduction to find max.
    for (var stride = 128u; stride > 0u; stride = stride >> 1u) {
        if (local_idx < stride) {
            shared_vals[local_idx] = max(shared_vals[local_idx], shared_vals[local_idx + stride]);
        }
        workgroupBarrier();
    }
    let max_val = shared_vals[0];
    workgroupBarrier();

    // Step 3: Compute exp(x - max) for each element, store in shared_exp.
    if (idx < n) {
        shared_exp[local_idx] = exp(input[idx] - max_val);
    } else {
        shared_exp[local_idx] = 0.0;
    }
    workgroupBarrier();

    // Step 4: Copy exp values into shared_vals for sum reduction.
    shared_vals[local_idx] = shared_exp[local_idx];
    workgroupBarrier();

    // Step 5: Parallel tree reduction to find sum.
    for (var stride = 128u; stride > 0u; stride = stride >> 1u) {
        if (local_idx < stride) {
            shared_vals[local_idx] = shared_vals[local_idx] + shared_vals[local_idx + stride];
        }
        workgroupBarrier();
    }
    let sum_val = shared_vals[0];

    // Step 6: Write normalised probability to output.
    if (idx < n) {
        output[idx] = shared_exp[local_idx] / sum_val;
    }
}
