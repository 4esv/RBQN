use std::sync::Arc;
use wgpu;

use crate::buffer::{ElementKind, GpuBuffer};
use crate::dispatch::{WORKGROUP_SIZE, workgroup_count};
use crate::pipeline::{PipelineCache, PipelineKey};

#[derive(Clone, Debug)]
pub enum FusedOp {
    Add,
    Sub,
    Mul,
    Div,
    ScalarAdd(f32),
    ScalarMul(f32),
}

impl FusedOp {
    fn is_binary(&self) -> bool {
        matches!(self, Self::Add | Self::Sub | Self::Mul | Self::Div)
    }
}

pub struct FusionBuilder {
    ops: Vec<FusedOp>,
}

impl FusionBuilder {
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }

    pub fn push(&mut self, op: FusedOp) {
        self.ops.push(op);
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn can_fuse(op: &str) -> bool {
        matches!(op, "add" | "sub" | "mul" | "div")
    }

    pub fn generate_wgsl(&self, elem: ElementKind) -> (String, String) {
        let ty = match elem {
            ElementKind::F32 => "f32",
            ElementKind::I32 => "i32",
            ElementKind::U32 => "u32",
        };

        let entry = format!("fused_{}", self.ops.len());

        let needs_b = self.ops.iter().any(|op| op.is_binary());

        let mut src = String::new();
        src.push_str(&format!(
            "@group(0) @binding(0) var<storage, read> input_a: array<{ty}>;\n"
        ));
        if needs_b {
            src.push_str(&format!(
                "@group(0) @binding(1) var<storage, read> input_b: array<{ty}>;\n"
            ));
            src.push_str(&format!(
                "@group(0) @binding(2) var<storage, read_write> output: array<{ty}>;\n\n"
            ));
        } else {
            src.push_str(&format!(
                "@group(0) @binding(1) var<storage, read_write> output: array<{ty}>;\n\n"
            ));
        }

        src.push_str(&format!(
            "@compute @workgroup_size(256)\nfn {entry}(@builtin(global_invocation_id) id: vec3<u32>) {{\n"
        ));
        src.push_str("    let idx = id.x;\n");

        let out_binding = "output";
        src.push_str(&format!(
            "    if (idx >= arrayLength(&{out_binding})) {{ return; }}\n"
        ));
        src.push_str(&format!("    var val: {ty} = input_a[idx];\n"));
        if needs_b {
            src.push_str(&format!("    let b_val: {ty} = input_b[idx];\n"));
        }

        for op in &self.ops {
            let expr = match op {
                FusedOp::Add => "val + b_val".to_string(),
                FusedOp::Sub => "val - b_val".to_string(),
                FusedOp::Mul => "val * b_val".to_string(),
                FusedOp::Div => "val / b_val".to_string(),
                FusedOp::ScalarAdd(s) => format!("val + {ty}({s})"),
                FusedOp::ScalarMul(s) => format!("val * {ty}({s})"),
            };
            src.push_str(&format!("    val = {expr};\n"));
        }

        src.push_str(&format!("    {out_binding}[idx] = val;\n"));
        src.push_str("}\n");

        (src, entry)
    }

    pub fn execute(
        &self,
        device: &Arc<wgpu::Device>,
        queue: &wgpu::Queue,
        cache: &mut PipelineCache,
        a: &GpuBuffer,
        b: Option<&GpuBuffer>,
        out: &GpuBuffer,
    ) {
        let elem = a.element_type();
        let (source, entry) = self.generate_wgsl(elem);

        let key = PipelineKey::raw("fused_dynamic", &entry);
        let pipeline = cache.get_or_create(&key, &source);

        let bind_group_layout = pipeline.get_bind_group_layout(0);
        let entries: Vec<wgpu::BindGroupEntry> = if let Some(b) = b {
            vec![
                wgpu::BindGroupEntry { binding: 0, resource: a.inner().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: b.inner().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: out.inner().as_entire_binding() },
            ]
        } else {
            vec![
                wgpu::BindGroupEntry { binding: 0, resource: a.inner().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: out.inner().as_entire_binding() },
            ]
        };

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &entries,
        });

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(workgroup_count(out.len(), WORKGROUP_SIZE), 1, 1);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }

    pub fn clear(&mut self) {
        self.ops.clear();
    }
}

impl Default for FusionBuilder {
    fn default() -> Self {
        Self::new()
    }
}
