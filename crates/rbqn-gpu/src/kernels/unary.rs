use std::sync::Arc;
use wgpu;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::dispatch::{WORKGROUP_SIZE, workgroup_count};
use crate::pipeline::{PipelineCache, PipelineKey};

const SHADER_F32: &str = include_str!("../shaders/unary_f32.wgsl");

pub fn unary_op(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    op: &str,
    input: &GpuBuffer,
    out: &GpuBuffer,
) {
    let elem = input.element_type();
    // NOTE: Only f32 is supported for unary ops currently.
    assert_eq!(elem, ElementKind::F32, "unary_op only supports f32 buffers");

    let key = PipelineKey::new("unary_f32", op, elem);
    let pipeline = cache.get_or_create(&key, SHADER_F32);

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: input.inner().as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: out.inner().as_entire_binding() },
        ],
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(workgroup_count(out.len(), WORKGROUP_SIZE), 1, 1);
    }
    queue.submit(std::iter::once(encoder.finish()));
}
