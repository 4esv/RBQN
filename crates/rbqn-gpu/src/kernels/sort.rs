//! Stable LSD radix sort / grade on the GPU.
//!
//! 8 passes of 4 bits. Per pass, in one command buffer: `hist` (per-tile digit
//! counts) -> `scan` (device-side exclusive scan of the digit-major count
//! table) -> `scatter` (workgroup-local stable ranking). No inter-workgroup
//! forward progress is assumed (Metal gives none), so no look-back. The whole
//! sort is one submit; callers upload once and download once.

use std::sync::Arc;
use wgpu;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::dispatch::workgroup_grid;
use crate::pipeline::{PipelineCache, PipelineKey};

const SHADER: &str = include_str!("../shaders/sort.wgsl");
const RADIX_BITS: u32 = 4;
const NUM_PASSES: u32 = 32 / RADIX_BITS;
const TILE: usize = 2048;

/// u32 ordering of i32 values (flip the sign bit).
const ASC_MASK: u32 = 0x8000_0000;
/// Bitwise complement of the ascending key: u32 ascending == i32 descending.
const DESC_MASK: u32 = 0x7FFF_FFFF;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    shift: u32,
    n: u32,
    num_tiles: u32,
    load_mask: u32,
    store_mask: u32,
    flags: u32,
    first: u32,
    pad: u32,
}

struct Sorted {
    keys: GpuBuffer,
    idx: Option<GpuBuffer>,
}

fn small_buf(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: crate::buffer::storage_usage(),
        mapped_at_creation: false,
    })
}

/// Core: sorts `input` (any 4-byte kind) as u32 after `load_mask`, undoing it
/// with `store_mask` on the final key store. `grade` carries the original
/// index; `want_keys` false skips storing keys on the last pass.
fn sort_impl(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
    load_mask: u32,
    store_mask: u32,
    grade: bool,
    want_keys: bool,
) -> Sorted {
    let n = input.len();
    if n == 0 {
        return Sorted {
            keys: GpuBuffer::storage(device, ElementKind::U32, 0),
            idx: grade.then(|| GpuBuffer::storage(device, ElementKind::U32, 0)),
        };
    }
    let num_tiles = n.div_ceil(TILE);
    let (gx, gy) = workgroup_grid(num_tiles as u32);

    let hist_p = cache.get_or_create(&PipelineKey::raw("sort", "hist"), SHADER).clone();
    let scan_p = cache.get_or_create(&PipelineKey::raw("sort", "scan"), SHADER).clone();
    let scat_p = cache.get_or_create(&PipelineKey::raw("sort", "scatter"), SHADER).clone();

    let mut ka = GpuBuffer::storage(device, ElementKind::U32, n);
    let mut kb = GpuBuffer::storage(device, ElementKind::U32, n);
    let (mut ia, mut ib) = if grade {
        (
            GpuBuffer::storage(device, ElementKind::U32, n),
            GpuBuffer::storage(device, ElementKind::U32, n),
        )
    } else {
        (
            GpuBuffer::storage(device, ElementKind::U32, 1),
            GpuBuffer::storage(device, ElementKind::U32, 1),
        )
    };
    let dummy_in = small_buf(device);
    let dummy_out = small_buf(device);
    let counts = GpuBuffer::storage(device, ElementKind::U32, 16 * num_tiles);

    let mut encoder = device.create_command_encoder(&Default::default());
    // Pass 0 reads straight from `input` (masked on load): no copy.
    let mut first = true;
    for pass in 0..NUM_PASSES {
        let last = pass == NUM_PASSES - 1;
        let mut flags = 0u32;
        if grade {
            flags |= 1;
        }
        if last && !want_keys {
            flags |= 2;
        }
        let params = Params {
            shift: pass * RADIX_BITS,
            n: n as u32,
            num_tiles: num_tiles as u32,
            load_mask: if first { load_mask } else { 0 },
            store_mask: if last { store_mask } else { 0 },
            flags,
            first: first as u32,
            pad: 0,
        };
        let pbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&pbuf, 0, bytemuck::bytes_of(&params));

        let kin: &wgpu::Buffer = if first { input.inner() } else { ka.inner() };
        let (iin, iout) = if grade {
            (ia.inner(), ib.inner())
        } else {
            (&dummy_in, &dummy_out)
        };

        let hist_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &hist_p.get_bind_group_layout(0),
            entries: &[entry(0, kin), entry(4, counts.inner()), entry(5, &pbuf)],
        });
        let scan_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &scan_p.get_bind_group_layout(0),
            entries: &[entry(4, counts.inner()), entry(5, &pbuf)],
        });
        let scat_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &scat_p.get_bind_group_layout(0),
            entries: &[
                entry(0, kin),
                entry(1, kb.inner()),
                entry(2, iin),
                entry(3, iout),
                entry(4, counts.inner()),
                entry(5, &pbuf),
            ],
        });

        {
            let mut cp = encoder.begin_compute_pass(&Default::default());
            cp.set_pipeline(&hist_p);
            cp.set_bind_group(0, &hist_bg, &[]);
            cp.dispatch_workgroups(gx, gy, 1);
            crate::stats::dispatch();
        }
        {
            let mut cp = encoder.begin_compute_pass(&Default::default());
            cp.set_pipeline(&scan_p);
            cp.set_bind_group(0, &scan_bg, &[]);
            cp.dispatch_workgroups(1, 1, 1);
            crate::stats::dispatch();
        }
        {
            let mut cp = encoder.begin_compute_pass(&Default::default());
            cp.set_pipeline(&scat_p);
            cp.set_bind_group(0, &scat_bg, &[]);
            cp.dispatch_workgroups(gx, gy, 1);
            crate::stats::dispatch();
        }
        std::mem::swap(&mut ka, &mut kb);
        std::mem::swap(&mut ia, &mut ib);
        first = false;
    }
    queue.submit(std::iter::once(encoder.finish()));
    crate::stats::submit();

    Sorted { keys: ka, idx: grade.then_some(ia) }
}

/// Stable ascending sort of a u32 buffer. Returns a new buffer.
pub fn radix_sort_u32(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
) -> GpuBuffer {
    sort_impl(device, queue, cache, input, 0, 0, false, true).keys
}

/// Stable grade of a u32 buffer: ascending permutation (U32 indices).
pub fn grade_u32(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
) -> GpuBuffer {
    sort_impl(device, queue, cache, input, 0, 0, true, false).idx.unwrap()
}

/// Stable grade of an i32 buffer, as an I32 buffer of indices. Descending keeps
/// earlier indices first among equal values (BQN `⍒`).
pub fn grade_i32(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
    descending: bool,
) -> GpuBuffer {
    let mask = if descending { DESC_MASK } else { ASC_MASK };
    let mut out = sort_impl(device, queue, cache, input, mask, 0, true, false)
        .idx
        .unwrap();
    out.element_type = ElementKind::I32;
    out
}

/// Ascending stable grade, downloaded (one upload, one download total).
pub fn argsort_i32(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
) -> Vec<i32> {
    let g = grade_i32(device, queue, cache, input, false);
    pollster::block_on(crate::buffer::download_i32(device, queue, &g))
}

/// Sorted copy of an i32 buffer, ascending or descending.
pub fn sort_i32_dir(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
    descending: bool,
) -> GpuBuffer {
    let mask = if descending { DESC_MASK } else { ASC_MASK };
    let mut out = sort_impl(device, queue, cache, input, mask, mask, false, true).keys;
    out.element_type = ElementKind::I32;
    out
}

pub fn sort_i32(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
) -> GpuBuffer {
    sort_i32_dir(device, queue, cache, input, false)
}

fn entry(binding: u32, b: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource: b.as_entire_binding() }
}
