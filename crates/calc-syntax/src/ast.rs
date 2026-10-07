use std::ops::Range;

use calc_expr::{IntegerTypeSpelling, LimitSide, Operator, ReductionShape, SortSpec};

use crate::lexer::TypedLiteralKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnitFactorSyntax {
    pub(crate) name: String,
    pub(crate) exponent: i64,
    pub(crate) is_divisor: bool,
    pub(crate) span: Range<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BinderSyntax {
    Integral,
    Sum(ReductionShape),
    Product(ReductionShape),
    Limit(LimitSide),
    Derivative,
    Root,
    Taylor,
    Sort(SortSpec),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AstKind {
    Number {
        text: String,
        is_negative: bool,
    },
    Typed {
        kind: TypedLiteralKind,
        content: String,
    },
    Name(String),
    Group(Box<Ast>),
    Operation {
        operator: Operator,
        operands: Vec<Ast>,
    },
    Quantity {
        value: Box<Ast>,
        unit: Vec<UnitFactorSyntax>,
    },
    Conversion {
        value: Box<Ast>,
        unit: Vec<UnitFactorSyntax>,
    },
    RadixConversion {
        value: Box<Ast>,
        target: RadixTarget,
    },
    Call {
        name: String,
        arguments: Vec<Ast>,
    },
    Lambda {
        parameter: String,
        body: Box<Ast>,
    },
    Array {
        rows: Vec<Vec<Ast>>,
    },
    Binder {
        binder: BinderSyntax,
        variable: String,
        arguments: Vec<Ast>,
        point_is_variable: bool,
        body: Box<Ast>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RadixTarget {
    Radix {
        base: u32,
        bits: u32,
        spelling: IntegerTypeSpelling,
    },
    Bytes {
        big_endian: bool,
        bits: u32,
        spelling: IntegerTypeSpelling,
    },
    Type {
        bits: u32,
        signed: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Ast {
    pub(crate) kind: AstKind,
    pub(crate) span: Range<usize>,
    pub(crate) depth: usize,
}

fn deepest(branches: impl IntoIterator<Item = usize>) -> usize {
    branches.into_iter().max().unwrap_or(0) + 1
}

fn depth_of(kind: &AstKind) -> usize {
    match kind {
        AstKind::Number { .. } | AstKind::Typed { .. } | AstKind::Name(_) => 1,
        AstKind::Group(inner) | AstKind::Lambda { body: inner, .. } => deepest([inner.depth]),
        AstKind::Quantity { value, .. }
        | AstKind::Conversion { value, .. }
        | AstKind::RadixConversion { value, .. } => deepest([value.depth]),
        AstKind::Operation { operands, .. }
        | AstKind::Call {
            arguments: operands,
            ..
        } => deepest(operands.iter().map(|operand| operand.depth)),
        AstKind::Array { rows } => deepest(rows.iter().flatten().map(|element| element.depth)),
        AstKind::Binder {
            arguments, body, ..
        } => deepest(
            arguments
                .iter()
                .map(|argument| argument.depth)
                .chain([body.depth]),
        ),
    }
}

impl Ast {
    pub(crate) fn new(kind: AstKind, span: Range<usize>) -> Self {
        let depth = depth_of(&kind);
        Self { kind, span, depth }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StatementSyntax {
    Naming {
        name: String,
        name_span: Range<usize>,
        value: Ast,
    },
    FunctionNaming {
        name: String,
        name_span: Range<usize>,
        parameters: Vec<String>,
        value: Ast,
    },
    Expression(Ast),
}
