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

/// GPU-accelerated argsort: returns permutation indices that would sort the input i32 array.
/// Uses GPU radix sort on sign-bit-flipped u32 keys, then reconstructs indices on CPU.
/// The radix sort (O(n)) runs on GPU; index reconstruction (O(n)) runs on CPU.
pub fn argsort_i32(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
) -> Vec<i32> {
    let n = input.len();
    if n == 0 {
        return vec![];
    }

    // Download i32 data from GPU.
    let i32_data = pollster::block_on(
        crate::buffer::download_i32(device, queue, input)
    );

    // Convert to sortable u32 keys (XOR sign bit so u32 ordering matches i32 ordering).
    let original_keys: Vec<u32> = i32_data.iter().map(|&v| (v as u32) ^ 0x8000_0000u32).collect();

    // Upload u32 keys and sort on GPU.
    let u32_buf = crate::buffer::upload_u32(device, queue, &original_keys);
    let sorted_u32_buf = radix_sort_u32(device, queue, cache, &u32_buf);
    let sorted_keys = pollster::block_on(
        crate::buffer::download_u32(device, queue, &sorted_u32_buf)
    );

    // Reconstruct permutation indices on CPU using a position queue per key value.
    // This is O(n) with a HashMap of Vec<usize> queues for stable matching.
    use std::collections::HashMap;
    let mut positions: HashMap<u32, std::collections::VecDeque<usize>> = HashMap::new();
    for (i, &k) in original_keys.iter().enumerate() {
        positions.entry(k).or_default().push_back(i);
    }

    let indices: Vec<i32> = sorted_keys.iter().map(|k| {
        positions.get_mut(k).and_then(|q| q.pop_front()).unwrap_or(0) as i32
    }).collect();

    indices
}

pub fn sort_i32(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
) -> GpuBuffer {
    // Download i32 data from GPU.
    let i32_data = pollster::block_on(
        crate::buffer::download_i32(device, queue, input)
    );

    // XOR sign bit to map i32 ordering to u32 ordering.
    // NOTE: This maps i32::MIN (0x80000000) to u32 0, i32::MAX (0x7FFFFFFF) to u32::MAX.
    let u32_data: Vec<u32> = i32_data.iter().map(|&v| (v as u32) ^ 0x8000_0000u32).collect();

    // Upload converted u32 data.
    let u32_buf = crate::buffer::upload_u32(device, queue, &u32_data);

    // Radix sort the u32 values.
    let sorted_u32_buf = radix_sort_u32(device, queue, cache, &u32_buf);

    // Download sorted u32 result.
    let sorted_u32 = pollster::block_on(
        crate::buffer::download_u32(device, queue, &sorted_u32_buf)
    );

    // XOR sign bit back to recover correctly ordered i32 values.
    let sorted_i32: Vec<i32> = sorted_u32.iter().map(|&v| (v ^ 0x8000_0000u32) as i32).collect();

    // Upload final i32 result.
    crate::buffer::upload_i32(device, queue, &sorted_i32)
}
