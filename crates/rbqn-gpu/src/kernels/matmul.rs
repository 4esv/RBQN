use std::sync::Arc;
use wgpu;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::pipeline::{PipelineCache, PipelineKey};

const SHADER_F32: &str = include_str!("../shaders/matmul_f32.wgsl");

/// Multiply A (M x K) by B (K x N) producing C (M x N).
pub fn matmul(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    a: &GpuBuffer,
    b: &GpuBuffer,
    out: &GpuBuffer,
    m: u32,
    n: u32,
    k: u32,
) {
    assert_eq!(a.element_type(), ElementKind::F32, "matmul only supports f32 buffers");

    let key = PipelineKey::raw("matmul_f32", "matmul_f32");
    let pipeline = cache.get_or_create(&key, SHADER_F32);

    // Uniform buffer: [M, N, K, pad] — 16 bytes
    let params: [u32; 4] = [m, n, k, 0];
    let params_bytes: &[u8] = bytemuck::cast_slice(&params);
    let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("matmul_params"),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&params_buf, 0, params_bytes);

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: a.inner().as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: b.inner().as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: out.inner().as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: params_buf.as_entire_binding() },
        ],
    });

    // Dispatch: x covers columns (N), y covers rows (M)
    let workgroups_x = (n + 15) / 16;
    let workgroups_y = (m + 15) / 16;

    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
    }
    queue.submit(std::iter::once(encoder.finish()));
}
