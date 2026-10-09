use std::sync::Arc;
use wgpu;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::dispatch::{WORKGROUP_SIZE, grid_for};
use crate::pipeline::{PipelineCache, PipelineKey};

const SHADER_F32: &str = include_str!("../shaders/arith_f32.wgsl");
const SHADER_I32: &str = include_str!("../shaders/arith_i32.wgsl");
const SHADER_SCALAR_F32: &str = include_str!("../shaders/arith_scalar_f32.wgsl");
const SHADER_SCALAR_I32: &str = include_str!("../shaders/arith_scalar_i32.wgsl");

fn shader_for(elem: ElementKind) -> (&'static str, &'static str) {
    match elem {
        ElementKind::F32 => ("arith_f32", SHADER_F32),
        ElementKind::I32 | ElementKind::U32 => ("arith_i32", SHADER_I32),
        ElementKind::I64 => unreachable!("I64 buffers use the *_i64 kernels"),
    }
}

fn scalar_shader_for(elem: ElementKind) -> (&'static str, &'static str) {
    match elem {
        ElementKind::F32 => ("arith_scalar_f32", SHADER_SCALAR_F32),
        ElementKind::I32 | ElementKind::U32 => ("arith_scalar_i32", SHADER_SCALAR_I32),
        ElementKind::I64 => unreachable!("I64 buffers use the *_i64 kernels"),
    }
}

pub fn arith_binary(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    op: &str,
    a: &GpuBuffer,
    b: &GpuBuffer,
    out: &GpuBuffer,
) {
    let elem = a.element_type();
    let (shader_id, source) = shader_for(elem);
    let key = PipelineKey::new(shader_id, op, elem);
    let pipeline = cache.get_or_create(&key, source);

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: a.inner().as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: b.inner().as_entire_binding() },
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

pub fn arith_scalar(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    op: &str,
    a: &GpuBuffer,
    scalar: f32,
    out: &GpuBuffer,
) {
    let elem = a.element_type();
    let (shader_id, source) = scalar_shader_for(elem);
    let entry = match elem {
        ElementKind::F32 => format!("scalar_{op}_f32"),
        ElementKind::I32 | ElementKind::U32 => format!("scalar_{op}_i32"),
        ElementKind::I64 => unreachable!("I64 buffers use the *_i64 kernels"),
    };
    let key = PipelineKey::raw(shader_id, &entry);
    let pipeline = cache.get_or_create(&key, source);

    let scalar_bytes: [u8; 4] = if elem == ElementKind::F32 {
        scalar.to_ne_bytes()
    } else {
        (scalar as i32).to_ne_bytes()
    };

    let scalar_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 4,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&scalar_buf, 0, &scalar_bytes);

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: a.inner().as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: scalar_buf.as_entire_binding() },
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
