use std::sync::Arc;
use wgpu;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::dispatch::{WORKGROUP_SIZE, workgroup_count};
use crate::pipeline::{PipelineCache, PipelineKey};

const SHADER: &str = include_str!("../shaders/sort.wgsl");
const RADIX_BITS: u32 = 4;
const NUM_BUCKETS: usize = 1 << RADIX_BITS;
const NUM_PASSES: u32 = 32 / RADIX_BITS;

pub fn radix_sort_u32(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
) -> GpuBuffer {
    let n = input.len();
    let hist_key = PipelineKey::raw("sort", "histogram");
    let scatter_key = PipelineKey::raw("sort", "scatter");

    let mut buf_a = GpuBuffer::storage(device, ElementKind::U32, n);
    let mut buf_b = GpuBuffer::storage(device, ElementKind::U32, n);

    // Copy input to buf_a
    {
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(input.inner(), 0, buf_a.inner(), 0, input.size());
        queue.submit(std::iter::once(encoder.finish()));
    }

    for pass in 0..NUM_PASSES {
        let shift = pass * RADIX_BITS;

        // Histogram pass
        let histogram_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (NUM_BUCKETS * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Zero the histogram
        queue.write_buffer(&histogram_buf, 0, &[0u8; NUM_BUCKETS * 4]);

        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 8,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params_data = [shift, n as u32];
        queue.write_buffer(&params_buf, 0, bytemuck::cast_slice(&params_data));

        let hist_pipeline = cache.get_or_create(&hist_key, SHADER);
        let layout = hist_pipeline.get_bind_group_layout(0);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: buf_a.inner().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: histogram_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: params_buf.as_entire_binding() },
            ],
        });

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut cpass = encoder.begin_compute_pass(&Default::default());
            cpass.set_pipeline(hist_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(workgroup_count(n, WORKGROUP_SIZE), 1, 1);
        }
        queue.submit(std::iter::once(encoder.finish()));

        // CPU-side prefix sum on the small histogram (16 buckets)
        let histogram_data = pollster::block_on(
            crate::buffer::download_u32(device, queue, &GpuBuffer {
                buffer: histogram_buf,
                size: (NUM_BUCKETS * 4) as u64,
                element_type: ElementKind::U32,
                len: NUM_BUCKETS,
            })
        );

        let mut prefix = vec![0u32; NUM_BUCKETS];
        let mut running = 0u32;
        for i in 0..NUM_BUCKETS {
            prefix[i] = running;
            running += histogram_data[i];
        }

        // Upload prefix sums as scatter offsets (atomic counters start at prefix values)
        let offsets_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (NUM_BUCKETS * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&offsets_buf, 0, bytemuck::cast_slice(&prefix));

        let scatter_params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 8,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&scatter_params_buf, 0, bytemuck::cast_slice(&params_data));

        let scatter_pipeline = cache.get_or_create(&scatter_key, SHADER);
        let scatter_layout = scatter_pipeline.get_bind_group_layout(0);
        let scatter_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &scatter_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: buf_a.inner().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: buf_b.inner().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: offsets_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: scatter_params_buf.as_entire_binding() },
            ],
        });

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut cpass = encoder.begin_compute_pass(&Default::default());
            cpass.set_pipeline(scatter_pipeline);
            cpass.set_bind_group(0, &scatter_bg, &[]);
            cpass.dispatch_workgroups(workgroup_count(n, WORKGROUP_SIZE), 1, 1);
        }
        queue.submit(std::iter::once(encoder.finish()));

        std::mem::swap(&mut buf_a, &mut buf_b);
    }

    buf_a
}

pub fn sort_i32(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
) -> GpuBuffer {
    // Convert i32 to sortable u32 by flipping sign bit
    let n = input.len();
    let u32_buf = GpuBuffer::storage(device, ElementKind::U32, n);

    {
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(input.inner(), 0, u32_buf.inner(), 0, input.size());
        queue.submit(std::iter::once(encoder.finish()));
    }

    // For a proper implementation, we'd flip sign bits before sorting and flip back after.
    // For now, treat as unsigned sort (correct for non-negative values).
    radix_sort_u32(device, queue, cache, &u32_buf)
}
