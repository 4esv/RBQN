// Monkey test: exact integer BQN on Metal, end to end.
// W2 shape: a←↕n ⋄ +´ (a×a)+(a×a), results exceed i32 so i64 is required.
use std::time::Instant;
use wgpu::util::DeviceExt;

const WG: u32 = 256;
const SHADER: &str = r#"
@group(0) @binding(0) var<storage, read_write> a: array<i32>;
@group(0) @binding(1) var<storage, read_write> b: array<i64>;
@group(0) @binding(2) var<storage, read_write> c: array<i64>;
@group(0) @binding(3) var<storage, read_write> o: array<i64>;
@group(0) @binding(4) var<uniform> n: u32;
var<workgroup> sh: array<i64, 256>;

@compute @workgroup_size(256)
fn iota(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x < n) { a[id.x] = i32(id.x); }
}
// b = a*a (i64)
@compute @workgroup_size(256)
fn sq(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x < n) { let v = i64(a[id.x]); b[id.x] = v * v; }
}
// c = b + b
@compute @workgroup_size(256)
fn add(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x < n) { c[id.x] = b[id.x] + b[id.x]; }
}
// o[wg] = sum of c over a grid stride
@compute @workgroup_size(256)
fn reduce(@builtin(global_invocation_id) id: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>, @builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>) {
  var s: i64 = 0;
  var i = id.x;
  let stride = nwg.x * 256u;
  loop { if (i >= n) { break; } s = s + c[i]; i = i + stride; }
  sh[lid.x] = s;
  workgroupBarrier();
  var off = 128u;
  loop { if (off == 0u) { break; }
    if (lid.x < off) { sh[lid.x] = sh[lid.x] + sh[lid.x + off]; }
    workgroupBarrier(); off = off >> 1u; }
  if (lid.x == 0u) { o[wid.x] = sh[0]; }
}
// fused: o[wg] = sum over stride of (a*a)+(a*a), straight from i32 input
@compute @workgroup_size(256)
fn fused(@builtin(global_invocation_id) id: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>, @builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>) {
  var s: i64 = 0;
  var i = id.x;
  let stride = nwg.x * 256u;
  loop { if (i >= n) { break; } let v = i64(a[i]); s = s + (v*v) + (v*v); i = i + stride; }
  sh[lid.x] = s;
  workgroupBarrier();
  var off = 128u;
  loop { if (off == 0u) { break; }
    if (lid.x < off) { sh[lid.x] = sh[lid.x] + sh[lid.x + off]; }
    workgroupBarrier(); off = off >> 1u; }
  if (lid.x == 0u) { o[wid.x] = sh[0]; }
}
// second-stage: sum o[0..n) into o[0]  (n small, one workgroup)
@compute @workgroup_size(256)
fn finish(@builtin(local_invocation_id) lid: vec3<u32>) {
  var s: i64 = 0;
  var i = lid.x;
  loop { if (i >= n) { break; } s = s + o[i]; i = i + 256u; }
  sh[lid.x] = s;
  workgroupBarrier();
  var off = 128u;
  loop { if (off == 0u) { break; }
    if (lid.x < off) { sh[lid.x] = sh[lid.x] + sh[lid.x + off]; }
    workgroupBarrier(); off = off >> 1u; }
  if (lid.x == 0u) { o[0] = sh[0]; }
}
"#;

fn ms(t: Instant) -> f64 { t.elapsed().as_secs_f64() * 1e3 }

fn main() {
    let n: usize = std::env::args().nth(1).map(|s| s.parse().unwrap()).unwrap_or(10_000_000);
    let reps = 10;
    let inst = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends: wgpu::Backends::METAL, ..Default::default() });
    let ad = pollster::block_on(inst.request_adapter(&Default::default())).unwrap();
    let (dev, q) = pollster::block_on(ad.request_device(&wgpu::DeviceDescriptor {
        label: None,
        required_features: wgpu::Features::SHADER_INT64 | wgpu::Features::MAPPABLE_PRIMARY_BUFFERS,
        required_limits: wgpu::Limits { max_storage_buffer_binding_size: ad.limits().max_storage_buffer_binding_size, max_buffer_size: ad.limits().max_buffer_size, ..Default::default() },
        ..Default::default() }, None)).unwrap();
    let module = dev.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
    let sb = |i: u32| wgpu::BindGroupLayoutEntry { binding: i, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None };
    let bgl = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &[sb(0), sb(1), sb(2), sb(3),
        wgpu::BindGroupLayoutEntry { binding: 4, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None }] });
    let pl = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&bgl], push_constant_ranges: &[] });
    let mk = |e: &str| dev.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some(e), layout: Some(&pl), module: &module, entry_point: Some(e), compilation_options: Default::default(), cache: None });
    let t = Instant::now();
    let (p_iota, p_sq, p_add, p_red, p_fused, p_fin) = (mk("iota"), mk("sq"), mk("add"), mk("reduce"), mk("fused"), mk("finish"));
    println!("n={n}  pipelines compiled in {:.2} ms", ms(t));

    let groups = ((n as u32).div_ceil(WG)).min(1024); // reduce grid
    let st = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
    let buf = |size: u64, usage| dev.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage, mapped_at_creation: false });
    let a = buf(4 * n as u64, st | wgpu::BufferUsages::MAP_WRITE);
    let b = buf(8 * n as u64, st);
    let c = buf(8 * n as u64, st);
    let o = buf(8 * 1024, st | wgpu::BufferUsages::MAP_READ);
    let nbuf = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: &(n as u32).to_ne_bytes(), usage: wgpu::BufferUsages::UNIFORM });
    let o_groups = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: &groups.to_ne_bytes(), usage: wgpu::BufferUsages::UNIFORM });
    let bg = |_p: &wgpu::ComputePipeline, nb: &wgpu::Buffer| dev.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &bgl, entries: &[
        wgpu::BindGroupEntry { binding: 0, resource: a.as_entire_binding() },
        wgpu::BindGroupEntry { binding: 1, resource: b.as_entire_binding() },
        wgpu::BindGroupEntry { binding: 2, resource: c.as_entire_binding() },
        wgpu::BindGroupEntry { binding: 3, resource: o.as_entire_binding() },
        wgpu::BindGroupEntry { binding: 4, resource: nb.as_entire_binding() },
    ]});
    let bg_n = bg(&p_iota, &nbuf);
    let bg_fin = bg(&p_fin, &o_groups);
    let staging = buf(8, wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);
    let full = (n as u32).div_ceil(WG);
    let (gx, gy) = if full <= 65535 { (full, 1) } else { (65535, full.div_ceil(65535)) };
    let _ = gy; // n ≤ 16.7M in this test
    assert!(full <= 65535, "keep n ≤ 16.7M for this probe");

    let readback = || -> i64 {
        let s = o.slice(0..8);
        let (tx, rx) = std::sync::mpsc::channel();
        s.map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        loop { dev.poll(wgpu::Maintain::Poll); if let Ok(r) = rx.try_recv() { r.unwrap(); break; } std::hint::spin_loop(); }
        let v = i64::from_ne_bytes(s.get_mapped_range()[..8].try_into().unwrap());
        o.unmap(); v
    };

    let upload = |data: &[i32]| {
        let s = a.slice(..); let (tx, rx) = std::sync::mpsc::channel(); s.map_async(wgpu::MapMode::Write, move |r| tx.send(r).unwrap());
        loop { dev.poll(wgpu::Maintain::Poll); if let Ok(r) = rx.try_recv() { r.unwrap(); break; } std::hint::spin_loop(); }
        s.get_mapped_range_mut().copy_from_slice(bytemuck::cast_slice(data)); a.unmap();
    };
    // CPU data origin, like ↕n in the interpreter
    let host: Vec<i32> = (0..n as i32).collect();
    let expect: i64 = host.iter().map(|&v| { let v = v as i64; v*v + v*v }).sum();

    // CPU baselines (best-effort optimal Rust)
    let t = Instant::now();
    for _ in 0..reps { let s: i64 = host.iter().map(|&v| { let v = v as i64; v*v + v*v }).sum(); assert_eq!(s, expect); }
    println!("CPU fused chain+sum          {:>8.3} ms", ms(t) / reps as f64);
    let t = Instant::now();
    for _ in 0..reps {
        let b: Vec<i64> = host.iter().map(|&v| (v as i64) * (v as i64)).collect();
        let c: Vec<i64> = b.iter().map(|&v| v + v).collect();
        let s: i64 = c.iter().sum(); assert_eq!(s, expect);
    }
    println!("CPU materialized (3 passes)  {:>8.3} ms", ms(t) / reps as f64);

    // Upload cost variants
    let t = Instant::now();
    for _ in 0..reps { q.write_buffer(&a, 0, bytemuck::cast_slice(&host)); q.submit([]); dev.poll(wgpu::Maintain::Wait); }
    println!("upload write_buffer          {:>8.3} ms", ms(t) / reps as f64);
    let t = Instant::now();
    for _ in 0..reps {
        let m = dev.create_buffer(&wgpu::BufferDescriptor { label: None, size: 4 * n as u64, usage: st | wgpu::BufferUsages::MAP_WRITE, mapped_at_creation: true });
        m.slice(..).get_mapped_range_mut().copy_from_slice(bytemuck::cast_slice(&host)); m.unmap();
        q.submit([]); dev.poll(wgpu::Maintain::Wait);
    }
    println!("upload mapped_at_creation    {:>8.3} ms  (unified memory, one memcpy)", ms(t) / reps as f64);

    let run = |label: &str, f: &dyn Fn(&mut wgpu::CommandEncoder)| {
        // warm
        let mut e = dev.create_command_encoder(&Default::default()); f(&mut e); q.submit([e.finish()]); assert_eq!(readback(), expect, "{label}");
        let t = Instant::now();
        for _ in 0..reps { let mut e = dev.create_command_encoder(&Default::default()); f(&mut e); q.submit([e.finish()]); let v = readback(); assert_eq!(v, expect); }
        println!("{label:28} {:>8.3} ms  (incl. 8-byte readback)", ms(t) / reps as f64);
    };
    let pass = |e: &mut wgpu::CommandEncoder, p: &wgpu::ComputePipeline, b: &wgpu::BindGroup, g: u32| {
        let mut ps = e.begin_compute_pass(&Default::default()); ps.set_pipeline(p); ps.set_bind_group(0, b, &[]); ps.dispatch_workgroups(g, 1, 1);
    };
    let (bg_sq, bg_add, bg_red, bg_fused) = (bg(&p_sq, &nbuf), bg(&p_add, &nbuf), bg(&p_red, &nbuf), bg(&p_fused, &nbuf));
    upload(&host);
    run("GPU unfused resident (sq,add,reduce)", &|e| { pass(e, &p_sq, &bg_sq, gx); pass(e, &p_add, &bg_add, gx); pass(e, &p_red, &bg_red, groups); pass(e, &p_fin, &bg_fin, 1); });
    run("GPU fused resident (1 map-reduce)", &|e| { pass(e, &p_fused, &bg_fused, groups); pass(e, &p_fin, &bg_fin, 1); });
    run("GPU iota-on-gpu + fused", &|e| { pass(e, &p_iota, &bg_n, gx); pass(e, &p_fused, &bg_fused, groups); pass(e, &p_fin, &bg_fin, 1); });
    // end-to-end from CPU-origin data: upload + fused
    let t = Instant::now();
    for _ in 0..reps { upload(&host); let mut e = dev.create_command_encoder(&Default::default()); pass(&mut e, &p_fused, &bg_fused, groups); pass(&mut e, &p_fin, &bg_fin, 1); q.submit([e.finish()]); assert_eq!(readback(), expect); }
    println!("GPU mapupload+fused+readback{:>9.3} ms  (CPU-origin data, end to end)", ms(t) / reps as f64);
    let t = Instant::now();
    for _ in 0..reps { upload(&host); let mut e = dev.create_command_encoder(&Default::default()); pass(&mut e, &p_sq, &bg_sq, gx); pass(&mut e, &p_add, &bg_add, gx); pass(&mut e, &p_red, &bg_red, groups); pass(&mut e, &p_fin, &bg_fin, 1); q.submit([e.finish()]); assert_eq!(readback(), expect); }
    println!("GPU mapupload+unfused+readbk{:>9.3} ms  (CPU-origin data, end to end)", ms(t) / reps as f64);
}
