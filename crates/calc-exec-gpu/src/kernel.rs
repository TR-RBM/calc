use calc_exec::ReduceOperation;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BufferId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalId(pub u32);

impl BufferId {
    pub fn index(self) -> usize {
        usize::try_from(self.0).unwrap_or(usize::MAX)
    }
}

impl LocalId {
    pub fn index(self) -> usize {
        usize::try_from(self.0).unwrap_or(usize::MAX)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BufferKind {
    Values,
    Indices,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnaryOperation {
    Neg,
    Abs,
    Sqrt,
    Floor,
    Ceil,
    Trunc,
    RoundTiesEven,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryOperation {
    Add,
    Sub,
    Mul,
    Div,
    Min,
    Max,
    CopySign,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Comparison {
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    Equal,
    NotEqual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Expression {
    Input {
        channel: u32,
    },
    Constant {
        index: u32,
    },
    Unary(UnaryOperation, LocalId),
    Binary(BinaryOperation, LocalId, LocalId),
    Compare(Comparison, LocalId, LocalId),
    And(LocalId, LocalId),
    Or(LocalId, LocalId),
    Not(LocalId),
    Iterate {
        iteration: u32,
    },
    IterationState {
        iteration: LocalId,
        slot: u32,
    },
    State {
        slot: u32,
    },
    Outer {
        local: LocalId,
    },
    Select {
        condition: LocalId,
        when_true: LocalId,
        when_false: LocalId,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapPass {
    pub inputs: BufferId,
    pub constants: BufferId,
    pub outputs: BufferId,
    pub input_channel_count: u32,
    pub statements: Vec<Expression>,
    pub results: Vec<LocalId>,
    pub iterations: Vec<KernelIteration>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KernelIteration {
    pub initial: Vec<LocalId>,
    pub body: Vec<Expression>,
    pub next: Vec<LocalId>,
    pub exit: LocalId,
    pub maximum_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReducePass {
    pub operation: ReduceOperation,
    pub source: BufferId,
    pub target: BufferId,
    pub triples: BufferId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Kernel {
    pub buffers: Vec<BufferKind>,
    pub constants: Vec<f32>,
    pub map: MapPass,
    pub reduce: Option<ReducePass>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PassId {
    Map,
    Reduce,
}

pub const CARRY: u32 = u32::MAX;

pub const INPUTS: BufferId = BufferId(0);
pub const CONSTANTS: BufferId = BufferId(1);
pub const OUTPUTS: BufferId = BufferId(2);
pub const REDUCE_TARGET: BufferId = BufferId(3);
pub const TRIPLES: BufferId = BufferId(4);
