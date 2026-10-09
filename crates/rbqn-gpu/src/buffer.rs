use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpu;

/// Set by `GpuContext::new` when the device has MAPPABLE_PRIMARY_BUFFERS.
/// Buffer constructors only see a `&Device`, so this is process-global.
static MAPPABLE: AtomicBool = AtomicBool::new(false);

pub fn set_mappable(on: bool) {
    MAPPABLE.store(on, Ordering::Relaxed);
}

pub fn mappable() -> bool {
    MAPPABLE.load(Ordering::Relaxed)
}

fn debug() -> bool {
    std::env::var_os("RBQN_GPU_DEBUG").is_some_and(|v| v == "1")
}

fn force_wait() -> bool {
    std::env::var_os("RBQN_GPU_SYNC").is_some_and(|v| v == "wait")
}

const SPIN_LIMIT: Duration = Duration::from_secs(2);

/// Block until `done` returns Some. Spins on `Maintain::Poll` (Wait sleeps
/// ~1.4 ms on Metal); after SPIN_LIMIT falls back to `Maintain::Wait`.
/// `RBQN_GPU_SYNC=wait` skips the spin entirely.
pub fn sync<T>(device: &wgpu::Device, mut done: impl FnMut() -> Option<T>) -> T {
    if !force_wait() {
        let start = Instant::now();
        loop {
            device.poll(wgpu::Maintain::Poll);
            if let Some(v) = done() {
                return v;
            }
            if start.elapsed() > SPIN_LIMIT {
                if debug() {
                    eprintln!("rbqn-gpu: spin-poll exceeded {:?}, falling back to Maintain::Wait", SPIN_LIMIT);
                }
                break;
            }
            std::hint::spin_loop();
        }
    }
    loop {
        device.poll(wgpu::Maintain::Wait);
        if let Some(v) = done() {
            return v;
        }
    }
}

/// Map `slice` and wait for the callback.
pub fn map_sync(device: &wgpu::Device, slice: wgpu::BufferSlice<'_>, mode: wgpu::MapMode) {
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(mode, move |r| {
        let _ = tx.send(r);
    });
    sync(device, || rx.try_recv().ok()).unwrap();
}

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
            storage_usage(),
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

/// Usage for every storage buffer: adds MAP_READ when the device has
/// mappable primary buffers, so downloads skip the staging copy. The pool
/// reuses buffers made here, so flags stay uniform.
/// NOTE: not MAP_WRITE: on Metal that made CPU reads of the mapped range
/// ~5x slower (40 MB to_vec 16 ms vs 3 ms). Upload buffers get MAP_WRITE only.
pub fn storage_usage() -> wgpu::BufferUsages {
    let base = wgpu::BufferUsages::STORAGE
        | wgpu::BufferUsages::COPY_SRC
        | wgpu::BufferUsages::COPY_DST;
    if mappable() {
        base | wgpu::BufferUsages::MAP_READ
    } else {
        base
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
        
        match idx {
            Some(i) => {
                let (buf_size, buffer) = self.free_buffers.swap_remove(i);
                GpuBuffer { buffer, size: buf_size, element_type, len }
            }
            None => GpuBuffer::storage(&self.device, element_type, len),
        }
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
    upload_raw(device, queue, ElementKind::I32, data)
}

pub fn upload_f32(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &[f32],
) -> GpuBuffer {
    upload_raw(device, queue, ElementKind::F32, data)
}

pub fn upload_u32(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &[u32],
) -> GpuBuffer {
    upload_raw(device, queue, ElementKind::U32, data)
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

fn upload_raw<T: bytemuck::Pod>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    kind: ElementKind,
    data: &[T],
) -> GpuBuffer {
    if !mappable() || data.is_empty() {
        let buf = GpuBuffer::storage(device, kind, data.len());
        queue.write_buffer(&buf.buffer, 0, bytemuck::cast_slice(data));
        return buf;
    }
    // Fresh MAP_WRITE buffer mapped at creation: memcpy + unmap, no blit,
    // no map_async round trip.
    let size = (data.len() as u64) * kind.byte_size();
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::MAP_WRITE,
        mapped_at_creation: true,
    });
    buffer
        .slice(..)
        .get_mapped_range_mut()
        .copy_from_slice(bytemuck::cast_slice(data));
    buffer.unmap();
    GpuBuffer { buffer, size, element_type: kind, len: data.len() }
}

async fn download_raw<T: bytemuck::Pod>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buf: &GpuBuffer,
) -> Vec<T> {
    if buf.size == 0 {
        return Vec::new();
    }
    if buf.buffer.usage().contains(wgpu::BufferUsages::MAP_READ) {
        let slice = buf.buffer.slice(..buf.size);
        map_sync(device, slice, wgpu::MapMode::Read);
        let view = slice.get_mapped_range();
        let result: Vec<T> = bytemuck::cast_slice(&view).to_vec();
        drop(view);
        buf.buffer.unmap();
        return result;
    }
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
    map_sync(device, slice, wgpu::MapMode::Read);

    let view = slice.get_mapped_range();
    let result: Vec<T> = bytemuck::cast_slice(&view).to_vec();
    drop(view);
    staging.unmap();
    result
}
