// Stable LSD radix sort, 4 bits per pass, 2048-element tiles.
// Per pass: hist (per-tile digit counts, digit-major) -> scan (one workgroup,
// exclusive, in place) -> scatter (workgroup-local stable ranking).
// No inter-workgroup forward progress is assumed anywhere.
//
// tile_counts layout: [digit * num_tiles + tile]. An exclusive scan over that
// order gives, for (digit, tile), the global output offset of the tile's first
// element with that digit; stability across tiles follows from tile order.

struct Params {
    shift: u32,
    n: u32,
    num_tiles: u32,
    load_mask: u32,   // xor applied to keys read from keys_in
    store_mask: u32,  // xor applied to keys written to keys_out
    flags: u32,       // bit0: carry index payload (grade), bit1: skip key store
    first: u32,       // 1 on the first pass: grade index = position
    pad1: u32,
}

@group(0) @binding(0) var<storage, read> keys_in: array<u32>;
@group(0) @binding(1) var<storage, read_write> keys_out: array<u32>;
@group(0) @binding(2) var<storage, read> idx_in: array<u32>;
@group(0) @binding(3) var<storage, read_write> idx_out: array<u32>;
@group(0) @binding(4) var<storage, read_write> tile_counts: array<u32>;
@group(0) @binding(5) var<uniform> p: Params;

const TILE: u32 = 2048u;
const EPT: u32 = 8u;

var<workgroup> tkeys: array<u32, 2048>;
var<workgroup> cnt: array<u32, 4096>;
var<workgroup> part: array<u32, 256>;
var<workgroup> base_of: array<u32, 16>;
var<workgroup> hist_sh: array<atomic<u32>, 16>;

@compute @workgroup_size(256)
fn hist(
    @builtin(local_invocation_index) li: u32,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let tile = wid.y * nwg.x + wid.x;
    if (tile >= p.num_tiles) { return; }
    if (li < 16u) { atomicStore(&hist_sh[li], 0u); }
    workgroupBarrier();
    for (var k = 0u; k < EPT; k++) {
        let pos = tile * TILE + k * 256u + li;
        if (pos < p.n) {
            let key = keys_in[pos] ^ p.load_mask;
            atomicAdd(&hist_sh[(key >> p.shift) & 15u], 1u);
        }
    }
    workgroupBarrier();
    if (li < 16u) {
        tile_counts[li * p.num_tiles + tile] = atomicLoad(&hist_sh[li]);
    }
}

// Exclusive scan of tile_counts[0 .. 16*num_tiles], in place, one workgroup.
@compute @workgroup_size(256)
fn scan(@builtin(local_invocation_index) li: u32) {
    let m = p.num_tiles * 16u;
    var carry = 0u;
    var seg = 0u;
    loop {
        if (seg >= m) { break; }
        let b = seg + li * 4u;
        var v: array<u32, 4>;
        var s = 0u;
        for (var i = 0u; i < 4u; i++) {
            var x = 0u;
            if (b + i < m) { x = tile_counts[b + i]; }
            v[i] = x;
            s += x;
        }
        part[li] = s;
        workgroupBarrier();
        var off = 1u;
        loop {
            if (off >= 256u) { break; }
            var x = 0u;
            if (li >= off) { x = part[li - off]; }
            workgroupBarrier();
            part[li] += x;
            workgroupBarrier();
            off = off << 1u;
        }
        let incl = part[li];
        let seg_total = part[255];
        var run = carry + incl - s;
        for (var i = 0u; i < 4u; i++) {
            if (b + i < m) { tile_counts[b + i] = run; }
            run += v[i];
        }
        carry += seg_total;
        workgroupBarrier();
        seg += 1024u;
    }
}

@compute @workgroup_size(256)
fn scatter(
    @builtin(local_invocation_index) li: u32,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let tile = wid.y * nwg.x + wid.x;
    if (tile >= p.num_tiles) { return; }
    let tbase = tile * TILE;

    // Coalesced load into shared memory.
    for (var k = 0u; k < EPT; k++) {
        let pos = tbase + k * 256u + li;
        var key = 0u;
        if (pos < p.n) { key = keys_in[pos] ^ p.load_mask; }
        tkeys[k * 256u + li] = key;
    }
    if (li < 16u) { base_of[li] = tile_counts[li * p.num_tiles + tile]; }
    for (var d = 0u; d < 16u; d++) { cnt[d * 256u + li] = 0u; }
    workgroupBarrier();

    // Thread li owns tile positions li*8 .. li*8+8 (contiguous, in order).
    let mine = li * EPT;
    for (var j = 0u; j < EPT; j++) {
        if (tbase + mine + j < p.n) {
            let d = (tkeys[mine + j] >> p.shift) & 15u;
            cnt[d * 256u + li] += 1u;
        }
    }
    workgroupBarrier();

    // Per-digit exclusive scan over the 256 threads: digit d = li/16, lane = li%16.
    let d0 = li >> 4u;
    let cb = d0 * 256u + (li & 15u) * 16u;
    var sum = 0u;
    for (var i = 0u; i < 16u; i++) { sum += cnt[cb + i]; }
    part[li] = sum;
    workgroupBarrier();
    if ((li & 15u) == 0u) {
        var r = 0u;
        for (var i = 0u; i < 16u; i++) {
            let v = part[li + i];
            part[li + i] = r;
            r += v;
        }
    }
    workgroupBarrier();
    var run = part[li];
    for (var i = 0u; i < 16u; i++) {
        let v = cnt[cb + i];
        cnt[cb + i] = run;
        run += v;
    }
    workgroupBarrier();

    let grade = (p.flags & 1u) != 0u;
    let skip_keys = (p.flags & 2u) != 0u;
    for (var j = 0u; j < EPT; j++) {
        let pos = tbase + mine + j;
        if (pos < p.n) {
            let key = tkeys[mine + j];
            let d = (key >> p.shift) & 15u;
            let slot = d * 256u + li;
            let dest = base_of[d] + cnt[slot];
            cnt[slot] += 1u;
            if (!skip_keys) { keys_out[dest] = key ^ p.store_mask; }
            if (grade) {
                var id = pos;
                if (p.first == 0u) { id = idx_in[pos]; }
                idx_out[dest] = id;
            }
        }
    }
}
