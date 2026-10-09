//! Inclusive scans into I64. Input is I64, or I32 (widening). Ops: add min max.
//! Reduce-then-scan: per-block scan + block totals, recursive scan of the
//! totals, propagate. No inter-workgroup waiting.

use std::sync::Arc;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::dispatch::workgroup_grid;
use crate::kernels::int64::{self, BLOCK};
use crate::pipeline::{PipelineCache, PipelineKey};

const TEMPLATE: &str = include_str!("../shaders/scan_i64.wgsl");

pub fn scan_i64(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    op: &str,
    input: &GpuBuffer,
) -> GpuBuffer {
    let elem = input.element_type();
    let n = input.len();
    assert!(n > 0, "scan_i64: empty input");
    let blocks = n.div_ceil(BLOCK);
    let out = GpuBuffer::storage(device, ElementKind::I64, n);
    let sums = GpuBuffer::storage(device, ElementKind::I64, blocks);
    let id = int64::shader_id("scan_i64", op, elem);
    let src = int64::expand(TEMPLATE, op, elem);

    let key = PipelineKey::raw(id, "scan_block");
    let pipeline = cache.get_or_create(&key, &src);
    let bg = int64::bind(device, pipeline, &[(0, input), (1, &out), (2, &sums)]);
    int64::run(device, queue, pipeline, &bg, workgroup_grid(blocks as u32));
    if blocks == 1 {
        return out;
    }

    let scanned = scan_i64(device, queue, cache, op, &sums);

    let key = PipelineKey::raw(id, "propagate");
    let pipeline = cache.get_or_create(&key, &src);
    let bg = int64::bind(device, pipeline, &[(1, &out), (2, &scanned)]);
    int64::run(device, queue, pipeline, &bg, workgroup_grid(blocks as u32));
    out
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
