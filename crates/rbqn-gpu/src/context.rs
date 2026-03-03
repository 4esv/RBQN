use std::sync::Arc;
use wgpu;

pub struct GpuContext {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub adapter_info: wgpu::AdapterInfo,
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
            max_compute_workgroups_per_dimension: adapter
                .limits()
                .max_compute_workgroups_per_dimension,
            ..Default::default()
        };

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("rbqn-gpu"),
                required_features: wgpu::Features::empty(),
                required_limits,
                ..Default::default()
            }, None)
            .await
            .ok()?;

        Some(Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
            adapter_info,
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
