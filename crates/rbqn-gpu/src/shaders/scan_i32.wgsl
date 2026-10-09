// i32/u32 scan, reduce-then-scan, barriers only (no inter-workgroup waits).
// A tile is 256 threads x 8 elements = 2048. Loads and stores are coalesced
// (thread t touches base + t + k*256) and staged through workgroup memory so
// each thread can scan its 8 contiguous elements serially.
//   block_sum: sums[wg] = sum of the tile.
//   scan_incl / scan_excl: output = carry + local scan, where
//   carry = sums[wg-1] and sums is the inclusive scan of the block sums.
// Every var<workgroup> slot is written by all threads before it is read, so
// zero-initialization is disabled for this module (pipeline.rs).
@group(0) @binding(0) var<storage, read> input: array<i32>;
@group(0) @binding(1) var<storage, read_write> output: array<i32>;
@group(0) @binding(2) var<storage, read_write> sums: array<i32>;

const EPT: u32 = 8u;
const TILE: u32 = 2048u;

var<workgroup> tile: array<i32, 2048>;
var<workgroup> hs: array<i32, 512>;

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
    var acc: i32 = 0;
    for (var k = 0u; k < EPT; k = k + 1u) {
        let i = base + k * 256u;
        if (i < n) { acc = acc + input[i]; }
    }
    hs[t] = acc;
    workgroupBarrier();
    for (var s = 128u; s > 0u; s = s >> 1u) {
        if (t < s) { hs[t] = hs[t] + hs[t + s]; }
        workgroupBarrier();
    }
    // Spare workgroups on the last grid row must not write.
    if (t == 0u && wg < arrayLength(&sums)) { sums[wg] = hs[0]; }
}

fn scan_tile(t: u32, wg: u32, exclusive: bool) {
    let n = arrayLength(&input);
    let base = wg * TILE;
    // coalesced load
    for (var k = 0u; k < EPT; k = k + 1u) {
        let i = base + k * 256u + t;
        var x: i32 = 0;
        if (i < n) { x = input[i]; }
        tile[k * 256u + t] = x;
    }
    workgroupBarrier();

    // serial inclusive scan of this thread's 8 elements
    var v: array<i32, 8>;
    var run: i32 = 0;
    for (var k = 0u; k < EPT; k = k + 1u) {
        run = run + tile[t * EPT + k];
        v[k] = run;
    }

    // Hillis-Steele over the 256 thread totals, ping-pong, one barrier per step
    hs[t] = run;
    workgroupBarrier();
    var src = 0u;
    for (var off = 1u; off < 256u; off = off << 1u) {
        var x = hs[src + t];
        if (t >= off) { x = x + hs[src + t - off]; }
        let dst = 256u - src;
        hs[dst + t] = x;
        workgroupBarrier();
        src = dst;
    }
    var pre: i32 = 0;
    if (t > 0u) { pre = hs[src + t - 1u]; }
    if (wg > 0u) { pre = pre + sums[wg - 1u]; }

    var prev: i32 = 0;
    for (var k = 0u; k < EPT; k = k + 1u) {
        tile[t * EPT + k] = pre + select(v[k], prev, exclusive);
        prev = v[k];
    }
    workgroupBarrier();
    for (var k = 0u; k < EPT; k = k + 1u) {
        let i = base + k * 256u + t;
        if (i < n) { output[i] = tile[k * 256u + t]; }
    }
}

@compute @workgroup_size(256)
fn scan_incl(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    scan_tile(lid.x, wid.y * nwg.x + wid.x, false);
}

@compute @workgroup_size(256)
fn scan_excl(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    scan_tile(lid.x, wid.y * nwg.x + wid.x, true);
}
