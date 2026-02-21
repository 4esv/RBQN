use std::sync::Arc;
use wgpu;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::dispatch::{WORKGROUP_SIZE, workgroup_count};
use crate::pipeline::{PipelineCache, PipelineKey};

const SHADER_F32: &str = include_str!("../shaders/reduce_f32.wgsl");
const SHADER_I32: &str = include_str!("../shaders/reduce_i32.wgsl");

fn shader_for(elem: ElementKind) -> (&'static str, &'static str) {
    match elem {
        ElementKind::F32 => ("reduce_f32", SHADER_F32),
        ElementKind::I32 | ElementKind::U32 => ("reduce_i32", SHADER_I32),
    }
}

fn dispatch_reduce_pass(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    pipeline: &wgpu::ComputePipeline,
    input: &GpuBuffer,
    output: &GpuBuffer,
) {
    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: input.inner().as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: output.inner().as_entire_binding() },
        ],
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(workgroup_count(input.len(), WORKGROUP_SIZE), 1, 1);
    }
    queue.submit(std::iter::once(encoder.finish()));
}

pub fn reduce(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    op: &str,
    input: &GpuBuffer,
) -> GpuBuffer {
    let elem = input.element_type();
    let (shader_id, source) = shader_for(elem);
    let key = PipelineKey::new(shader_id, &format!("reduce_{op}"), elem);

    let mut current_len = input.len();
    let mut current_input = None::<GpuBuffer>;
    let mut intermediates = Vec::new();

    loop {
        let num_groups = workgroup_count(current_len, WORKGROUP_SIZE) as usize;
        let output = GpuBuffer::storage(device, elem, num_groups);

        let pipeline = cache.get_or_create(&key, source);
        let src = current_input.as_ref().unwrap_or(input);
        dispatch_reduce_pass(device, queue, pipeline, src, &output);

        if num_groups == 1 {
            return output;
        }

        current_len = num_groups;
        if let Some(old) = current_input.take() {
            intermediates.push(old);
        }
        current_input = Some(output);
    }
}
