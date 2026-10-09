// Template: @IN@ is i64 or i32 (widening); @COMB@ / @ID@ define the monoid.
// Reduce-then-scan, workgroupBarrier only. A tile is 256 threads x 4 = 1024
// elements. Loads and stores are coalesced (thread t touches base + t + k*256)
// and staged through workgroup memory so each thread scans 4 contiguous
// elements serially; Hillis-Steele (ping-pong) over the 256 thread totals.
//   block_sum: sums[wg] = comb over the tile.
//   scan_block: output = comb(carry, local inclusive scan), carry = sums[wg-1]
//   where sums is the inclusive scan of the block totals.
// Every var<workgroup> slot is written by all threads before it is read, so
// workgroup zero-initialization is disabled for this module (pipeline.rs).
@group(0) @binding(0) var<storage, read> input: array<@IN@>;
@group(0) @binding(1) var<storage, read_write> output: array<i64>;
@group(0) @binding(2) var<storage, read_write> sums: array<i64>;

const EPT: u32 = @EPT@u;
const TILE: u32 = @TILE@u;

var<workgroup> tile: array<i64, @TILE@>;
var<workgroup> hs: array<i64, 512>;

fn comb(a: i64, b: i64) -> i64 {
    return @COMB@;
}

@compute @workgroup_size(256)
fn block_sum(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let wg = wid.y * nwg.x + wid.x;
    let t = lid.x;
    let n = arrayLength(&input);
    let base = wg * TILE + t;
    var acc: i64 = @ID@;
    for (var k = 0u; k < EPT; k = k + 1u) {
        let i = base + k * 256u;
        if (i < n) { acc = comb(acc, i64(input[i])); }
    }
    hs[t] = acc;
    workgroupBarrier();
    for (var s = 128u; s > 0u; s = s >> 1u) {
        if (t < s) { hs[t] = comb(hs[t], hs[t + s]); }
        workgroupBarrier();
    }
    // Spare workgroups on the last grid row must not write.
    if (t == 0u && wg < arrayLength(&sums)) { sums[wg] = hs[0]; }
}

@compute @workgroup_size(256)
fn scan_block(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let wg = wid.y * nwg.x + wid.x;
    let t = lid.x;
    let n = arrayLength(&input);
    let base = wg * TILE;

    for (var k = 0u; k < EPT; k = k + 1u) {
        let i = base + k * 256u + t;
        var x: i64 = @ID@;
        if (i < n) { x = i64(input[i]); }
        tile[k * 256u + t] = x;
    }
    workgroupBarrier();

    var v: array<i64, @EPT@>;
    var run: i64 = @ID@;
    for (var k = 0u; k < EPT; k = k + 1u) {
        run = comb(run, tile[t * EPT + k]);
        v[k] = run;
    }

    hs[t] = run;
    workgroupBarrier();
    var src = 0u;
    for (var off = 1u; off < 256u; off = off << 1u) {
        var x = hs[src + t];
        if (t >= off) { x = comb(hs[src + t - off], x); }
        let dst = 256u - src;
        hs[dst + t] = x;
        workgroupBarrier();
        src = dst;
    }
    var pre: i64 = @ID@;
    if (t > 0u) { pre = hs[src + t - 1u]; }
    if (wg > 0u) { pre = comb(sums[wg - 1u], pre); }

    for (var k = 0u; k < EPT; k = k + 1u) {
        tile[t * EPT + k] = comb(pre, v[k]);
    }
    workgroupBarrier();
    for (var k = 0u; k < EPT; k = k + 1u) {
        let i = base + k * 256u + t;
        if (i < n) { output[i] = tile[k * 256u + t]; }
    }
}
