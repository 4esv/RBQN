use std::sync::Arc;
use wgpu;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::dispatch::{WORKGROUP_SIZE, workgroup_grid};
use crate::kernels::int64;
use crate::pipeline::{PassBatch, PipelineCache, PipelineKey};

const SHADER_F32: &str = include_str!("../shaders/scan_f32.wgsl");
pub(crate) const SHADER_I32: &str = include_str!("../shaders/scan_i32.wgsl");

/// Elements per workgroup in scan_i32.wgsl (256 threads x 8).
const TILE_I32: usize = 2048;

const ELEMENTS_PER_WORKGROUP: usize = (WORKGROUP_SIZE * 2) as usize;

fn shader_for(elem: ElementKind) -> (&'static str, &'static str) {
    match elem {
        ElementKind::F32 => ("scan_f32", SHADER_F32),
        ElementKind::I32 | ElementKind::U32 => unreachable!("I32/U32 use scan_i32_blocks"),
        ElementKind::I64 => unreachable!("I64 buffers use the *_i64 kernels"),
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

fn scan_i32_blocks(
    device: &Arc<wgpu::Device>,
    batch: &mut PassBatch,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
    out: &GpuBuffer,
    exclusive: bool,
) {
    let elem = input.element_type();
    let n = input.len();
    assert_eq!(out.len(), n);
    assert_eq!(out.element_type(), elem);
    let blocks = n.div_ceil(TILE_I32);
    let final_key = PipelineKey::raw("scan_i32", if exclusive { "scan_excl" } else { "scan_incl" });
    if blocks == 1 {
        let dummy = GpuBuffer::storage(device, elem, 1);
        let pipeline = cache.get_or_create(&final_key, SHADER_I32);
        let bg = int64::bind(device, pipeline, &[(0, input), (1, out), (2, &dummy)]);
        batch.push(pipeline, &bg, workgroup_grid(1));
        return;
    }
    let sums = GpuBuffer::storage(device, elem, blocks);
    let pipeline = cache.get_or_create(&PipelineKey::raw("scan_i32", "block_sum"), SHADER_I32);
    let bg = int64::bind(device, pipeline, &[(0, input), (2, &sums)]);
    batch.push(pipeline, &bg, workgroup_grid(blocks as u32));
    // carries: inclusive scan of the block sums
    let scanned = GpuBuffer::storage(device, elem, blocks);
    scan_i32_blocks(device, batch, cache, &sums, &scanned, false);
    let pipeline = cache.get_or_create(&final_key, SHADER_I32);
    let bg = int64::bind(device, pipeline, &[(0, input), (1, out), (2, &scanned)]);
    batch.push(pipeline, &bg, workgroup_grid(blocks as u32));
}

fn scan_i32_all(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
    exclusive: bool,
) -> GpuBuffer {
    let out = GpuBuffer::storage(device, input.element_type(), input.len());
    scan_i32_into(device, queue, cache, input, &out, exclusive);
    out
}

fn scan_i32_into(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
    out: &GpuBuffer,
    exclusive: bool,
) {
    let mut batch = PassBatch::new(device);
    scan_i32_blocks(device, &mut batch, cache, input, out, exclusive);
    batch.submit(queue);
}

/// Inclusive scan of an I32/U32 buffer into a caller-provided `out` (same
/// length and type), so the caller can pool the n-sized allocation: a fresh
/// 40 MB buffer costs ~1 ms (allocation plus wgpu's lazy zero-fill).
pub fn inclusive_scan_into(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
    out: &GpuBuffer,
) {
    assert!(matches!(input.element_type(), ElementKind::I32 | ElementKind::U32));
    scan_i32_into(device, queue, cache, input, out, false);
}

pub fn exclusive_scan(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
) -> GpuBuffer {
    match input.element_type() {
        ElementKind::I32 | ElementKind::U32 => scan_i32_all(device, queue, cache, input, true),
        _ => exclusive_scan_f32(device, queue, cache, input),
    }
}

fn exclusive_scan_f32(
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

    let scanned_block_sums = exclusive_scan_f32(device, queue, cache, &block_sums);

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
    if matches!(elem, ElementKind::I32 | ElementKind::U32) {
        return scan_i32_all(device, queue, cache, input, false);
    }

    let exclusive = exclusive_scan_f32(device, queue, cache, input);

    // inclusive = exclusive + input (element-wise)
    let result = GpuBuffer::storage(device, elem, input.len());
    crate::kernels::arith::arith_binary(
        device, queue, cache, "add", &exclusive, input, &result,
    );
    result
}
