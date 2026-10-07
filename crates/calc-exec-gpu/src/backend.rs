use calc_exec::{
    Backend, BackendKind, Batch, Capabilities, Domain, ExecutionMode, OperationClass, Plan, PlanOp,
    PrepareError, Prepared, Reduce, ReduceShape, RunError, RunReport,
};

use crate::device::{BufferData, DeviceError, DispatchSize, GpuDevice};
use crate::kernel::{
    BufferKind, CONSTANTS, INPUTS, Kernel, OUTPUTS, PassId, REDUCE_TARGET, TRIPLES,
};
use crate::lower::{halving_stages, lower_plan};
use crate::modes::{EXACT_IN_BOTH_FORMS, OperationModes};
use crate::software::combine;

const TRIPLE_WIDTH: usize = 3;
const LARGEST_ITERATION_COUNT: u32 = 65_536;

pub struct GpuBackend<D: GpuDevice> {
    device: D,
    capabilities: Capabilities,
    modes: OperationModes,
}

pub struct GpuPrepared<D: GpuDevice> {
    device: D,
    plan: Plan,
    kernel: Kernel,
    compiled: D::Compiled,
}

impl<D: GpuDevice> GpuBackend<D> {
    pub fn new(device: D) -> GpuBackend<D> {
        GpuBackend::with_modes(device, OperationModes::EXACT)
    }

    pub fn with_modes(device: D, modes: OperationModes) -> GpuBackend<D> {
        GpuBackend {
            device,
            modes,
            capabilities: capabilities(),
        }
    }
}

pub fn capabilities() -> Capabilities {
    Capabilities {
        domains: vec![Domain::F32],
        operations: PlanOp::ALL
            .into_iter()
            .filter(|operation| {
                operation.class() == OperationClass::CorrectlyRounded
                    && *operation != PlanOp::MulAdd
            })
            .collect(),
        largest_batch_length: usize::try_from(u32::MAX).unwrap_or(usize::MAX),
        largest_iteration_count: LARGEST_ITERATION_COUNT,
        reference_approximate_operations: false,
        cost: calc_exec::cost::GPU,
    }
}

fn prepare_error(error: DeviceError) -> PrepareError {
    match error {
        DeviceError::OutOfMemory => PrepareError::OutOfDeviceMemory,
        DeviceError::Lost => PrepareError::DeviceLost,
        DeviceError::KernelRejected(_)
        | DeviceError::ShaderRejected
        | DeviceError::BufferKindMismatch
        | DeviceError::BufferTooShort => PrepareError::KernelRejected,
    }
}

fn run_error(error: DeviceError) -> RunError {
    match error {
        DeviceError::OutOfMemory => RunError::OutOfDeviceMemory,
        DeviceError::Lost => RunError::DeviceLost,
        DeviceError::KernelRejected(_)
        | DeviceError::ShaderRejected
        | DeviceError::BufferKindMismatch
        | DeviceError::BufferTooShort => RunError::DeviceRejected,
    }
}

impl<D: GpuDevice> Backend for GpuBackend<D> {
    fn kind(&self) -> BackendKind {
        BackendKind::Gpu
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn execution_modes(&self) -> Vec<(PlanOp, ExecutionMode)> {
        let mut found: Vec<(PlanOp, ExecutionMode)> = EXACT_IN_BOTH_FORMS
            .into_iter()
            .map(|operation| (operation, ExecutionMode::ExactInBothForms))
            .collect();
        for operation in &self.capabilities.operations {
            if EXACT_IN_BOTH_FORMS.contains(operation) {
                continue;
            }
            let mode = if self.modes.is_native(*operation) {
                ExecutionMode::Native
            } else {
                ExecutionMode::IntegerExact
            };
            found.push((*operation, mode));
        }
        found.sort_by_key(|(operation, _)| {
            PlanOp::ALL
                .iter()
                .position(|listed| listed == operation)
                .unwrap_or(usize::MAX)
        });
        found
    }

    fn prepare(&self, plan: &Plan) -> Result<Box<dyn Prepared>, PrepareError> {
        self.capabilities
            .check_plan(plan)
            .map_err(PrepareError::Unsupported)?;
        let kernel = lower_plan(plan)?;
        let compiled = self
            .device
            .compile(&kernel, self.modes)
            .map_err(prepare_error)?;
        Ok(Box::new(GpuPrepared {
            device: self.device.clone(),
            plan: plan.clone(),
            kernel,
            compiled,
        }))
    }
}

struct Transfers {
    report: RunReport,
}

impl Transfers {
    fn count(&mut self, entries: usize) {
        self.report.transfers = self.report.transfers.saturating_add(1);
        self.report.moved_entries = self
            .report
            .moved_entries
            .saturating_add(u64::try_from(entries).unwrap_or(u64::MAX));
    }
}

impl<D: GpuDevice> GpuPrepared<D> {
    fn buffers(&self, length: usize) -> Result<Vec<D::Buffer>, DeviceError> {
        let channels = self.plan.input_channel_count();
        let results = self.plan.output_channel_count();
        let lengths = [
            (BufferKind::Values, channels.saturating_mul(length).max(1)),
            (BufferKind::Values, self.kernel.constants.len().max(1)),
            (BufferKind::Values, results.saturating_mul(length).max(1)),
            (BufferKind::Values, length.max(1)),
            (
                BufferKind::Indices,
                length.saturating_mul(TRIPLE_WIDTH).max(TRIPLE_WIDTH),
            ),
        ];
        lengths
            .into_iter()
            .map(|(kind, entries)| self.device.create_buffer(kind, entries))
            .collect()
    }

    fn flat_inputs(inputs: &Batch) -> Vec<f32> {
        (0..inputs.channel_count())
            .filter_map(|channel| inputs.f32_channel(channel))
            .flat_map(|column| column.iter().copied())
            .collect()
    }

    fn run_on_device(
        &mut self,
        inputs: &Batch,
        outputs: &mut Batch,
    ) -> Result<RunReport, DeviceError> {
        let length = inputs.len();
        let element_count = u32::try_from(length).map_err(|_| DeviceError::OutOfMemory)?;
        let mut transfers = Transfers {
            report: RunReport::default(),
        };
        let mut buffers = self.buffers(length)?;
        let flat = Self::flat_inputs(inputs);
        if !flat.is_empty() {
            let buffer = buffers
                .get_mut(INPUTS.index())
                .ok_or(DeviceError::BufferTooShort)?;
            self.device
                .write_buffer(buffer, BufferData::Values(&flat))?;
            transfers.count(flat.len());
        }
        if !self.kernel.constants.is_empty() {
            let buffer = buffers
                .get_mut(CONSTANTS.index())
                .ok_or(DeviceError::BufferTooShort)?;
            self.device
                .write_buffer(buffer, BufferData::Values(&self.kernel.constants))?;
            transfers.count(self.kernel.constants.len());
        }
        self.device.dispatch(
            &self.compiled,
            PassId::Map,
            &mut buffers,
            DispatchSize {
                element_count,
                invocations: element_count,
            },
        )?;
        match self.plan.reduce() {
            None => {
                let entries = self.plan.output_channel_count() * length;
                let buffer = buffers
                    .get(OUTPUTS.index())
                    .ok_or(DeviceError::BufferTooShort)?;
                let values = self.device.read_buffer(buffer, entries)?;
                transfers.count(entries);
                for channel in 0..self.plan.output_channel_count() {
                    let source = values
                        .get(channel * length..(channel + 1) * length)
                        .ok_or(DeviceError::BufferTooShort)?;
                    if let Some(target) = outputs.f32_channel_mut(channel) {
                        target.copy_from_slice(source);
                    }
                }
            }
            Some(Reduce {
                operation,
                shape: ReduceShape::LeftFold,
            }) => {
                let buffer = buffers
                    .get(OUTPUTS.index())
                    .ok_or(DeviceError::BufferTooShort)?;
                let values = self.device.read_buffer(buffer, length)?;
                transfers.count(length);
                let (first, rest) = values.split_first().ok_or(DeviceError::BufferTooShort)?;
                let value = rest
                    .iter()
                    .fold(*first, |sum, value| combine(operation, sum, *value));
                if let Some(target) = outputs.f32_channel_mut(0) {
                    target.copy_from_slice(&[value]);
                }
            }
            Some(Reduce {
                shape: ReduceShape::Halving,
                ..
            }) => {
                for triples in halving_stages(element_count) {
                    let buffer = buffers
                        .get_mut(TRIPLES.index())
                        .ok_or(DeviceError::BufferTooShort)?;
                    self.device
                        .write_buffer(buffer, BufferData::Indices(&triples))?;
                    transfers.count(triples.len());
                    let invocations = u32::try_from(triples.len() / TRIPLE_WIDTH)
                        .map_err(|_| DeviceError::OutOfMemory)?;
                    self.device.dispatch(
                        &self.compiled,
                        PassId::Reduce,
                        &mut buffers,
                        DispatchSize {
                            element_count,
                            invocations,
                        },
                    )?;
                    buffers.swap(OUTPUTS.index(), REDUCE_TARGET.index());
                }
                let buffer = buffers
                    .get(OUTPUTS.index())
                    .ok_or(DeviceError::BufferTooShort)?;
                let value = self.device.read_buffer(buffer, 1)?;
                transfers.count(1);
                if let Some(target) = outputs.f32_channel_mut(0) {
                    target.copy_from_slice(&value);
                }
            }
        }
        Ok(transfers.report)
    }
}

impl<D: GpuDevice> Prepared for GpuPrepared<D> {
    fn run(&mut self, inputs: &Batch, outputs: &mut Batch) -> Result<RunReport, RunError> {
        self.plan
            .check_batches(inputs, outputs)
            .map_err(RunError::BatchShape)?;
        if self.plan.reduce().is_some() && inputs.is_empty() {
            return Err(RunError::EmptyReduction);
        }
        self.run_on_device(inputs, outputs).map_err(run_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::software::{SoftwareDevice, SoftwareFault};
    use calc_conformance::{
        Case, ConformanceError, ESCAPE_TIME_CASE_ITERATIONS, check_bitwise, check_equivalent,
        check_expected, check_unrolled, contraction_case, escape_time_cases,
        escape_time_equivalent_cases, iteration_cases, operation_case, reduce_case,
    };
    use calc_exec::{ReduceOperation, expected_transfers};

    const SEEDS: [u64; 3] = [1, 7, 12_345];

    fn check_on_every_seed(case: &Case) -> Result<(), ConformanceError> {
        SEEDS
            .iter()
            .try_for_each(|seed| check_bitwise(&GpuBackend::new(SoftwareDevice::new(*seed)), case))
    }

    macro_rules! operation_conformance {
        ($($name:ident => $operation:ident),* $(,)?) => {
            $(
                #[test]
                fn $name() {
                    let case = operation_case(PlanOp::$operation, Domain::F32).unwrap();

                    let result = check_on_every_seed(&case);

                    assert_eq!(result, Ok(()));
                }
            )*
        };
    }

    operation_conformance! {
        add_conforms => Add,
        sub_conforms => Sub,
        mul_conforms => Mul,
        div_conforms => Div,
        neg_conforms => Neg,
        abs_conforms => Abs,
        sqrt_conforms => Sqrt,
        min_conforms => Min,
        max_conforms => Max,
        floor_conforms => Floor,
        ceil_conforms => Ceil,
        trunc_conforms => Trunc,
        round_ties_even_conforms => RoundTiesEven,
        copy_sign_conforms => CopySign,
        less_conforms => Less,
        less_or_equal_conforms => LessOrEqual,
        greater_conforms => Greater,
        greater_or_equal_conforms => GreaterOrEqual,
        equal_conforms => Equal,
        not_equal_conforms => NotEqual,
        and_conforms => And,
        or_conforms => Or,
        not_conforms => Not,
        select_conforms => Select,
    }

    macro_rules! reduce_conformance {
        ($($name:ident => $operation:ident, $shape:ident),* $(,)?) => {
            $(
                #[test]
                fn $name() {
                    let failures: Vec<usize> = calc_conformance::REDUCE_LENGTHS
                        .into_iter()
                        .filter(|length| {
                            let case = reduce_case(
                                ReduceOperation::$operation,
                                ReduceShape::$shape,
                                *length,
                                Domain::F32,
                            )
                            .unwrap();
                            check_on_every_seed(&case).is_err()
                        })
                        .collect();

                    assert!(failures.is_empty(), "{failures:?}");
                }
            )*
        };
    }

    reduce_conformance! {
        add_left_fold_conforms => Add, LeftFold,
        add_halving_conforms => Add, Halving,
        mul_left_fold_conforms => Mul, LeftFold,
        mul_halving_conforms => Mul, Halving,
        min_left_fold_conforms => Min, LeftFold,
        min_halving_conforms => Min, Halving,
        max_left_fold_conforms => Max, LeftFold,
        max_halving_conforms => Max, Halving,
    }

    #[test]
    fn contraction_case_conforms() {
        let case = contraction_case(Domain::F32).unwrap();

        assert_eq!(check_on_every_seed(&case), Ok(()));
    }

    fn run_case(device: SoftwareDevice, case: &Case) -> Result<RunReport, RunError> {
        let backend = GpuBackend::new(device);
        let mut prepared = backend
            .prepare(&case.plan)
            .map_err(|_| RunError::DeviceRejected)?;
        let mut outputs = Batch::zeroed(
            Domain::F32,
            case.plan.output_channel_count(),
            case.plan.output_length(case.inputs.len()),
        );
        prepared.run(&case.inputs, &mut outputs)
    }

    #[test]
    fn map_run_transfers_inputs_constants_and_outputs_once_each() {
        let case = operation_case(PlanOp::Less, Domain::F32).unwrap();
        let length = case.inputs.len();

        let report = run_case(SoftwareDevice::new(1), &case).unwrap();

        assert_eq!(
            report,
            RunReport {
                transfers: 3,
                moved_entries: u64::try_from(2 * length + 2 + length).unwrap()
            }
        );
    }

    #[test]
    fn map_run_without_constants_matches_the_selection_model() {
        let case = operation_case(PlanOp::Add, Domain::F32).unwrap();

        let report = run_case(SoftwareDevice::new(1), &case).unwrap();

        let model = expected_transfers(&case.plan, case.inputs.len());
        assert_eq!(
            (report.transfers, report.moved_entries),
            (2, model.moved_entries)
        );
    }

    #[test]
    fn halving_run_transfers_one_index_list_per_stage() {
        let case = reduce_case(
            ReduceOperation::Add,
            ReduceShape::Halving,
            1025,
            Domain::F32,
        )
        .unwrap();
        let stages = halving_stages(1025);

        let report = run_case(SoftwareDevice::new(1), &case).unwrap();

        let triples: usize = stages.iter().map(Vec::len).sum();
        assert_eq!(
            report,
            RunReport {
                transfers: 2 + u64::try_from(stages.len()).unwrap(),
                moved_entries: u64::try_from(1025 + triples + 1).unwrap()
            }
        );
    }

    #[test]
    fn left_fold_run_reads_the_whole_map_output() {
        let case =
            reduce_case(ReduceOperation::Add, ReduceShape::LeftFold, 5, Domain::F32).unwrap();

        let report = run_case(SoftwareDevice::new(1), &case).unwrap();

        assert_eq!(
            report,
            RunReport {
                transfers: 2,
                moved_entries: 10
            }
        );
    }

    #[test]
    fn fused_multiply_add_is_unsupported() {
        let case = operation_case(PlanOp::MulAdd, Domain::F32).unwrap();
        let backend = GpuBackend::new(SoftwareDevice::new(1));

        let prepared = backend.prepare(&case.plan);

        assert_eq!(
            prepared.err(),
            Some(PrepareError::Unsupported(
                calc_exec::Unsupported::Operation(PlanOp::MulAdd)
            ))
        );
    }

    #[test]
    fn f64_plan_is_unsupported() {
        let case = operation_case(PlanOp::Add, Domain::F64).unwrap();
        let backend = GpuBackend::new(SoftwareDevice::new(1));

        let prepared = backend.prepare(&case.plan);

        assert_eq!(
            prepared.err(),
            Some(PrepareError::Unsupported(calc_exec::Unsupported::Domain(
                Domain::F64
            )))
        );
    }

    #[test]
    fn approximate_operation_is_unsupported() {
        let case = operation_case(PlanOp::Sin, Domain::F32).unwrap();
        let backend = GpuBackend::new(SoftwareDevice::new(1));

        let prepared = backend.prepare(&case.plan);

        assert_eq!(
            prepared.err(),
            Some(PrepareError::Unsupported(
                calc_exec::Unsupported::Operation(PlanOp::Sin)
            ))
        );
    }

    #[test]
    fn device_lost_while_compiling_fails_prepare() {
        let case = operation_case(PlanOp::Add, Domain::F32).unwrap();
        let backend = GpuBackend::new(SoftwareDevice::with_fault(1, SoftwareFault::LostAtCompile));

        let prepared = backend.prepare(&case.plan);

        assert_eq!(prepared.err(), Some(PrepareError::DeviceLost));
    }

    #[test]
    fn device_lost_while_dispatching_fails_the_run() {
        let case = operation_case(PlanOp::Add, Domain::F32).unwrap();

        let result = run_case(
            SoftwareDevice::with_fault(1, SoftwareFault::LostAtDispatch),
            &case,
        );

        assert_eq!(result, Err(RunError::DeviceLost));
    }

    #[test]
    fn buffer_beyond_device_memory_fails_the_run() {
        let case = operation_case(PlanOp::Add, Domain::F32).unwrap();

        let result = run_case(
            SoftwareDevice::with_fault(1, SoftwareFault::OutOfMemoryAboveEntries(16)),
            &case,
        );

        assert_eq!(result, Err(RunError::OutOfDeviceMemory));
    }

    #[test]
    fn device_out_of_memory_while_compiling_fails_prepare() {
        let case = operation_case(PlanOp::Add, Domain::F32).unwrap();
        let backend = GpuBackend::new(SoftwareDevice::with_fault(
            1,
            SoftwareFault::OutOfMemoryAtCompile,
        ));

        let prepared = backend.prepare(&case.plan);

        assert_eq!(prepared.err(), Some(PrepareError::OutOfDeviceMemory));
    }

    #[test]
    fn device_rejecting_a_buffer_fails_the_run() {
        let case = operation_case(PlanOp::Add, Domain::F32).unwrap();

        let result = run_case(
            SoftwareDevice::with_fault(1, SoftwareFault::ShortBufferAtDispatch),
            &case,
        );

        assert_eq!(result, Err(RunError::DeviceRejected));
    }

    #[test]
    fn empty_batch_on_reduced_plan_is_an_error() {
        let case = reduce_case(ReduceOperation::Add, ReduceShape::Halving, 0, Domain::F32).unwrap();

        let result = run_case(SoftwareDevice::new(1), &case);

        assert_eq!(result, Err(RunError::EmptyReduction));
    }

    #[test]
    fn backend_reports_gpu_kind_and_the_escape_time_iteration_cap() {
        let backend = GpuBackend::new(SoftwareDevice::new(1));

        let capabilities = backend.capabilities();

        assert_eq!(
            (
                backend.kind(),
                capabilities.largest_iteration_count,
                capabilities.domains.clone()
            ),
            (BackendKind::Gpu, 65_536, vec![Domain::F32])
        );
    }

    macro_rules! iteration_conformance {
        ($($name:ident => $index:expr),* $(,)?) => {
            $(
                #[test]
                fn $name() {
                    let case = iteration_cases(Domain::F32).remove($index);

                    let result = SEEDS.iter().try_for_each(|seed| {
                        check_unrolled(&GpuBackend::new(SoftwareDevice::new(*seed)), &case)
                    });

                    assert_eq!(result, Ok(()));
                }
            )*
        };
    }

    iteration_conformance! {
        doubling_iteration_conforms => 0,
        doubling_iteration_without_steps_conforms => 1,
        squaring_iteration_conforms => 2,
        boolean_slot_iteration_conforms => 3,
    }

    #[test]
    fn iteration_with_approximate_body_is_unsupported() {
        let case = iteration_cases(Domain::F32).remove(4);

        let result = check_unrolled(&GpuBackend::new(SoftwareDevice::new(1)), &case);

        assert!(matches!(
            result,
            Err(ConformanceError::Prepare(PrepareError::Unsupported(_)))
        ));
    }

    #[test]
    fn escape_time_parameter_iteration_matches_the_unrolled_circuit() {
        let case = escape_time_equivalent_cases(Domain::F32, ESCAPE_TIME_CASE_ITERATIONS).remove(0);

        let result = SEEDS.iter().try_for_each(|seed| {
            check_equivalent(&GpuBackend::new(SoftwareDevice::new(*seed)), &case)
        });

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn escape_time_initial_iteration_matches_the_unrolled_circuit() {
        let case = escape_time_equivalent_cases(Domain::F32, ESCAPE_TIME_CASE_ITERATIONS).remove(1);

        let result = SEEDS.iter().try_for_each(|seed| {
            check_equivalent(&GpuBackend::new(SoftwareDevice::new(*seed)), &case)
        });

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn escape_time_expected_counts_hold_on_the_software_device() {
        let failures = escape_time_cases(Domain::F32)
            .iter()
            .filter(|case| check_expected(&GpuBackend::new(SoftwareDevice::new(3)), case).is_err())
            .count();

        assert_eq!(failures, 0);
    }

    #[test]
    fn iteration_above_the_cap_is_unsupported() {
        let plan = calc_exec::escape_time_iteration_plan(
            Domain::F32,
            calc_exec::EscapeTimePlanForm::Parameter,
            65_537,
        )
        .unwrap();

        let prepared = GpuBackend::new(SoftwareDevice::new(1)).prepare(&plan);

        assert_eq!(
            prepared.err(),
            Some(PrepareError::Unsupported(
                calc_exec::Unsupported::IterationCount(65_537)
            ))
        );
    }

    #[test]
    fn escape_time_kernel_size_does_not_grow_with_the_cap() {
        let kernel = |iterations| {
            let plan = calc_exec::escape_time_iteration_plan(
                Domain::F32,
                calc_exec::EscapeTimePlanForm::Parameter,
                iterations,
            )
            .unwrap();
            let kernel = lower_plan(&plan).unwrap();
            kernel.map.statements.len()
                + kernel
                    .map
                    .iterations
                    .iter()
                    .map(|iteration| iteration.body.len())
                    .sum::<usize>()
        };

        let counts = (kernel(8), kernel(65_536));

        assert_eq!(counts, (20 + 17, 20 + 17));
    }
}
