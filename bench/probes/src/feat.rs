fn main() {
    let inst = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends: wgpu::Backends::METAL, ..Default::default() });
    let ad = pollster::block_on(inst.request_adapter(&Default::default())).unwrap();
    let f = ad.features();
    println!("{:?}", ad.get_info().name);
    for (n, x) in [("SHADER_F64", wgpu::Features::SHADER_F64), ("SUBGROUP", wgpu::Features::SUBGROUP), ("SHADER_INT64", wgpu::Features::SHADER_INT64), ("SHADER_F16", wgpu::Features::SHADER_F16), ("MAPPABLE_PRIMARY_BUFFERS", wgpu::Features::MAPPABLE_PRIMARY_BUFFERS), ("TIMESTAMP_QUERY", wgpu::Features::TIMESTAMP_QUERY)] { println!("{n:26} {}", f.contains(x)); }
    let l = ad.limits(); println!("max_buffer_size {} GB, subgroup {}..{}", l.max_buffer_size as f64/1e9, l.min_subgroup_size, l.max_subgroup_size);
}
