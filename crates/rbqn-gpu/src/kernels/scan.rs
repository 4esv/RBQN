use std::sync::Arc;
use wgpu;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::dispatch::{WORKGROUP_SIZE, workgroup_grid};
use crate::pipeline::{PipelineCache, PipelineKey};

const SHADER_F32: &str = include_str!("../shaders/scan_f32.wgsl");
const SHADER_I32: &str = include_str!("../shaders/scan_i32.wgsl");

const ELEMENTS_PER_WORKGROUP: usize = (WORKGROUP_SIZE * 2) as usize;

fn shader_for(elem: ElementKind) -> (&'static str, &'static str) {
    match elem {
        ElementKind::F32 => ("scan_f32", SHADER_F32),
        ElementKind::I32 | ElementKind::U32 => ("scan_i32", SHADER_I32),
    }
}

fn dispatch_scan_pass(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    pipeline: &wgpu::ComputePipeline,
    input: &GpuBuffer,
    output: &GpuBuffer,
    block_sums: &GpuBuffer,
) {
    let num_groups = input.len().div_ceil(ELEMENTS_PER_WORKGROUP);

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: input.inner().as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: output.inner().as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: block_sums.inner().as_entire_binding() },
        ],
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let (gx, gy) = workgroup_grid(num_groups as u32);
        pass.dispatch_workgroups(gx, gy, 1); crate::stats::dispatch();
    }
    queue.submit(std::iter::once(encoder.finish())); crate::stats::submit();
}

fn dispatch_propagate(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    pipeline: &wgpu::ComputePipeline,
    data: &GpuBuffer,
    output: &GpuBuffer,
    prefix_sums: &GpuBuffer,
) {
    let num_groups = data.len().div_ceil(WORKGROUP_SIZE as usize);

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: data.inner().as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: output.inner().as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: prefix_sums.inner().as_entire_binding() },
        ],
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let (gx, gy) = workgroup_grid(num_groups as u32);
        pass.dispatch_workgroups(gx, gy, 1); crate::stats::dispatch();
    }
    queue.submit(std::iter::once(encoder.finish())); crate::stats::submit();
}

pub fn exclusive_scan(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
) -> GpuBuffer {
    let elem = input.element_type();
    let (shader_id, source) = shader_for(elem);
    let scan_key = PipelineKey::new(shader_id, "scan_add", elem);
    let prop_key = PipelineKey::new(shader_id, "propagate", elem);
    let n = input.len();

    if n <= ELEMENTS_PER_WORKGROUP {
        let output = GpuBuffer::storage(device, elem, n);
        let block_sums = GpuBuffer::storage(device, elem, 1);
        let pipeline = cache.get_or_create(&scan_key, source);
        dispatch_scan_pass(device, queue, pipeline, input, &output, &block_sums);
        return output;
    }

    let num_blocks = n.div_ceil(ELEMENTS_PER_WORKGROUP);
    let output = GpuBuffer::storage(device, elem, n);
    let block_sums = GpuBuffer::storage(device, elem, num_blocks);

    let pipeline = cache.get_or_create(&scan_key, source);
    dispatch_scan_pass(device, queue, pipeline, input, &output, &block_sums);

    let scanned_block_sums = exclusive_scan(device, queue, cache, &block_sums);

    let pipeline = cache.get_or_create(&prop_key, source);
    let final_output = GpuBuffer::storage(device, elem, n);
    dispatch_propagate(device, queue, pipeline, &output, &final_output, &scanned_block_sums);

    final_output
}

pub fn inclusive_scan(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
) -> GpuBuffer {
    let elem = input.element_type();
    let (shader_id, source) = shader_for(elem);

    let exclusive = exclusive_scan(device, queue, cache, input);

    // inclusive = exclusive + input (element-wise)
    let _ = (shader_id, source);

    let result = GpuBuffer::storage(device, elem, input.len());
    crate::kernels::arith::arith_binary(
        device, queue, cache, "add", &exclusive, input, &result,
    );
    result
}
