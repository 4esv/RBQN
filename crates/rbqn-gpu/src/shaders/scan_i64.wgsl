// Template: @IN@ is i64 or i32 (widening); @COMB@ / @ID@ define the monoid.
// scan_block: inclusive scan of 1024 elements per workgroup (4 per thread:
// serial scan, Hillis-Steele over the 256 thread totals), block total to
// sums[wg]. propagate: out[i] = comb(sums[wg-1], out[i]) for wg > 0, where
// sums is the already-scanned block totals. Reduce-then-scan, barriers only.
@group(0) @binding(0) var<storage, read> input: array<@IN@>;
@group(0) @binding(1) var<storage, read_write> output: array<i64>;
@group(0) @binding(2) var<storage, read_write> sums: array<i64>;

var<workgroup> tot: array<i64, 256>;

fn comb(a: i64, b: i64) -> i64 {
    return @COMB@;
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
    let base = wg * 1024u + t * 4u;

    var v: array<i64, 4>;
    var run: i64 = @ID@;
    for (var k = 0u; k < 4u; k = k + 1u) {
        if (base + k < n) {
            run = comb(run, i64(input[base + k]));
        }
        v[k] = run;
    }
    tot[t] = run;
    workgroupBarrier();

    for (var off = 1u; off < 256u; off = off << 1u) {
        var x = tot[t];
        if (t >= off) {
            x = comb(tot[t - off], x);
        }
        workgroupBarrier();
        tot[t] = x;
        workgroupBarrier();
    }

    var pre: i64 = @ID@;
    if (t > 0u) { pre = tot[t - 1u]; }
    for (var k = 0u; k < 4u; k = k + 1u) {
        if (base + k < n) {
            output[base + k] = comb(pre, v[k]);
        }
    }
    if (t == 0u && wg < arrayLength(&sums)) {
        sums[wg] = tot[255];
    }
}

@compute @workgroup_size(256)
fn propagate(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let wg = wid.y * nwg.x + wid.x;
    if (wg == 0u) { return; }
    let carry = sums[wg - 1u];
    let n = arrayLength(&output);
    for (var k = 0u; k < 4u; k = k + 1u) {
        let i = wg * 1024u + lid.x + k * 256u;
        if (i < n) {
            output[i] = comb(carry, output[i]);
        }
    }
}
