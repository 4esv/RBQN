//! Blocked, packed, multicore matrix multiply for `•math.MatMul` (issue #166).
//!
//! Every output element is `((0 + w[i,0]·x[0,j]) + w[i,1]·x[1,j]) + …`: the same
//! left-to-right sum, with a separate multiply and add (no FMA), as the naive
//! triple loop, so results are bit-identical to it (and to the old kernel) at
//! every thread count. Blocking only changes which elements are in flight:
//! `x` is packed once into 8-column panels, `w` into MR-row tiles per block,
//! and an MR×8 register tile walks `k` sequentially, accumulating into the output.
//!
//! With `seed_first` the sum starts from the first product instead of `0 +` it, which is what
//! `+˝` does (the two differ only for a sum of negative zeros).
//!
//! NOTE: nothing here touches the thread-local array store; only slices cross threads.

use crate::par;
use rayon::prelude::*;

/// Output columns per register tile.
const NR: usize = 8;
/// Output rows per register tile.
const MR: usize = 4;
/// `k` block: a packed panel of KC×NR f64 is 16 KB, resident in L1.
const KC: usize = 256;
/// Largest row block one task owns.
const MC_MAX: usize = 64;
/// Below this many multiply-adds a single thread is faster than waking the pool.
const PAR_MACS: usize = 1 << 21;

/// A numeric matrix operand, read as f64. Integer operands are widened while packing,
/// so no f64 copy of the whole matrix is made (exact: integers below 2^53 convert exactly).
#[derive(Clone, Copy)]
pub enum Mat<'a> {
    F64(&'a [f64]),
    I32(&'a [i32]),
    I16(&'a [i16]),
    I8(&'a [i8]),
}

impl Mat<'_> {
    /// Copy `out.len()` elements starting at `at` into `out`, widened to f64.
    #[inline]
    fn load(&self, at: usize, out: &mut [f64]) {
        let n = out.len();
        match self {
            Mat::F64(v) => out.copy_from_slice(&v[at..at + n]),
            Mat::I32(v) => out.iter_mut().zip(&v[at..at + n]).for_each(|(o, &x)| *o = x as f64),
            Mat::I16(v) => out.iter_mut().zip(&v[at..at + n]).for_each(|(o, &x)| *o = x as f64),
            Mat::I8(v) => out.iter_mut().zip(&v[at..at + n]).for_each(|(o, &x)| *o = x as f64),
        }
    }
    fn len(&self) -> usize {
        match self {
            Mat::F64(v) => v.len(),
            Mat::I32(v) => v.len(),
            Mat::I16(v) => v.len(),
            Mat::I8(v) => v.len(),
        }
    }
}

/// `w` (m×k) times `x` (k×n), row-major, as an m×n row-major vector.
pub fn matmul(w: Mat, x: Mat, m: usize, k: usize, n: usize, seed_first: bool) -> Vec<f64> {
    assert_eq!(w.len(), m * k);
    assert_eq!(x.len(), k * n);
    let mut c = vec![0f64; m * n];
    if m == 0 || n == 0 || k == 0 {
        return c;
    }
    let par_on = par::threads() > 1 && m.saturating_mul(k).saturating_mul(n) >= PAR_MACS;
    let panels = n.div_ceil(NR);
    let nkb = k.div_ceil(KC);
    // Packed x: for each k block, for each 8-column panel, kc rows of 8 (zero padded past n).
    let mut bp = vec![0f64; k * panels * NR];
    let pack_panel = |kb: usize, jp: usize, dst: &mut [f64]| {
        let (k0, j0) = (kb * KC, jp * NR);
        let w_ = NR.min(n - j0);
        for (kk, row) in dst.as_chunks_mut::<NR>().0.iter_mut().enumerate() {
            x.load((k0 + kk) * n + j0, &mut row[..w_]);
        }
    };
    for kb in 0..nkb {
        let kc = KC.min(k - kb * KC);
        let blk = &mut bp[kb * KC * panels * NR..][..kc * panels * NR];
        if par_on {
            par::guarded(|| blk.par_chunks_mut(kc * NR).enumerate().for_each(|(jp, d)| pack_panel(kb, jp, d)));
        } else {
            blk.chunks_mut(kc * NR).enumerate().for_each(|(jp, d)| pack_panel(kb, jp, d));
        }
    }
    let mc = if par_on {
        (m.div_ceil(par::threads() * 4)).next_multiple_of(MR).clamp(MR, MC_MAX)
    } else {
        MC_MAX
    };
    let work = |(ci, cblk): (usize, &mut [f64])| {
        let rows = cblk.len() / n;
        row_block(w, &bp, cblk, Job { i0: ci * mc, rows, k, n, sf: seed_first });
    };
    if par_on {
        par::guarded(|| c.par_chunks_mut(mc * n).enumerate().for_each(work));
    } else {
        c.chunks_mut(mc * n).enumerate().for_each(work);
    }
    c
}

/// Rows `i0..i0+rows` of the product into `cblk` (rows×n), choosing the best instruction set.
/// One task: `rows` rows of the product starting at row `i0`.
#[derive(Clone, Copy)]
struct Job {
    i0: usize,
    rows: usize,
    k: usize,
    n: usize,
    sf: bool,
}

fn row_block(w: Mat, bp: &[f64], cblk: &mut [f64], job: Job) {
    #[cfg(all(target_arch = "x86_64", not(miri)))]
    if std::arch::is_x86_feature_detected!("avx2") {
        // SAFETY: the avx2 feature was detected at run time.
        return unsafe { row_block_avx2(w, bp, cblk, job) };
    }
    row_block_impl(w, bp, cblk, job)
}

#[cfg(all(target_arch = "x86_64", not(miri)))]
#[target_feature(enable = "avx2")]
fn row_block_avx2(w: Mat, bp: &[f64], cblk: &mut [f64], job: Job) {
    row_block_impl(w, bp, cblk, job)
}

#[inline(always)]
fn row_block_impl(w: Mat, bp: &[f64], cblk: &mut [f64], job: Job) {
    let Job { i0, rows, k, n, sf } = job;
    let panels = n.div_ceil(NR);
    let mut ap = vec![0f64; rows.next_multiple_of(MR) * KC];
    let mut tmp = [0f64; KC];
    for kb in 0..k.div_ceil(KC) {
        let (k0, kc) = (kb * KC, KC.min(k - kb * KC));
        // Pack w[i0.., k0..k0+kc] into MR-row tiles: tile t, step kk, row r at (t*kc + kk)*MR + r.
        for (t, tile) in ap.chunks_exact_mut(kc * MR).take(rows.div_ceil(MR)).enumerate() {
            for r in 0..MR {
                let i = t * MR + r;
                if i < rows {
                    w.load((i0 + i) * k + k0, &mut tmp[..kc]);
                    for kk in 0..kc {
                        tile[kk * MR + r] = tmp[kk];
                    }
                } else {
                    for kk in 0..kc {
                        tile[kk * MR + r] = 0.0;
                    }
                }
            }
        }
        let bblk = &bp[k0 * panels * NR..][..kc * panels * NR];
        for jp in 0..panels {
            let b = &bblk[jp * kc * NR..][..kc * NR];
            let (j0, cols) = (jp * NR, NR.min(n - jp * NR));
            for t in 0..rows.div_ceil(MR) {
                let a = &ap[t * kc * MR..][..kc * MR];
                let mr = MR.min(rows - t * MR);
                tile(a, b, kc, &mut cblk[t * MR * n..], n, j0, mr, cols, sf && kb == 0);
            }
        }
    }
}

/// One MR×NR register tile: `c[0..mr, j0..j0+cols] += a·b` summed over kc steps in order
/// (`first`: the tile starts from step 0's product and ignores `c`).
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn tile(a: &[f64], b: &[f64], kc: usize, c: &mut [f64], n: usize, j0: usize, mr: usize, cols: usize, first: bool) {
    let mut acc = [[0f64; NR]; MR];
    let mut skip = 0;
    if first {
        for r in 0..MR {
            for j in 0..NR {
                acc[r][j] = a[r] * b[j];
            }
        }
        skip = 1;
    } else {
        for r in 0..mr {
            acc[r][..cols].copy_from_slice(&c[r * n + j0..][..cols]);
        }
    }
    for (av, bv) in a.as_chunks::<MR>().0.iter().zip(b.as_chunks::<NR>().0).take(kc).skip(skip) {
        for r in 0..MR {
            for j in 0..NR {
                acc[r][j] += av[r] * bv[j];
            }
        }
    }
    for r in 0..mr {
        c[r * n + j0..][..cols].copy_from_slice(&acc[r][..cols]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn naive(a: &[f64], b: &[f64], m: usize, k: usize, n: usize) -> Vec<f64> {
        let mut c = vec![0.0; m * n];
        for i in 0..m {
            for j in 0..n {
                let mut s = 0.0;
                for l in 0..k {
                    s += a[i * k + l] * b[l * n + j];
                }
                c[i * n + j] = s;
            }
        }
        c
    }

    fn lcg(seed: &mut u64) -> f64 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*seed >> 11) as f64) / ((1u64 << 53) as f64) * 2.0 - 1.0
    }

    #[test]
    fn f64_64x64_bit_identical_and_within_one_ulp() {
        let mut s = 7u64;
        let a: Vec<f64> = (0..64 * 64).map(|_| lcg(&mut s)).collect();
        let b: Vec<f64> = (0..64 * 64).map(|_| lcg(&mut s)).collect();
        let want = naive(&a, &b, 64, 64, 64);
        let got = matmul(Mat::F64(&a), Mat::F64(&b), 64, 64, 64, false);
        for (g, w) in got.iter().zip(&want) {
            let ulp = (g.to_bits() as i64 - w.to_bits() as i64).abs();
            assert!(ulp <= 1, "{g} vs {w}");
        }
        assert!(got.iter().zip(&want).all(|(g, w)| g.to_bits() == w.to_bits()));
    }

    #[test]
    fn ragged_shapes_cross_k_blocks() {
        let mut s = 11u64;
        for &(m, k, n) in &[(1, 1, 1), (3, 5, 7), (5, 300, 9), (7, 513, 17), (66, 258, 10), (2, 1, 40), (130, 3, 1)] {
            let a: Vec<f64> = (0..m * k).map(|_| lcg(&mut s)).collect();
            let b: Vec<f64> = (0..k * n).map(|_| lcg(&mut s)).collect();
            let got = matmul(Mat::F64(&a), Mat::F64(&b), m, k, n, false);
            assert!(got.iter().zip(naive(&a, &b, m, k, n)).all(|(g, w)| g.to_bits() == w.to_bits()), "{m}x{k}x{n}");
        }
    }

    #[test]
    fn integers_exact() {
        let (m, k, n) = (37, 300, 29);
        let a: Vec<i32> = (0..m * k).map(|i| ((i * 7919) % 2001) as i32 - 1000).collect();
        let b: Vec<i16> = (0..k * n).map(|i| ((i * 104729) % 301) as i16 - 150).collect();
        let af: Vec<f64> = a.iter().map(|&v| v as f64).collect();
        let bf: Vec<f64> = b.iter().map(|&v| v as f64).collect();
        let got = matmul(Mat::I32(&a), Mat::I16(&b), m, k, n, false);
        assert_eq!(got, naive(&af, &bf, m, k, n));
    }

    #[test]
    fn empty_and_nan() {
        assert_eq!(matmul(Mat::F64(&[]), Mat::F64(&[]), 0, 3, 0, false), Vec::<f64>::new());
        assert_eq!(matmul(Mat::F64(&[]), Mat::F64(&[]), 2, 0, 2, false), vec![0.0; 4]);
        let c = matmul(Mat::F64(&[f64::NAN, 1.0]), Mat::F64(&[1.0, 1.0]), 1, 2, 1, false);
        assert!(c[0].is_nan());
    }

    #[test]
    #[cfg_attr(miri, ignore = "rayon's crossbeam-epoch trips Stacked Borrows (dependency, not this crate)")]
    fn parallel_matches_naive() {
        par::init(4);
        let mut s = 3u64;
        let (m, k, n) = (203, 301, 157);
        assert!(m * k * n >= PAR_MACS);
        let a: Vec<f64> = (0..m * k).map(|_| lcg(&mut s)).collect();
        let b: Vec<f64> = (0..k * n).map(|_| lcg(&mut s)).collect();
        let got = matmul(Mat::F64(&a), Mat::F64(&b), m, k, n, false);
        assert!(got.iter().zip(naive(&a, &b, m, k, n)).all(|(g, w)| g.to_bits() == w.to_bits()));
    }

    #[test]
    fn seed_first_matches_fold_and_keeps_negative_zero() {
        let mut s = 5u64;
        let (m, k, n) = (9, 300, 11);
        let a: Vec<f64> = (0..m * k).map(|_| lcg(&mut s)).collect();
        let b: Vec<f64> = (0..k * n).map(|_| lcg(&mut s)).collect();
        let got = matmul(Mat::F64(&a), Mat::F64(&b), m, k, n, true);
        for i in 0..m {
            for j in 0..n {
                let mut acc = a[i * k] * b[j];
                for l in 1..k {
                    acc += a[i * k + l] * b[l * n + j];
                }
                assert_eq!(got[i * n + j].to_bits(), acc.to_bits());
            }
        }
        let z = matmul(Mat::F64(&[-1.0]), Mat::F64(&[0.0]), 1, 1, 1, true);
        assert_eq!(z[0].to_bits(), (-0.0f64).to_bits());
        let z = matmul(Mat::F64(&[-1.0]), Mat::F64(&[0.0]), 1, 1, 1, false);
        assert_eq!(z[0].to_bits(), 0.0f64.to_bits());
    }
}
