use crate::kernel::{BufferKind, Kernel, PassId};
use crate::modes::OperationModes;
use crate::validate::KernelError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceError {
    OutOfMemory,
    Lost,
    KernelRejected(KernelError),
    ShaderRejected,
    BufferKindMismatch,
    BufferTooShort,
}

#[derive(Clone, Copy, Debug)]
pub enum BufferData<'data> {
    Values(&'data [f32]),
    Indices(&'data [u32]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DispatchSize {
    pub element_count: u32,
    pub invocations: u32,
}

pub trait GpuDevice: Clone + Send + Sync + 'static {
    type Buffer: Send;
    type Compiled: Send;

    fn create_buffer(&self, kind: BufferKind, length: usize) -> Result<Self::Buffer, DeviceError>;
    fn write_buffer(
        &self,
        buffer: &mut Self::Buffer,
        data: BufferData<'_>,
    ) -> Result<(), DeviceError>;
    fn read_buffer(&self, buffer: &Self::Buffer, length: usize) -> Result<Vec<f32>, DeviceError>;
    fn compile(
        &self,
        kernel: &Kernel,
        modes: OperationModes,
    ) -> Result<Self::Compiled, DeviceError>;
    fn dispatch(
        &self,
        compiled: &Self::Compiled,
        pass: PassId,
        buffers: &mut [Self::Buffer],
        size: DispatchSize,
    ) -> Result<(), DeviceError>;
}
