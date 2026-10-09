use std::sync::Arc;
use wgpu;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::pipeline::{PipelineCache, PipelineKey};

const SHADER_F32: &str = include_str!("../shaders/softmax_f32.wgsl");

/// Compute softmax of `input`, writing probabilities to `out`.
///
/// Single-workgroup path handles vectors up to 256 elements.
/// For larger vectors, falls back to a multi-pass composition using
/// existing reduce and arith_scalar kernels.
pub fn softmax(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
    out: &GpuBuffer,
) {
    assert_eq!(input.element_type(), ElementKind::F32, "softmax only supports f32 buffers");

    if input.len() <= 256 {
        softmax_single_workgroup(device, queue, cache, input, out);
    } else {
        softmax_multi_pass(device, queue, cache, input, out);
    }
}

fn softmax_single_workgroup(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
    out: &GpuBuffer,
) {
    let key = PipelineKey::raw("softmax_f32", "softmax_f32");
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
        // Single workgroup — the shader handles all 256 lanes internally.
        pass.dispatch_workgroups(1, 1, 1);
    }
    queue.submit(std::iter::once(encoder.finish()));
}

/// Multi-pass softmax for vectors larger than 256 elements.
/// Composes: reduce(max) -> arith_scalar(sub) -> unary(exp) -> reduce(add) -> arith_scalar(div).
fn softmax_multi_pass(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
    out: &GpuBuffer,
) {
    use crate::buffer::GpuBuffer as Buf;
    use crate::kernels::{arith, reduce, unary};

    // 1. Find global max.
    let max_buf = reduce::reduce(device, queue, cache, "max", input);

    // 2. Download max scalar (single element).
    let max_val: f32 = {
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(max_buf.inner(), 0, &staging, 0, 4);
        queue.submit(std::iter::once(encoder.finish()));

        let slice = staging.slice(..);
        crate::buffer::map_sync(device, slice, wgpu::MapMode::Read);
        let view = slice.get_mapped_range();
        let v: f32 = bytemuck::cast_slice::<u8, f32>(&view)[0];
        drop(view);
        staging.unmap();
        v
    };

    // 3. Subtract max from all elements -> shifted buffer.
    let shifted = Buf::storage(device, ElementKind::F32, input.len());
    arith::arith_scalar(device, queue, cache, "sub", input, max_val, &shifted);

    // 4. exp(shifted) -> exp buffer.
    let exp_buf = Buf::storage(device, ElementKind::F32, input.len());
    unary::unary_op(device, queue, cache, "exp", &shifted, &exp_buf);

    // 5. Find sum of exp values.
    let sum_buf = reduce::reduce(device, queue, cache, "add", &exp_buf);

    // 6. Download sum scalar.
    let sum_val: f32 = {
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(sum_buf.inner(), 0, &staging, 0, 4);
        queue.submit(std::iter::once(encoder.finish()));

        let slice = staging.slice(..);
        crate::buffer::map_sync(device, slice, wgpu::MapMode::Read);
        let view = slice.get_mapped_range();
        let v: f32 = bytemuck::cast_slice::<u8, f32>(&view)[0];
        drop(view);
        staging.unmap();
        v
    };

    // 7. Divide exp values by sum -> output probabilities.
    arith::arith_scalar(device, queue, cache, "div", &exp_buf, sum_val, out);
}
