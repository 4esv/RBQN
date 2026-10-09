//! Reductions to a single i64 (a length-1 I64 buffer). Input is I64, or I32
//! (widening: accumulates in i64). Ops: add min max. Multi-level tree
//! reduction, workgroupBarrier only.

use std::sync::Arc;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::dispatch::workgroup_grid;
use crate::kernels::int64::{self, BLOCK};
use crate::pipeline::{PassBatch, PipelineCache, PipelineKey};

const TEMPLATE: &str = include_str!("../shaders/reduce_i64.wgsl");

pub fn reduce_i64(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    op: &str,
    input: &GpuBuffer,
) -> GpuBuffer {
    assert!(!input.is_empty(), "reduce_i64: empty input");
    let mut cur_elem = input.element_type();
    let mut cur: Option<GpuBuffer> = None;
    let mut batch = PassBatch::new(device);
    loop {
        let src = cur.as_ref().unwrap_or(input);
        let groups = src.len().div_ceil(BLOCK);
        let out = GpuBuffer::storage(device, ElementKind::I64, groups);
        let key = PipelineKey::raw(int64::shader_id("reduce_i64", op, cur_elem), "main");
        let pipeline = cache.get_or_create(&key, &int64::expand(TEMPLATE, op, cur_elem));
        let bg = int64::bind(device, pipeline, &[(0, src), (1, &out)]);
        batch.push(pipeline, &bg, workgroup_grid(groups as u32));
        if groups == 1 {
            batch.submit(queue);
            return out;
        }
        cur = Some(out);
        cur_elem = ElementKind::I64;
    }
}

/// Reduce i32 inputs accumulating in i64.
pub fn reduce_i32_to_i64(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    op: &str,
    input: &GpuBuffer,
) -> GpuBuffer {
    assert_eq!(input.element_type(), ElementKind::I32);
    reduce_i64(device, queue, cache, op, input)
}
