// Template: @IN@ is i64 (i64 x i64 -> i64) or i32 (widening, i32 x i32 -> i64);
// @COMB@ is an expression over a, b: i64. Filled in by kernels/arith_i64.rs.
@group(0) @binding(0) var<storage, read> input_a: array<@IN@>;
@group(0) @binding(1) var<storage, read> input_b: array<@IN@>;
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
        output[idx] = comb(i64(input_a[idx]), i64(input_b[idx]));
    }
}
