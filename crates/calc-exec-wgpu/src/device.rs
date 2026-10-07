use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::sync::mpsc;
use std::task::{Context, Poll, Wake, Waker};

use calc_exec_gpu::{
    BufferData, BufferKind, DeviceError, DispatchSize, GpuDevice, Kernel, MAP_ENTRY_POINT,
    OperationModes, PassId, REDUCE_ENTRY_POINT, WORKGROUP_SIZE, WORKGROUPS_PER_DIMENSION,
    emit_map_with, emit_reduce,
};

const BYTES_PER_ENTRY: u64 = 4;
const SIZE_UNIFORM_BYTES: u64 = 8;
const BINDING_COUNT: u32 = 4;

struct ThreadWaker(std::thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

pub fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        std::thread::park();
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WgpuDeviceOptions {
    pub allow_cpu_adapter: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceCreationError {
    NoAdapter { message: String },
    NoVulkanBackend,
    CpuAdapterRefused { adapter: String },
    DeviceRequest { message: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterDescription {
    pub name: String,
    pub backend: String,
    pub driver: String,
    pub is_cpu: bool,
}

#[derive(Clone, Debug)]
pub struct WgpuDevice {
    device: wgpu::Device,
    queue: wgpu::Queue,
    description: AdapterDescription,
}

#[derive(Debug)]
pub struct WgpuBuffer {
    buffer: wgpu::Buffer,
    kind: BufferKind,
    entries: usize,
}

#[derive(Debug)]
pub struct WgpuKernel {
    map: wgpu::ComputePipeline,
    reduce: Option<wgpu::ComputePipeline>,
    layout: wgpu::BindGroupLayout,
}

fn buffer_entry(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

pub fn workgroup_counts(invocations: u32) -> (u32, u32) {
    let groups = invocations.div_ceil(WORKGROUP_SIZE);
    if groups <= WORKGROUPS_PER_DIMENSION {
        (groups.max(1), 1)
    } else {
        (
            WORKGROUPS_PER_DIMENSION,
            groups.div_ceil(WORKGROUPS_PER_DIMENSION),
        )
    }
}

pub fn value_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

pub fn index_bytes(indices: &[u32]) -> Vec<u8> {
    indices
        .iter()
        .flat_map(|index| index.to_le_bytes())
        .collect()
}

pub fn values_from_bytes(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect()
}

fn byte_length(entries: usize) -> Result<u64, DeviceError> {
    u64::try_from(entries.max(1))
        .ok()
        .and_then(|entries| entries.checked_mul(BYTES_PER_ENTRY))
        .ok_or(DeviceError::OutOfMemory)
}

fn scope_error(error: Option<wgpu::Error>) -> Result<(), DeviceError> {
    match error {
        None => Ok(()),
        Some(wgpu::Error::OutOfMemory { .. }) => Err(DeviceError::OutOfMemory),
        Some(wgpu::Error::Validation { .. }) => Err(DeviceError::ShaderRejected),
        Some(wgpu::Error::Internal { .. }) => Err(DeviceError::Lost),
    }
}

impl WgpuDevice {
    pub fn new(options: WgpuDeviceOptions) -> Result<WgpuDevice, DeviceCreationError> {
        if !wgpu::Instance::enabled_backend_features().contains(wgpu::Backends::VULKAN) {
            return Err(DeviceCreationError::NoVulkanBackend);
        }
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::VULKAN;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..wgpu::RequestAdapterOptions::default()
        }))
        .map_err(|error| DeviceCreationError::NoAdapter {
            message: error.to_string(),
        })?;
        let info = adapter.get_info();
        let description = AdapterDescription {
            name: info.name.clone(),
            backend: format!("{:?}", info.backend),
            driver: format!("{} {}", info.driver, info.driver_info),
            is_cpu: info.device_type == wgpu::DeviceType::Cpu,
        };
        if description.is_cpu && !options.allow_cpu_adapter {
            return Err(DeviceCreationError::CpuAdapterRefused {
                adapter: description.name,
            });
        }
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("calculator"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|error| DeviceCreationError::DeviceRequest {
            message: error.to_string(),
        })?;
        Ok(WgpuDevice {
            device,
            queue,
            description,
        })
    }

    pub fn description(&self) -> &AdapterDescription {
        &self.description
    }

    fn wait(&self) -> Result<(), DeviceError> {
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map(|_| ())
            .map_err(|_| DeviceError::Lost)
    }

    fn scoped<T>(&self, work: impl FnOnce() -> T) -> Result<T, DeviceError> {
        let out_of_memory = self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let internal = self.device.push_error_scope(wgpu::ErrorFilter::Internal);
        let value = work();
        let internal_error = block_on(internal.pop());
        let validation_error = block_on(validation.pop());
        let memory_error = block_on(out_of_memory.pop());
        scope_error(memory_error)?;
        scope_error(validation_error)?;
        scope_error(internal_error)?;
        Ok(value)
    }

    fn bind_group_layout(&self) -> Result<wgpu::BindGroupLayout, DeviceError> {
        self.scoped(|| {
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("kernel"),
                    entries: &[
                        buffer_entry(0, wgpu::BufferBindingType::Storage { read_only: true }),
                        buffer_entry(1, wgpu::BufferBindingType::Storage { read_only: true }),
                        buffer_entry(2, wgpu::BufferBindingType::Storage { read_only: false }),
                        buffer_entry(BINDING_COUNT - 1, wgpu::BufferBindingType::Uniform),
                    ],
                })
        })
    }

    fn pipeline(
        &self,
        text: &str,
        entry_point: &str,
        layout: &wgpu::BindGroupLayout,
    ) -> Result<wgpu::ComputePipeline, DeviceError> {
        self.scoped(|| {
            let pipeline_layout =
                self.device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some(entry_point),
                        bind_group_layouts: &[Some(layout)],
                        immediate_size: 0,
                    });
            let module = self
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(entry_point),
                    source: wgpu::ShaderSource::Wgsl(text.into()),
                });
            self.device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(entry_point),
                    layout: Some(&pipeline_layout),
                    module: &module,
                    entry_point: Some(entry_point),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                })
        })
    }
}

impl GpuDevice for WgpuDevice {
    type Buffer = WgpuBuffer;
    type Compiled = WgpuKernel;

    fn create_buffer(&self, kind: BufferKind, length: usize) -> Result<WgpuBuffer, DeviceError> {
        let size = byte_length(length)?;
        let buffer = self.scoped(|| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        })?;
        Ok(WgpuBuffer {
            buffer,
            kind,
            entries: length.max(1),
        })
    }

    fn write_buffer(
        &self,
        buffer: &mut WgpuBuffer,
        data: BufferData<'_>,
    ) -> Result<(), DeviceError> {
        let (bytes, entries) = match (buffer.kind, data) {
            (BufferKind::Values, BufferData::Values(values)) => (value_bytes(values), values.len()),
            (BufferKind::Indices, BufferData::Indices(indices)) => {
                (index_bytes(indices), indices.len())
            }
            _ => return Err(DeviceError::BufferKindMismatch),
        };
        if entries > buffer.entries {
            return Err(DeviceError::BufferTooShort);
        }
        self.scoped(|| self.queue.write_buffer(&buffer.buffer, 0, &bytes))
    }

    fn read_buffer(&self, buffer: &WgpuBuffer, length: usize) -> Result<Vec<f32>, DeviceError> {
        if buffer.kind != BufferKind::Values {
            return Err(DeviceError::BufferKindMismatch);
        }
        if length > buffer.entries {
            return Err(DeviceError::BufferTooShort);
        }
        let size = byte_length(length)?;
        let staging = self.scoped(|| {
            let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            encoder.copy_buffer_to_buffer(&buffer.buffer, 0, &staging, 0, size);
            self.queue.submit([encoder.finish()]);
            staging
        })?;
        let (sender, receiver) = mpsc::channel();
        staging.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = sender.send(result);
        });
        self.wait()?;
        match receiver.recv() {
            Ok(Ok(())) => {}
            Ok(Err(_)) | Err(_) => return Err(DeviceError::Lost),
        }
        let values = {
            let view = staging
                .get_mapped_range(..)
                .map_err(|_| DeviceError::Lost)?;
            values_from_bytes(&view)
        };
        staging.unmap();
        Ok(values.into_iter().take(length).collect())
    }

    fn compile(&self, kernel: &Kernel, modes: OperationModes) -> Result<WgpuKernel, DeviceError> {
        let map_text = emit_map_with(kernel, modes).map_err(|_| DeviceError::ShaderRejected)?;
        let reduce_text = emit_reduce(kernel).map_err(|_| DeviceError::ShaderRejected)?;
        let layout = self.bind_group_layout()?;
        let map = self.pipeline(&map_text, MAP_ENTRY_POINT, &layout)?;
        let reduce = match reduce_text {
            Some(text) => Some(self.pipeline(&text, REDUCE_ENTRY_POINT, &layout)?),
            None => None,
        };
        Ok(WgpuKernel {
            map,
            reduce,
            layout,
        })
    }

    fn dispatch(
        &self,
        compiled: &WgpuKernel,
        pass: PassId,
        buffers: &mut [WgpuBuffer],
        size: DispatchSize,
    ) -> Result<(), DeviceError> {
        let (pipeline, bound) = match pass {
            PassId::Map => (&compiled.map, [0_usize, 1, 2]),
            PassId::Reduce => (
                compiled
                    .reduce
                    .as_ref()
                    .ok_or(DeviceError::ShaderRejected)?,
                [2_usize, 4, 3],
            ),
        };
        let mut uniform_bytes = Vec::with_capacity(8);
        uniform_bytes.extend(size.element_count.to_le_bytes());
        uniform_bytes.extend(size.invocations.to_le_bytes());
        let (x, y) = workgroup_counts(size.invocations);
        self.scoped(|| {
            let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: SIZE_UNIFORM_BYTES,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue.write_buffer(&uniform, 0, &uniform_bytes);
            let mut entries: Vec<wgpu::BindGroupEntry<'_>> = bound
                .iter()
                .zip(0_u32..)
                .filter_map(|(index, binding)| {
                    buffers.get(*index).map(|buffer| wgpu::BindGroupEntry {
                        binding,
                        resource: buffer.buffer.as_entire_binding(),
                    })
                })
                .collect();
            entries.push(wgpu::BindGroupEntry {
                binding: BINDING_COUNT - 1,
                resource: uniform.as_entire_binding(),
            });
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &compiled.layout,
                entries: &entries,
            });
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            {
                let mut compute = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: None,
                    timestamp_writes: None,
                });
                compute.set_pipeline(pipeline);
                compute.set_bind_group(0, &bind_group, &[]);
                compute.dispatch_workgroups(x, y, 1);
            }
            self.queue.submit([encoder.finish()]);
        })?;
        self.wait()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_dispatch_uses_one_row_of_workgroups() {
        let counts = workgroup_counts(1_000);

        assert_eq!(counts, (16, 1));
    }

    #[test]
    fn empty_dispatch_still_names_one_workgroup() {
        let counts = workgroup_counts(0);

        assert_eq!(counts, (1, 1));
    }

    #[test]
    fn large_dispatch_spills_into_a_second_dimension() {
        let counts = workgroup_counts(8_294_400);

        assert_eq!(counts, (65_535, 2));
    }

    #[test]
    fn values_survive_the_byte_round_trip_bit_for_bit() {
        let values = [0.0_f32, -0.0, f32::NAN, f32::INFINITY, f32::from_bits(1)];

        let back = values_from_bytes(&value_bytes(&values));

        assert_eq!(
            back.iter().map(|value| value.to_bits()).collect::<Vec<_>>(),
            values
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn indices_are_little_endian() {
        let bytes = index_bytes(&[1, 0x0102_0304]);

        assert_eq!(bytes, vec![1, 0, 0, 0, 4, 3, 2, 1]);
    }

    #[test]
    fn device_creation_reports_why_it_failed_or_names_a_hardware_adapter() {
        let device = WgpuDevice::new(WgpuDeviceOptions::default());

        let acceptable = match &device {
            Ok(device) => !device.description().is_cpu,
            Err(DeviceCreationError::NoAdapter { message })
            | Err(DeviceCreationError::DeviceRequest { message }) => !message.is_empty(),
            Err(DeviceCreationError::CpuAdapterRefused { adapter }) => !adapter.is_empty(),
            Err(DeviceCreationError::NoVulkanBackend) => true,
        };
        assert!(acceptable, "{device:?}");
    }
}
