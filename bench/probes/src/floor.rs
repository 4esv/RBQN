// Sync floor and upload floor on Metal via wgpu 24.
use std::time::Instant;
fn ms(t: Instant) -> f64 { t.elapsed().as_secs_f64() * 1e3 }
fn main() {
    let n: usize = 10_000_000; let reps = 20;
    let inst = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends: wgpu::Backends::METAL, ..Default::default() });
    let ad = pollster::block_on(inst.request_adapter(&Default::default())).unwrap();
    let (dev, q) = pollster::block_on(ad.request_device(&wgpu::DeviceDescriptor { label: None,
        required_features: wgpu::Features::MAPPABLE_PRIMARY_BUFFERS,
        required_limits: wgpu::Limits { max_storage_buffer_binding_size: ad.limits().max_storage_buffer_binding_size, max_buffer_size: ad.limits().max_buffer_size, ..Default::default() }, ..Default::default() }, None)).unwrap();
    let host: Vec<i32> = (0..n as i32).collect();
    let mut dst: Vec<i32> = vec![0; n];
    let t = Instant::now(); for _ in 0..reps { dst.copy_from_slice(&host); std::hint::black_box(&dst); }
    println!("CPU memcpy 40MB                 {:>8.3} ms", ms(t)/reps as f64);

    // 1. empty submit + poll(Wait)
    let t = Instant::now(); for _ in 0..reps { let e = dev.create_command_encoder(&Default::default()); q.submit([e.finish()]); dev.poll(wgpu::Maintain::Wait); }
    println!("empty submit + poll(Wait)       {:>8.3} ms", ms(t)/reps as f64);
    // 2. empty submit + spin poll(Poll) until on_submitted_work_done
    let t = Instant::now(); for _ in 0..reps { let e = dev.create_command_encoder(&Default::default()); q.submit([e.finish()]);
        let (tx, rx) = std::sync::mpsc::channel(); q.on_submitted_work_done(move || tx.send(()).unwrap());
        loop { dev.poll(wgpu::Maintain::Poll); if rx.try_recv().is_ok() { break; } std::hint::spin_loop(); } }
    println!("empty submit + spin Poll        {:>8.3} ms", ms(t)/reps as f64);
    // 3. 8-byte readback: staging copy + map_async + Wait (what rbqn does)
    let o = dev.create_buffer(&wgpu::BufferDescriptor { label: None, size: 8, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
    let staging = dev.create_buffer(&wgpu::BufferDescriptor { label: None, size: 8, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let t = Instant::now(); for _ in 0..reps { let mut e = dev.create_command_encoder(&Default::default()); e.copy_buffer_to_buffer(&o, 0, &staging, 0, 8); q.submit([e.finish()]);
        let s = staging.slice(..); let (tx, rx) = std::sync::mpsc::channel(); s.map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        dev.poll(wgpu::Maintain::Wait); rx.recv().unwrap().unwrap(); let _ = s.get_mapped_range()[0]; staging.unmap(); }
    println!("8B readback staging+map+Wait    {:>8.3} ms", ms(t)/reps as f64);
    // 4. 8-byte readback via spin Poll
    let t = Instant::now(); for _ in 0..reps { let mut e = dev.create_command_encoder(&Default::default()); e.copy_buffer_to_buffer(&o, 0, &staging, 0, 8); q.submit([e.finish()]);
        let s = staging.slice(..); let (tx, rx) = std::sync::mpsc::channel(); s.map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        loop { dev.poll(wgpu::Maintain::Poll); if let Ok(r) = rx.try_recv() { r.unwrap(); break; } std::hint::spin_loop(); } let _ = s.get_mapped_range()[0]; staging.unmap(); }
    println!("8B readback staging+map+spin    {:>8.3} ms", ms(t)/reps as f64);
    // 5. 8-byte readback: mappable primary (no staging copy), map directly
    let om = dev.create_buffer(&wgpu::BufferDescriptor { label: None, size: 8, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    let t = Instant::now(); for _ in 0..reps { let e = dev.create_command_encoder(&Default::default()); q.submit([e.finish()]);
        let s = om.slice(..); let (tx, rx) = std::sync::mpsc::channel(); s.map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        loop { dev.poll(wgpu::Maintain::Poll); if let Ok(r) = rx.try_recv() { r.unwrap(); break; } std::hint::spin_loop(); } let _ = s.get_mapped_range()[0]; om.unmap(); }
    println!("8B readback mappable+spin       {:>8.3} ms", ms(t)/reps as f64);

    // Upload: persistent STORAGE|MAP_WRITE buffer, map → memcpy → unmap, no blit
    let a = dev.create_buffer(&wgpu::BufferDescriptor { label: None, size: 4 * n as u64, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::MAP_WRITE, mapped_at_creation: false });
    let t = Instant::now(); for _ in 0..reps {
        let s = a.slice(..); let (tx, rx) = std::sync::mpsc::channel(); s.map_async(wgpu::MapMode::Write, move |r| tx.send(r).unwrap());
        loop { dev.poll(wgpu::Maintain::Poll); if let Ok(r) = rx.try_recv() { r.unwrap(); break; } std::hint::spin_loop(); }
        s.get_mapped_range_mut().copy_from_slice(bytemuck::cast_slice(&host)); a.unmap(); }
    println!("upload 40MB persistent MAP_WRITE{:>8.3} ms (map+memcpy+unmap)", ms(t)/reps as f64);
    // Upload: write_buffer into persistent STORAGE|COPY_DST (what rbqn does), with Wait
    let b = dev.create_buffer(&wgpu::BufferDescriptor { label: None, size: 4 * n as u64, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let t = Instant::now(); for _ in 0..reps { q.write_buffer(&b, 0, bytemuck::cast_slice(&host)); q.submit([]); dev.poll(wgpu::Maintain::Wait); }
    println!("upload 40MB write_buffer+Wait   {:>8.3} ms", ms(t)/reps as f64);
    // Upload: write_buffer, no wait (cost on caller thread only)
    let t = Instant::now(); for _ in 0..reps { q.write_buffer(&b, 0, bytemuck::cast_slice(&host)); }
    q.submit([]); dev.poll(wgpu::Maintain::Wait);
    println!("upload 40MB write_buffer nowait {:>8.3} ms", ms(t)/reps as f64);
}
