use crate::domain::{Constant, Domain};
use crate::operation::{PlanOp, ValueKind};
use crate::plan::{GateId, PlanError, index_of};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SlotId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BodyGateId(pub u32);

impl SlotId {
    pub fn index(self) -> usize {
        index_of(self.0)
    }
}

impl BodyGateId {
    pub fn index(self) -> usize {
        index_of(self.0)
    }
}

#[derive(Clone, Debug)]
pub enum BodyGate {
    State(SlotId),
    Outer(GateId),
    Constant(Constant),
    Operation {
        operation: PlanOp,
        arguments: Vec<BodyGateId>,
    },
}

#[derive(Clone, Debug)]
pub struct Iteration {
    pub initial: Vec<GateId>,
    pub body: Vec<BodyGate>,
    pub next: Vec<BodyGateId>,
    pub exit: BodyGateId,
    pub maximum_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyError {
    StateOutOfRange(SlotId),
    OuterNotEarlier(GateId),
    OuterNotValue(GateId),
    ConstantDomainMismatch(Domain),
    ArityMismatch {
        operation: PlanOp,
        expected: usize,
        actual: usize,
    },
    ArgumentNotEarlier(BodyGateId),
    ArgumentKindMismatch {
        operation: PlanOp,
        position: usize,
        expected: ValueKind,
        actual: ValueKind,
    },
}

fn body_error(gate_index: usize, body_gate: usize, error: BodyError) -> PlanError {
    PlanError::Body {
        gate_index,
        body_gate,
        error,
    }
}

fn body_kinds(
    domain: Domain,
    gate_index: usize,
    iteration: &Iteration,
    slot_kinds: &[ValueKind],
    outer_kinds: &[Option<ValueKind>],
) -> Result<Vec<ValueKind>, PlanError> {
    let mut kinds: Vec<ValueKind> = Vec::with_capacity(iteration.body.len());
    for (body_gate, gate) in iteration.body.iter().enumerate() {
        let failure = |error| body_error(gate_index, body_gate, error);
        let kind = match gate {
            BodyGate::State(slot) => *slot_kinds
                .get(slot.index())
                .ok_or(failure(BodyError::StateOutOfRange(*slot)))?,
            BodyGate::Outer(outer) => match outer_kinds.get(outer.index()) {
                None => return Err(failure(BodyError::OuterNotEarlier(*outer))),
                Some(None) => return Err(failure(BodyError::OuterNotValue(*outer))),
                Some(Some(kind)) => *kind,
            },
            BodyGate::Constant(constant) => {
                if constant.domain() != domain {
                    return Err(failure(BodyError::ConstantDomainMismatch(
                        constant.domain(),
                    )));
                }
                ValueKind::Scalar
            }
            BodyGate::Operation {
                operation,
                arguments,
            } => {
                let signature = operation.signature();
                if arguments.len() != signature.arguments.len() {
                    return Err(failure(BodyError::ArityMismatch {
                        operation: *operation,
                        expected: signature.arguments.len(),
                        actual: arguments.len(),
                    }));
                }
                for (position, (argument, expected)) in
                    arguments.iter().zip(signature.arguments).enumerate()
                {
                    let Some(actual) = kinds.get(argument.index()) else {
                        return Err(failure(BodyError::ArgumentNotEarlier(*argument)));
                    };
                    if actual != expected {
                        return Err(failure(BodyError::ArgumentKindMismatch {
                            operation: *operation,
                            position,
                            expected: *expected,
                            actual: *actual,
                        }));
                    }
                }
                signature.result
            }
        };
        kinds.push(kind);
    }
    Ok(kinds)
}

pub(crate) fn slot_kinds(
    domain: Domain,
    gate_index: usize,
    iteration: &Iteration,
    outer_kinds: &[Option<ValueKind>],
) -> Result<Vec<ValueKind>, PlanError> {
    let mut slots = Vec::with_capacity(iteration.initial.len());
    for initial in &iteration.initial {
        match outer_kinds.get(initial.index()) {
            None => {
                return Err(PlanError::IterationInitialNotEarlier {
                    gate_index,
                    initial: *initial,
                });
            }
            Some(None) => {
                return Err(PlanError::ValueExpected {
                    gate_index,
                    argument: *initial,
                });
            }
            Some(Some(kind)) => slots.push(*kind),
        }
    }
    if iteration.next.len() != slots.len() {
        return Err(PlanError::IterationSlotCountMismatch {
            gate_index,
            initial: slots.len(),
            next: iteration.next.len(),
        });
    }
    let kinds = body_kinds(domain, gate_index, iteration, &slots, outer_kinds)?;
    for (slot, (next, initial)) in iteration.next.iter().zip(&slots).enumerate() {
        let Some(next_kind) = kinds.get(next.index()) else {
            return Err(PlanError::IterationResultOutOfRange {
                gate_index,
                body_gate: *next,
            });
        };
        if next_kind != initial {
            return Err(PlanError::IterationSlotKindMismatch {
                gate_index,
                slot,
                initial: *initial,
                next: *next_kind,
            });
        }
    }
    match kinds.get(iteration.exit.index()) {
        None => Err(PlanError::IterationResultOutOfRange {
            gate_index,
            body_gate: iteration.exit,
        }),
        Some(ValueKind::Scalar) => Err(PlanError::IterationExitNotBoolean { gate_index }),
        Some(ValueKind::Boolean) => Ok(slots),
    }
}
