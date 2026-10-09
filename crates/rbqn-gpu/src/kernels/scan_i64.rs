//! Inclusive scans into I64. Input is I64, or I32 (widening). Ops: add min max.
//! Reduce-then-scan: per-block totals, recursive scan of the totals, then one
//! pass that rescans each block with its carry. No inter-workgroup waiting.

use std::sync::Arc;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::dispatch::workgroup_grid;
use crate::kernels::int64;
use crate::pipeline::{PassBatch, PipelineCache, PipelineKey};

const TEMPLATE: &str = include_str!("../shaders/scan_i64.wgsl");

/// Elements per thread; a workgroup scans 256 * EPT elements.
const EPT: usize = 4;
const BLOCK: usize = 256 * EPT;

pub fn scan_i64(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    op: &str,
    input: &GpuBuffer,
) -> GpuBuffer {
    let out = GpuBuffer::storage(device, ElementKind::I64, input.len());
    scan_i64_into(device, queue, cache, op, input, &out);
    out
}

/// Inclusive scan into a caller-provided I64 `out` of the same length, so the
/// caller can pool the n-sized allocation (a fresh buffer costs ~1 ms per 40 MB).
pub fn scan_i64_into(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    op: &str,
    input: &GpuBuffer,
    out: &GpuBuffer,
) {
    assert_eq!(out.len(), input.len());
    assert_eq!(out.element_type(), ElementKind::I64);
    let mut batch = PassBatch::new(device);
    scan_levels(device, &mut batch, cache, op, input, out);
    batch.submit(queue);
}

fn scan_levels(
    device: &Arc<wgpu::Device>,
    batch: &mut PassBatch,
    cache: &mut PipelineCache,
    op: &str,
    input: &GpuBuffer,
    out: &GpuBuffer,
) {
    let elem = input.element_type();
    let n = input.len();
    assert!(n > 0, "scan_i64: empty input");
    let blocks = n.div_ceil(BLOCK);
    let id = int64::shader_id("scan_i64", op, elem);
    let src = int64::expand(TEMPLATE, op, elem)
        .replace("@EPT@", &EPT.to_string())
        .replace("@TILE@", &BLOCK.to_string());

    if blocks == 1 {
        let dummy = GpuBuffer::storage(device, ElementKind::I64, 1);
        let pipeline = cache.get_or_create(&PipelineKey::raw(id, "scan_block"), &src);
        let bg = int64::bind(device, pipeline, &[(0, input), (1, out), (2, &dummy)]);
        batch.push(pipeline, &bg, workgroup_grid(1));
        return;
    }

    // Reduce: per-block totals, then their inclusive scan (recursive), then a
    // single pass that rescans each block and adds its carry.
    let sums = GpuBuffer::storage(device, ElementKind::I64, blocks);
    let pipeline = cache.get_or_create(&PipelineKey::raw(id, "block_sum"), &src);
    let bg = int64::bind(device, pipeline, &[(0, input), (2, &sums)]);
    batch.push(pipeline, &bg, workgroup_grid(blocks as u32));

    let scanned = GpuBuffer::storage(device, ElementKind::I64, blocks);
    scan_levels(device, batch, cache, op, &sums, &scanned);

    let pipeline = cache.get_or_create(&PipelineKey::raw(id, "scan_block"), &src);
    let bg = int64::bind(device, pipeline, &[(0, input), (1, out), (2, &scanned)]);
    batch.push(pipeline, &bg, workgroup_grid(blocks as u32));
}

pub fn scan_i32_to_i64(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    op: &str,
    input: &GpuBuffer,
) -> GpuBuffer {
    assert_eq!(input.element_type(), ElementKind::I32);
    scan_i64(device, queue, cache, op, input)
}
