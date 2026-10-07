use calc_exec::{Backend, Batch, Domain, Plan, PlanError, PrepareError, RunError};
use calc_exec_cpu::CpuBackend;

use crate::cases::{Case, EquivalentCase, ExpectedCase};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConformanceError {
    ReferencePrepare(PrepareError),
    ReferenceRun(RunError),
    Prepare(PrepareError),
    Run(RunError),
    Mismatch {
        channel: usize,
        element: usize,
        expected_bits: u64,
        actual_bits: u64,
    },
    OutsideBound {
        channel: usize,
        element: usize,
        expected_bits: u64,
        actual_bits: u64,
    },
    Unrolling(PlanError),
}

enum StageError {
    Prepare(PrepareError),
    Run(RunError),
}

pub fn bits_match_f64(expected: f64, actual: f64) -> bool {
    (expected.is_nan() && actual.is_nan()) || expected.to_bits() == actual.to_bits()
}

pub fn bits_match_f32(expected: f32, actual: f32) -> bool {
    (expected.is_nan() && actual.is_nan()) || expected.to_bits() == actual.to_bits()
}

pub fn check_bitwise(backend: &dyn Backend, case: &Case) -> Result<(), ConformanceError> {
    let (expected, actual) = run_both(backend, case)?;
    match first_mismatch(&expected, &actual) {
        Some((channel, element, expected_bits, actual_bits)) => Err(ConformanceError::Mismatch {
            channel,
            element,
            expected_bits,
            actual_bits,
        }),
        None => Ok(()),
    }
}

pub fn check_unrolled(backend: &dyn Backend, case: &Case) -> Result<(), ConformanceError> {
    let unrolled = case.plan.unrolled().map_err(ConformanceError::Unrolling)?;
    compare_plans(backend, None, &unrolled, &case.plan, &case.inputs)
}

pub fn check_equivalent(
    backend: &dyn Backend,
    equivalent: &EquivalentCase,
) -> Result<(), ConformanceError> {
    compare_plans(
        backend,
        None,
        &equivalent.reference_plan,
        &equivalent.case.plan,
        &equivalent.case.inputs,
    )
}

pub(crate) fn compare_plans(
    backend: &dyn Backend,
    reference: Option<&dyn Backend>,
    reference_plan: &Plan,
    tested_plan: &Plan,
    inputs: &Batch,
) -> Result<(), ConformanceError> {
    let default_reference = CpuBackend::new();
    let reference = reference.unwrap_or(&default_reference);
    let expected = run(reference, reference_plan, inputs).map_err(|error| match error {
        StageError::Prepare(error) => ConformanceError::ReferencePrepare(error),
        StageError::Run(error) => ConformanceError::ReferenceRun(error),
    })?;
    let actual = run(backend, tested_plan, inputs).map_err(|error| match error {
        StageError::Prepare(error) => ConformanceError::Prepare(error),
        StageError::Run(error) => ConformanceError::Run(error),
    })?;
    match first_mismatch(&expected, &actual) {
        Some((channel, element, expected_bits, actual_bits)) => Err(ConformanceError::Mismatch {
            channel,
            element,
            expected_bits,
            actual_bits,
        }),
        None => Ok(()),
    }
}

pub fn check_bound(backend: &dyn Backend, case: &Case) -> Result<(), ConformanceError> {
    let (expected, actual) = run_both(backend, case)?;
    match first_mismatch(&expected, &actual) {
        Some((channel, element, expected_bits, actual_bits)) => {
            Err(ConformanceError::OutsideBound {
                channel,
                element,
                expected_bits,
                actual_bits,
            })
        }
        None => Ok(()),
    }
}

pub fn check_expected(backend: &dyn Backend, case: &ExpectedCase) -> Result<(), ConformanceError> {
    let actual = run(backend, &case.plan, &case.inputs).map_err(|error| match error {
        StageError::Prepare(error) => ConformanceError::Prepare(error),
        StageError::Run(error) => ConformanceError::Run(error),
    })?;
    let mismatch = match case.expected.domain() {
        Domain::F64 => first_mismatch(&case.expected, &actual),
        Domain::F32 => first_f32_outside_neighbours(&case.expected, &actual),
    };
    match mismatch {
        Some((channel, element, expected_bits, actual_bits)) => {
            Err(ConformanceError::OutsideBound {
                channel,
                element,
                expected_bits,
                actual_bits,
            })
        }
        None => Ok(()),
    }
}

fn first_f32_outside_neighbours(
    expected: &Batch,
    actual: &Batch,
) -> Option<(usize, usize, u64, u64)> {
    (0..expected.channel_count()).find_map(|channel| {
        let expected_values = expected.f32_channel(channel).unwrap_or(&[]);
        let actual_values = actual.f32_channel(channel).unwrap_or(&[]);
        expected_values
            .iter()
            .zip(actual_values)
            .enumerate()
            .find(|(_, (expected, actual))| {
                let is_neighbour = bits_match_f32(**expected, **actual)
                    || bits_match_f32(expected.next_up(), **actual)
                    || bits_match_f32(expected.next_down(), **actual);
                !is_neighbour
            })
            .map(|(element, (expected, actual))| {
                (
                    channel,
                    element,
                    u64::from(expected.to_bits()),
                    u64::from(actual.to_bits()),
                )
            })
    })
}

fn run_both(backend: &dyn Backend, case: &Case) -> Result<(Batch, Batch), ConformanceError> {
    let expected =
        run(&CpuBackend::new(), &case.plan, &case.inputs).map_err(|error| match error {
            StageError::Prepare(error) => ConformanceError::ReferencePrepare(error),
            StageError::Run(error) => ConformanceError::ReferenceRun(error),
        })?;
    let actual = run(backend, &case.plan, &case.inputs).map_err(|error| match error {
        StageError::Prepare(error) => ConformanceError::Prepare(error),
        StageError::Run(error) => ConformanceError::Run(error),
    })?;
    Ok((expected, actual))
}

fn run(backend: &dyn Backend, plan: &Plan, inputs: &Batch) -> Result<Batch, StageError> {
    let mut prepared = backend.prepare(plan).map_err(StageError::Prepare)?;
    let mut outputs = Batch::zeroed(
        plan.domain(),
        plan.output_channel_count(),
        plan.output_length(inputs.len()),
    );
    prepared
        .run(inputs, &mut outputs)
        .map_err(StageError::Run)?;
    Ok(outputs)
}

fn channel_bits(expected: &Batch, actual: &Batch, channel: usize) -> Vec<(u64, u64, bool)> {
    match expected.domain() {
        Domain::F32 => {
            let expected = expected.f32_channel(channel).unwrap_or(&[]);
            let actual = actual.f32_channel(channel).unwrap_or(&[]);
            expected
                .iter()
                .zip(actual)
                .map(|(expected, actual)| {
                    (
                        u64::from(expected.to_bits()),
                        u64::from(actual.to_bits()),
                        bits_match_f32(*expected, *actual),
                    )
                })
                .collect()
        }
        Domain::F64 => {
            let expected = expected.f64_channel(channel).unwrap_or(&[]);
            let actual = actual.f64_channel(channel).unwrap_or(&[]);
            expected
                .iter()
                .zip(actual)
                .map(|(expected, actual)| {
                    (
                        expected.to_bits(),
                        actual.to_bits(),
                        bits_match_f64(*expected, *actual),
                    )
                })
                .collect()
        }
    }
}

fn first_mismatch(expected: &Batch, actual: &Batch) -> Option<(usize, usize, u64, u64)> {
    (0..expected.channel_count()).find_map(|channel| {
        channel_bits(expected, actual, channel)
            .into_iter()
            .enumerate()
            .find(|(_, (_, _, matches))| !matches)
            .map(|(element, (expected_bits, actual_bits, _))| {
                (channel, element, expected_bits, actual_bits)
            })
    })
}
