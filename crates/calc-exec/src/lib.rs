mod backend;
mod batch;
pub mod cost;
mod domain;
mod escape_time;
mod iteration;
mod lowering;
mod operation;
mod parallelism;
mod plan;
mod selection;

pub use backend::{
    Backend, BackendKind, Capabilities, CostParameters, ExecutionMode, PrepareError, Prepared,
    RunError, RunReport, TransferCost, Unsupported, expected_transfers,
};
pub use batch::{Batch, BatchError, BatchShapeError};
pub use domain::{Constant, Domain};
pub use escape_time::{EscapeTimePlanForm, escape_time_iteration_plan, escape_time_plan};
pub use iteration::{BodyError, BodyGate, BodyGateId, Iteration, SlotId};
pub use lowering::{LowerError, LowerInput, LowerSpec, ValueForm, lower};
pub use operation::{OperationClass, PlanOp, Signature, ValueKind};
pub use parallelism::{Parallelism, SequentialParallelism, SharedParallelism};
pub use plan::{
    BatchRole, ChannelId, Gate, GateId, Plan, PlanBatchError, PlanError, Reduce, ReduceOperation,
    ReduceShape,
};
pub use selection::{Preference, SelectError, Selection, SkipReason, SkippedBackend, select};
