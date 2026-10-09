// Template: @IN@ is i64 or i32 (widening); @COMB@ is an expression over a, b: i64;
// @ARGS@ is "v, s" (array op scalar) or "s, v" (scalar op array).
// Filled in by kernels/arith_i64.rs (arith_scalar_i64).
@group(0) @binding(0) var<storage, read> input_a: array<@IN@>;
@group(0) @binding(1) var<uniform> scalar_b: i32;
@group(0) @binding(2) var<storage, read_write> output: array<i64>;

fn comb(a: i64, b: i64) -> i64 {
    return @COMB@;
}

@compute @workgroup_size(256)
fn main(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let idx = id.x + id.y * nwg.x * 256u; // 2D grid, see dispatch::workgroup_grid
    if (idx < arrayLength(&output)) {
        let v = i64(input_a[idx]);
        let s = i64(scalar_b);
        output[idx] = comb(@ARGS@);
    }
}
