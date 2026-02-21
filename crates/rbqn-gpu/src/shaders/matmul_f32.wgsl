// NOTE: Tiled matrix multiply. Tile size = 16x16.
// C[row, col] = sum_k A[row, k] * B[k, col]
// Dimensions: A is M x K, B is K x N, C is M x N.

struct Params {
    M: u32,
    N: u32,
    K: u32,
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> mat_a: array<f32>;
@group(0) @binding(1) var<storage, read> mat_b: array<f32>;
@group(0) @binding(2) var<storage, read_write> mat_c: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

const TILE: u32 = 16u;

var<workgroup> tile_a: array<f32, 256>; // 16 * 16
var<workgroup> tile_b: array<f32, 256>; // 16 * 16

@compute @workgroup_size(16, 16)
fn matmul_f32(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
) {
    let row = gid.y;
    let col = gid.x;
    let local_row = lid.y;
    let local_col = lid.x;

    if (row >= params.M || col >= params.N) {
        return;
    }

    var acc: f32 = 0.0;
    let num_tiles = (params.K + TILE - 1u) / TILE;

    for (var t = 0u; t < num_tiles; t = t + 1u) {
        // Load tile of A into shared memory: A[row, t*TILE + local_col]
        let a_col = t * TILE + local_col;
        if (a_col < params.K) {
            tile_a[local_row * TILE + local_col] = mat_a[row * params.K + a_col];
        } else {
            tile_a[local_row * TILE + local_col] = 0.0;
        }

        // Load tile of B into shared memory: B[t*TILE + local_row, col]
        let b_row = t * TILE + local_row;
        if (b_row < params.K) {
            tile_b[local_row * TILE + local_col] = mat_b[b_row * params.N + col];
        } else {
            tile_b[local_row * TILE + local_col] = 0.0;
        }

        workgroupBarrier();

        // Accumulate dot product for this tile
        for (var k = 0u; k < TILE; k = k + 1u) {
            acc = acc + tile_a[local_row * TILE + k] * tile_b[k * TILE + local_col];
        }

        workgroupBarrier();
    }

    mat_c[row * params.N + col] = acc;
}
