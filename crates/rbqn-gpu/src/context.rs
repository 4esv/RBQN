use std::sync::Arc;
use wgpu;

pub struct GpuContext {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub adapter_info: wgpu::AdapterInfo,
    pub mappable_primary_buffers: bool,
    pub shader_int64: bool,
    pub subgroup: bool,
}

impl GpuContext {
    pub async fn new() -> Option<Self> {
        let backends = if cfg!(target_os = "macos") {
            wgpu::Backends::METAL
        } else {
            wgpu::Backends::all()
        };

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await?;

        let adapter_info = adapter.get_info();

        let required_limits = wgpu::Limits {
            max_storage_buffer_binding_size: adapter.limits().max_storage_buffer_binding_size,
            max_buffer_size: adapter.limits().max_buffer_size,
            max_storage_buffers_per_shader_stage: adapter.limits().max_storage_buffers_per_shader_stage,
            max_compute_workgroups_per_dimension: adapter
                .limits()
                .max_compute_workgroups_per_dimension,
            ..Default::default()
        };

        let wanted = wgpu::Features::MAPPABLE_PRIMARY_BUFFERS
            | wgpu::Features::SHADER_INT64
            | wgpu::Features::SUBGROUP;
        let required_features = wanted & adapter.features();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("rbqn-gpu"),
                required_features,
                required_limits,
                ..Default::default()
            }, None)
            .await
            .ok()?;

        let feats = device.features();
        let mappable_primary_buffers = feats.contains(wgpu::Features::MAPPABLE_PRIMARY_BUFFERS);
        crate::buffer::set_mappable(mappable_primary_buffers);
        Some(Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
            adapter_info,
            mappable_primary_buffers,
            shader_int64: feats.contains(wgpu::Features::SHADER_INT64),
            subgroup: feats.contains(wgpu::Features::SUBGROUP),
        })
    }

    pub fn has_subgroup_ops(&self) -> bool {
        self.device
            .features()
            .contains(wgpu::Features::SUBGROUP)
    }

    pub fn max_buffer_size(&self) -> u64 {
        self.device.limits().max_buffer_size
    }

    pub fn max_storage_buffer_binding_size(&self) -> u32 {
        self.device.limits().max_storage_buffer_binding_size
    }
}
