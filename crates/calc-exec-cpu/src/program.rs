use calc_exec::{
    Batch, BodyGate, Gate, Iteration, OperationClass, Plan, PlanOp, PrepareError, Reduce,
    ReduceOperation, ReduceShape, RunError, Unsupported, ValueKind,
};

use std::num::NonZeroUsize;
use std::ops::Range;

use calc_exec::SharedParallelism;

use crate::scalar::Scalar;

#[derive(Clone, Copy, Debug)]
enum UnaryOperation {
    Neg,
    Abs,
    Sqrt,
    Floor,
    Ceil,
    Trunc,
    RoundTiesEven,
}

#[derive(Clone, Copy, Debug)]
enum BinaryOperation {
    Add,
    Sub,
    Mul,
    Div,
    Min,
    Max,
    CopySign,
}

#[derive(Clone, Copy, Debug)]
enum Comparison {
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    Equal,
    NotEqual,
}

#[derive(Clone, Copy, Debug)]
enum Logic {
    And,
    Or,
}

#[derive(Clone, Copy, Debug)]
enum Step<T> {
    Input(usize),
    Constant(T),
    Unary(UnaryOperation, usize),
    Binary(BinaryOperation, usize, usize),
    MulAdd(usize, usize, usize),
    Compare(Comparison, usize, usize),
    Logic(Logic, usize, usize),
    Not(usize),
    Select(usize, usize, usize),
    UnaryReference(fn(T) -> T, usize),
    BinaryReference(fn(T, T) -> T, usize, usize),
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
struct IterationProgram<T> {
    initial: Vec<(usize, bool)>,
    body: Vec<Step<T>>,
    next: Vec<(usize, bool)>,
    exit: usize,
    maximum_count: u32,
}

struct IterationScratch<T> {
    body_scalars: Vec<T>,
    body_booleans: Vec<bool>,
    state_scalars: Vec<T>,
    state_booleans: Vec<bool>,
}

#[derive(Clone, Debug)]
pub(crate) struct Program<T> {
    steps: Vec<Step<T>>,
    iterations: Vec<IterationProgram<T>>,
    outputs: Vec<usize>,
    reduce: Option<Reduce>,
}

fn constant_step<T: Scalar>(constant: calc_exec::Constant) -> Result<Step<T>, PrepareError> {
    T::from_constant(constant)
        .map(Step::Constant)
        .ok_or(PrepareError::Unsupported(Unsupported::Domain(
            constant.domain(),
        )))
}

fn is_boolean(kind: Option<ValueKind>) -> bool {
    kind == Some(ValueKind::Boolean)
}

fn compile_iteration<T: Scalar>(
    plan: &Plan,
    iteration: &Iteration,
) -> Result<IterationProgram<T>, PrepareError> {
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

fn operate<T: Scalar>(
    step: &Step<T>,
    index: usize,
    scalars: &mut [T],
    booleans: &mut [bool],
) -> bool {
    match *step {
        Step::Constant(value) => scalars[index] = value,
        Step::Unary(operation, value) => {
            scalars[index] = unary(operation, scalars[value]);
        }
        Step::Binary(operation, left, right) => {
            scalars[index] = binary(operation, scalars[left], scalars[right]);
        }
        Step::MulAdd(left, right, addend) => {
            scalars[index] = scalars[left].fused_multiply_add(scalars[right], scalars[addend]);
        }
        Step::Compare(comparison, left, right) => {
            booleans[index] = compare(comparison, scalars[left], scalars[right]);
        }
        Step::Logic(Logic::And, left, right) => {
            booleans[index] = booleans[left] && booleans[right];
        }
        Step::Logic(Logic::Or, left, right) => {
            booleans[index] = booleans[left] || booleans[right];
        }
        Step::Not(value) => booleans[index] = !booleans[value],
        Step::UnaryReference(function, value) => {
            scalars[index] = function(scalars[value]);
        }
        Step::BinaryReference(function, left, right) => {
            scalars[index] = function(scalars[left], scalars[right]);
        }
        Step::Select(condition, when_true, when_false) => {
            scalars[index] = if booleans[condition] {
                scalars[when_true]
            } else {
                scalars[when_false]
            };
        }
        Step::Input(_)
        | Step::State { .. }
        | Step::Outer { .. }
        | Step::Iterate(_)
        | Step::IterationState { .. } => return false,
    }
    true
}

fn unsupported_operation(operation: PlanOp) -> PrepareError {
    PrepareError::Unsupported(Unsupported::Operation(operation))
}

fn step_for<T: Scalar>(operation: PlanOp, arguments: &[usize]) -> Result<Step<T>, PrepareError> {
    if operation.class() == OperationClass::Approximate {
        return match *arguments {
            [value] => {
                T::unary_reference(operation).map(|function| Step::UnaryReference(function, value))
            }
            [left, right] => T::binary_reference(operation)
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
        (PlanOp::And, &[left, right]) => Step::Logic(Logic::And, left, right),
        (PlanOp::Or, &[left, right]) => Step::Logic(Logic::Or, left, right),
        (PlanOp::Not, &[value]) => Step::Not(value),
        (PlanOp::Select, &[condition, when_true, when_false]) => {
            Step::Select(condition, when_true, when_false)
        }
        _ => return Err(unsupported_operation(operation)),
    };
    Ok(step)
}

impl<T: Scalar> Program<T> {
    pub(crate) fn compile(plan: &Plan) -> Result<Program<T>, PrepareError> {
        let mut steps = Vec::with_capacity(plan.gate_count());
        let mut iterations = Vec::new();
        let mut iteration_numbers: Vec<Option<usize>> = Vec::with_capacity(plan.gate_count());
        for (index, gate) in plan.gates().iter().enumerate() {
            let step =
                match gate {
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
                    Gate::IterationState { iteration, slot } => Step::IterationState {
                        iteration: iteration_numbers
                            .get(iteration.index())
                            .copied()
                            .flatten()
                            .unwrap_or(usize::MAX),
                        slot: slot.index(),
                        boolean: is_boolean(plan.value_kind(calc_exec::GateId(
                            u32::try_from(index).unwrap_or(u32::MAX),
                        ))),
                    },
                };
            iteration_numbers.push(match step {
                Step::Iterate(number) => Some(number),
                _ => None,
            });
            steps.push(step);
        }
        Ok(Program {
            steps,
            iterations,
            outputs: plan.outputs().iter().map(|output| output.index()).collect(),
            reduce: plan.reduce(),
        })
    }

    fn scratch(&self) -> Vec<IterationScratch<T>> {
        self.iterations
            .iter()
            .map(|iteration| IterationScratch {
                body_scalars: vec![T::ZERO; iteration.body.len()],
                body_booleans: vec![false; iteration.body.len()],
                state_scalars: vec![T::ZERO; iteration.initial.len()],
                state_booleans: vec![false; iteration.initial.len()],
            })
            .collect()
    }

    fn evaluate_range(&self, input_columns: &[&[T]], elements: Range<usize>) -> Vec<Vec<T>> {
        let mut columns: Vec<Vec<T>> = vec![Vec::with_capacity(elements.len()); self.outputs.len()];
        let mut scalars = vec![T::ZERO; self.steps.len()];
        let mut booleans = vec![false; self.steps.len()];
        let mut scratch = self.scratch();
        for element in elements {
            self.evaluate_element(
                input_columns,
                element,
                &mut scalars,
                &mut booleans,
                &mut scratch,
            );
            for (column, output) in columns.iter_mut().zip(&self.outputs) {
                column.push(scalars[*output]);
            }
        }
        columns
    }

    pub(crate) fn run(
        &self,
        inputs: &Batch,
        outputs: &mut Batch,
        parallelism: &SharedParallelism,
        minimum_chunk_length: NonZeroUsize,
    ) -> Result<(), RunError> {
        let input_columns: Vec<&[T]> = (0..inputs.channel_count())
            .filter_map(|channel| T::channel(inputs, channel))
            .collect();
        let length = inputs.len();
        let ranges = chunk_ranges(length, parallelism.worker_count(), minimum_chunk_length);
        let columns: Vec<Vec<T>> = if ranges.len() <= 1 {
            self.evaluate_range(&input_columns, 0..length)
        } else {
            let mut chunk_columns: Vec<Vec<Vec<T>>> = vec![Vec::new(); ranges.len()];
            let tasks: Vec<Box<dyn FnOnce() + Send + '_>> = chunk_columns
                .iter_mut()
                .zip(ranges)
                .map(|(target, range)| {
                    let input_columns = &input_columns;
                    Box::new(move || *target = self.evaluate_range(input_columns, range))
                        as Box<dyn FnOnce() + Send + '_>
                })
                .collect();
            parallelism.run_all(tasks);
            let mut joined: Vec<Vec<T>> = vec![Vec::with_capacity(length); self.outputs.len()];
            for chunk in chunk_columns {
                for (column, part) in joined.iter_mut().zip(chunk) {
                    column.extend(part);
                }
            }
            joined
        };
        if columns.iter().any(|column| column.len() != length) {
            return Err(RunError::ParallelTaskNotRun);
        }
        match self.reduce {
            None => {
                for (channel, column) in columns.iter().enumerate() {
                    if let Some(target) = T::channel_mut(outputs, channel) {
                        target.copy_from_slice(column);
                    }
                }
            }
            Some(reduce) => {
                let Some(column) = columns.first() else {
                    return Err(RunError::EmptyReduction);
                };
                let value = reduce_column(column, reduce).ok_or(RunError::EmptyReduction)?;
                if let Some(target) = T::channel_mut(outputs, 0) {
                    target.copy_from_slice(&[value]);
                }
            }
        }
        Ok(())
    }

    fn evaluate_element(
        &self,
        input_columns: &[&[T]],
        element: usize,
        scalars: &mut [T],
        booleans: &mut [bool],
        scratch: &mut [IterationScratch<T>],
    ) {
        for (index, step) in self.steps.iter().enumerate() {
            if operate(step, index, scalars, booleans) {
                continue;
            }
            match *step {
                Step::Input(channel) => scalars[index] = input_columns[channel][element],
                Step::Iterate(number) => {
                    if let (Some(iteration), Some(scratch)) =
                        (self.iterations.get(number), scratch.get_mut(number))
                    {
                        run_iteration(iteration, scalars, booleans, scratch);
                    }
                }
                Step::IterationState {
                    iteration,
                    slot,
                    boolean,
                } => {
                    if let Some(scratch) = scratch.get(iteration) {
                        if boolean {
                            booleans[index] = scratch.state_booleans[slot];
                        } else {
                            scalars[index] = scratch.state_scalars[slot];
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

pub(crate) fn chunk_ranges(
    length: usize,
    workers: NonZeroUsize,
    minimum_chunk_length: NonZeroUsize,
) -> Vec<Range<usize>> {
    let chunk_count = workers
        .get()
        .min(length.div_ceil(minimum_chunk_length.get()))
        .max(1);
    let chunk_length = length.div_ceil(chunk_count).max(1);
    (0..length)
        .step_by(chunk_length)
        .map(|start| start..(start + chunk_length).min(length))
        .collect()
}

fn run_iteration<T: Scalar>(
    iteration: &IterationProgram<T>,
    outer_scalars: &[T],
    outer_booleans: &[bool],
    scratch: &mut IterationScratch<T>,
) {
    for (slot, (gate, boolean)) in iteration.initial.iter().enumerate() {
        if *boolean {
            scratch.state_booleans[slot] = outer_booleans[*gate];
        } else {
            scratch.state_scalars[slot] = outer_scalars[*gate];
        }
    }
    for _ in 0..iteration.maximum_count {
        for (index, step) in iteration.body.iter().enumerate() {
            if operate(
                step,
                index,
                &mut scratch.body_scalars,
                &mut scratch.body_booleans,
            ) {
                continue;
            }
            match *step {
                Step::State { slot, boolean } => {
                    if boolean {
                        scratch.body_booleans[index] = scratch.state_booleans[slot];
                    } else {
                        scratch.body_scalars[index] = scratch.state_scalars[slot];
                    }
                }
                Step::Outer { gate, boolean } => {
                    if boolean {
                        scratch.body_booleans[index] = outer_booleans[gate];
                    } else {
                        scratch.body_scalars[index] = outer_scalars[gate];
                    }
                }
                _ => {}
            }
        }
        if scratch.body_booleans[iteration.exit] {
            break;
        }
        for (slot, (next, boolean)) in iteration.next.iter().enumerate() {
            if *boolean {
                scratch.state_booleans[slot] = scratch.body_booleans[*next];
            } else {
                scratch.state_scalars[slot] = scratch.body_scalars[*next];
            }
        }
    }
}

fn unary<T: Scalar>(operation: UnaryOperation, value: T) -> T {
    match operation {
        UnaryOperation::Neg => -value,
        UnaryOperation::Abs => value.abs(),
        UnaryOperation::Sqrt => value.sqrt(),
        UnaryOperation::Floor => value.floor(),
        UnaryOperation::Ceil => value.ceil(),
        UnaryOperation::Trunc => value.trunc(),
        UnaryOperation::RoundTiesEven => value.round_ties_even(),
    }
}

fn binary<T: Scalar>(operation: BinaryOperation, left: T, right: T) -> T {
    match operation {
        BinaryOperation::Add => left + right,
        BinaryOperation::Sub => left - right,
        BinaryOperation::Mul => left * right,
        BinaryOperation::Div => left / right,
        BinaryOperation::Min => left.minimum(right),
        BinaryOperation::Max => left.maximum(right),
        BinaryOperation::CopySign => left.copysign(right),
    }
}

fn compare<T: Scalar>(comparison: Comparison, left: T, right: T) -> bool {
    match comparison {
        Comparison::Less => left < right,
        Comparison::LessOrEqual => left <= right,
        Comparison::Greater => left > right,
        Comparison::GreaterOrEqual => left >= right,
        Comparison::Equal => left == right,
        Comparison::NotEqual => left != right,
    }
}

fn combine<T: Scalar>(operation: ReduceOperation, left: T, right: T) -> T {
    match operation {
        ReduceOperation::Add => left + right,
        ReduceOperation::Mul => left * right,
        ReduceOperation::Min => left.minimum(right),
        ReduceOperation::Max => left.maximum(right),
    }
}

pub(crate) fn reduce_column<T: Scalar>(values: &[T], reduce: Reduce) -> Option<T> {
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

fn halving<T: Scalar>(values: &[T], operation: ReduceOperation) -> Option<T> {
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
