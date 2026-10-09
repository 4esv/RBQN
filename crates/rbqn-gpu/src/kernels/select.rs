use std::sync::Arc;
use wgpu;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::dispatch::{WORKGROUP_SIZE, grid_for};
use crate::pipeline::{PipelineCache, PipelineKey};

const SHADER_F32: &str = include_str!("../shaders/select_f32.wgsl");
const SHADER_I32: &str = include_str!("../shaders/select_i32.wgsl");
const SHADER_U32: &str = include_str!("../shaders/select_u32.wgsl");

fn shader_for(elem: ElementKind) -> (&'static str, &'static str) {
    match elem {
        ElementKind::F32 => ("select_f32", SHADER_F32),
        ElementKind::I32 => ("select_i32", SHADER_I32),
        ElementKind::U32 => ("select_u32", SHADER_U32),
        ElementKind::I64 => unreachable!("I64 buffers use the *_i64 kernels"),
    }
}

pub fn gather(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    data: &GpuBuffer,
    indices: &GpuBuffer,
    out: &GpuBuffer,
) {
    let elem = data.element_type();
    let (shader_id, source) = shader_for(elem);
    let key = PipelineKey::new(shader_id, "gather", elem);
    let pipeline = cache.get_or_create(&key, source);

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: data.inner().as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: indices.inner().as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: out.inner().as_entire_binding() },
        ],
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let (gx, gy) = grid_for(out.len(), WORKGROUP_SIZE);
        pass.dispatch_workgroups(gx, gy, 1); crate::stats::dispatch();
    }
    queue.submit(std::iter::once(encoder.finish())); crate::stats::submit();
}
