use std::collections::HashMap;
use std::sync::Arc;
use wgpu;

use crate::buffer::ElementKind;

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct PipelineKey {
    pub shader_id: &'static str,
    pub entry_point: String,
}

impl PipelineKey {
    pub fn new(shader_id: &'static str, op: &str, elem: ElementKind) -> Self {
        let suffix = match elem {
            ElementKind::I32 => "i32",
            ElementKind::F32 => "f32",
            ElementKind::U32 => "u32",
            ElementKind::I64 => "i64",
        };
        Self {
            shader_id,
            entry_point: format!("{op}_{suffix}"),
        }
    }

    pub fn raw(shader_id: &'static str, entry_point: &str) -> Self {
        Self {
            shader_id,
            entry_point: entry_point.to_string(),
        }
    }
}

pub struct PipelineCache {
    device: Arc<wgpu::Device>,
    modules: HashMap<&'static str, wgpu::ShaderModule>,
    pipelines: HashMap<PipelineKey, wgpu::ComputePipeline>,
}

impl PipelineCache {
    pub fn new(device: Arc<wgpu::Device>) -> Self {
        Self {
            device,
            modules: HashMap::new(),
            pipelines: HashMap::new(),
        }
    }

    pub fn ensure_module(&mut self, shader_id: &'static str, source: &str) {
        self.modules.entry(shader_id).or_insert_with(|| {
            self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(shader_id),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            })
        });
    }

    pub fn get_or_create(
        &mut self,
        key: &PipelineKey,
        source: &str,
    ) -> &wgpu::ComputePipeline {
        if !self.pipelines.contains_key(key) {
            self.ensure_module(key.shader_id, source);
            let module = &self.modules[key.shader_id];
            let pipeline = self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(&key.entry_point),
                layout: None,
                module,
                entry_point: Some(&key.entry_point),
                compilation_options: wgpu::PipelineCompilationOptions {
                    // NOTE: the sort kernels write all workgroup memory before reading it; the
                    // driver-inserted zeroing of ~25 KB per workgroup cost ~9 us per tile.
                    zero_initialize_workgroup_memory: key.shader_id != "sort",
                    ..Default::default()
                },
                cache: None,
            });
            self.pipelines.insert(key.clone(), pipeline); crate::stats::PIPELINE_COMPILES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        &self.pipelines[key]
    }
}
