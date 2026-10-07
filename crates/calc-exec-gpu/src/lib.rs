mod backend;
mod device;
mod kernel;
mod lower;
mod modes;
mod probe;
pub mod softfloat;
mod software;
mod validate;
mod wgsl;

pub use backend::{GpuBackend, GpuPrepared, capabilities};
pub use device::{BufferData, DeviceError, DispatchSize, GpuDevice};
pub use kernel::{
    BinaryOperation, BufferId, BufferKind, CARRY, CONSTANTS, Comparison, Expression, INPUTS,
    Kernel, KernelIteration, LocalId, MapPass, OUTPUTS, PassId, REDUCE_TARGET, ReducePass, TRIPLES,
    UnaryOperation,
};
pub use lower::{halving_stages, lower_plan};
pub use modes::{ELIGIBLE_FOR_NATIVE, EXACT_IN_BOTH_FORMS, OperationModes};
pub use probe::{OperationMode, OperationOutcome, ProbeCase, ProbeRefusal, ProbeReport, probe};
pub use softfloat::SOFTFLOAT_WGSL;
pub use software::{SoftwareBuffer, SoftwareDevice, SoftwareFault, SoftwareKernel, combine};
pub use validate::{IterationError, KernelError, LocalKind, StatementError, validate};
pub use wgsl::{
    EmitError, MAP_ENTRY_POINT, REDUCE_ENTRY_POINT, ShaderTextError, WORKGROUP_SIZE,
    WORKGROUPS_PER_DIMENSION, check_shader_text, combine_text, emit_map, emit_map_with,
    emit_reduce, loop_text, loop_text_with, statement_text, statement_text_with,
};
