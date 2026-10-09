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

/// Whether wgpu must zero workgroup memory before the shader runs. The zeroing
/// cost ~9 us per workgroup for sort. It is safe to skip only for shaders where
/// every thread writes its slots of every `var<workgroup>` before any read:
///   sort, scan_i32/scan_f32 (temp[ai], temp[bi] written by all 256 threads),
///   reduce_i32/reduce_f32/reduce_i64 (shared_data[t] written by all threads),
///   minmax_i32 (smin/smax[t]), scan_i64 (tot[t]).
/// Left on: matmul (tile loads are guarded by bounds), softmax (not audited).
/// `RBQN_GPU_ZERO_WG=1` forces zeroing everywhere (for measuring).
fn zero_workgroup_memory(shader_id: &str) -> bool {
    if std::env::var_os("RBQN_GPU_ZERO_WG").is_some() {
        return true;
    }
    !(shader_id == "sort"
        || shader_id == "minmax_i32"
        || shader_id.starts_with("scan_")
        || shader_id.starts_with("reduce_"))
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
            let pipeline = build_pipeline(&self.device, &self.modules[key.shader_id], key);
            self.pipelines.insert(key.clone(), pipeline); crate::stats::PIPELINE_COMPILES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        &self.pipelines[key]
    }

    pub fn contains(&self, key: &PipelineKey) -> bool {
        self.pipelines.contains_key(key)
    }

    /// Insert a pipeline compiled outside the lock (`compile_detached`).
    /// Keeps an existing entry; not counted in PIPELINE_COMPILES.
    pub fn insert(&mut self, key: PipelineKey, pipeline: wgpu::ComputePipeline) {
        self.pipelines.entry(key).or_insert(pipeline);
    }
}

fn build_pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    key: &PipelineKey,
) -> wgpu::ComputePipeline {
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(&key.entry_point),
        layout: None,
        module,
        entry_point: Some(&key.entry_point),
        compilation_options: wgpu::PipelineCompilationOptions {
            zero_initialize_workgroup_memory: zero_workgroup_memory(key.shader_id),
            ..Default::default()
        },
        cache: None,
    })
}

/// Compile one pipeline without touching a `PipelineCache`, so a background
/// precompile can build it with no lock held and `insert` it afterwards.
/// `modules` caches shader modules across calls by shader id.
pub fn compile_detached(
    device: &wgpu::Device,
    modules: &mut HashMap<&'static str, wgpu::ShaderModule>,
    key: &PipelineKey,
    source: &str,
) -> wgpu::ComputePipeline {
    let module = modules.entry(key.shader_id).or_insert_with(|| {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(key.shader_id),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        })
    });
    build_pipeline(device, module, key)
}

/// Several dependent compute passes recorded into one command buffer and
/// submitted once. Passes in one command buffer run in order, so a multi-level
/// reduce or scan pays one submit instead of one per level.
pub struct PassBatch {
    encoder: wgpu::CommandEncoder,
}

impl PassBatch {
    pub fn new(device: &wgpu::Device) -> Self {
        Self { encoder: device.create_command_encoder(&Default::default()) }
    }

    pub fn push(
        &mut self,
        pipeline: &wgpu::ComputePipeline,
        bind_group: &wgpu::BindGroup,
        grid: (u32, u32),
    ) {
        let mut pass = self.encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(grid.0, grid.1, 1);
        crate::stats::dispatch();
    }

    pub fn submit(self, queue: &wgpu::Queue) {
        queue.submit(std::iter::once(self.encoder.finish()));
        crate::stats::submit();
    }
}
