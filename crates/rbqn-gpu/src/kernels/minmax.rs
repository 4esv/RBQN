//! (min, max) of an I32 buffer in one dispatch chain. Returns an I32 buffer of
//! length 2: [min, max]. The exactness guard uses it to bound values.

use std::sync::Arc;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::dispatch::workgroup_grid;
use crate::kernels::int64::{self, BLOCK};
use crate::pipeline::{PipelineCache, PipelineKey};

const SOURCE: &str = include_str!("../shaders/minmax_i32.wgsl");

pub fn minmax_i32(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    input: &GpuBuffer,
) -> GpuBuffer {
    assert_eq!(input.element_type(), ElementKind::I32);
    assert!(!input.is_empty(), "minmax_i32: empty input");
    let mut cur: Option<GpuBuffer> = None;
    loop {
        let (src, count) = match &cur {
            Some(b) => (b, b.len() / 2),
            None => (input, input.len()),
        };
        let groups = count.div_ceil(BLOCK);
        let out = GpuBuffer::storage(device, ElementKind::I32, groups * 2);
        let entry = if cur.is_some() { "minmax_pairs" } else { "minmax_first" };
        let key = PipelineKey::raw("minmax_i32", entry);
        let pipeline = cache.get_or_create(&key, SOURCE);
        let bg = int64::bind(device, pipeline, &[(0, src), (1, &out)]);
        int64::run(device, queue, pipeline, &bg, workgroup_grid(groups as u32));
        if groups == 1 {
            return out;
        }
        cur = Some(out);
    }
}
