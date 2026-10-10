use rbqn_gpu::buffer::{download_u32, upload_u32};
use rbqn_gpu::context::GpuContext;
use rbqn_gpu::kernels::sort::radix_sort_u32;
use rbqn_gpu::pipeline::PipelineCache;
use std::time::Instant;
fn main() {
    let ctx = pollster::block_on(GpuContext::new()).unwrap();
    let (device, queue) = (ctx.device.clone(), ctx.queue.clone());
    let mut cache = PipelineCache::new(device.clone());
    for &n in &[1_000_000usize, 10_000_000] {
        let data: Vec<u32> = (0..n as u32).map(|i| (i.wrapping_mul(2654435761)) % 1000).collect();
        let mut cpu = data.clone(); let t = Instant::now(); cpu.sort_unstable(); let cpu_ms = t.elapsed().as_secs_f64()*1e3;
        let buf = upload_u32(&device, &queue, &data);
        let _ = radix_sort_u32(&device, &queue, &mut cache, &buf); // warm
        let reps = 5; let t = Instant::now();
        let mut out = None;
        for _ in 0..reps { out = Some(radix_sort_u32(&device, &queue, &mut cache, &buf)); }
        device.poll(wgpu::Maintain::Wait);
        let gpu_ms = t.elapsed().as_secs_f64()*1e3/reps as f64;
        let res = pollster::block_on(download_u32(&device, &queue, out.as_ref().unwrap()));
        println!("n={n:>9}  cpu sort_unstable {cpu_ms:8.3} ms   gpu radix (existing kernel, resident) {gpu_ms:8.3} ms   correct={}", res == cpu);
    }
}
