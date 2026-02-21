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
