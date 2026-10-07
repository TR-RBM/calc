use calc_units::UnitId;

use crate::ids::{ExprId, NumberId, SymbolId};
use crate::operator::Operator;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Head {
    Operator(Operator),
    Function(SymbolId),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ReductionShape {
    #[default]
    LeftFold,
    Halving,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LimitSide {
    #[default]
    Both,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SortBubbleForm {
    Full,
    Shrinking,
    EarlyExit,
    LastExchange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SortShakerForm {
    Full,
    Shrinking,
    LastExchange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SortOddEvenForm {
    UntilSorted,
    FixedPasses,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SortCombForm {
    LaceyBox,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SortGaps {
    Shell,
    Knuth,
    Ciura,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SortPartition {
    LomutoLast,
    HoareFirst,
    LomutoRandom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SortMethod {
    Insertion,
    BinaryInsertion,
    Selection,
    Bubble(SortBubbleForm),
    Merge,
    Heap,
    Quick(SortPartition),
    Counting,
    DoubleSelection,
    CocktailShaker(SortShakerForm),
    Gnome,
    OddEven(SortOddEvenForm),
    Comb(SortCombForm),
    Cycle,
    Pancake,
    Shell(SortGaps),
    BottomUpMerge,
    NaturalMerge,
    Radix,
    Bead,
    Bitonic,
    Bogo,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SortOrder {
    #[default]
    Increasing,
    Decreasing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SortSpec {
    pub method: SortMethod,
    pub order: SortOrder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinderKind {
    Lambda,
    Sum(ReductionShape),
    Product(ReductionShape),
    Integral,
    Limit(LimitSide),
    Derivative,
    Root,
    Taylor,
    Sort(SortSpec),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Node {
    Number(NumberId),
    Symbol(SymbolId),
    Bound(u32),
    Apply {
        head: Head,
        arguments: Box<[ExprId]>,
    },
    Bind {
        binder: BinderKind,
        arguments: Box<[ExprId]>,
        body: ExprId,
    },
    Quantity {
        value: ExprId,
        unit: UnitId,
    },
    Array {
        shape: Box<[u32]>,
        elements: Box<[ExprId]>,
    },
}

impl Node {
    pub(crate) fn children(&self) -> Vec<ExprId> {
        match self {
            Node::Number(_) | Node::Symbol(_) | Node::Bound(_) => Vec::new(),
            Node::Apply { arguments, .. } => arguments.to_vec(),
            Node::Bind {
                arguments, body, ..
            } => arguments.iter().copied().chain([*body]).collect(),
            Node::Quantity { value, .. } => vec![*value],
            Node::Array { elements, .. } => elements.to_vec(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeView<'pool> {
    Number(NumberId),
    Symbol(SymbolId),
    Bound(u32),
    Apply {
        head: Head,
        arguments: &'pool [ExprId],
    },
    Bind {
        binder: BinderKind,
        arguments: &'pool [ExprId],
        body: ExprId,
    },
    Quantity {
        value: ExprId,
        unit: UnitId,
    },
    Array {
        shape: &'pool [u32],
        elements: &'pool [ExprId],
    },
}

impl<'pool> From<&'pool Node> for NodeView<'pool> {
    fn from(node: &'pool Node) -> Self {
        match node {
            Node::Number(number) => NodeView::Number(*number),
            Node::Symbol(symbol) => NodeView::Symbol(*symbol),
            Node::Bound(index) => NodeView::Bound(*index),
            Node::Apply { head, arguments } => NodeView::Apply {
                head: *head,
                arguments,
            },
            Node::Bind {
                binder,
                arguments,
                body,
            } => NodeView::Bind {
                binder: *binder,
                arguments,
                body: *body,
            },
            Node::Quantity { value, unit } => NodeView::Quantity {
                value: *value,
                unit: *unit,
            },
            Node::Array { shape, elements } => NodeView::Array { shape, elements },
        }
    }
}
