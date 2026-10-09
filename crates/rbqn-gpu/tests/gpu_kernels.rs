// NOTE: Integration tests for GPU kernels. Requires a GPU to run.
// If no GPU is available the tests skip gracefully.

use rbqn_gpu::{
    buffer::{ElementKind, GpuBuffer, download_f32, upload_f32},
    context::GpuContext,
    kernels::{matmul, softmax, unary},
    pipeline::PipelineCache,
};

// ---- helpers ----------------------------------------------------------------

async fn setup() -> Option<(GpuContext, PipelineCache)> {
    let ctx = GpuContext::new().await?;
    let cache = PipelineCache::new(ctx.device.clone());
    Some((ctx, cache))
}

fn assert_approx(label: &str, got: &[f32], expected: &[f32], tol: f32) {
    assert_eq!(
        got.len(),
        expected.len(),
        "{label}: length mismatch — got {} expected {}",
        got.len(),
        expected.len()
    );
    for (i, (g, e)) in got.iter().zip(expected.iter()).enumerate() {
        assert!(
            (g - e).abs() <= tol,
            "{label}[{i}]: got {g} expected {e} (tol {tol})"
        );
    }
}

// ---- unary ops --------------------------------------------------------------

#[test]
fn test_unary_exp() {
    pollster::block_on(async {
        let Some((ctx, mut cache)) = setup().await else {
            println!("No GPU available — skipping test_unary_exp");
            return;
        };
        let input_data: Vec<f32> = vec![0.0, 1.0, 2.0, -1.0];
        let expected: Vec<f32> = input_data.iter().map(|x| x.exp()).collect();

        let input = upload_f32(&ctx.device, &ctx.queue, &input_data);
        let out = GpuBuffer::storage(&ctx.device, ElementKind::F32, input_data.len());

        unary::unary_op(&ctx.device, &ctx.queue, &mut cache, "exp", &input, &out);

        let result = download_f32(&ctx.device, &ctx.queue, &out).await;
        assert_approx("test_unary_exp", &result, &expected, 1e-5);
    });
}

#[test]
fn test_unary_sqrt() {
    pollster::block_on(async {
        let Some((ctx, mut cache)) = setup().await else {
            println!("No GPU available — skipping test_unary_sqrt");
            return;
        };
        let input_data: Vec<f32> = vec![0.0, 1.0, 4.0, 9.0];
        let expected: Vec<f32> = vec![0.0, 1.0, 2.0, 3.0];

        let input = upload_f32(&ctx.device, &ctx.queue, &input_data);
        let out = GpuBuffer::storage(&ctx.device, ElementKind::F32, input_data.len());

        unary::unary_op(&ctx.device, &ctx.queue, &mut cache, "sqrt", &input, &out);

        let result = download_f32(&ctx.device, &ctx.queue, &out).await;
        assert_approx("test_unary_sqrt", &result, &expected, 1e-5);
    });
}

#[test]
fn test_unary_neg() {
    pollster::block_on(async {
        let Some((ctx, mut cache)) = setup().await else {
            println!("No GPU available — skipping test_unary_neg");
            return;
        };
        let input_data: Vec<f32> = vec![1.0, -2.0, 0.0, 3.5];
        let expected: Vec<f32> = vec![-1.0, 2.0, 0.0, -3.5];

        let input = upload_f32(&ctx.device, &ctx.queue, &input_data);
        let out = GpuBuffer::storage(&ctx.device, ElementKind::F32, input_data.len());

        unary::unary_op(&ctx.device, &ctx.queue, &mut cache, "neg", &input, &out);

        let result = download_f32(&ctx.device, &ctx.queue, &out).await;
        assert_approx("test_unary_neg", &result, &expected, 1e-6);
    });
}

#[test]
fn test_unary_abs() {
    pollster::block_on(async {
        let Some((ctx, mut cache)) = setup().await else {
            println!("No GPU available — skipping test_unary_abs");
            return;
        };
        let input_data: Vec<f32> = vec![-1.0, 2.0, -3.5, 0.0];
        let expected: Vec<f32> = vec![1.0, 2.0, 3.5, 0.0];

        let input = upload_f32(&ctx.device, &ctx.queue, &input_data);
        let out = GpuBuffer::storage(&ctx.device, ElementKind::F32, input_data.len());

        unary::unary_op(&ctx.device, &ctx.queue, &mut cache, "abs", &input, &out);

        let result = download_f32(&ctx.device, &ctx.queue, &out).await;
        assert_approx("test_unary_abs", &result, &expected, 1e-6);
    });
}

// ---- matmul -----------------------------------------------------------------

#[test]
fn test_matmul_2x2() {
    pollster::block_on(async {
        let Some((ctx, mut cache)) = setup().await else {
            println!("No GPU available — skipping test_matmul_2x2");
            return;
        };
        // A = [[1,2],[3,4]], B = [[5,6],[7,8]]
        // C = [[19,22],[43,50]]
        let a_data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        let b_data: Vec<f32> = vec![5.0, 6.0, 7.0, 8.0];
        let expected: Vec<f32> = vec![19.0, 22.0, 43.0, 50.0];

        let a = upload_f32(&ctx.device, &ctx.queue, &a_data);
        let b = upload_f32(&ctx.device, &ctx.queue, &b_data);
        let out = GpuBuffer::storage(&ctx.device, ElementKind::F32, 4);

        matmul::matmul(&ctx.device, &ctx.queue, &mut cache, &a, &b, &out, 2, 2, 2);

        let result = download_f32(&ctx.device, &ctx.queue, &out).await;
        assert_approx("test_matmul_2x2", &result, &expected, 1e-4);
    });
}

#[test]
fn test_matmul_2x3_3x2() {
    pollster::block_on(async {
        let Some((ctx, mut cache)) = setup().await else {
            println!("No GPU available — skipping test_matmul_2x3_3x2");
            return;
        };
        // A (2x3) = [[1,2,3],[4,5,6]]
        // B (3x2) = [[7,8],[9,10],[11,12]]
        // C (2x2) = [[58,64],[139,154]]
        let a_data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let b_data: Vec<f32> = vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
        let expected: Vec<f32> = vec![58.0, 64.0, 139.0, 154.0];

        let a = upload_f32(&ctx.device, &ctx.queue, &a_data);
        let b = upload_f32(&ctx.device, &ctx.queue, &b_data);
        let out = GpuBuffer::storage(&ctx.device, ElementKind::F32, 4);

        // M=2, N=2, K=3
        matmul::matmul(&ctx.device, &ctx.queue, &mut cache, &a, &b, &out, 2, 2, 3);

        let result = download_f32(&ctx.device, &ctx.queue, &out).await;
        assert_approx("test_matmul_2x3_3x2", &result, &expected, 1e-4);
    });
}

// ---- softmax ----------------------------------------------------------------

#[test]
fn test_softmax_simple() {
    pollster::block_on(async {
        let Some((ctx, mut cache)) = setup().await else {
            println!("No GPU available — skipping test_softmax_simple");
            return;
        };
        let input_data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];

        let input = upload_f32(&ctx.device, &ctx.queue, &input_data);
        let out = GpuBuffer::storage(&ctx.device, ElementKind::F32, input_data.len());

        softmax::softmax(&ctx.device, &ctx.queue, &mut cache, &input, &out);

        let result = download_f32(&ctx.device, &ctx.queue, &out).await;

        // All values must be positive
        for (i, &v) in result.iter().enumerate() {
            assert!(v > 0.0, "test_softmax_simple[{i}]: expected positive, got {v}");
        }
        // Sum must be ~1.0
        let sum: f32 = result.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-5,
            "test_softmax_simple: sum = {sum}, expected ~1.0"
        );
        // Monotonically increasing (higher input -> higher probability)
        assert!(result[3] > result[2], "expected p[3] > p[2]");
        assert!(result[2] > result[1], "expected p[2] > p[1]");
        assert!(result[1] > result[0], "expected p[1] > p[0]");
    });
}

#[test]
fn test_softmax_uniform() {
    pollster::block_on(async {
        let Some((ctx, mut cache)) = setup().await else {
            println!("No GPU available — skipping test_softmax_uniform");
            return;
        };
        let input_data: Vec<f32> = vec![1.0, 1.0, 1.0, 1.0];
        let expected_each = 0.25f32;

        let input = upload_f32(&ctx.device, &ctx.queue, &input_data);
        let out = GpuBuffer::storage(&ctx.device, ElementKind::F32, input_data.len());

        softmax::softmax(&ctx.device, &ctx.queue, &mut cache, &input, &out);

        let result = download_f32(&ctx.device, &ctx.queue, &out).await;
        assert_approx(
            "test_softmax_uniform",
            &result,
            &vec![expected_each; 4],
            1e-5,
        );
    });
}

// ---- 2D workgroup grid (issue #10) -----------------------------------------
// Arrays over 65535 * 256 = 16,777,216 elements need more than one workgroup
// row. Each test below crosses that boundary and checks against a CPU result.

const OVER_1D_LIMIT: usize = 20_000_000;

#[test]
fn test_arith_add_over_workgroup_limit() {
    use rbqn_gpu::buffer::{download_i32, upload_i32};
    use rbqn_gpu::kernels::arith;
    pollster::block_on(async {
        let Some((ctx, mut cache)) = setup().await else {
            println!("No GPU available — skipping");
            return;
        };
        let n = OVER_1D_LIMIT;
        let a: Vec<i32> = (0..n as i32).collect();
        let b: Vec<i32> = (0..n as i32).map(|x| x % 7).collect();
        let a_buf = upload_i32(&ctx.device, &ctx.queue, &a);
        let b_buf = upload_i32(&ctx.device, &ctx.queue, &b);
        let out = GpuBuffer::storage(&ctx.device, ElementKind::I32, n);
        arith::arith_binary(&ctx.device, &ctx.queue, &mut cache, "add", &a_buf, &b_buf, &out);
        let got = download_i32(&ctx.device, &ctx.queue, &out).await;
        assert_eq!(got.len(), n);
        // Spot-check the last element of every row plus the tail: a stale 1D
        // index leaves everything past 16,777,216 as zero.
        for i in [0, 16_777_215, 16_777_216, 16_777_217, n - 1] {
            assert_eq!(got[i], a[i] + b[i], "index {i}");
        }
        assert!(got.iter().zip(a.iter().zip(b.iter())).all(|(g, (x, y))| *g == x + y));
    });
}

#[test]
fn test_reduce_add_over_workgroup_limit() {
    use rbqn_gpu::buffer::{download_i32, upload_i32};
    use rbqn_gpu::kernels::reduce;
    pollster::block_on(async {
        let Some((ctx, mut cache)) = setup().await else {
            println!("No GPU available — skipping");
            return;
        };
        let n = OVER_1D_LIMIT;
        let data: Vec<i32> = (0..n as i32).map(|x| x % 100).collect();
        let expected: i64 = data.iter().map(|&x| x as i64).sum();
        assert!(expected < i32::MAX as i64);
        let buf = upload_i32(&ctx.device, &ctx.queue, &data);
        let out = reduce::reduce(&ctx.device, &ctx.queue, &mut cache, "add", &buf);
        let got = download_i32(&ctx.device, &ctx.queue, &out).await;
        assert_eq!(got.len(), 1);
        assert_eq!(got[0] as i64, expected);
    });
}

#[test]
fn test_scan_add_over_workgroup_limit() {
    use rbqn_gpu::buffer::{download_i32, upload_i32};
    use rbqn_gpu::kernels::scan;
    pollster::block_on(async {
        let Some((ctx, mut cache)) = setup().await else {
            println!("No GPU available — skipping");
            return;
        };
        // Scan blocks cover 512 elements, so the 1D cap is 65535 * 512.
        let n = 65535 * 512 + 1000;
        let data: Vec<i32> = vec![1; n];
        let buf = upload_i32(&ctx.device, &ctx.queue, &data);
        let out = scan::inclusive_scan(&ctx.device, &ctx.queue, &mut cache, &buf);
        let got = download_i32(&ctx.device, &ctx.queue, &out).await;
        assert_eq!(got.len(), n);
        assert_eq!(got[0], 1);
        assert_eq!(got[65535 * 512 - 1], 65535 * 512);
        assert_eq!(got[65535 * 512], 65535 * 512 + 1);
        assert_eq!(got[n - 1], n as i32);
    });
}

#[test]
#[ignore = "GPU radix sort scatter is unstable across passes, issue #26"]
fn test_sort_over_workgroup_limit() {
    use rbqn_gpu::buffer::{download_i32, upload_i32};
    use rbqn_gpu::kernels::sort;
    pollster::block_on(async {
        let Some((ctx, mut cache)) = setup().await else {
            println!("No GPU available — skipping");
            return;
        };
        let n = OVER_1D_LIMIT;
        // Deterministic pseudo-random keys, negatives included.
        let data: Vec<i32> = (0..n as u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 8) as i32 - 5_000_000)
            .collect();
        let buf = upload_i32(&ctx.device, &ctx.queue, &data);
        let out = sort::sort_i32(&ctx.device, &ctx.queue, &mut cache, &buf);
        let got = download_i32(&ctx.device, &ctx.queue, &out).await;
        let mut expected = data;
        expected.sort_unstable();
        assert_eq!(got.len(), n);
        assert!(got == expected, "sorted output differs from CPU sort");
    });
}

#[test]
#[ignore = "GPU radix sort scatter is unstable across passes, issue #26"]
fn test_sort_100k() {
    use rbqn_gpu::buffer::{download_i32, upload_i32};
    use rbqn_gpu::kernels::sort;
    pollster::block_on(async {
        let Some((ctx, mut cache)) = setup().await else {
            println!("No GPU available — skipping");
            return;
        };
        let n = 100_000;
        let data: Vec<i32> = (0..n as u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 8) as i32 - 5_000_000)
            .collect();
        let buf = upload_i32(&ctx.device, &ctx.queue, &data);
        let out = sort::sort_i32(&ctx.device, &ctx.queue, &mut cache, &buf);
        let got = download_i32(&ctx.device, &ctx.queue, &out).await;
        let mut expected = data;
        expected.sort_unstable();
        let first_bad = got.iter().zip(&expected).position(|(g, e)| g != e);
        assert!(first_bad.is_none(), "first mismatch at {first_bad:?}");
    });
}

// ---- i64 arith / reduce / scan, i32 minmax -------------------------------------

mod int64_tests {
    use rbqn_gpu::{
        buffer::{ElementKind, GpuBuffer, download_i32, download_i64, upload_i32, upload_i64},
        context::GpuContext,
        kernels::{arith_i64, minmax, reduce_i64, scan_i64},
        pipeline::PipelineCache,
    };

    const LENGTHS: [usize; 7] = [1, 7, 256, 257, 65536, 1_000_003, 10_000_000];
    const OPS: [&str; 3] = ["add", "min", "max"];

    fn setup() -> Option<(GpuContext, PipelineCache)> {
        let Some(ctx) = pollster::block_on(GpuContext::new()) else {
            println!("No GPU available, skipping i64 tests");
            return None;
        };
        if !ctx.shader_int64 {
            println!("no SHADER_INT64 on this adapter: skipping i64 tests");
            return None;
        }
        let cache = PipelineCache::new(ctx.device.clone());
        Some((ctx, cache))
    }

    fn lcg(state: &mut u64) -> u64 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *state >> 11
    }

    const I32_EDGES: [i32; 10] = [
        i32::MIN, i32::MIN + 1, -1, 0, 1, i32::MAX - 1, i32::MAX, 65535, -65536, 1 << 30,
    ];

    /// "mixed": full-range random i32 with negatives and +-2^31 boundaries.
    /// "high": all in [2^30, 2^31-1], so the sum of 1e7 of them is ~2^53.
    fn gen_i32(n: usize, kind: &str, seed: u64) -> Vec<i32> {
        let mut s = seed ^ (n as u64).wrapping_mul(0x9E3779B97F4A7C15);
        (0..n)
            .map(|i| match kind {
                "high" => (1i32 << 30) + (lcg(&mut s) % (1 << 30)) as i32,
                _ => {
                    if i % 97 == 3 {
                        I32_EDGES[(lcg(&mut s) % 10) as usize]
                    } else {
                        lcg(&mut s) as i32
                    }
                }
            })
            .collect()
    }

    /// i64 data: magnitudes up to 2^40, with +-2^31 boundaries and +-2^53 edges.
    fn gen_i64(n: usize, kind: &str, seed: u64) -> Vec<i64> {
        let mut s = seed ^ (n as u64).wrapping_mul(0x9E3779B97F4A7C15);
        const E: [i64; 10] = [
            i32::MIN as i64 - 1, i32::MIN as i64, i32::MAX as i64, i32::MAX as i64 + 1, 1 << 31,
            -(1 << 31), 1 << 53, -(1 << 53), -1, 0,
        ];
        (0..n)
            .map(|i| match kind {
                // sums of 1e7 of these reach ~2^52..2^53
                "high" => (1i64 << 28) + (lcg(&mut s) % (1 << 28)) as i64,
                _ => {
                    if i % 89 == 5 {
                        E[(lcg(&mut s) % 10) as usize]
                    } else {
                        (lcg(&mut s) as i64) >> 24 // 40-bit signed
                    }
                }
            })
            .collect()
    }

    fn first_diff<T: PartialEq + std::fmt::Debug>(label: &str, got: &[T], want: &[T]) {
        assert_eq!(got.len(), want.len(), "{label}: length");
        if let Some(i) = (0..got.len()).find(|&i| got[i] != want[i]) {
            panic!("{label}: first mismatch at {i}: got {:?} want {:?}", got[i], want[i]);
        }
    }

    fn cpu_op(op: &str, a: i64, b: i64) -> i64 {
        match op {
            "add" => a.wrapping_add(b),
            "sub" => a.wrapping_sub(b),
            "mul" => a.wrapping_mul(b),
            "min" => a.min(b),
            "max" => a.max(b),
            _ => unreachable!(),
        }
    }

    fn id(op: &str) -> i64 {
        match op {
            "min" => i64::MAX,
            "max" => i64::MIN,
            _ => 0,
        }
    }

    fn dl64(ctx: &GpuContext, b: &GpuBuffer) -> Vec<i64> {
        pollster::block_on(download_i64(&ctx.device, &ctx.queue, b))
    }

    // ---- elementwise ----

    fn check_arith(ctx: &GpuContext, cache: &mut PipelineCache, n: usize) {
        let kinds = if n == 1_000_003 { vec!["mixed", "high"] } else { vec!["mixed"] };
        for kind in kinds {
            let a = gen_i64(n, kind, 1);
            let b = gen_i64(n, kind, 2);
            let ba = upload_i64(&ctx.device, &ctx.queue, &a);
            let bb = upload_i64(&ctx.device, &ctx.queue, &b);
            for op in ["add", "sub", "mul", "min", "max"] {
                let out = GpuBuffer::storage(&ctx.device, ElementKind::I64, n);
                arith_i64::arith_binary_i64(&ctx.device, &ctx.queue, cache, op, &ba, &bb, &out);
                let want: Vec<i64> = a.iter().zip(&b).map(|(x, y)| cpu_op(op, *x, *y)).collect();
                first_diff(&format!("arith_i64 {op} {kind} n={n}"), &dl64(ctx, &out), &want);
            }
            // widening: i32 inputs, exact i64 out
            let a = gen_i32(n, kind, 3);
            let b = gen_i32(n, kind, 4);
            let ba = upload_i32(&ctx.device, &ctx.queue, &a);
            let bb = upload_i32(&ctx.device, &ctx.queue, &b);
            for op in ["add", "sub", "mul", "min", "max"] {
                let out = GpuBuffer::storage(&ctx.device, ElementKind::I64, n);
                arith_i64::arith_binary_i32_to_i64(&ctx.device, &ctx.queue, cache, op, &ba, &bb, &out);
                let want: Vec<i64> =
                    a.iter().zip(&b).map(|(x, y)| cpu_op(op, *x as i64, *y as i64)).collect();
                first_diff(&format!("arith_i32_to_i64 {op} {kind} n={n}"), &dl64(ctx, &out), &want);
            }
        }
    }

    #[test]
    fn test_arith_i64_lengths() {
        let Some((ctx, mut cache)) = setup() else { return };
        for n in LENGTHS {
            check_arith(&ctx, &mut cache, n);
        }
    }

    #[test]
    fn test_arith_i64_2d_grid_20m() {
        let Some((ctx, mut cache)) = setup() else { return };
        check_arith(&ctx, &mut cache, 20_000_000);
    }

    // ---- reduce ----

    fn cpu_reduce(op: &str, v: impl Iterator<Item = i64>) -> i64 {
        v.fold(id(op), |a, b| cpu_op(op, a, b))
    }

    fn check_reduce(ctx: &GpuContext, cache: &mut PipelineCache, n: usize) {
        let kinds = if n == 1_000_003 || n == 10_000_000 { vec!["mixed", "high"] } else { vec!["mixed"] };
        for kind in kinds {
            let a = gen_i64(n, kind, 5);
            let ba = upload_i64(&ctx.device, &ctx.queue, &a);
            let a32 = gen_i32(n, kind, 6);
            let ba32 = upload_i32(&ctx.device, &ctx.queue, &a32);
            for op in OPS {
                let r = reduce_i64::reduce_i64(&ctx.device, &ctx.queue, cache, op, &ba);
                assert_eq!(dl64(ctx, &r), vec![cpu_reduce(op, a.iter().copied())], "reduce_i64 {op} {kind} n={n}");
                let r = reduce_i64::reduce_i32_to_i64(&ctx.device, &ctx.queue, cache, op, &ba32);
                assert_eq!(
                    dl64(ctx, &r),
                    vec![cpu_reduce(op, a32.iter().map(|&x| x as i64))],
                    "reduce_i32_to_i64 {op} {kind} n={n}"
                );
            }
            if kind == "high" && n == 10_000_000 {
                let s: i64 = a32.iter().map(|&x| x as i64).sum();
                assert!(s > 1 << 52, "high i32 sum should exceed 2^52, got {s}");
            }
        }
    }

    #[test]
    fn test_reduce_i64_lengths() {
        let Some((ctx, mut cache)) = setup() else { return };
        for n in LENGTHS {
            check_reduce(&ctx, &mut cache, n);
        }
    }

    // ---- scan ----

    fn cpu_scan(op: &str, v: impl Iterator<Item = i64>) -> Vec<i64> {
        let mut acc = id(op);
        v.map(|x| {
            acc = cpu_op(op, acc, x);
            acc
        })
        .collect()
    }

    fn check_scan(ctx: &GpuContext, cache: &mut PipelineCache, n: usize) {
        let kinds = if n == 1_000_003 || n == 10_000_000 { vec!["mixed", "high"] } else { vec!["mixed"] };
        for kind in kinds {
            let a = gen_i64(n, kind, 7);
            let ba = upload_i64(&ctx.device, &ctx.queue, &a);
            let a32 = gen_i32(n, kind, 8);
            let ba32 = upload_i32(&ctx.device, &ctx.queue, &a32);
            for op in OPS {
                let r = scan_i64::scan_i64(&ctx.device, &ctx.queue, cache, op, &ba);
                first_diff(&format!("scan_i64 {op} {kind} n={n}"), &dl64(ctx, &r), &cpu_scan(op, a.iter().copied()));
                let r = scan_i64::scan_i32_to_i64(&ctx.device, &ctx.queue, cache, op, &ba32);
                first_diff(
                    &format!("scan_i32_to_i64 {op} {kind} n={n}"),
                    &dl64(ctx, &r),
                    &cpu_scan(op, a32.iter().map(|&x| x as i64)),
                );
            }
        }
    }

    #[test]
    fn test_scan_i64_lengths() {
        let Some((ctx, mut cache)) = setup() else { return };
        for n in LENGTHS {
            check_scan(&ctx, &mut cache, n);
        }
    }

    // ---- minmax ----

    fn check_minmax(ctx: &GpuContext, cache: &mut PipelineCache, n: usize) {
        for kind in ["mixed", "high"] {
            let a = gen_i32(n, kind, 9);
            let ba = upload_i32(&ctx.device, &ctx.queue, &a);
            let r = minmax::minmax_i32(&ctx.device, &ctx.queue, cache, &ba);
            let got = pollster::block_on(download_i32(&ctx.device, &ctx.queue, &r));
            let want = vec![*a.iter().min().unwrap(), *a.iter().max().unwrap()];
            assert_eq!(got, want, "minmax_i32 {kind} n={n}");
        }
    }

    #[test]
    fn test_minmax_i32_lengths() {
        let Some((ctx, mut cache)) = setup() else { return };
        for n in LENGTHS {
            check_minmax(&ctx, &mut cache, n);
        }
    }

    /// Reduce/scan/minmax switch to a 2D grid only above 65535 blocks of 1024
    /// (67.1M elements); the 20M arith test cannot reach that path.
    #[test]
    fn test_reduce_scan_minmax_2d_grid_70m() {
        let Some((ctx, mut cache)) = setup() else { return };
        let n = 70_000_001;
        let a = gen_i32(n, "high", 11);
        let ba = upload_i32(&ctx.device, &ctx.queue, &a);
        let r = minmax::minmax_i32(&ctx.device, &ctx.queue, &mut cache, &ba);
        let got = pollster::block_on(download_i32(&ctx.device, &ctx.queue, &r));
        assert_eq!(got, vec![*a.iter().min().unwrap(), *a.iter().max().unwrap()], "minmax 70M");
        let r = reduce_i64::reduce_i32_to_i64(&ctx.device, &ctx.queue, &mut cache, "add", &ba);
        assert_eq!(dl64(&ctx, &r), vec![a.iter().map(|&x| x as i64).sum::<i64>()], "reduce 70M");
        let r = scan_i64::scan_i32_to_i64(&ctx.device, &ctx.queue, &mut cache, "add", &ba);
        first_diff("scan 70M", &dl64(&ctx, &r), &cpu_scan("add", a.iter().map(|&x| x as i64)));
    }

    // ---- microbench ----

    /// `cargo test -p rbqn-gpu --release -- --ignored --nocapture bench_i64_vs_i32`
    #[test]
    #[ignore = "microbench, prints a table"]
    fn bench_i64_vs_i32() {
        use rbqn_gpu::kernels::{arith, reduce};
        use std::time::Instant;
        let Some((ctx, mut cache)) = setup() else { return };
        let n = 10_000_000;
        let a32 = gen_i32(n, "mixed", 1);
        let b32 = gen_i32(n, "mixed", 2);
        let a64: Vec<i64> = a32.iter().map(|&x| x as i64).collect();
        let b64: Vec<i64> = b32.iter().map(|&x| x as i64).collect();
        let (x32, y32) = (upload_i32(&ctx.device, &ctx.queue, &a32), upload_i32(&ctx.device, &ctx.queue, &b32));
        let (x64, y64) = (upload_i64(&ctx.device, &ctx.queue, &a64), upload_i64(&ctx.device, &ctx.queue, &b64));
        let o32 = GpuBuffer::storage(&ctx.device, ElementKind::I32, n);
        let o64 = GpuBuffer::storage(&ctx.device, ElementKind::I64, n);
        let dev = ctx.device.clone();
        let q = ctx.queue.clone();
        // submit + spin-sync via buffer::sync
        let wait = |dev: &wgpu::Device, q: &wgpu::Queue| {
            let (tx, rx) = std::sync::mpsc::channel();
            q.on_submitted_work_done(move || {
                let _ = tx.send(());
            });
            rbqn_gpu::buffer::sync(dev, || rx.try_recv().ok());
        };
        let bench = |label: &str, f: &mut dyn FnMut()| -> f64 {
            for _ in 0..5 {
                f();
                wait(&dev, &q);
            }
            let reps = 30;
            let mut times: Vec<f64> = (0..reps)
                .map(|_| {
                    let t = Instant::now();
                    f();
                    wait(&dev, &q);
                    t.elapsed().as_secs_f64() * 1e3
                })
                .collect();
            times.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let med = times[reps / 2];
            println!("{label:<44} median {med:7.3} ms  min {:7.3} ms", times[0]);
            med
        };
        let add32 = bench("elementwise add i32 (10M)", &mut || {
            arith::arith_binary(&dev, &q, &mut cache, "add", &x32, &y32, &o32)
        });
        let add64 = bench("elementwise add i64 (10M)", &mut || {
            arith_i64::arith_binary_i64(&dev, &q, &mut cache, "add", &x64, &y64, &o64)
        });
        let mul32w = bench("elementwise mul i32->i64 widening (10M)", &mut || {
            arith_i64::arith_binary_i32_to_i64(&dev, &q, &mut cache, "mul", &x32, &y32, &o64)
        });
        let mul64 = bench("elementwise mul i64 (10M)", &mut || {
            arith_i64::arith_binary_i64(&dev, &q, &mut cache, "mul", &x64, &y64, &o64)
        });
        let r32 = bench("reduce add i32 (10M, existing kernel)", &mut || {
            let _ = reduce::reduce(&dev, &q, &mut cache, "add", &x32);
        });
        let r64 = bench("reduce add i64 (10M)", &mut || {
            let _ = reduce_i64::reduce_i64(&dev, &q, &mut cache, "add", &x64);
        });
        let r32w = bench("reduce add i32->i64 widening (10M)", &mut || {
            let _ = reduce_i64::reduce_i32_to_i64(&dev, &q, &mut cache, "add", &x32);
        });
        bench("scan add i64 (10M)", &mut || {
            let _ = scan_i64::scan_i64(&dev, &q, &mut cache, "add", &x64);
        });
        bench("scan add i32->i64 widening (10M)", &mut || {
            let _ = scan_i64::scan_i32_to_i64(&dev, &q, &mut cache, "add", &x32);
        });
        bench("minmax i32 (10M)", &mut || {
            let _ = minmax::minmax_i32(&dev, &q, &mut cache, &x32);
        });
        println!(
            "ratio add i64/i32 = {:.2}, mul i64/add i32 = {:.2}, widening mul/add i32 = {:.2}",
            add64 / add32, mul64 / add32, mul32w / add32
        );
        println!("ratio reduce i64/i32 = {:.2}, widening/i32 = {:.2}", r64 / r32, r32w / r32);
    }
}
