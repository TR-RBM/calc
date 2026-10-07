use calc_exec::PlanOp;

use crate::kernel::{BinaryOperation, Comparison, UnaryOperation};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OperationModes {
    native: u32,
}

pub const EXACT_IN_BOTH_FORMS: [PlanOp; 7] = [
    PlanOp::Neg,
    PlanOp::Abs,
    PlanOp::CopySign,
    PlanOp::And,
    PlanOp::Or,
    PlanOp::Not,
    PlanOp::Select,
];

pub const ELIGIBLE_FOR_NATIVE: [PlanOp; 12] = [
    PlanOp::Floor,
    PlanOp::Ceil,
    PlanOp::Trunc,
    PlanOp::RoundTiesEven,
    PlanOp::Min,
    PlanOp::Max,
    PlanOp::Less,
    PlanOp::LessOrEqual,
    PlanOp::Greater,
    PlanOp::GreaterOrEqual,
    PlanOp::Equal,
    PlanOp::NotEqual,
];

pub(crate) fn plan_unary(operation: UnaryOperation) -> PlanOp {
    match operation {
        UnaryOperation::Neg => PlanOp::Neg,
        UnaryOperation::Abs => PlanOp::Abs,
        UnaryOperation::Sqrt => PlanOp::Sqrt,
        UnaryOperation::Floor => PlanOp::Floor,
        UnaryOperation::Ceil => PlanOp::Ceil,
        UnaryOperation::Trunc => PlanOp::Trunc,
        UnaryOperation::RoundTiesEven => PlanOp::RoundTiesEven,
    }
}

pub(crate) fn plan_binary(operation: BinaryOperation) -> PlanOp {
    match operation {
        BinaryOperation::Add => PlanOp::Add,
        BinaryOperation::Sub => PlanOp::Sub,
        BinaryOperation::Mul => PlanOp::Mul,
        BinaryOperation::Div => PlanOp::Div,
        BinaryOperation::Min => PlanOp::Min,
        BinaryOperation::Max => PlanOp::Max,
        BinaryOperation::CopySign => PlanOp::CopySign,
    }
}

pub(crate) fn plan_comparison(comparison: Comparison) -> PlanOp {
    match comparison {
        Comparison::Less => PlanOp::Less,
        Comparison::LessOrEqual => PlanOp::LessOrEqual,
        Comparison::Greater => PlanOp::Greater,
        Comparison::GreaterOrEqual => PlanOp::GreaterOrEqual,
        Comparison::Equal => PlanOp::Equal,
        Comparison::NotEqual => PlanOp::NotEqual,
    }
}

fn slot(operation: PlanOp) -> Option<u32> {
    ELIGIBLE_FOR_NATIVE
        .iter()
        .position(|eligible| *eligible == operation)
        .and_then(|position| u32::try_from(position).ok())
}

impl OperationModes {
    pub const EXACT: OperationModes = OperationModes { native: 0 };

    pub fn eligible(operation: PlanOp) -> bool {
        slot(operation).is_some()
    }

    pub fn is_native(self, operation: PlanOp) -> bool {
        slot(operation).is_some_and(|bit| self.native & (1 << bit) != 0)
    }

    pub fn with_native(self, operation: PlanOp) -> OperationModes {
        match slot(operation) {
            Some(bit) => OperationModes {
                native: self.native | (1 << bit),
            },
            None => self,
        }
    }

    pub fn native_operations(self) -> Vec<PlanOp> {
        ELIGIBLE_FOR_NATIVE
            .into_iter()
            .filter(|operation| self.is_native(*operation))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_native_by_default() {
        let modes = OperationModes::EXACT;

        assert_eq!(modes.native_operations(), Vec::new());
    }

    #[test]
    fn an_eligible_operation_can_be_marked_native() {
        let modes = OperationModes::EXACT.with_native(PlanOp::Floor);

        assert!(modes.is_native(PlanOp::Floor) && !modes.is_native(PlanOp::Ceil));
    }

    #[test]
    fn arithmetic_is_never_native() {
        let modes = OperationModes::EXACT
            .with_native(PlanOp::Add)
            .with_native(PlanOp::Mul)
            .with_native(PlanOp::Div)
            .with_native(PlanOp::Sqrt);

        assert_eq!(modes, OperationModes::EXACT);
    }

    #[test]
    fn a_bit_operation_is_not_eligible_because_it_never_reaches_the_driver() {
        let found = [PlanOp::Neg, PlanOp::Abs, PlanOp::CopySign]
            .into_iter()
            .any(OperationModes::eligible);

        assert!(!found);
    }
}
