use crate::batch::Batch;
use crate::domain::Domain;
use crate::operation::{OperationClass, PlanOp};
use crate::plan::{Plan, PlanBatchError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BackendKind {
    Cpu,
    Simd,
    Gpu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransferCost {
    pub latency_picoseconds: u64,
    pub picoseconds_per_entry: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CostParameters {
    pub setup_picoseconds: u64,
    pub picoseconds_per_gate_evaluation: u64,
    pub transfer: Option<TransferCost>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub domains: Vec<Domain>,
    pub operations: Vec<PlanOp>,
    pub largest_batch_length: usize,
    pub largest_iteration_count: u32,
    pub reference_approximate_operations: bool,
    pub cost: CostParameters,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsupported {
    Domain(Domain),
    Operation(PlanOp),
    IterationCount(u32),
    ApproximateOperationInIteration(PlanOp),
    DeviceRefused,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunReport {
    pub transfers: u64,
    pub moved_entries: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrepareError {
    Unsupported(Unsupported),
    OutOfDeviceMemory,
    DeviceLost,
    KernelRejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunError {
    BatchShape(PlanBatchError),
    EmptyReduction,
    ParallelTaskNotRun,
    OutOfDeviceMemory,
    DeviceLost,
    DeviceRejected,
}

pub trait Backend: Send + Sync {
    fn kind(&self) -> BackendKind;
    fn capabilities(&self) -> &Capabilities;
    fn prepare(&self, plan: &Plan) -> Result<Box<dyn Prepared>, PrepareError>;

    fn execution_modes(&self) -> Vec<(PlanOp, ExecutionMode)> {
        Vec::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionMode {
    Native,
    IntegerExact,
    ExactInBothForms,
}

pub trait Prepared: Send {
    fn run(&mut self, inputs: &Batch, outputs: &mut Batch) -> Result<RunReport, RunError>;
}

fn saturating_u64(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}

pub fn expected_transfers(plan: &Plan, batch_length: usize) -> RunReport {
    let input_entries =
        saturating_u64(plan.input_channel_count()).saturating_mul(saturating_u64(batch_length));
    let output_entries = saturating_u64(plan.output_channel_count())
        .saturating_mul(saturating_u64(plan.output_length(batch_length)));
    RunReport {
        transfers: saturating_u64(plan.input_channel_count())
            .saturating_add(saturating_u64(plan.output_channel_count())),
        moved_entries: input_entries.saturating_add(output_entries),
    }
}

impl Capabilities {
    pub fn check_plan(&self, plan: &Plan) -> Result<(), Unsupported> {
        if !self.domains.contains(&plan.domain()) {
            return Err(Unsupported::Domain(plan.domain()));
        }
        if let Some(operation) = plan
            .operations()
            .find(|operation| !self.operations.contains(operation))
        {
            return Err(Unsupported::Operation(operation));
        }
        if !self.reference_approximate_operations
            && let Some(operation) = plan
                .iteration_body_operations()
                .find(|operation| operation.class() == OperationClass::Approximate)
        {
            return Err(Unsupported::ApproximateOperationInIteration(operation));
        }
        let requested = plan.largest_iteration_count();
        if plan.has_iterations()
            && (self.largest_iteration_count == 0 || requested > self.largest_iteration_count)
        {
            return Err(Unsupported::IterationCount(requested));
        }
        Ok(())
    }

    pub fn estimate_picoseconds(&self, plan: &Plan, batch_length: usize) -> u128 {
        let evaluations = u128::from(plan.estimated_gate_evaluations_per_element())
            .saturating_mul(u128::from(saturating_u64(batch_length)));
        let evaluation = evaluations
            .saturating_mul(u128::from(self.cost.picoseconds_per_gate_evaluation))
            .saturating_add(u128::from(self.cost.setup_picoseconds));
        match self.cost.transfer {
            None => evaluation,
            Some(transfer) => {
                let report = expected_transfers(plan, batch_length);
                let latency = u128::from(report.transfers)
                    .saturating_mul(u128::from(transfer.latency_picoseconds));
                let moving = u128::from(report.moved_entries)
                    .saturating_mul(u128::from(transfer.picoseconds_per_entry));
                evaluation.saturating_add(latency).saturating_add(moving)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Constant;
    use crate::plan::{ChannelId, Gate, GateId, Reduce, ReduceOperation, ReduceShape};

    fn add_plan(reduce: Option<Reduce>) -> Plan {
        Plan::new(
            Domain::F32,
            2,
            vec![
                Gate::Input(ChannelId(0)),
                Gate::Input(ChannelId(1)),
                Gate::Operation {
                    operation: PlanOp::Add,
                    arguments: vec![GateId(0), GateId(1)],
                },
            ],
            vec![GateId(2)],
            reduce,
        )
        .unwrap()
    }

    fn capabilities(transfer: Option<TransferCost>) -> Capabilities {
        Capabilities {
            domains: vec![Domain::F32],
            operations: vec![PlanOp::Add],
            largest_batch_length: 1 << 20,
            largest_iteration_count: 0,
            reference_approximate_operations: false,
            cost: CostParameters {
                setup_picoseconds: 100,
                picoseconds_per_gate_evaluation: 3,
                transfer,
            },
        }
    }

    #[test]
    fn map_plan_transfers_every_input_and_output_entry() {
        let plan = add_plan(None);

        let report = expected_transfers(&plan, 10);

        assert_eq!(
            report,
            RunReport {
                transfers: 3,
                moved_entries: 30
            }
        );
    }

    #[test]
    fn reduced_plan_transfers_one_output_entry() {
        let plan = add_plan(Some(Reduce {
            operation: ReduceOperation::Add,
            shape: ReduceShape::Halving,
        }));

        let report = expected_transfers(&plan, 10);

        assert_eq!(
            report,
            RunReport {
                transfers: 3,
                moved_entries: 21
            }
        );
    }

    #[test]
    fn host_estimate_is_setup_plus_gate_evaluations() {
        let plan = add_plan(None);

        let estimate = capabilities(None).estimate_picoseconds(&plan, 10);

        assert_eq!(estimate, 100 + 3 * 10 * 3);
    }

    #[test]
    fn device_estimate_adds_a_latency_for_each_transfer_and_a_time_for_each_entry() {
        let plan = add_plan(None);
        let transfer = TransferCost {
            latency_picoseconds: 1_000,
            picoseconds_per_entry: 7,
        };

        let estimate = capabilities(Some(transfer)).estimate_picoseconds(&plan, 10);

        assert_eq!(estimate, 190 + 3 * 1_000 + 30 * 7);
    }

    #[test]
    fn a_single_entry_costs_its_own_time_rather_than_nothing() {
        let plan = add_plan(None);
        let transfer = TransferCost {
            latency_picoseconds: 0,
            picoseconds_per_entry: 4_119,
        };

        let estimate = capabilities(Some(transfer)).estimate_picoseconds(&plan, 1);

        assert_eq!(estimate, 100 + 3 * 3 + 3 * 4_119);
    }

    #[test]
    fn plan_in_missing_domain_is_unsupported() {
        let plan = Plan::new(
            Domain::F64,
            0,
            vec![Gate::Constant(Constant::F64(2.0))],
            vec![GateId(0)],
            None,
        )
        .unwrap();

        let checked = capabilities(None).check_plan(&plan);

        assert_eq!(checked, Err(Unsupported::Domain(Domain::F64)));
    }

    #[test]
    fn plan_with_missing_operation_is_unsupported() {
        let plan = Plan::new(
            Domain::F32,
            1,
            vec![
                Gate::Input(ChannelId(0)),
                Gate::Operation {
                    operation: PlanOp::Sqrt,
                    arguments: vec![GateId(0)],
                },
            ],
            vec![GateId(1)],
            None,
        )
        .unwrap();

        let checked = capabilities(None).check_plan(&plan);

        assert_eq!(checked, Err(Unsupported::Operation(PlanOp::Sqrt)));
    }

    fn sine_iteration_plan(maximum_count: u32) -> Plan {
        use crate::iteration::{BodyGate, BodyGateId, Iteration, SlotId};
        let iteration = Iteration {
            initial: vec![GateId(0)],
            body: vec![
                BodyGate::State(SlotId(0)),
                BodyGate::Operation {
                    operation: PlanOp::Sin,
                    arguments: vec![BodyGateId(0)],
                },
                BodyGate::Operation {
                    operation: PlanOp::Less,
                    arguments: vec![BodyGateId(0), BodyGateId(0)],
                },
            ],
            next: vec![BodyGateId(1)],
            exit: BodyGateId(2),
            maximum_count,
        };
        Plan::new(
            Domain::F32,
            1,
            vec![
                Gate::Input(ChannelId(0)),
                Gate::Iterate(Box::new(iteration)),
                Gate::IterationState {
                    iteration: GateId(1),
                    slot: SlotId(0),
                },
            ],
            vec![GateId(2)],
            None,
        )
        .unwrap()
    }

    fn iterating_capabilities() -> Capabilities {
        let mut capabilities = capabilities(None);
        capabilities.operations = vec![PlanOp::Sin, PlanOp::Less];
        capabilities.largest_iteration_count = 16;
        capabilities.reference_approximate_operations = true;
        capabilities
    }

    #[test]
    fn iteration_plan_on_backend_without_iterations_is_unsupported() {
        let mut capabilities = iterating_capabilities();
        capabilities.largest_iteration_count = 0;

        let checked = capabilities.check_plan(&sine_iteration_plan(0));

        assert_eq!(checked, Err(Unsupported::IterationCount(0)));
    }

    #[test]
    fn iteration_count_above_the_capability_is_unsupported() {
        let capabilities = iterating_capabilities();

        let checked = capabilities.check_plan(&sine_iteration_plan(17));

        assert_eq!(checked, Err(Unsupported::IterationCount(17)));
    }

    #[test]
    fn approximate_body_operation_without_reference_implementation_is_unsupported() {
        let mut capabilities = iterating_capabilities();
        capabilities.reference_approximate_operations = false;

        let checked = capabilities.check_plan(&sine_iteration_plan(8));

        assert_eq!(
            checked,
            Err(Unsupported::ApproximateOperationInIteration(PlanOp::Sin))
        );
    }

    #[test]
    fn approximate_body_operation_with_reference_implementation_is_supported() {
        let capabilities = iterating_capabilities();

        let checked = capabilities.check_plan(&sine_iteration_plan(8));

        assert_eq!(checked, Ok(()));
    }

    #[test]
    fn estimate_counts_an_iteration_body_once_per_element() {
        let plan = sine_iteration_plan(8);

        let estimate = iterating_capabilities().estimate_picoseconds(&plan, 10);

        assert_eq!(estimate, 100 + (1 + 3) * 10 * 3);
    }

    #[test]
    fn the_iteration_limit_does_not_move_the_estimate() {
        let capabilities = iterating_capabilities();

        let few = capabilities.estimate_picoseconds(&sine_iteration_plan(8), 10);
        let many = capabilities.estimate_picoseconds(&sine_iteration_plan(4_096), 10);

        assert_eq!(few, many);
    }

    #[test]
    fn the_worst_case_count_is_still_available_and_still_counts_the_limit() {
        let plan = sine_iteration_plan(8);

        let counts = (
            plan.worst_case_gate_evaluations_per_element(),
            plan.estimated_gate_evaluations_per_element(),
        );

        assert_eq!(counts, (1 + 3 * 8, 1 + 3));
    }
}
