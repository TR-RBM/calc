#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlanOp {
    Add,
    Sub,
    Mul,
    Div,
    Neg,
    Abs,
    Sqrt,
    MulAdd,
    Min,
    Max,
    Floor,
    Ceil,
    Trunc,
    RoundTiesEven,
    CopySign,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    Equal,
    NotEqual,
    And,
    Or,
    Not,
    Select,
    Exp,
    Ln,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Pow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OperationClass {
    CorrectlyRounded,
    Approximate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueKind {
    Scalar,
    Boolean,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Signature {
    pub arguments: &'static [ValueKind],
    pub result: ValueKind,
}

const SCALAR_UNARY: Signature = Signature {
    arguments: &[ValueKind::Scalar],
    result: ValueKind::Scalar,
};
const SCALAR_BINARY: Signature = Signature {
    arguments: &[ValueKind::Scalar, ValueKind::Scalar],
    result: ValueKind::Scalar,
};
const SCALAR_TERNARY: Signature = Signature {
    arguments: &[ValueKind::Scalar, ValueKind::Scalar, ValueKind::Scalar],
    result: ValueKind::Scalar,
};
const COMPARISON: Signature = Signature {
    arguments: &[ValueKind::Scalar, ValueKind::Scalar],
    result: ValueKind::Boolean,
};
const BOOLEAN_UNARY: Signature = Signature {
    arguments: &[ValueKind::Boolean],
    result: ValueKind::Boolean,
};
const BOOLEAN_BINARY: Signature = Signature {
    arguments: &[ValueKind::Boolean, ValueKind::Boolean],
    result: ValueKind::Boolean,
};
const SELECT: Signature = Signature {
    arguments: &[ValueKind::Boolean, ValueKind::Scalar, ValueKind::Scalar],
    result: ValueKind::Scalar,
};

impl PlanOp {
    pub const ALL: [PlanOp; 35] = [
        PlanOp::Add,
        PlanOp::Sub,
        PlanOp::Mul,
        PlanOp::Div,
        PlanOp::Neg,
        PlanOp::Abs,
        PlanOp::Sqrt,
        PlanOp::MulAdd,
        PlanOp::Min,
        PlanOp::Max,
        PlanOp::Floor,
        PlanOp::Ceil,
        PlanOp::Trunc,
        PlanOp::RoundTiesEven,
        PlanOp::CopySign,
        PlanOp::Less,
        PlanOp::LessOrEqual,
        PlanOp::Greater,
        PlanOp::GreaterOrEqual,
        PlanOp::Equal,
        PlanOp::NotEqual,
        PlanOp::And,
        PlanOp::Or,
        PlanOp::Not,
        PlanOp::Select,
        PlanOp::Exp,
        PlanOp::Ln,
        PlanOp::Sin,
        PlanOp::Cos,
        PlanOp::Tan,
        PlanOp::Asin,
        PlanOp::Acos,
        PlanOp::Atan,
        PlanOp::Atan2,
        PlanOp::Pow,
    ];

    pub fn class(self) -> OperationClass {
        match self {
            PlanOp::Exp
            | PlanOp::Ln
            | PlanOp::Sin
            | PlanOp::Cos
            | PlanOp::Tan
            | PlanOp::Asin
            | PlanOp::Acos
            | PlanOp::Atan
            | PlanOp::Atan2
            | PlanOp::Pow => OperationClass::Approximate,
            _ => OperationClass::CorrectlyRounded,
        }
    }

    pub fn signature(self) -> Signature {
        match self {
            PlanOp::Neg
            | PlanOp::Abs
            | PlanOp::Sqrt
            | PlanOp::Floor
            | PlanOp::Ceil
            | PlanOp::Trunc
            | PlanOp::RoundTiesEven
            | PlanOp::Exp
            | PlanOp::Ln
            | PlanOp::Sin
            | PlanOp::Cos
            | PlanOp::Tan
            | PlanOp::Asin
            | PlanOp::Acos
            | PlanOp::Atan => SCALAR_UNARY,
            PlanOp::Add
            | PlanOp::Sub
            | PlanOp::Mul
            | PlanOp::Div
            | PlanOp::Min
            | PlanOp::Max
            | PlanOp::CopySign
            | PlanOp::Atan2
            | PlanOp::Pow => SCALAR_BINARY,
            PlanOp::MulAdd => SCALAR_TERNARY,
            PlanOp::Less
            | PlanOp::LessOrEqual
            | PlanOp::Greater
            | PlanOp::GreaterOrEqual
            | PlanOp::Equal
            | PlanOp::NotEqual => COMPARISON,
            PlanOp::Not => BOOLEAN_UNARY,
            PlanOp::And | PlanOp::Or => BOOLEAN_BINARY,
            PlanOp::Select => SELECT,
        }
    }

    pub fn arity(self) -> usize {
        self.signature().arguments.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_lists_every_operation_once() {
        let operations = PlanOp::ALL;

        let duplicates = operations
            .iter()
            .enumerate()
            .filter(|(index, operation)| operations[..*index].contains(operation))
            .count();

        assert_eq!(duplicates, 0);
    }

    #[test]
    fn only_select_and_mul_add_take_three_arguments() {
        let operations = PlanOp::ALL;

        let ternary: Vec<PlanOp> = operations
            .into_iter()
            .filter(|operation| operation.arity() == 3)
            .collect();

        assert_eq!(ternary, vec![PlanOp::MulAdd, PlanOp::Select]);
    }

    #[test]
    fn comparison_produces_boolean_from_scalars() {
        let operation = PlanOp::LessOrEqual;

        let signature = operation.signature();

        assert_eq!(signature, COMPARISON);
    }

    #[test]
    fn select_takes_boolean_condition_and_two_scalars() {
        let operation = PlanOp::Select;

        let signature = operation.signature();

        assert_eq!(
            signature.arguments,
            &[ValueKind::Boolean, ValueKind::Scalar, ValueKind::Scalar]
        );
    }

    #[test]
    fn elementary_functions_are_approximate() {
        let operations = [
            PlanOp::Exp,
            PlanOp::Ln,
            PlanOp::Sin,
            PlanOp::Cos,
            PlanOp::Tan,
            PlanOp::Asin,
            PlanOp::Acos,
            PlanOp::Atan,
            PlanOp::Atan2,
            PlanOp::Pow,
        ];

        let all_approximate = operations
            .iter()
            .all(|operation| operation.class() == OperationClass::Approximate);

        assert!(all_approximate);
    }

    #[test]
    fn min_is_correctly_rounded() {
        let operation = PlanOp::Min;

        let class = operation.class();

        assert_eq!(class, OperationClass::CorrectlyRounded);
    }
}
