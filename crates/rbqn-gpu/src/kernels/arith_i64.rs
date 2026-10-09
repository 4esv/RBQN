//! Elementwise i64 arithmetic. Inputs are both I64, or both I32 (widening:
//! the op runs in i64 so an i32 x i32 product is exact). Output is I64.
//! Ops: add sub mul min max. Wraps on i64 overflow.

use std::sync::Arc;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::dispatch::{WORKGROUP_SIZE, grid_for};
use crate::kernels::int64;
use crate::pipeline::{PipelineCache, PipelineKey};

const TEMPLATE: &str = include_str!("../shaders/arith_i64.wgsl");

pub fn arith_binary_i64(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    op: &str,
    a: &GpuBuffer,
    b: &GpuBuffer,
    out: &GpuBuffer,
) {
    let elem = a.element_type();
    assert_eq!(elem, b.element_type(), "arith_binary_i64: mixed input kinds");
    assert_eq!(out.element_type(), ElementKind::I64, "arith_binary_i64: output must be I64");
    let key = PipelineKey::raw(int64::shader_id("arith_i64", op, elem), "main");
    let pipeline = cache.get_or_create(&key, &int64::expand(TEMPLATE, op, elem));
    let bg = int64::bind(device, pipeline, &[(0, a), (1, b), (2, out)]);
    int64::run(device, queue, pipeline, &bg, grid_for(out.len(), WORKGROUP_SIZE));
}

/// i32 x i32 -> exact i64.
pub fn arith_binary_i32_to_i64(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    op: &str,
    a: &GpuBuffer,
    b: &GpuBuffer,
    out: &GpuBuffer,
) {
    assert_eq!(a.element_type(), ElementKind::I32);
    arith_binary_i64(device, queue, cache, op, a, b, out)
}
