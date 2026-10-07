use calc_exec::{PlanOp, ReduceOperation};
use calc_numbers::{maximum_f32, minimum_f32};

use crate::device::{BufferData, DeviceError, DispatchSize, GpuDevice};
use crate::kernel::{
    BinaryOperation, BufferKind, CARRY, Comparison, Expression, Kernel, KernelIteration, PassId,
    UnaryOperation,
};
use crate::modes::{OperationModes, plan_binary, plan_comparison, plan_unary};
use crate::validate::validate;

const PERMUTATION_MULTIPLIER: u64 = 6_364_136_223_846_793_005;
const PERMUTATION_INCREMENT: u64 = 1_442_695_040_888_963_407;
const MAP_SEED_SALT: u64 = 0x6d61_7070_6173_7300;
const REDUCE_SEED_SALT: u64 = 0x7265_6475_6365_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftwareFault {
    OutOfMemoryAboveEntries(usize),
    OutOfMemoryAtCompile,
    LostAtDispatch,
    LostAtCompile,
    ShortBufferAtDispatch,
    FlushesSubnormalsNatively,
    WrongResultAtDispatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoftwareDevice {
    seed: u64,
    fault: Option<SoftwareFault>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SoftwareBuffer {
    Values(Vec<f32>),
    Indices(Vec<u32>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoftwareKernel {
    kernel: Kernel,
    flushing: Flushing,
}

impl SoftwareDevice {
    pub fn new(seed: u64) -> SoftwareDevice {
        SoftwareDevice { seed, fault: None }
    }

    pub fn with_fault(seed: u64, fault: SoftwareFault) -> SoftwareDevice {
        SoftwareDevice {
            seed,
            fault: Some(fault),
        }
    }

    fn fails_with(&self, fault: SoftwareFault) -> bool {
        self.fault == Some(fault)
    }
}

fn permutation(length: usize, seed: u64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..length).collect();
    let mut state = seed;
    for index in (1..length).rev() {
        state = state
            .wrapping_mul(PERMUTATION_MULTIPLIER)
            .wrapping_add(PERMUTATION_INCREMENT);
        let bound = u64::try_from(index + 1).unwrap_or(u64::MAX);
        let other = usize::try_from((state >> 33) % bound).unwrap_or(0);
        order.swap(index, other);
    }
    order
}

fn values(buffer: &SoftwareBuffer) -> Result<&[f32], DeviceError> {
    match buffer {
        SoftwareBuffer::Values(values) => Ok(values),
        SoftwareBuffer::Indices(_) => Err(DeviceError::BufferKindMismatch),
    }
}

fn unary(operation: UnaryOperation, value: f32) -> f32 {
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

fn binary(operation: BinaryOperation, left: f32, right: f32) -> f32 {
    match operation {
        BinaryOperation::Add => left + right,
        BinaryOperation::Sub => left - right,
        BinaryOperation::Mul => left * right,
        BinaryOperation::Div => left / right,
        BinaryOperation::Min => minimum_f32(left, right),
        BinaryOperation::Max => maximum_f32(left, right),
        BinaryOperation::CopySign => left.copysign(right),
    }
}

fn compare(comparison: Comparison, left: f32, right: f32) -> bool {
    match comparison {
        Comparison::Less => left < right,
        Comparison::LessOrEqual => left <= right,
        Comparison::Greater => left > right,
        Comparison::GreaterOrEqual => left >= right,
        Comparison::Equal => left == right,
        Comparison::NotEqual => left != right,
    }
}

pub fn combine(operation: ReduceOperation, left: f32, right: f32) -> f32 {
    match operation {
        ReduceOperation::Add => left + right,
        ReduceOperation::Mul => left * right,
        ReduceOperation::Min => minimum_f32(left, right),
        ReduceOperation::Max => maximum_f32(left, right),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flushing {
    modes: OperationModes,
    active: bool,
}

impl Flushing {
    fn of(self, operation: PlanOp, value: f32) -> f32 {
        if !self.active || !self.modes.is_native(operation) {
            return value;
        }
        if value != 0.0 && value.abs() < f32::MIN_POSITIVE {
            return 0.0_f32.copysign(value);
        }
        value
    }
}

struct Invocation<'kernel> {
    kernel: &'kernel Kernel,
    flushing: Flushing,
    inputs: &'kernel [f32],
    constants: &'kernel [f32],
    element_count: usize,
}

struct Locals {
    values: Vec<f32>,
    truths: Vec<bool>,
}

impl Locals {
    fn new(length: usize) -> Locals {
        Locals {
            values: vec![0.0; length],
            truths: vec![false; length],
        }
    }
}

struct IterationFrame {
    body: Locals,
    state: Locals,
}

fn operate(
    expression: Expression,
    index: usize,
    locals: &mut Locals,
    constants: &[f32],
    flushing: Flushing,
) -> bool {
    let values = &mut locals.values;
    let truths = &mut locals.truths;
    match expression {
        Expression::Constant { index: constant } => {
            values[index] = constants
                .get(usize::try_from(constant).unwrap_or(usize::MAX))
                .copied()
                .unwrap_or(f32::NAN);
        }
        Expression::Unary(operation, argument) => {
            let plan = plan_unary(operation);
            values[index] = unary(operation, flushing.of(plan, values[argument.index()]));
        }
        Expression::Binary(operation, left, right) => {
            let plan = plan_binary(operation);
            values[index] = binary(
                operation,
                flushing.of(plan, values[left.index()]),
                flushing.of(plan, values[right.index()]),
            );
        }
        Expression::Compare(comparison, left, right) => {
            let plan = plan_comparison(comparison);
            truths[index] = compare(
                comparison,
                flushing.of(plan, values[left.index()]),
                flushing.of(plan, values[right.index()]),
            );
        }
        Expression::And(left, right) => {
            truths[index] = truths[left.index()] && truths[right.index()];
        }
        Expression::Or(left, right) => {
            truths[index] = truths[left.index()] || truths[right.index()];
        }
        Expression::Not(argument) => truths[index] = !truths[argument.index()],
        Expression::Select {
            condition,
            when_true,
            when_false,
        } => {
            values[index] = if truths[condition.index()] {
                values[when_true.index()]
            } else {
                values[when_false.index()]
            };
        }
        Expression::Input { .. }
        | Expression::Iterate { .. }
        | Expression::IterationState { .. }
        | Expression::State { .. }
        | Expression::Outer { .. } => return false,
    }
    true
}

impl Invocation<'_> {
    fn run_iteration(
        &self,
        iteration: &KernelIteration,
        outer: &Locals,
        frame: &mut IterationFrame,
    ) {
        for (slot, initial) in iteration.initial.iter().enumerate() {
            frame.state.values[slot] = outer.values[initial.index()];
            frame.state.truths[slot] = outer.truths[initial.index()];
        }
        for _ in 0..iteration.maximum_count {
            for (index, expression) in iteration.body.iter().enumerate() {
                if operate(
                    *expression,
                    index,
                    &mut frame.body,
                    self.constants,
                    self.flushing,
                ) {
                    continue;
                }
                match *expression {
                    Expression::State { slot } => {
                        let slot = usize::try_from(slot).unwrap_or(usize::MAX);
                        frame.body.values[index] = frame.state.values[slot];
                        frame.body.truths[index] = frame.state.truths[slot];
                    }
                    Expression::Outer { local } => {
                        frame.body.values[index] = outer.values[local.index()];
                        frame.body.truths[index] = outer.truths[local.index()];
                    }
                    _ => {}
                }
            }
            if frame.body.truths[iteration.exit.index()] {
                break;
            }
            for (slot, next) in iteration.next.iter().enumerate() {
                frame.state.values[slot] = frame.body.values[next.index()];
                frame.state.truths[slot] = frame.body.truths[next.index()];
            }
        }
    }

    fn run(&self, element: usize, locals: &mut Locals, frames: &mut [IterationFrame]) {
        for (index, statement) in self.kernel.map.statements.iter().enumerate() {
            if operate(*statement, index, locals, self.constants, self.flushing) {
                continue;
            }
            match *statement {
                Expression::Input { channel } => {
                    let channel = usize::try_from(channel).unwrap_or(usize::MAX);
                    locals.values[index] = self
                        .inputs
                        .get(channel * self.element_count + element)
                        .copied()
                        .unwrap_or(f32::NAN);
                }
                Expression::Iterate { iteration } => {
                    let number = usize::try_from(iteration).unwrap_or(usize::MAX);
                    if let (Some(found), Some(frame)) = (
                        self.kernel.map.iterations.get(number),
                        frames.get_mut(number),
                    ) {
                        self.run_iteration(found, locals, frame);
                    }
                }
                Expression::IterationState { iteration, slot } => {
                    let number = match self.kernel.map.statements.get(iteration.index()) {
                        Some(Expression::Iterate { iteration }) => {
                            usize::try_from(*iteration).unwrap_or(usize::MAX)
                        }
                        _ => usize::MAX,
                    };
                    if let Some(frame) = frames.get(number) {
                        let slot = usize::try_from(slot).unwrap_or(usize::MAX);
                        locals.values[index] = frame.state.values[slot];
                        locals.truths[index] = frame.state.truths[slot];
                    }
                }
                _ => {}
            }
        }
    }
}

impl SoftwareDevice {
    fn dispatch_map(
        &self,
        kernel: &Kernel,
        flushing: Flushing,
        buffers: &mut [SoftwareBuffer],
        size: DispatchSize,
    ) -> Result<(), DeviceError> {
        let map = &kernel.map;
        let element_count = usize::try_from(size.element_count).unwrap_or(usize::MAX);
        let result_count = map.results.len();
        let mut produced = vec![0.0_f32; element_count * result_count];
        {
            let buffer = |id: crate::kernel::BufferId| {
                buffers.get(id.index()).ok_or(DeviceError::BufferTooShort)
            };
            let invocation = Invocation {
                kernel,
                flushing,
                inputs: values(buffer(map.inputs)?)?,
                constants: values(buffer(map.constants)?)?,
                element_count,
            };
            let mut locals = Locals::new(map.statements.len());
            let mut frames: Vec<IterationFrame> = map
                .iterations
                .iter()
                .map(|iteration| IterationFrame {
                    body: Locals::new(iteration.body.len()),
                    state: Locals::new(iteration.initial.len()),
                })
                .collect();
            for element in permutation(element_count, self.seed ^ MAP_SEED_SALT) {
                invocation.run(element, &mut locals, &mut frames);
                for (channel, result) in map.results.iter().enumerate() {
                    produced[channel * element_count + element] = locals.values[result.index()];
                }
            }
        }
        match buffers.get_mut(map.outputs.index()) {
            Some(SoftwareBuffer::Values(outputs)) if outputs.len() >= produced.len() => {
                outputs[..produced.len()].copy_from_slice(&produced);
                Ok(())
            }
            Some(SoftwareBuffer::Values(_)) | None => Err(DeviceError::BufferTooShort),
            Some(SoftwareBuffer::Indices(_)) => Err(DeviceError::BufferKindMismatch),
        }
    }

    fn dispatch_reduce(
        &self,
        kernel: &Kernel,
        buffers: &mut [SoftwareBuffer],
        size: DispatchSize,
    ) -> Result<(), DeviceError> {
        let Some(reduce) = kernel.reduce else {
            return Err(DeviceError::KernelRejected(
                crate::validate::KernelError::NoResults,
            ));
        };
        let invocations = usize::try_from(size.invocations).unwrap_or(usize::MAX);
        let mut writes = Vec::with_capacity(invocations);
        {
            let source = values(
                buffers
                    .get(reduce.source.index())
                    .ok_or(DeviceError::BufferTooShort)?,
            )?;
            let SoftwareBuffer::Indices(triples) = buffers
                .get(reduce.triples.index())
                .ok_or(DeviceError::BufferTooShort)?
            else {
                return Err(DeviceError::BufferKindMismatch);
            };
            if triples.len() < invocations * 3 {
                return Err(DeviceError::BufferTooShort);
            }
            let read = |position: u32| {
                source
                    .get(usize::try_from(position).unwrap_or(usize::MAX))
                    .copied()
                    .ok_or(DeviceError::BufferTooShort)
            };
            for invocation in permutation(invocations, self.seed ^ REDUCE_SEED_SALT) {
                let triple = &triples[invocation * 3..invocation * 3 + 3];
                let value = if triple[1] == CARRY {
                    read(triple[0])?
                } else {
                    combine(reduce.operation, read(triple[0])?, read(triple[1])?)
                };
                writes.push((usize::try_from(triple[2]).unwrap_or(usize::MAX), value));
            }
        }
        let Some(SoftwareBuffer::Values(target)) = buffers.get_mut(reduce.target.index()) else {
            return Err(DeviceError::BufferKindMismatch);
        };
        for (position, value) in writes {
            let slot = target
                .get_mut(position)
                .ok_or(DeviceError::BufferTooShort)?;
            *slot = value;
        }
        Ok(())
    }
}

impl GpuDevice for SoftwareDevice {
    type Buffer = SoftwareBuffer;
    type Compiled = SoftwareKernel;

    fn create_buffer(
        &self,
        kind: BufferKind,
        length: usize,
    ) -> Result<SoftwareBuffer, DeviceError> {
        if let Some(SoftwareFault::OutOfMemoryAboveEntries(limit)) = self.fault
            && length > limit
        {
            return Err(DeviceError::OutOfMemory);
        }
        Ok(match kind {
            BufferKind::Values => SoftwareBuffer::Values(vec![0.0; length]),
            BufferKind::Indices => SoftwareBuffer::Indices(vec![0; length]),
        })
    }

    fn write_buffer(
        &self,
        buffer: &mut SoftwareBuffer,
        data: BufferData<'_>,
    ) -> Result<(), DeviceError> {
        match (buffer, data) {
            (SoftwareBuffer::Values(target), BufferData::Values(source))
                if target.len() >= source.len() =>
            {
                target[..source.len()].copy_from_slice(source);
                Ok(())
            }
            (SoftwareBuffer::Indices(target), BufferData::Indices(source))
                if target.len() >= source.len() =>
            {
                target[..source.len()].copy_from_slice(source);
                Ok(())
            }
            (SoftwareBuffer::Values(_), BufferData::Values(_))
            | (SoftwareBuffer::Indices(_), BufferData::Indices(_)) => {
                Err(DeviceError::BufferTooShort)
            }
            _ => Err(DeviceError::BufferKindMismatch),
        }
    }

    fn read_buffer(&self, buffer: &SoftwareBuffer, length: usize) -> Result<Vec<f32>, DeviceError> {
        values(buffer)?
            .get(..length)
            .map(<[f32]>::to_vec)
            .ok_or(DeviceError::BufferTooShort)
    }

    fn compile(
        &self,
        kernel: &Kernel,
        modes: OperationModes,
    ) -> Result<SoftwareKernel, DeviceError> {
        if self.fails_with(SoftwareFault::LostAtCompile) {
            return Err(DeviceError::Lost);
        }
        if self.fails_with(SoftwareFault::OutOfMemoryAtCompile) {
            return Err(DeviceError::OutOfMemory);
        }
        validate(kernel).map_err(DeviceError::KernelRejected)?;
        Ok(SoftwareKernel {
            kernel: kernel.clone(),
            flushing: Flushing {
                modes,
                active: self.fails_with(SoftwareFault::FlushesSubnormalsNatively),
            },
        })
    }

    fn dispatch(
        &self,
        compiled: &SoftwareKernel,
        pass: PassId,
        buffers: &mut [SoftwareBuffer],
        size: DispatchSize,
    ) -> Result<(), DeviceError> {
        if self.fails_with(SoftwareFault::LostAtDispatch) {
            return Err(DeviceError::Lost);
        }
        if self.fails_with(SoftwareFault::ShortBufferAtDispatch) {
            return Err(DeviceError::BufferTooShort);
        }
        match pass {
            PassId::Map => self.dispatch_map(&compiled.kernel, compiled.flushing, buffers, size)?,
            PassId::Reduce => self.dispatch_reduce(&compiled.kernel, buffers, size)?,
        }
        if self.fails_with(SoftwareFault::WrongResultAtDispatch) {
            let written = match pass {
                PassId::Map => compiled.kernel.map.outputs,
                PassId::Reduce => match compiled.kernel.reduce {
                    Some(reduce) => reduce.target,
                    None => return Ok(()),
                },
            };
            if let Some(SoftwareBuffer::Values(values)) = buffers.get_mut(written.index())
                && let Some(first) = values.first_mut()
            {
                *first = f32::from_bits(first.to_bits() ^ 1);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{
        CONSTANTS, INPUTS, LocalId, MapPass, OUTPUTS, REDUCE_TARGET, ReducePass, TRIPLES,
    };
    use crate::validate::KernelError;

    fn doubling_kernel() -> Kernel {
        Kernel {
            buffers: vec![
                BufferKind::Values,
                BufferKind::Values,
                BufferKind::Values,
                BufferKind::Values,
                BufferKind::Indices,
            ],
            constants: vec![2.0],
            map: MapPass {
                inputs: INPUTS,
                constants: CONSTANTS,
                outputs: OUTPUTS,
                input_channel_count: 1,
                statements: vec![
                    Expression::Input { channel: 0 },
                    Expression::Constant { index: 0 },
                    Expression::Binary(BinaryOperation::Mul, LocalId(0), LocalId(1)),
                ],
                results: vec![LocalId(2)],
                iterations: Vec::new(),
            },
            reduce: Some(ReducePass {
                operation: ReduceOperation::Add,
                source: OUTPUTS,
                target: REDUCE_TARGET,
                triples: TRIPLES,
            }),
        }
    }

    fn buffers(device: &SoftwareDevice, inputs: &[f32]) -> Vec<SoftwareBuffer> {
        let mut buffers: Vec<SoftwareBuffer> = [
            (BufferKind::Values, inputs.len()),
            (BufferKind::Values, 1),
            (BufferKind::Values, inputs.len()),
            (BufferKind::Values, inputs.len()),
            (BufferKind::Indices, 3 * inputs.len()),
        ]
        .into_iter()
        .map(|(kind, length)| device.create_buffer(kind, length).unwrap())
        .collect();
        device
            .write_buffer(&mut buffers[0], BufferData::Values(inputs))
            .unwrap();
        device
            .write_buffer(&mut buffers[1], BufferData::Values(&[2.0]))
            .unwrap();
        buffers
    }

    fn doubled(seed: u64) -> Vec<f32> {
        let device = SoftwareDevice::new(seed);
        let compiled = device
            .compile(&doubling_kernel(), OperationModes::EXACT)
            .unwrap();
        let mut buffers = buffers(&device, &[1.0, -0.0, 3.5, f32::INFINITY]);
        device
            .dispatch(
                &compiled,
                PassId::Map,
                &mut buffers,
                DispatchSize {
                    element_count: 4,
                    invocations: 4,
                },
            )
            .unwrap();
        device.read_buffer(&buffers[2], 4).unwrap()
    }

    #[test]
    fn map_pass_writes_each_element_at_its_own_index() {
        let values = doubled(3);

        assert_eq!(
            values
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            [2.0_f32, -0.0, 7.0, f32::INFINITY]
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn invocation_order_differs_between_seeds() {
        let first = permutation(16, 1);

        let second = permutation(16, 2);

        assert_ne!(first, second);
    }

    #[test]
    fn map_output_does_not_depend_on_the_seed() {
        let first = doubled(1);

        let second = doubled(99);

        assert_eq!(first, second);
    }

    #[test]
    fn reduce_pass_combines_and_carries_by_triples() {
        let device = SoftwareDevice::new(5);
        let compiled = device
            .compile(&doubling_kernel(), OperationModes::EXACT)
            .unwrap();
        let mut buffers = buffers(&device, &[1.0, 2.0, 4.0]);
        buffers[2] = SoftwareBuffer::Values(vec![1.0, 2.0, 4.0]);
        device
            .write_buffer(
                &mut buffers[4],
                BufferData::Indices(&[0, CARRY, 0, 1, 2, 1]),
            )
            .unwrap();

        device
            .dispatch(
                &compiled,
                PassId::Reduce,
                &mut buffers,
                DispatchSize {
                    element_count: 3,
                    invocations: 2,
                },
            )
            .unwrap();

        assert_eq!(device.read_buffer(&buffers[3], 2).unwrap(), vec![1.0, 6.0]);
    }

    #[test]
    fn invalid_kernel_is_rejected_at_compile() {
        let mut kernel = doubling_kernel();
        kernel.map.results = Vec::new();

        let compiled = SoftwareDevice::new(1).compile(&kernel, OperationModes::EXACT);

        assert_eq!(
            compiled.err(),
            Some(DeviceError::KernelRejected(KernelError::NoResults))
        );
    }

    #[test]
    fn writing_indices_into_a_value_buffer_is_rejected() {
        let device = SoftwareDevice::new(1);
        let mut buffer = device.create_buffer(BufferKind::Values, 3).unwrap();

        let written = device.write_buffer(&mut buffer, BufferData::Indices(&[1, 2, 3]));

        assert_eq!(written, Err(DeviceError::BufferKindMismatch));
    }

    #[test]
    fn reading_past_the_buffer_is_rejected() {
        let device = SoftwareDevice::new(1);
        let buffer = device.create_buffer(BufferKind::Values, 3).unwrap();

        let read = device.read_buffer(&buffer, 4);

        assert_eq!(read, Err(DeviceError::BufferTooShort));
    }
}
