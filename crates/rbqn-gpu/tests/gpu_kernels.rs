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
