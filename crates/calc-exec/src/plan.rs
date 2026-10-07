use crate::batch::{Batch, BatchShapeError};
use crate::domain::{Constant, Domain};
use crate::iteration::{BodyError, BodyGate, BodyGateId, Iteration, SlotId, slot_kinds};
use crate::operation::{PlanOp, ValueKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GateId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChannelId(pub u32);

impl GateId {
    pub fn index(self) -> usize {
        index_of(self.0)
    }
}

impl ChannelId {
    pub fn index(self) -> usize {
        index_of(self.0)
    }
}

pub(crate) fn index_of(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

#[derive(Clone, Debug)]
pub enum Gate {
    Input(ChannelId),
    Constant(Constant),
    Operation {
        operation: PlanOp,
        arguments: Vec<GateId>,
    },
    Iterate(Box<Iteration>),
    IterationState {
        iteration: GateId,
        slot: SlotId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReduceOperation {
    Add,
    Mul,
    Min,
    Max,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReduceShape {
    LeftFold,
    Halving,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Reduce {
    pub operation: ReduceOperation,
    pub shape: ReduceShape,
}

#[derive(Clone, Debug)]
pub struct Plan {
    domain: Domain,
    input_channel_count: usize,
    gates: Vec<Gate>,
    kinds: Vec<Option<ValueKind>>,
    outputs: Vec<GateId>,
    reduce: Option<Reduce>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanError {
    InputChannelOutOfRange {
        gate_index: usize,
        channel: ChannelId,
    },
    ConstantDomainMismatch {
        gate_index: usize,
        constant: Domain,
    },
    ArityMismatch {
        gate_index: usize,
        operation: PlanOp,
        expected: usize,
        actual: usize,
    },
    ArgumentNotEarlier {
        gate_index: usize,
        argument: GateId,
    },
    ArgumentKindMismatch {
        gate_index: usize,
        operation: PlanOp,
        position: usize,
        expected: ValueKind,
        actual: ValueKind,
    },
    NoOutputs,
    OutputOutOfRange {
        output: GateId,
    },
    OutputNotScalar {
        output: GateId,
    },
    ReduceNeedsOneOutput {
        outputs: usize,
    },
    ValueExpected {
        gate_index: usize,
        argument: GateId,
    },
    IterationInitialNotEarlier {
        gate_index: usize,
        initial: GateId,
    },
    IterationSlotCountMismatch {
        gate_index: usize,
        initial: usize,
        next: usize,
    },
    IterationSlotKindMismatch {
        gate_index: usize,
        slot: usize,
        initial: ValueKind,
        next: ValueKind,
    },
    IterationResultOutOfRange {
        gate_index: usize,
        body_gate: BodyGateId,
    },
    IterationExitNotBoolean {
        gate_index: usize,
    },
    NotAnIteration {
        gate_index: usize,
        iteration: GateId,
    },
    IterationSlotOutOfRange {
        gate_index: usize,
        slot: SlotId,
    },
    UnrolledTooLarge {
        gates: u64,
    },
    Body {
        gate_index: usize,
        body_gate: usize,
        error: BodyError,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchRole {
    Inputs,
    Outputs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanBatchError {
    pub role: BatchRole,
    pub shape: BatchShapeError,
}

impl Plan {
    pub fn new(
        domain: Domain,
        input_channel_count: usize,
        gates: Vec<Gate>,
        outputs: Vec<GateId>,
        reduce: Option<Reduce>,
    ) -> Result<Plan, PlanError> {
        let kinds = gate_kinds(domain, input_channel_count, &gates)?;
        check_outputs(&kinds, &outputs, reduce)?;
        Ok(Plan {
            domain,
            input_channel_count,
            gates,
            kinds,
            outputs,
            reduce,
        })
    }

    pub fn domain(&self) -> Domain {
        self.domain
    }

    pub fn input_channel_count(&self) -> usize {
        self.input_channel_count
    }

    pub fn output_channel_count(&self) -> usize {
        self.outputs.len()
    }

    pub fn gates(&self) -> &[Gate] {
        &self.gates
    }

    pub fn gate_count(&self) -> usize {
        self.gates.len()
    }

    pub fn outputs(&self) -> &[GateId] {
        &self.outputs
    }

    pub fn reduce(&self) -> Option<Reduce> {
        self.reduce
    }

    pub fn operations(&self) -> impl Iterator<Item = PlanOp> + '_ {
        self.gates.iter().flat_map(|gate| match gate {
            Gate::Operation { operation, .. } => vec![*operation],
            Gate::Iterate(iteration) => iteration
                .body
                .iter()
                .filter_map(|body_gate| match body_gate {
                    BodyGate::Operation { operation, .. } => Some(*operation),
                    BodyGate::State(_) | BodyGate::Outer(_) | BodyGate::Constant(_) => None,
                })
                .collect(),
            Gate::Input(_) | Gate::Constant(_) | Gate::IterationState { .. } => Vec::new(),
        })
    }

    pub fn iteration_body_operations(&self) -> impl Iterator<Item = PlanOp> + '_ {
        self.gates.iter().flat_map(|gate| match gate {
            Gate::Iterate(iteration) => iteration
                .body
                .iter()
                .filter_map(|body_gate| match body_gate {
                    BodyGate::Operation { operation, .. } => Some(*operation),
                    BodyGate::State(_) | BodyGate::Outer(_) | BodyGate::Constant(_) => None,
                })
                .collect(),
            Gate::Input(_)
            | Gate::Constant(_)
            | Gate::Operation { .. }
            | Gate::IterationState { .. } => Vec::new(),
        })
    }

    pub fn value_kind(&self, gate: GateId) -> Option<ValueKind> {
        self.kinds.get(gate.index()).copied().flatten()
    }

    pub fn has_iterations(&self) -> bool {
        self.gates
            .iter()
            .any(|gate| matches!(gate, Gate::Iterate(_)))
    }

    pub fn largest_iteration_count(&self) -> u32 {
        self.gates
            .iter()
            .filter_map(|gate| match gate {
                Gate::Iterate(iteration) => Some(iteration.maximum_count),
                _ => None,
            })
            .max()
            .unwrap_or(0)
    }

    pub fn worst_case_gate_evaluations_per_element(&self) -> u64 {
        self.gates.iter().fold(0_u64, |count, gate| {
            let evaluations = match gate {
                Gate::Iterate(iteration) => u64::try_from(iteration.body.len())
                    .unwrap_or(u64::MAX)
                    .saturating_mul(u64::from(iteration.maximum_count)),
                Gate::IterationState { .. } => 0,
                Gate::Input(_) | Gate::Constant(_) | Gate::Operation { .. } => 1,
            };
            count.saturating_add(evaluations)
        })
    }

    pub fn estimated_gate_evaluations_per_element(&self) -> u64 {
        self.gates.iter().fold(0_u64, |count, gate| {
            let evaluations = match gate {
                Gate::Iterate(iteration) => u64::try_from(iteration.body.len()).unwrap_or(u64::MAX),
                Gate::IterationState { .. } => 0,
                Gate::Input(_) | Gate::Constant(_) | Gate::Operation { .. } => 1,
            };
            count.saturating_add(evaluations)
        })
    }

    fn unrolled_gate_bound(&self) -> u64 {
        self.gates.iter().fold(0_u64, |count, gate| {
            let gates = match gate {
                Gate::Iterate(iteration) => {
                    let per_step = u64::try_from(iteration.body.len())
                        .unwrap_or(u64::MAX)
                        .saturating_add(
                            u64::try_from(iteration.initial.len())
                                .unwrap_or(u64::MAX)
                                .saturating_mul(4),
                        )
                        .saturating_add(2);
                    per_step.saturating_mul(u64::from(iteration.maximum_count))
                }
                Gate::IterationState { .. } => 0,
                Gate::Input(_) | Gate::Constant(_) | Gate::Operation { .. } => 1,
            };
            count.saturating_add(gates)
        })
    }

    pub fn unrolled(&self) -> Result<Plan, PlanError> {
        let gates = self.unrolled_gate_bound();
        if gates > u64::from(u32::MAX) {
            return Err(PlanError::UnrolledTooLarge { gates });
        }
        let mut unrolling = Unrolling {
            gates: Vec::new(),
            positions: Vec::with_capacity(self.gates.len()),
            final_states: Vec::with_capacity(self.gates.len()),
        };
        for gate in &self.gates {
            let (position, final_state) = match gate {
                Gate::Input(_) | Gate::Constant(_) => (Some(unrolling.push(gate.clone())), None),
                Gate::Operation {
                    operation,
                    arguments,
                } => {
                    let arguments = arguments
                        .iter()
                        .map(|argument| unrolling.position_of(*argument))
                        .collect();
                    (
                        Some(unrolling.push(Gate::Operation {
                            operation: *operation,
                            arguments,
                        })),
                        None,
                    )
                }
                Gate::Iterate(iteration) => (None, Some(unrolling.iteration(self, iteration))),
                Gate::IterationState { iteration, slot } => (
                    unrolling
                        .final_states
                        .get(iteration.index())
                        .and_then(Option::as_ref)
                        .and_then(|states| states.get(slot.index()))
                        .copied(),
                    None,
                ),
            };
            unrolling.positions.push(position);
            unrolling.final_states.push(final_state);
        }
        let outputs = self
            .outputs
            .iter()
            .map(|output| unrolling.position_of(*output))
            .collect();
        Plan::new(
            self.domain,
            self.input_channel_count,
            unrolling.gates,
            outputs,
            self.reduce,
        )
    }

    pub fn output_length(&self, batch_length: usize) -> usize {
        match self.reduce {
            Some(_) => 1,
            None => batch_length,
        }
    }

    pub fn check_batches(&self, inputs: &Batch, outputs: &Batch) -> Result<(), PlanBatchError> {
        inputs
            .check_shape(self.domain, self.input_channel_count, inputs.len())
            .map_err(|shape| PlanBatchError {
                role: BatchRole::Inputs,
                shape,
            })?;
        outputs
            .check_shape(
                self.domain,
                self.outputs.len(),
                self.output_length(inputs.len()),
            )
            .map_err(|shape| PlanBatchError {
                role: BatchRole::Outputs,
                shape,
            })
    }
}

struct Unrolling {
    gates: Vec<Gate>,
    positions: Vec<Option<GateId>>,
    final_states: Vec<Option<Vec<GateId>>>,
}

impl Unrolling {
    fn push(&mut self, gate: Gate) -> GateId {
        self.gates.push(gate);
        GateId(u32::try_from(self.gates.len() - 1).unwrap_or(u32::MAX))
    }

    fn position_of(&self, gate: GateId) -> GateId {
        self.positions
            .get(gate.index())
            .copied()
            .flatten()
            .unwrap_or(GateId(u32::MAX))
    }

    fn operation(&mut self, operation: PlanOp, arguments: &[GateId]) -> GateId {
        self.push(Gate::Operation {
            operation,
            arguments: arguments.to_vec(),
        })
    }

    fn iteration(&mut self, plan: &Plan, iteration: &Iteration) -> Vec<GateId> {
        let mut state: Vec<GateId> = iteration
            .initial
            .iter()
            .map(|initial| self.position_of(*initial))
            .collect();
        let slot_kinds: Vec<Option<ValueKind>> = iteration
            .initial
            .iter()
            .map(|initial| plan.value_kind(*initial))
            .collect();
        let mut done: Option<GateId> = None;
        for _ in 0..iteration.maximum_count {
            let mut body: Vec<GateId> = Vec::with_capacity(iteration.body.len());
            for body_gate in &iteration.body {
                let position = match body_gate {
                    BodyGate::State(slot) => {
                        state.get(slot.index()).copied().unwrap_or(GateId(u32::MAX))
                    }
                    BodyGate::Outer(outer) => self.position_of(*outer),
                    BodyGate::Constant(constant) => self.push(Gate::Constant(*constant)),
                    BodyGate::Operation {
                        operation,
                        arguments,
                    } => {
                        let arguments: Vec<GateId> = arguments
                            .iter()
                            .map(|argument| {
                                body.get(argument.index())
                                    .copied()
                                    .unwrap_or(GateId(u32::MAX))
                            })
                            .collect();
                        self.operation(*operation, &arguments)
                    }
                };
                body.push(position);
            }
            let body_position =
                |gate: BodyGateId| body.get(gate.index()).copied().unwrap_or(GateId(u32::MAX));
            let exit = body_position(iteration.exit);
            let step_done = match done {
                None => exit,
                Some(previous) => self.operation(PlanOp::Or, &[previous, exit]),
            };
            let nexts: Vec<GateId> = iteration
                .next
                .iter()
                .map(|next| body_position(*next))
                .collect();
            let mut not_done: Option<GateId> = None;
            let mut new_state = Vec::with_capacity(state.len());
            for ((current, next), kind) in state.iter().zip(&nexts).zip(&slot_kinds) {
                let frozen = match kind {
                    Some(ValueKind::Boolean) => {
                        let inverted = match not_done {
                            Some(gate) => gate,
                            None => {
                                let gate = self.operation(PlanOp::Not, &[step_done]);
                                not_done = Some(gate);
                                gate
                            }
                        };
                        let kept = self.operation(PlanOp::And, &[step_done, *current]);
                        let advanced = self.operation(PlanOp::And, &[inverted, *next]);
                        self.operation(PlanOp::Or, &[kept, advanced])
                    }
                    _ => self.operation(PlanOp::Select, &[step_done, *current, *next]),
                };
                new_state.push(frozen);
            }
            state = new_state;
            done = Some(step_done);
        }
        state
    }
}

fn gate_kinds(
    domain: Domain,
    input_channel_count: usize,
    gates: &[Gate],
) -> Result<Vec<Option<ValueKind>>, PlanError> {
    let mut kinds: Vec<Option<ValueKind>> = Vec::with_capacity(gates.len());
    let mut iteration_slots: Vec<Option<Vec<ValueKind>>> = Vec::with_capacity(gates.len());
    for (gate_index, gate) in gates.iter().enumerate() {
        let kind = match gate {
            Gate::Input(channel) => {
                if channel.index() >= input_channel_count {
                    return Err(PlanError::InputChannelOutOfRange {
                        gate_index,
                        channel: *channel,
                    });
                }
                Some(ValueKind::Scalar)
            }
            Gate::Constant(constant) => {
                if constant.domain() != domain {
                    return Err(PlanError::ConstantDomainMismatch {
                        gate_index,
                        constant: constant.domain(),
                    });
                }
                Some(ValueKind::Scalar)
            }
            Gate::Operation {
                operation,
                arguments,
            } => Some(operation_kind(gate_index, *operation, arguments, &kinds)?),
            Gate::Iterate(iteration) => {
                let slots = slot_kinds(domain, gate_index, iteration, &kinds)?;
                kinds.push(None);
                iteration_slots.push(Some(slots));
                continue;
            }
            Gate::IterationState { iteration, slot } => {
                let Some(Some(slots)) = iteration_slots.get(iteration.index()) else {
                    return Err(PlanError::NotAnIteration {
                        gate_index,
                        iteration: *iteration,
                    });
                };
                let Some(kind) = slots.get(slot.index()) else {
                    return Err(PlanError::IterationSlotOutOfRange {
                        gate_index,
                        slot: *slot,
                    });
                };
                Some(*kind)
            }
        };
        kinds.push(kind);
        iteration_slots.push(None);
    }
    Ok(kinds)
}

fn operation_kind(
    gate_index: usize,
    operation: PlanOp,
    arguments: &[GateId],
    earlier_kinds: &[Option<ValueKind>],
) -> Result<ValueKind, PlanError> {
    let signature = operation.signature();
    if arguments.len() != signature.arguments.len() {
        return Err(PlanError::ArityMismatch {
            gate_index,
            operation,
            expected: signature.arguments.len(),
            actual: arguments.len(),
        });
    }
    for (position, (argument, expected)) in arguments.iter().zip(signature.arguments).enumerate() {
        let actual = match earlier_kinds.get(argument.index()) {
            None => {
                return Err(PlanError::ArgumentNotEarlier {
                    gate_index,
                    argument: *argument,
                });
            }
            Some(None) => {
                return Err(PlanError::ValueExpected {
                    gate_index,
                    argument: *argument,
                });
            }
            Some(Some(kind)) => kind,
        };
        if actual != expected {
            return Err(PlanError::ArgumentKindMismatch {
                gate_index,
                operation,
                position,
                expected: *expected,
                actual: *actual,
            });
        }
    }
    Ok(signature.result)
}

fn check_outputs(
    kinds: &[Option<ValueKind>],
    outputs: &[GateId],
    reduce: Option<Reduce>,
) -> Result<(), PlanError> {
    if outputs.is_empty() {
        return Err(PlanError::NoOutputs);
    }
    for output in outputs {
        match kinds.get(output.index()) {
            None => return Err(PlanError::OutputOutOfRange { output: *output }),
            Some(None | Some(ValueKind::Boolean)) => {
                return Err(PlanError::OutputNotScalar { output: *output });
            }
            Some(Some(ValueKind::Scalar)) => {}
        }
    }
    if reduce.is_some() && outputs.len() != 1 {
        return Err(PlanError::ReduceNeedsOneOutput {
            outputs: outputs.len(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation(operation: PlanOp, arguments: &[u32]) -> Gate {
        Gate::Operation {
            operation,
            arguments: arguments.iter().copied().map(GateId).collect(),
        }
    }

    fn sum_of_two_inputs() -> Vec<Gate> {
        vec![
            Gate::Input(ChannelId(0)),
            Gate::Input(ChannelId(1)),
            operation(PlanOp::Add, &[0, 1]),
        ]
    }

    #[test]
    fn plan_and_batch_can_move_to_another_thread() {
        fn require_send<T: Send>() {}

        require_send::<Plan>();

        require_send::<Batch>();
    }

    #[test]
    fn valid_circuit_is_accepted() {
        let gates = sum_of_two_inputs();

        let plan = Plan::new(Domain::F64, 2, gates, vec![GateId(2)], None);

        assert!(plan.is_ok());
    }

    #[test]
    fn input_channel_beyond_count_is_rejected() {
        let gates = vec![Gate::Input(ChannelId(1))];

        let plan = Plan::new(Domain::F32, 1, gates, vec![GateId(0)], None);

        assert_eq!(
            plan.err(),
            Some(PlanError::InputChannelOutOfRange {
                gate_index: 0,
                channel: ChannelId(1)
            })
        );
    }

    #[test]
    fn constant_of_other_domain_is_rejected() {
        let gates = vec![Gate::Constant(Constant::F64(1.0))];

        let plan = Plan::new(Domain::F32, 0, gates, vec![GateId(0)], None);

        assert_eq!(
            plan.err(),
            Some(PlanError::ConstantDomainMismatch {
                gate_index: 0,
                constant: Domain::F64
            })
        );
    }

    #[test]
    fn wrong_argument_count_is_rejected() {
        let gates = vec![Gate::Input(ChannelId(0)), operation(PlanOp::Add, &[0])];

        let plan = Plan::new(Domain::F64, 1, gates, vec![GateId(1)], None);

        assert_eq!(
            plan.err(),
            Some(PlanError::ArityMismatch {
                gate_index: 1,
                operation: PlanOp::Add,
                expected: 2,
                actual: 1
            })
        );
    }

    #[test]
    fn argument_referring_to_own_gate_is_rejected() {
        let gates = vec![Gate::Input(ChannelId(0)), operation(PlanOp::Neg, &[1])];

        let plan = Plan::new(Domain::F64, 1, gates, vec![GateId(1)], None);

        assert_eq!(
            plan.err(),
            Some(PlanError::ArgumentNotEarlier {
                gate_index: 1,
                argument: GateId(1)
            })
        );
    }

    #[test]
    fn boolean_argument_to_arithmetic_is_rejected() {
        let gates = vec![
            Gate::Input(ChannelId(0)),
            operation(PlanOp::Less, &[0, 0]),
            operation(PlanOp::Neg, &[1]),
        ];

        let plan = Plan::new(Domain::F64, 1, gates, vec![GateId(2)], None);

        assert_eq!(
            plan.err(),
            Some(PlanError::ArgumentKindMismatch {
                gate_index: 2,
                operation: PlanOp::Neg,
                position: 0,
                expected: ValueKind::Scalar,
                actual: ValueKind::Boolean
            })
        );
    }

    #[test]
    fn plan_without_outputs_is_rejected() {
        let gates = sum_of_two_inputs();

        let plan = Plan::new(Domain::F64, 2, gates, Vec::new(), None);

        assert_eq!(plan.err(), Some(PlanError::NoOutputs));
    }

    #[test]
    fn output_beyond_gates_is_rejected() {
        let gates = sum_of_two_inputs();

        let plan = Plan::new(Domain::F64, 2, gates, vec![GateId(3)], None);

        assert_eq!(
            plan.err(),
            Some(PlanError::OutputOutOfRange { output: GateId(3) })
        );
    }

    #[test]
    fn boolean_output_is_rejected() {
        let gates = vec![Gate::Input(ChannelId(0)), operation(PlanOp::Equal, &[0, 0])];

        let plan = Plan::new(Domain::F64, 1, gates, vec![GateId(1)], None);

        assert_eq!(
            plan.err(),
            Some(PlanError::OutputNotScalar { output: GateId(1) })
        );
    }

    #[test]
    fn reduce_with_two_outputs_is_rejected() {
        let gates = sum_of_two_inputs();
        let reduce = Reduce {
            operation: ReduceOperation::Add,
            shape: ReduceShape::Halving,
        };

        let plan = Plan::new(
            Domain::F64,
            2,
            gates,
            vec![GateId(0), GateId(2)],
            Some(reduce),
        );

        assert_eq!(
            plan.err(),
            Some(PlanError::ReduceNeedsOneOutput { outputs: 2 })
        );
    }

    #[test]
    fn reduced_plan_has_output_length_one() {
        let reduce = Reduce {
            operation: ReduceOperation::Max,
            shape: ReduceShape::LeftFold,
        };
        let plan = Plan::new(
            Domain::F64,
            2,
            sum_of_two_inputs(),
            vec![GateId(2)],
            Some(reduce),
        )
        .unwrap();

        let length = plan.output_length(1025);

        assert_eq!(length, 1);
    }

    #[test]
    fn operations_lists_only_operation_gates() {
        let plan = Plan::new(Domain::F64, 2, sum_of_two_inputs(), vec![GateId(2)], None).unwrap();

        let operations: Vec<PlanOp> = plan.operations().collect();

        assert_eq!(operations, vec![PlanOp::Add]);
    }

    #[test]
    fn matching_batches_pass_the_check() {
        let plan = Plan::new(Domain::F64, 2, sum_of_two_inputs(), vec![GateId(2)], None).unwrap();
        let inputs = Batch::zeroed(Domain::F64, 2, 5);
        let outputs = Batch::zeroed(Domain::F64, 1, 5);

        let checked = plan.check_batches(&inputs, &outputs);

        assert_eq!(checked, Ok(()));
    }

    #[test]
    fn input_batch_with_missing_channel_fails_the_check() {
        let plan = Plan::new(Domain::F64, 2, sum_of_two_inputs(), vec![GateId(2)], None).unwrap();
        let inputs = Batch::zeroed(Domain::F64, 1, 5);
        let outputs = Batch::zeroed(Domain::F64, 1, 5);

        let checked = plan.check_batches(&inputs, &outputs);

        assert_eq!(
            checked,
            Err(PlanBatchError {
                role: BatchRole::Inputs,
                shape: BatchShapeError::ChannelCountMismatch {
                    expected: 2,
                    actual: 1
                }
            })
        );
    }

    #[test]
    fn output_batch_of_map_length_fails_the_check_for_reduced_plan() {
        let reduce = Reduce {
            operation: ReduceOperation::Add,
            shape: ReduceShape::Halving,
        };
        let plan = Plan::new(
            Domain::F64,
            2,
            sum_of_two_inputs(),
            vec![GateId(2)],
            Some(reduce),
        )
        .unwrap();
        let inputs = Batch::zeroed(Domain::F64, 2, 5);
        let outputs = Batch::zeroed(Domain::F64, 1, 5);

        let checked = plan.check_batches(&inputs, &outputs);

        assert_eq!(
            checked,
            Err(PlanBatchError {
                role: BatchRole::Outputs,
                shape: BatchShapeError::LengthMismatch {
                    expected: 1,
                    actual: 5
                }
            })
        );
    }

    fn doubling_iteration(maximum_count: u32) -> Iteration {
        Iteration {
            initial: vec![GateId(0)],
            body: vec![
                BodyGate::State(SlotId(0)),
                BodyGate::Outer(GateId(1)),
                BodyGate::Constant(Constant::F64(2.0)),
                BodyGate::Operation {
                    operation: PlanOp::Mul,
                    arguments: vec![BodyGateId(0), BodyGateId(2)],
                },
                BodyGate::Operation {
                    operation: PlanOp::Greater,
                    arguments: vec![BodyGateId(0), BodyGateId(1)],
                },
            ],
            next: vec![BodyGateId(3)],
            exit: BodyGateId(4),
            maximum_count,
        }
    }

    fn doubling_gates(iteration: Iteration) -> Vec<Gate> {
        vec![
            Gate::Input(ChannelId(0)),
            Gate::Constant(Constant::F64(100.0)),
            Gate::Iterate(Box::new(iteration)),
            Gate::IterationState {
                iteration: GateId(2),
                slot: SlotId(0),
            },
        ]
    }

    fn doubling_plan(iteration: Iteration) -> Result<Plan, PlanError> {
        Plan::new(
            Domain::F64,
            1,
            doubling_gates(iteration),
            vec![GateId(3)],
            None,
        )
    }

    fn body_failure(plan: Result<Plan, PlanError>) -> Option<BodyError> {
        match plan {
            Err(PlanError::Body { error, .. }) => Some(error),
            _ => None,
        }
    }

    #[test]
    fn valid_iteration_is_accepted() {
        let plan = doubling_plan(doubling_iteration(8));

        assert!(plan.is_ok());
    }

    #[test]
    fn iteration_state_has_the_kind_of_its_slot() {
        let plan = doubling_plan(doubling_iteration(8)).unwrap();

        let kind = plan.value_kind(GateId(3));

        assert_eq!(kind, Some(ValueKind::Scalar));
    }

    #[test]
    fn iterate_gate_has_no_value() {
        let plan = doubling_plan(doubling_iteration(8)).unwrap();

        let kind = plan.value_kind(GateId(2));

        assert_eq!(kind, None);
    }

    #[test]
    fn iterate_gate_as_an_argument_is_rejected() {
        let mut gates = doubling_gates(doubling_iteration(8));
        gates.push(operation(PlanOp::Neg, &[2]));

        let plan = Plan::new(Domain::F64, 1, gates, vec![GateId(4)], None);

        assert_eq!(
            plan.err(),
            Some(PlanError::ValueExpected {
                gate_index: 4,
                argument: GateId(2)
            })
        );
    }

    #[test]
    fn iterate_gate_as_an_output_is_rejected() {
        let gates = doubling_gates(doubling_iteration(8));

        let plan = Plan::new(Domain::F64, 1, gates, vec![GateId(2)], None);

        assert_eq!(
            plan.err(),
            Some(PlanError::OutputNotScalar { output: GateId(2) })
        );
    }

    #[test]
    fn initial_gate_after_the_iteration_is_rejected() {
        let mut iteration = doubling_iteration(8);
        iteration.initial = vec![GateId(3)];

        let plan = doubling_plan(iteration);

        assert_eq!(
            plan.err(),
            Some(PlanError::IterationInitialNotEarlier {
                gate_index: 2,
                initial: GateId(3)
            })
        );
    }

    #[test]
    fn next_list_of_other_length_is_rejected() {
        let mut iteration = doubling_iteration(8);
        iteration.next = vec![BodyGateId(3), BodyGateId(3)];

        let plan = doubling_plan(iteration);

        assert_eq!(
            plan.err(),
            Some(PlanError::IterationSlotCountMismatch {
                gate_index: 2,
                initial: 1,
                next: 2
            })
        );
    }

    #[test]
    fn boolean_next_for_scalar_slot_is_rejected() {
        let mut iteration = doubling_iteration(8);
        iteration.next = vec![BodyGateId(4)];

        let plan = doubling_plan(iteration);

        assert_eq!(
            plan.err(),
            Some(PlanError::IterationSlotKindMismatch {
                gate_index: 2,
                slot: 0,
                initial: ValueKind::Scalar,
                next: ValueKind::Boolean
            })
        );
    }

    #[test]
    fn next_outside_the_body_is_rejected() {
        let mut iteration = doubling_iteration(8);
        iteration.next = vec![BodyGateId(9)];

        let plan = doubling_plan(iteration);

        assert_eq!(
            plan.err(),
            Some(PlanError::IterationResultOutOfRange {
                gate_index: 2,
                body_gate: BodyGateId(9)
            })
        );
    }

    #[test]
    fn scalar_exit_is_rejected() {
        let mut iteration = doubling_iteration(8);
        iteration.exit = BodyGateId(3);

        let plan = doubling_plan(iteration);

        assert_eq!(
            plan.err(),
            Some(PlanError::IterationExitNotBoolean { gate_index: 2 })
        );
    }

    #[test]
    fn iteration_state_of_a_non_iteration_gate_is_rejected() {
        let mut gates = doubling_gates(doubling_iteration(8));
        gates[3] = Gate::IterationState {
            iteration: GateId(1),
            slot: SlotId(0),
        };

        let plan = Plan::new(Domain::F64, 1, gates, vec![GateId(3)], None);

        assert_eq!(
            plan.err(),
            Some(PlanError::NotAnIteration {
                gate_index: 3,
                iteration: GateId(1)
            })
        );
    }

    #[test]
    fn iteration_state_beyond_the_slots_is_rejected() {
        let mut gates = doubling_gates(doubling_iteration(8));
        gates[3] = Gate::IterationState {
            iteration: GateId(2),
            slot: SlotId(1),
        };

        let plan = Plan::new(Domain::F64, 1, gates, vec![GateId(3)], None);

        assert_eq!(
            plan.err(),
            Some(PlanError::IterationSlotOutOfRange {
                gate_index: 3,
                slot: SlotId(1)
            })
        );
    }

    #[test]
    fn body_state_beyond_the_slots_is_rejected() {
        let mut iteration = doubling_iteration(8);
        iteration.body[0] = BodyGate::State(SlotId(1));

        let plan = doubling_plan(iteration);

        assert_eq!(
            body_failure(plan),
            Some(BodyError::StateOutOfRange(SlotId(1)))
        );
    }

    #[test]
    fn body_outer_gate_after_the_iteration_is_rejected() {
        let mut iteration = doubling_iteration(8);
        iteration.body[1] = BodyGate::Outer(GateId(3));

        let plan = doubling_plan(iteration);

        assert_eq!(
            body_failure(plan),
            Some(BodyError::OuterNotEarlier(GateId(3)))
        );
    }

    #[test]
    fn body_outer_iterate_gate_is_rejected() {
        let mut gates = doubling_gates(doubling_iteration(8));
        let mut second = doubling_iteration(8);
        second.body[1] = BodyGate::Outer(GateId(2));
        gates.push(Gate::Iterate(Box::new(second)));

        let plan = Plan::new(Domain::F64, 1, gates, vec![GateId(3)], None);

        assert_eq!(
            body_failure(plan),
            Some(BodyError::OuterNotValue(GateId(2)))
        );
    }

    #[test]
    fn body_constant_of_other_domain_is_rejected() {
        let mut iteration = doubling_iteration(8);
        iteration.body[2] = BodyGate::Constant(Constant::F32(2.0));

        let plan = doubling_plan(iteration);

        assert_eq!(
            body_failure(plan),
            Some(BodyError::ConstantDomainMismatch(Domain::F32))
        );
    }

    #[test]
    fn body_operation_with_wrong_arity_is_rejected() {
        let mut iteration = doubling_iteration(8);
        iteration.body[3] = BodyGate::Operation {
            operation: PlanOp::Mul,
            arguments: vec![BodyGateId(0)],
        };

        let plan = doubling_plan(iteration);

        assert_eq!(
            body_failure(plan),
            Some(BodyError::ArityMismatch {
                operation: PlanOp::Mul,
                expected: 2,
                actual: 1
            })
        );
    }

    #[test]
    fn body_argument_that_is_not_earlier_is_rejected() {
        let mut iteration = doubling_iteration(8);
        iteration.body[3] = BodyGate::Operation {
            operation: PlanOp::Mul,
            arguments: vec![BodyGateId(0), BodyGateId(3)],
        };

        let plan = doubling_plan(iteration);

        assert_eq!(
            body_failure(plan),
            Some(BodyError::ArgumentNotEarlier(BodyGateId(3)))
        );
    }

    #[test]
    fn body_argument_of_the_wrong_kind_is_rejected() {
        let mut iteration = doubling_iteration(8);
        iteration.body.push(BodyGate::Operation {
            operation: PlanOp::Neg,
            arguments: vec![BodyGateId(4)],
        });

        let plan = doubling_plan(iteration);

        assert_eq!(
            body_failure(plan),
            Some(BodyError::ArgumentKindMismatch {
                operation: PlanOp::Neg,
                position: 0,
                expected: ValueKind::Scalar,
                actual: ValueKind::Boolean
            })
        );
    }

    #[test]
    fn operations_include_the_body_operations() {
        let plan = doubling_plan(doubling_iteration(8)).unwrap();

        let operations: Vec<PlanOp> = plan.operations().collect();

        assert_eq!(operations, vec![PlanOp::Mul, PlanOp::Greater]);
    }

    #[test]
    fn largest_iteration_count_is_the_maximum_count() {
        let plan = doubling_plan(doubling_iteration(8)).unwrap();

        let count = plan.largest_iteration_count();

        assert_eq!((plan.has_iterations(), count), (true, 8));
    }

    #[test]
    fn unrolling_past_the_gate_index_range_is_rejected() {
        let plan = doubling_plan(doubling_iteration(u32::MAX)).unwrap();

        let unrolled = plan.unrolled();

        assert!(matches!(
            unrolled.err(),
            Some(PlanError::UnrolledTooLarge { gates }) if gates > u64::from(u32::MAX)
        ));
    }

    #[test]
    fn boolean_iteration_state_as_arithmetic_argument_is_rejected() {
        let iteration = Iteration {
            initial: vec![GateId(1)],
            body: vec![BodyGate::State(SlotId(0))],
            next: vec![BodyGateId(0)],
            exit: BodyGateId(0),
            maximum_count: 1,
        };
        let gates = vec![
            Gate::Input(ChannelId(0)),
            operation(PlanOp::Less, &[0, 0]),
            Gate::Iterate(Box::new(iteration)),
            Gate::IterationState {
                iteration: GateId(2),
                slot: SlotId(0),
            },
            operation(PlanOp::Neg, &[3]),
        ];

        let plan = Plan::new(Domain::F64, 1, gates, vec![GateId(4)], None);

        assert_eq!(
            plan.err(),
            Some(PlanError::ArgumentKindMismatch {
                gate_index: 4,
                operation: PlanOp::Neg,
                position: 0,
                expected: ValueKind::Scalar,
                actual: ValueKind::Boolean
            })
        );
    }

    #[test]
    fn unrolled_plan_has_no_iteration_gates() {
        let plan = doubling_plan(doubling_iteration(3)).unwrap();

        let unrolled = plan.unrolled().unwrap();

        assert!(!unrolled.has_iterations());
    }

    #[test]
    fn unrolled_plan_writes_the_body_once_per_step() {
        let plan = doubling_plan(doubling_iteration(3)).unwrap();

        let unrolled = plan.unrolled().unwrap();

        assert_eq!(unrolled.gate_count(), 2 + 3 * 4 + 2);
    }

    #[test]
    fn unrolled_plan_of_zero_steps_outputs_the_initial_state() {
        let plan = doubling_plan(doubling_iteration(0)).unwrap();

        let unrolled = plan.unrolled().unwrap();

        assert!(matches!(
            unrolled.gates()[unrolled.outputs()[0].index()],
            Gate::Input(ChannelId(0))
        ));
    }

    #[test]
    fn unrolled_boolean_slot_is_frozen_with_logic_gates() {
        let iteration = Iteration {
            initial: vec![GateId(1)],
            body: vec![
                BodyGate::State(SlotId(0)),
                BodyGate::Operation {
                    operation: PlanOp::Not,
                    arguments: vec![BodyGateId(0)],
                },
            ],
            next: vec![BodyGateId(1)],
            exit: BodyGateId(0),
            maximum_count: 1,
        };
        let gates = vec![
            Gate::Input(ChannelId(0)),
            operation(PlanOp::Less, &[0, 0]),
            Gate::Iterate(Box::new(iteration)),
            Gate::IterationState {
                iteration: GateId(2),
                slot: SlotId(0),
            },
            Gate::Constant(Constant::F64(1.0)),
            Gate::Constant(Constant::F64(0.0)),
            operation(PlanOp::Select, &[3, 4, 5]),
        ];
        let plan = Plan::new(Domain::F64, 1, gates, vec![GateId(6)], None).unwrap();

        let unrolled = plan.unrolled().unwrap();

        let operations: Vec<PlanOp> = unrolled.operations().collect();
        assert_eq!(
            operations,
            vec![
                PlanOp::Less,
                PlanOp::Not,
                PlanOp::Not,
                PlanOp::And,
                PlanOp::And,
                PlanOp::Or,
                PlanOp::Select
            ]
        );
    }
}
