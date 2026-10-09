use rbqn_gpu::buffer::{download_u32, upload_u32};
use rbqn_gpu::context::GpuContext;
use rbqn_gpu::kernels::sort::radix_sort_u32;
use rbqn_gpu::pipeline::PipelineCache;
fn main() {
    let ctx = pollster::block_on(GpuContext::new()).unwrap();
    let (device, queue) = (ctx.device.clone(), ctx.queue.clone());
    let mut cache = PipelineCache::new(device.clone());
    for &(n, range) in &[(1000usize, 1000u32), (100_000, 1000), (1_000_000, 1000), (1_000_000, u32::MAX), (10_000_000, 1000)] {
        let data: Vec<u32> = (0..n as u32).map(|i| i.wrapping_mul(2654435761) % range).collect();
        let mut cpu = data.clone(); cpu.sort_unstable();
        let buf = upload_u32(&device, &queue, &data);
        let out = radix_sort_u32(&device, &queue, &mut cache, &buf);
        let res = pollster::block_on(download_u32(&device, &queue, &out));
        let sorted = res.windows(2).all(|w| w[0] <= w[1]);
        let mut rs = res.clone(); rs.sort_unstable();
        let first = res.iter().zip(&cpu).position(|(a, b)| a != b);
        println!("n={n:>9} range={range:>10}  equal={}  is_sorted={sorted}  same_multiset={}  first_mismatch={:?}", res == cpu, rs == cpu, first);
    }
}
