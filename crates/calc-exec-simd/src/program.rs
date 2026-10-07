use calc_exec::{
    Batch, BatchRole, BatchShapeError, BodyGate, Gate, Iteration, OperationClass, Plan,
    PlanBatchError, PlanOp, PrepareError, Reduce, ReduceOperation, ReduceShape, RunError,
    Unsupported, ValueKind,
};

use crate::lanes::{Element, Lanes};

#[derive(Clone, Copy, Debug)]
pub(crate) enum UnaryOperation {
    Neg,
    Abs,
    Sqrt,
    Floor,
    Ceil,
    Trunc,
    RoundTiesEven,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum BinaryOperation {
    Add,
    Sub,
    Mul,
    Div,
    Min,
    Max,
    CopySign,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Comparison {
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    Equal,
    NotEqual,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Step<E> {
    Input(usize),
    Constant(E),
    Unary(UnaryOperation, usize),
    Binary(BinaryOperation, usize, usize),
    MulAdd(usize, usize, usize),
    Compare(Comparison, usize, usize),
    And(usize, usize),
    Or(usize, usize),
    Not(usize),
    Select(usize, usize, usize),
    UnaryReference(fn(E) -> E, usize),
    BinaryReference(fn(E, E) -> E, usize, usize),
    State {
        slot: usize,
        boolean: bool,
    },
    Outer {
        gate: usize,
        boolean: bool,
    },
    Iterate(usize),
    IterationState {
        iteration: usize,
        slot: usize,
        boolean: bool,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct IterationProgram<E> {
    initial: Vec<(usize, bool)>,
    body: Vec<Step<E>>,
    next: Vec<(usize, bool)>,
    exit: usize,
    maximum_count: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct Program<E> {
    steps: Vec<Step<E>>,
    iterations: Vec<IterationProgram<E>>,
    outputs: Vec<usize>,
    reduce: Option<Reduce>,
    input_channel_count: usize,
}

fn unsupported_operation(operation: PlanOp) -> PrepareError {
    PrepareError::Unsupported(Unsupported::Operation(operation))
}

fn is_boolean(kind: Option<ValueKind>) -> bool {
    kind == Some(ValueKind::Boolean)
}

fn constant_step<E: Element>(constant: calc_exec::Constant) -> Result<Step<E>, PrepareError> {
    E::from_constant(constant)
        .map(Step::Constant)
        .ok_or(PrepareError::Unsupported(Unsupported::Domain(
            constant.domain(),
        )))
}

fn step_for<E: Element>(operation: PlanOp, arguments: &[usize]) -> Result<Step<E>, PrepareError> {
    if operation.class() == OperationClass::Approximate {
        return match *arguments {
            [value] => {
                E::unary_reference(operation).map(|function| Step::UnaryReference(function, value))
            }
            [left, right] => E::binary_reference(operation)
                .map(|function| Step::BinaryReference(function, left, right)),
            _ => None,
        }
        .ok_or_else(|| unsupported_operation(operation));
    }
    let step = match (operation, arguments) {
        (PlanOp::Neg, &[value]) => Step::Unary(UnaryOperation::Neg, value),
        (PlanOp::Abs, &[value]) => Step::Unary(UnaryOperation::Abs, value),
        (PlanOp::Sqrt, &[value]) => Step::Unary(UnaryOperation::Sqrt, value),
        (PlanOp::Floor, &[value]) => Step::Unary(UnaryOperation::Floor, value),
        (PlanOp::Ceil, &[value]) => Step::Unary(UnaryOperation::Ceil, value),
        (PlanOp::Trunc, &[value]) => Step::Unary(UnaryOperation::Trunc, value),
        (PlanOp::RoundTiesEven, &[value]) => Step::Unary(UnaryOperation::RoundTiesEven, value),
        (PlanOp::Add, &[left, right]) => Step::Binary(BinaryOperation::Add, left, right),
        (PlanOp::Sub, &[left, right]) => Step::Binary(BinaryOperation::Sub, left, right),
        (PlanOp::Mul, &[left, right]) => Step::Binary(BinaryOperation::Mul, left, right),
        (PlanOp::Div, &[left, right]) => Step::Binary(BinaryOperation::Div, left, right),
        (PlanOp::Min, &[left, right]) => Step::Binary(BinaryOperation::Min, left, right),
        (PlanOp::Max, &[left, right]) => Step::Binary(BinaryOperation::Max, left, right),
        (PlanOp::CopySign, &[left, right]) => Step::Binary(BinaryOperation::CopySign, left, right),
        (PlanOp::MulAdd, &[left, right, addend]) => Step::MulAdd(left, right, addend),
        (PlanOp::Less, &[left, right]) => Step::Compare(Comparison::Less, left, right),
        (PlanOp::LessOrEqual, &[left, right]) => {
            Step::Compare(Comparison::LessOrEqual, left, right)
        }
        (PlanOp::Greater, &[left, right]) => Step::Compare(Comparison::Greater, left, right),
        (PlanOp::GreaterOrEqual, &[left, right]) => {
            Step::Compare(Comparison::GreaterOrEqual, left, right)
        }
        (PlanOp::Equal, &[left, right]) => Step::Compare(Comparison::Equal, left, right),
        (PlanOp::NotEqual, &[left, right]) => Step::Compare(Comparison::NotEqual, left, right),
        (PlanOp::And, &[left, right]) => Step::And(left, right),
        (PlanOp::Or, &[left, right]) => Step::Or(left, right),
        (PlanOp::Not, &[value]) => Step::Not(value),
        (PlanOp::Select, &[condition, when_true, when_false]) => {
            Step::Select(condition, when_true, when_false)
        }
        _ => return Err(unsupported_operation(operation)),
    };
    Ok(step)
}

fn compile_iteration<E: Element>(
    plan: &Plan,
    iteration: &Iteration,
) -> Result<IterationProgram<E>, PrepareError> {
    let slot_is_boolean: Vec<bool> = iteration
        .initial
        .iter()
        .map(|initial| is_boolean(plan.value_kind(*initial)))
        .collect();
    let mut body = Vec::with_capacity(iteration.body.len());
    for body_gate in &iteration.body {
        let step = match body_gate {
            BodyGate::State(slot) => Step::State {
                slot: slot.index(),
                boolean: slot_is_boolean.get(slot.index()).copied().unwrap_or(false),
            },
            BodyGate::Outer(gate) => Step::Outer {
                gate: gate.index(),
                boolean: is_boolean(plan.value_kind(*gate)),
            },
            BodyGate::Constant(constant) => constant_step(*constant)?,
            BodyGate::Operation {
                operation,
                arguments,
            } => {
                let indices: Vec<usize> =
                    arguments.iter().map(|argument| argument.index()).collect();
                step_for(*operation, &indices)?
            }
        };
        body.push(step);
    }
    Ok(IterationProgram {
        initial: iteration
            .initial
            .iter()
            .zip(&slot_is_boolean)
            .map(|(initial, boolean)| (initial.index(), *boolean))
            .collect(),
        body,
        next: iteration
            .next
            .iter()
            .zip(&slot_is_boolean)
            .map(|(next, boolean)| (next.index(), *boolean))
            .collect(),
        exit: iteration.exit.index(),
        maximum_count: iteration.maximum_count,
    })
}

pub(crate) fn compile<E: Element>(plan: &Plan) -> Result<Program<E>, PrepareError> {
    let mut steps = Vec::with_capacity(plan.gates().len());
    let mut iterations = Vec::new();
    for gate in plan.gates() {
        let step = match gate {
            Gate::Input(channel) => Step::Input(channel.index()),
            Gate::Constant(constant) => constant_step(*constant)?,
            Gate::Operation {
                operation,
                arguments,
            } => {
                let indices: Vec<usize> =
                    arguments.iter().map(|argument| argument.index()).collect();
                step_for(*operation, &indices)?
            }
            Gate::Iterate(iteration) => {
                iterations.push(compile_iteration(plan, iteration)?);
                Step::Iterate(iterations.len() - 1)
            }
            Gate::IterationState { iteration, slot } => {
                let number = state_iteration_index(plan, *iteration);
                let boolean = iterations
                    .get(number)
                    .and_then(|program: &IterationProgram<E>| {
                        program
                            .initial
                            .get(slot.index())
                            .map(|(_, boolean)| *boolean)
                    })
                    .unwrap_or(false);
                Step::IterationState {
                    iteration: number,
                    slot: slot.index(),
                    boolean,
                }
            }
        };
        steps.push(step);
    }
    Ok(Program {
        steps,
        iterations,
        outputs: plan.outputs().iter().map(|gate| gate.index()).collect(),
        reduce: plan.reduce(),
        input_channel_count: plan.input_channel_count(),
    })
}

fn state_iteration_index(plan: &Plan, iteration: calc_exec::GateId) -> usize {
    plan.gates()
        .iter()
        .take(iteration.index())
        .filter(|gate| matches!(gate, Gate::Iterate(_)))
        .count()
}

struct Lane<L: Lanes> {
    values: Vec<L>,
    masks: Vec<L::Mask>,
}

impl<L: Lanes> Lane<L> {
    fn new(length: usize) -> Lane<L> {
        Lane {
            values: vec![L::splat(L::Element::ZERO); length],
            masks: vec![L::mask_empty(); length],
        }
    }
}

fn unary<L: Lanes>(operation: UnaryOperation, value: L) -> L {
    match operation {
        UnaryOperation::Neg => value.neg(),
        UnaryOperation::Abs => value.abs(),
        UnaryOperation::Sqrt => value.sqrt(),
        UnaryOperation::Floor => value.by_lane(L::Element::floor),
        UnaryOperation::Ceil => value.by_lane(L::Element::ceil),
        UnaryOperation::Trunc => value.by_lane(L::Element::trunc),
        UnaryOperation::RoundTiesEven => value.by_lane(L::Element::round_ties_even),
    }
}

fn binary<L: Lanes>(operation: BinaryOperation, left: L, right: L) -> L {
    match operation {
        BinaryOperation::Add => left.add(right),
        BinaryOperation::Sub => left.sub(right),
        BinaryOperation::Mul => left.mul(right),
        BinaryOperation::Div => left.div(right),
        BinaryOperation::Min => left.minimum(right),
        BinaryOperation::Max => left.maximum(right),
        BinaryOperation::CopySign => left.copysign(right),
    }
}

fn compare<L: Lanes>(comparison: Comparison, left: L, right: L) -> L::Mask {
    match comparison {
        Comparison::Less => left.less(right),
        Comparison::LessOrEqual => left.less_or_equal(right),
        Comparison::Greater => right.less(left),
        Comparison::GreaterOrEqual => right.less_or_equal(left),
        Comparison::Equal => left.equal(right),
        Comparison::NotEqual => left.not_equal(right),
    }
}

fn operate<L: Lanes>(step: &Step<L::Element>, index: usize, lane: &mut Lane<L>) -> bool {
    match *step {
        Step::Constant(value) => lane.values[index] = L::splat(value),
        Step::Unary(operation, value) => {
            lane.values[index] = unary::<L>(operation, lane.values[value]);
        }
        Step::Binary(operation, left, right) => {
            lane.values[index] = binary::<L>(operation, lane.values[left], lane.values[right]);
        }
        Step::MulAdd(left, right, addend) => {
            let mut factors = [L::Element::ZERO; 8];
            let mut multipliers = [L::Element::ZERO; 8];
            let mut addends = [L::Element::ZERO; 8];
            let width = L::WIDTH;
            lane.values[left].store(&mut factors[..width]);
            lane.values[right].store(&mut multipliers[..width]);
            lane.values[addend].store(&mut addends[..width]);
            for position in 0..width {
                factors[position] =
                    factors[position].fused_multiply_add(multipliers[position], addends[position]);
            }
            lane.values[index] = L::load(&factors[..width]);
        }
        Step::Compare(comparison, left, right) => {
            lane.masks[index] = compare::<L>(comparison, lane.values[left], lane.values[right]);
        }
        Step::And(left, right) => {
            lane.masks[index] = L::mask_and(lane.masks[left], lane.masks[right]);
        }
        Step::Or(left, right) => {
            lane.masks[index] = L::mask_or(lane.masks[left], lane.masks[right]);
        }
        Step::Not(value) => lane.masks[index] = L::mask_not(lane.masks[value]),
        Step::UnaryReference(function, value) => {
            lane.values[index] = lane.values[value].by_lane(function);
        }
        Step::BinaryReference(function, left, right) => {
            lane.values[index] = lane.values[left].by_lane_pair(lane.values[right], function);
        }
        Step::Select(condition, when_true, when_false) => {
            lane.values[index] = L::select(
                lane.masks[condition],
                lane.values[when_true],
                lane.values[when_false],
            );
        }
        Step::Input(_)
        | Step::State { .. }
        | Step::Outer { .. }
        | Step::Iterate(_)
        | Step::IterationState { .. } => return false,
    }
    true
}

fn run_iteration<L: Lanes>(program: &IterationProgram<L::Element>, outer: &Lane<L>) -> Lane<L> {
    let mut state = Lane::<L>::new(program.initial.len());
    for (slot, (gate, boolean)) in program.initial.iter().enumerate() {
        if *boolean {
            state.masks[slot] = outer.masks[*gate];
        } else {
            state.values[slot] = outer.values[*gate];
        }
    }
    let mut body = Lane::<L>::new(program.body.len());
    let mut active = L::mask_not(L::mask_empty());
    for _ in 0..program.maximum_count {
        if L::mask_none(active) {
            break;
        }
        for (index, step) in program.body.iter().enumerate() {
            if operate::<L>(step, index, &mut body) {
                continue;
            }
            match *step {
                Step::State { slot, boolean } => {
                    if boolean {
                        body.masks[index] = state.masks[slot];
                    } else {
                        body.values[index] = state.values[slot];
                    }
                }
                Step::Outer { gate, boolean } => {
                    if boolean {
                        body.masks[index] = outer.masks[gate];
                    } else {
                        body.values[index] = outer.values[gate];
                    }
                }
                _ => {}
            }
        }
        let leaving = body.masks[program.exit];
        let still = L::mask_and(active, L::mask_not(leaving));
        for (slot, (gate, boolean)) in program.next.iter().enumerate() {
            if *boolean {
                let taken = L::mask_and(still, body.masks[*gate]);
                let kept = L::mask_and(L::mask_not(still), state.masks[slot]);
                state.masks[slot] = L::mask_or(taken, kept);
            } else {
                state.values[slot] = L::select(still, body.values[*gate], state.values[slot]);
            }
        }
        active = still;
    }
    state
}

fn run_block<L: Lanes>(
    program: &Program<L::Element>,
    inputs: &[Vec<L::Element>],
    position: usize,
    outputs: &mut [Vec<L::Element>],
) {
    let mut lane = Lane::<L>::new(program.steps.len());
    let mut states: Vec<Lane<L>> = Vec::with_capacity(program.iterations.len());
    for (index, step) in program.steps.iter().enumerate() {
        if operate::<L>(step, index, &mut lane) {
            continue;
        }
        match *step {
            Step::Input(channel) => {
                lane.values[index] = L::load(&inputs[channel][position..]);
            }
            Step::Iterate(iteration) => {
                let state = run_iteration::<L>(&program.iterations[iteration], &lane);
                states.push(state);
            }
            Step::IterationState {
                iteration,
                slot,
                boolean,
            } => {
                if let Some(state) = states.get(iteration) {
                    if boolean {
                        lane.masks[index] = state.masks[slot];
                    } else {
                        lane.values[index] = state.values[slot];
                    }
                }
            }
            _ => {}
        }
    }
    for (channel, gate) in program.outputs.iter().enumerate() {
        lane.values[*gate].store(&mut outputs[channel][position..]);
    }
}

fn shape_error(role: BatchRole, expected: usize, actual: usize) -> RunError {
    RunError::BatchShape(PlanBatchError {
        role,
        shape: BatchShapeError::ChannelCountMismatch { expected, actual },
    })
}

fn channel_error(role: BatchRole, expected: usize, batch: &Batch) -> RunError {
    shape_error(role, expected, batch.channel_count())
}

fn combine<E: Element>(operation: ReduceOperation, left: E, right: E) -> E {
    E::combine(operation, left, right)
}

fn halving<E: Element>(values: &[E], operation: ReduceOperation) -> Option<E> {
    match values {
        [] => None,
        [single] => Some(*single),
        _ => {
            let (left, right) = values.split_at(values.len() / 2);
            Some(combine(
                operation,
                halving(left, operation)?,
                halving(right, operation)?,
            ))
        }
    }
}

fn reduce_column<E: Element>(values: &[E], reduce: Reduce) -> Option<E> {
    match reduce.shape {
        ReduceShape::LeftFold => {
            let (first, rest) = values.split_first()?;
            Some(
                rest.iter()
                    .fold(*first, |sum, value| combine(reduce.operation, sum, *value)),
            )
        }
        ReduceShape::Halving => halving(values, reduce.operation),
    }
}

impl<E: Element> Program<E> {
    pub(crate) fn run<L: Lanes<Element = E>>(
        &self,
        inputs: &Batch,
        outputs: &mut Batch,
    ) -> Result<(), RunError> {
        let length = inputs.len();
        let mut columns: Vec<Vec<E>> = Vec::with_capacity(self.input_channel_count);
        for channel in 0..self.input_channel_count {
            let values = E::channel(inputs, channel).ok_or_else(|| {
                channel_error(BatchRole::Inputs, self.input_channel_count, inputs)
            })?;
            let mut padded = values.to_vec();
            padded.resize(length + L::WIDTH, E::ZERO);
            columns.push(padded);
        }
        let mut produced: Vec<Vec<E>> = (0..self.outputs.len())
            .map(|_| vec![E::ZERO; length + L::WIDTH])
            .collect();
        let mut position = 0;
        while position < length {
            run_block::<L>(self, &columns, position, &mut produced);
            position += L::WIDTH;
        }
        match self.reduce {
            None => {
                for (channel, values) in produced.iter().enumerate() {
                    let wanted = self.outputs.len();
                    let found = outputs.channel_count();
                    let target = E::channel_mut(outputs, channel)
                        .ok_or_else(|| shape_error(BatchRole::Outputs, wanted, found))?;
                    target.copy_from_slice(&values[..length]);
                }
            }
            Some(reduce) => {
                for (channel, values) in produced.iter().enumerate() {
                    let found =
                        reduce_column(&values[..length], reduce).ok_or(RunError::EmptyReduction)?;
                    let wanted = self.outputs.len();
                    let channel_count = outputs.channel_count();
                    let target = E::channel_mut(outputs, channel)
                        .ok_or_else(|| shape_error(BatchRole::Outputs, wanted, channel_count))?;
                    target[0] = found;
                }
            }
        }
        Ok(())
    }
}
