use std::sync::Arc;
use wgpu;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::dispatch::{WORKGROUP_SIZE, grid_for, workgroup_count};
use crate::pipeline::{PassBatch, PipelineCache, PipelineKey};

const SHADER_F32: &str = include_str!("../shaders/reduce_f32.wgsl");
const SHADER_I32: &str = include_str!("../shaders/reduce_i32.wgsl");

fn shader_for(elem: ElementKind) -> (&'static str, &'static str) {
    match elem {
        ElementKind::F32 => ("reduce_f32", SHADER_F32),
        ElementKind::I32 | ElementKind::U32 => ("reduce_i32", SHADER_I32),
        ElementKind::I64 => unreachable!("I64 buffers use the *_i64 kernels"),
    }
}

/// Elements per workgroup: the i32 kernels load 4 per thread, f32 loads 1.
fn block_for(elem: ElementKind) -> u32 {
    match elem {
        ElementKind::I32 | ElementKind::U32 => WORKGROUP_SIZE * 4,
        _ => WORKGROUP_SIZE,
    }
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

    let mut current: Option<GpuBuffer> = None;
    let mut batch = PassBatch::new(device);

    loop {
        let src = current.as_ref().unwrap_or(input);
        let num_groups = workgroup_count(src.len(), block_for(elem)) as usize;
        let output = GpuBuffer::storage(device, elem, num_groups);

        let pipeline = cache.get_or_create(&key, source);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: src.inner().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: output.inner().as_entire_binding() },
            ],
        });
        batch.push(pipeline, &bind_group, grid_for(src.len(), block_for(elem)));

        if num_groups == 1 {
            batch.submit(queue);
            return output;
        }
        current = Some(output);
    }
}
