use std::sync::Arc;
use wgpu;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ElementKind {
    I32,
    F32,
    U32,
}

impl ElementKind {
    pub fn byte_size(self) -> u64 {
        match self {
            Self::I32 | Self::F32 | Self::U32 => 4,
        }
    }
}

pub struct GpuBuffer {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) size: u64,
    pub(crate) element_type: ElementKind,
    pub(crate) len: usize,
}

impl GpuBuffer {
    pub fn new(
        device: &wgpu::Device,
        element_type: ElementKind,
        len: usize,
        usage: wgpu::BufferUsages,
    ) -> Self {
        let size = (len as u64) * element_type.byte_size();
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage,
            mapped_at_creation: false,
        });
        Self { buffer, size, element_type, len }
    }

    pub fn storage(device: &wgpu::Device, element_type: ElementKind, len: usize) -> Self {
        Self::new(
            device,
            element_type,
            len,
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        )
    }

    pub fn element_type(&self) -> ElementKind {
        self.element_type
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn inner(&self) -> &wgpu::Buffer {
        &self.buffer
    }
}

pub struct BufferPool {
    device: Arc<wgpu::Device>,
    free_buffers: Vec<(u64, wgpu::Buffer)>,
}

impl BufferPool {
    pub fn new(device: Arc<wgpu::Device>) -> Self {
        Self {
            device,
            free_buffers: Vec::new(),
        }
    }

    pub fn acquire(&mut self, element_type: ElementKind, len: usize) -> GpuBuffer {
        let size = (len as u64) * element_type.byte_size();
        let idx = self.free_buffers.iter().position(|(s, _)| *s >= size);
        let buffer = match idx {
            Some(i) => {
                let (buf_size, buffer) = self.free_buffers.swap_remove(i);
                GpuBuffer { buffer, size: buf_size, element_type, len }
            }
            None => GpuBuffer::storage(&self.device, element_type, len),
        };
        buffer
    }

    pub fn release(&mut self, buf: GpuBuffer) {
        self.free_buffers.push((buf.size, buf.buffer));
    }

    pub fn clear(&mut self) {
        self.free_buffers.clear();
    }
}

pub fn upload_i32(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &[i32],
) -> GpuBuffer {
    let buf = GpuBuffer::storage(device, ElementKind::I32, data.len());
    queue.write_buffer(&buf.buffer, 0, bytemuck::cast_slice(data));
    buf
}

pub fn upload_f32(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &[f32],
) -> GpuBuffer {
    let buf = GpuBuffer::storage(device, ElementKind::F32, data.len());
    queue.write_buffer(&buf.buffer, 0, bytemuck::cast_slice(data));
    buf
}

pub fn upload_u32(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &[u32],
) -> GpuBuffer {
    let buf = GpuBuffer::storage(device, ElementKind::U32, data.len());
    queue.write_buffer(&buf.buffer, 0, bytemuck::cast_slice(data));
    buf
}

pub async fn download_f32(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buf: &GpuBuffer,
) -> Vec<f32> {
    download_raw(device, queue, buf).await
}

pub async fn download_i32(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buf: &GpuBuffer,
) -> Vec<i32> {
    download_raw(device, queue, buf).await
}

pub async fn download_u32(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buf: &GpuBuffer,
) -> Vec<u32> {
    download_raw(device, queue, buf).await
}

async fn download_raw<T: bytemuck::Pod>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buf: &GpuBuffer,
) -> Vec<T> {
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: buf.size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(&buf.buffer, 0, &staging, 0, buf.size);
    queue.submit(std::iter::once(encoder.finish()));

    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });
    device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().unwrap();

    let view = slice.get_mapped_range();
    let result: Vec<T> = bytemuck::cast_slice(&view).to_vec();
    drop(view);
    staging.unmap();
    result
}
