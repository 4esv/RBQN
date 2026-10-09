use rbqn_gpu::buffer::{download_i32, upload_i32, ElementKind, GpuBuffer};
use rbqn_gpu::context::GpuContext;
use rbqn_gpu::dispatch::{grid_for, WORKGROUP_SIZE};
use rbqn_gpu::kernels::arith::arith_binary;
use rbqn_gpu::pipeline::{PipelineCache, PipelineKey};
use std::time::Instant;

fn main() {
    let n: usize = std::env::args().nth(1).map(|s| s.parse().unwrap()).unwrap_or(1_000_000);
    let reps = 100;
    let t = Instant::now();
    let ctx = pollster::block_on(GpuContext::new()).expect("gpu");
    println!("n={n} reps={reps}");
    println!("device init           {:>9.3} ms", t.elapsed().as_secs_f64() * 1e3);
    let (device, queue) = (ctx.device.clone(), ctx.queue.clone());
    let mut cache = PipelineCache::new(device.clone());
    let data: Vec<i32> = (0..n as i32).collect();

    let t = Instant::now();
    let a = upload_i32(&device, &queue, &data);
    let b = upload_i32(&device, &queue, &data);
    queue.submit([]); device.poll(wgpu::Maintain::Wait);
    println!("upload 2x{n} i32       {:>9.3} ms", t.elapsed().as_secs_f64() * 1e3);
    let out = GpuBuffer::storage(&device, ElementKind::I32, n);

    let t = Instant::now();
    arith_binary(&device, &queue, &mut cache, "add", &a, &b, &out);
    device.poll(wgpu::Maintain::Wait);
    println!("first dispatch+compile{:>9.3} ms", t.elapsed().as_secs_f64() * 1e3);

    // Pattern A: current code path. One submit per op, blocking download per op.
    let t = Instant::now();
    for _ in 0..reps {
        arith_binary(&device, &queue, &mut cache, "add", &a, &b, &out);
        let v = pollster::block_on(download_i32(&device, &queue, &out));
        assert_eq!(v[1], 2);
    }
    let a_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("A submit+readback/op  {:>9.3} ms/op", a_ms / reps as f64);

    // Pattern B: one submit per op, no readback, single wait at end.
    let t = Instant::now();
    for _ in 0..reps { arith_binary(&device, &queue, &mut cache, "add", &a, &b, &out); }
    device.poll(wgpu::Maintain::Wait);
    let b_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("B submit/op, 1 wait   {:>9.3} ms/op", b_ms / reps as f64);

    // Pattern C: all dispatches in one encoder, one submit, one wait.
    let t = Instant::now();
    let key = PipelineKey::new("arith_i32", "add", ElementKind::I32);
    let pipeline = cache.get_or_create(&key, include_str!("../../../crates/rbqn-gpu/src/shaders/arith_i32.wgsl"));
    let bgl = pipeline.get_bind_group_layout(0);
    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &bgl, entries: &[
        wgpu::BindGroupEntry { binding: 0, resource: a.inner().as_entire_binding() },
        wgpu::BindGroupEntry { binding: 1, resource: b.inner().as_entire_binding() },
        wgpu::BindGroupEntry { binding: 2, resource: out.inner().as_entire_binding() },
    ]});
    let mut enc = device.create_command_encoder(&Default::default());
    {
        let mut pass = enc.begin_compute_pass(&Default::default());
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bg, &[]);
        let (gx, gy) = grid_for(n, WORKGROUP_SIZE);
        for _ in 0..reps { pass.dispatch_workgroups(gx, gy, 1); }
    }
    queue.submit(std::iter::once(enc.finish()));
    device.poll(wgpu::Maintain::Wait);
    let c_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("C 1 encoder, 1 wait   {:>9.3} ms/op", c_ms / reps as f64);

    let t = Instant::now();
    let v = pollster::block_on(download_i32(&device, &queue, &out));
    println!("download {n} i32       {:>9.3} ms (v[1]={})", t.elapsed().as_secs_f64() * 1e3, v[1]);
    let t = Instant::now();
    let s: i64 = data.iter().zip(&data).map(|(x, y)| (x + y) as i64).sum();
    println!("cpu add+sum {n}        {:>9.3} ms ({s})", t.elapsed().as_secs_f64() * 1e3);
}
