//! Shared plumbing for the i64 kernels. Shaders are templates (`@IN@`,
//! `@COMB@`, `@ID@`) expanded per (op, input width) and cached by shader id.
//! Needs `GpuContext::shader_int64`; callers must check it first.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::buffer::{ElementKind, GpuBuffer};

/// Elements covered by one workgroup in the reduce/scan/minmax kernels.
pub const BLOCK: usize = 1024;

/// Combine expression over `a, b: i64` and identity literal for `op`.
pub(crate) fn monoid(op: &str) -> (&'static str, &'static str) {
    match op {
        "add" => ("a + b", "0li"),
        "sub" => ("a - b", "0li"),
        "mul" => ("a * b", "1li"),
        "min" => ("min(a, b)", "9223372036854775807li"),
        "max" => ("max(a, b)", "(-9223372036854775807li - 1li)"),
        _ => panic!("rbqn-gpu: unknown i64 op {op:?}"),
    }
}

/// WGSL element type name for the input side.
pub(crate) fn wgsl_in(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::I64 => "i64",
        ElementKind::I32 => "i32",
        k => panic!("rbqn-gpu: i64 kernels take I32 or I64 input, got {k:?}"),
    }
}

pub(crate) fn expand(template: &str, op: &str, input: ElementKind) -> String {
    let (comb, id) = monoid(op);
    template
        .replace("@IN@", wgsl_in(input))
        .replace("@COMB@", comb)
        .replace("@ID@", id)
}

/// Leak-once interner: `PipelineCache` wants `&'static str` shader ids.
pub(crate) fn shader_id(base: &str, op: &str, input: ElementKind) -> &'static str {
    static IDS: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let name = format!("{base}_{op}_{}", wgsl_in(input));
    let mut m = IDS.get_or_init(Default::default).lock().unwrap();
    m.entry(name.clone()).or_insert_with(|| Box::leak(name.into_boxed_str()))
}

pub(crate) fn bind(
    device: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
    bufs: &[(u32, &GpuBuffer)],
) -> wgpu::BindGroup {
    let layout = pipeline.get_bind_group_layout(0);
    let entries: Vec<_> = bufs
        .iter()
        .map(|(binding, b)| wgpu::BindGroupEntry {
            binding: *binding,
            resource: b.inner().as_entire_binding(),
        })
        .collect();
    device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &layout, entries: &entries })
}

/// One compute pass, one submit (same convention as the other kernels).
pub(crate) fn run(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipeline: &wgpu::ComputePipeline,
    bind_group: &wgpu::BindGroup,
    grid: (u32, u32),
) {
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(grid.0, grid.1, 1);
        crate::stats::dispatch();
    }
    queue.submit(std::iter::once(encoder.finish()));
    crate::stats::submit();
}
