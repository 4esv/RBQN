use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rbqn_gpu::buffer::{ElementKind, GpuBuffer, upload_f32, upload_i32};
use rbqn_gpu::context::GpuContext;
use rbqn_gpu::kernels::arith::arith_binary;
use rbqn_gpu::kernels::reduce::reduce;
use rbqn_gpu::kernels::scan::inclusive_scan;
use rbqn_gpu::pipeline::PipelineCache;

fn setup_gpu() -> Option<GpuContext> {
    pollster::block_on(GpuContext::new())
}

fn bench_reduce_add(c: &mut Criterion) {
    let ctx = match setup_gpu() {
        Some(c) => c,
        None => {
            eprintln!("WARNING: No GPU available, skipping reduce_add benchmarks");
            return;
        }
    };

    let mut group = c.benchmark_group("reduce_add");

    for size in [50_000usize, 100_000, 500_000] {
        let data: Vec<i32> = (0..size).map(|i| (i % 100) as i32).collect();
        let buf = upload_i32(&ctx.device, &ctx.queue, &data);

        group.bench_with_input(BenchmarkId::new("gpu", size), &size, |b, _| {
            let mut cache = PipelineCache::new(ctx.device.clone());
            b.iter(|| {
                let result = reduce(&ctx.device, &ctx.queue, &mut cache, "add", &buf);
                ctx.device.poll(wgpu::Maintain::Wait);
                result
            });
        });

        group.bench_with_input(BenchmarkId::new("cpu", size), &size, |b, _| {
            b.iter(|| data.iter().copied().sum::<i32>());
        });
    }

    group.finish();
}

fn bench_arith_add(c: &mut Criterion) {
    let ctx = match setup_gpu() {
        Some(c) => c,
        None => {
            eprintln!("WARNING: No GPU available, skipping arith_add benchmarks");
            return;
        }
    };

    let mut group = c.benchmark_group("arith_add");

    for size in [50_000usize, 100_000, 500_000] {
        let a_data: Vec<f32> = (0..size).map(|i| (i % 1000) as f32).collect();
        let b_data: Vec<f32> = (0..size).map(|i| ((i + 1) % 1000) as f32).collect();
        let a_buf = upload_f32(&ctx.device, &ctx.queue, &a_data);
        let b_buf = upload_f32(&ctx.device, &ctx.queue, &b_data);

        group.bench_with_input(BenchmarkId::new("gpu", size), &size, |b, _| {
            let mut cache = PipelineCache::new(ctx.device.clone());
            b.iter(|| {
                let out = GpuBuffer::storage(&ctx.device, ElementKind::F32, size);
                arith_binary(&ctx.device, &ctx.queue, &mut cache, "add", &a_buf, &b_buf, &out);
                ctx.device.poll(wgpu::Maintain::Wait);
                out
            });
        });

        group.bench_with_input(BenchmarkId::new("cpu", size), &size, |b, _| {
            b.iter(|| {
                a_data.iter().zip(b_data.iter()).map(|(a, b)| a + b).collect::<Vec<f32>>()
            });
        });
    }

    group.finish();
}

fn bench_scan_add(c: &mut Criterion) {
    let ctx = match setup_gpu() {
        Some(c) => c,
        None => {
            eprintln!("WARNING: No GPU available, skipping scan_add benchmarks");
            return;
        }
    };

    let mut group = c.benchmark_group("scan_add");

    for size in [50_000usize, 100_000, 500_000] {
        let data: Vec<i32> = (0..size).map(|i| (i % 100) as i32).collect();
        let buf = upload_i32(&ctx.device, &ctx.queue, &data);

        group.bench_with_input(BenchmarkId::new("gpu", size), &size, |b, _| {
            let mut cache = PipelineCache::new(ctx.device.clone());
            b.iter(|| {
                let result = inclusive_scan(&ctx.device, &ctx.queue, &mut cache, &buf);
                ctx.device.poll(wgpu::Maintain::Wait);
                result
            });
        });

        group.bench_with_input(BenchmarkId::new("cpu", size), &size, |b, _| {
            b.iter(|| {
                let mut acc = 0i32;
                data.iter()
                    .map(|&x| {
                        acc += x;
                        acc
                    })
                    .collect::<Vec<i32>>()
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_reduce_add, bench_arith_add, bench_scan_add);
criterion_main!(benches);
