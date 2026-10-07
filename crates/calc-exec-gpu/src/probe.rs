use calc_exec::{Backend, Batch, Plan, PlanOp, PrepareError};

use crate::backend::GpuBackend;
use crate::device::GpuDevice;
use crate::modes::{ELIGIBLE_FOR_NATIVE, EXACT_IN_BOTH_FORMS, OperationModes};

#[derive(Clone, Copy, Debug)]
pub struct ProbeCase<'case> {
    pub operation: Option<PlanOp>,
    pub plan: &'case Plan,
    pub inputs: &'case Batch,
    pub expected: &'case Batch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProbeRefusal {
    ExactCaseFailed(Option<PlanOp>),
    CaseNotRun(Option<PlanOp>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationMode {
    Native,
    IntegerExact,
    ExactInBothForms,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationOutcome {
    pub operation: PlanOp,
    pub mode: OperationMode,
    pub cases: usize,
    pub subnormal_cases: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeReport {
    pub modes: OperationModes,
    pub operations: Vec<OperationOutcome>,
    pub cases_run: usize,
}

impl ProbeReport {
    pub fn exact_operations(&self) -> Vec<PlanOp> {
        self.operations
            .iter()
            .filter(|outcome| outcome.mode == OperationMode::IntegerExact)
            .map(|outcome| outcome.operation)
            .collect()
    }
}

fn carries_a_subnormal(batch: &Batch) -> bool {
    (0..batch.channel_count()).any(|channel| {
        batch.f32_channel(channel).is_some_and(|values| {
            values
                .iter()
                .any(|value| *value != 0.0 && value.abs() < f32::MIN_POSITIVE)
        })
    })
}

fn bits_match(found: f32, wanted: f32) -> bool {
    found.to_bits() == wanted.to_bits() || (found.is_nan() && wanted.is_nan())
}

fn batch_matches(found: &Batch, wanted: &Batch) -> bool {
    if found.channel_count() != wanted.channel_count() || found.len() != wanted.len() {
        return false;
    }
    (0..wanted.channel_count()).all(|channel| {
        match (found.f32_channel(channel), wanted.f32_channel(channel)) {
            (Some(left), Some(right)) => left.iter().zip(right).all(|(a, b)| bits_match(*a, *b)),
            _ => false,
        }
    })
}

fn run_case<D: GpuDevice>(
    device: &D,
    case: &ProbeCase<'_>,
    modes: OperationModes,
) -> Result<Option<bool>, ProbeRefusal> {
    let backend = GpuBackend::with_modes(device.clone(), modes);
    let mut prepared = match backend.prepare(case.plan) {
        Ok(prepared) => prepared,
        Err(PrepareError::Unsupported(_)) => return Ok(None),
        Err(_) => return Err(ProbeRefusal::CaseNotRun(case.operation)),
    };
    let mut found = Batch::zeroed(
        case.expected.domain(),
        case.expected.channel_count(),
        case.expected.len(),
    );
    match prepared.run(case.inputs, &mut found) {
        Ok(_) => Ok(Some(batch_matches(&found, case.expected))),
        Err(_) => Err(ProbeRefusal::CaseNotRun(case.operation)),
    }
}

pub fn probe<D: GpuDevice>(
    device: &D,
    cases: &[ProbeCase<'_>],
) -> Result<ProbeReport, ProbeRefusal> {
    let mut cases_run = 0;
    let mut exact_counts: Vec<(PlanOp, usize)> = Vec::new();
    for case in cases {
        match run_case(device, case, OperationModes::EXACT)? {
            None => continue,
            Some(true) => {
                cases_run += 1;
                if let Some(operation) = case.operation {
                    match exact_counts.iter_mut().find(|found| found.0 == operation) {
                        Some(found) => found.1 += 1,
                        None => exact_counts.push((operation, 1)),
                    }
                }
            }
            Some(false) => return Err(ProbeRefusal::ExactCaseFailed(case.operation)),
        }
    }
    let exact_cases = |operation: PlanOp| {
        exact_counts
            .iter()
            .find(|found| found.0 == operation)
            .map_or(0, |found| found.1)
    };
    let mut modes = OperationModes::EXACT;
    let mut operations = Vec::new();
    for operation in ELIGIBLE_FOR_NATIVE {
        let native = OperationModes::EXACT.with_native(operation);
        let mut ran = 0;
        let mut with_subnormal = 0;
        let mut approved = true;
        for case in cases
            .iter()
            .filter(|case| case.operation == Some(operation))
        {
            match run_case(device, case, native)? {
                None => continue,
                Some(matched) => {
                    ran += 1;
                    approved &= matched;
                    if matched && carries_a_subnormal(case.inputs) {
                        with_subnormal += 1;
                    }
                }
            }
        }
        let native_approved = ran > 0 && approved && with_subnormal > 0;
        if native_approved {
            modes = modes.with_native(operation);
        }
        operations.push(OperationOutcome {
            operation,
            mode: if native_approved {
                OperationMode::Native
            } else {
                OperationMode::IntegerExact
            },
            cases: if native_approved {
                ran
            } else {
                exact_cases(operation)
            },
            subnormal_cases: with_subnormal,
        });
    }
    for operation in EXACT_IN_BOTH_FORMS {
        operations.push(OperationOutcome {
            operation,
            mode: OperationMode::ExactInBothForms,
            cases: exact_cases(operation),
            subnormal_cases: 0,
        });
    }
    for operation in [
        PlanOp::Add,
        PlanOp::Sub,
        PlanOp::Mul,
        PlanOp::Div,
        PlanOp::Sqrt,
    ] {
        operations.push(OperationOutcome {
            operation,
            mode: OperationMode::IntegerExact,
            cases: exact_cases(operation),
            subnormal_cases: 0,
        });
    }
    Ok(ProbeReport {
        modes,
        operations,
        cases_run,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::software::{SoftwareDevice, SoftwareFault};
    use calc_conformance::operation_case;
    use calc_exec::Domain;

    struct Case {
        operation: PlanOp,
        plan: Plan,
        inputs: Batch,
        expected: Batch,
    }

    fn strip_subnormals(inputs: &Batch) -> Batch {
        let columns: Vec<Vec<f32>> = (0..inputs.channel_count())
            .map(|channel| {
                inputs
                    .f32_channel(channel)
                    .expect("channel")
                    .iter()
                    .map(|value| {
                        if *value != 0.0 && value.abs() < f32::MIN_POSITIVE {
                            0.0_f32.copysign(*value)
                        } else {
                            *value
                        }
                    })
                    .collect()
            })
            .collect();
        Batch::from_f32_columns(inputs.len(), columns).expect("batch")
    }

    fn reference(plan: &Plan, inputs: &Batch) -> Batch {
        let cpu = calc_exec_cpu::CpuBackend::new();
        let mut expected = Batch::zeroed(Domain::F32, plan.output_channel_count(), inputs.len());
        cpu.prepare(plan)
            .expect("prepare")
            .run(inputs, &mut expected)
            .expect("run");
        expected
    }

    fn cases() -> Vec<Case> {
        let cpu = calc_exec_cpu::CpuBackend::new();
        let mut found = Vec::new();
        for operation in ELIGIBLE_FOR_NATIVE
            .into_iter()
            .chain([PlanOp::Add, PlanOp::Mul])
        {
            let Some(case) = operation_case(operation, Domain::F32) else {
                continue;
            };
            let mut expected = Batch::zeroed(
                Domain::F32,
                case.plan.output_channel_count(),
                case.inputs.len(),
            );
            cpu.prepare(&case.plan)
                .expect("prepare")
                .run(&case.inputs, &mut expected)
                .expect("run");
            found.push(Case {
                operation,
                plan: case.plan,
                inputs: case.inputs,
                expected,
            });
        }
        found
    }

    fn probe_cases(cases: &[Case]) -> Vec<ProbeCase<'_>> {
        cases
            .iter()
            .map(|case| ProbeCase {
                operation: Some(case.operation),
                plan: &case.plan,
                inputs: &case.inputs,
                expected: &case.expected,
            })
            .collect()
    }

    #[test]
    fn an_exact_device_runs_every_supported_case() {
        let cases = cases();

        let report = probe(&SoftwareDevice::new(1), &probe_cases(&cases)).expect("report");

        assert!(report.cases_run > 0);
    }

    #[test]
    fn an_exact_device_may_run_every_eligible_operation_natively() {
        let cases = cases();

        let report = probe(&SoftwareDevice::new(1), &probe_cases(&cases)).expect("report");

        let still_exact: Vec<PlanOp> = report
            .exact_operations()
            .into_iter()
            .filter(|operation| OperationModes::eligible(*operation))
            .collect();
        assert_eq!(still_exact, Vec::new());
    }

    #[test]
    fn a_native_mode_names_the_cases_it_rests_on() {
        let cases = cases();

        let report = probe(&SoftwareDevice::new(1), &probe_cases(&cases)).expect("report");

        let approvals: Vec<&OperationOutcome> = report
            .operations
            .iter()
            .filter(|outcome| outcome.mode == OperationMode::Native)
            .collect();
        assert!(
            !approvals.is_empty()
                && approvals
                    .iter()
                    .all(|outcome| outcome.subnormal_cases > 0 && outcome.cases > 0)
        );
    }

    #[test]
    fn an_operation_whose_cases_carry_no_subnormal_stays_exact() {
        let cases = cases();
        let mut without_subnormals: Vec<Case> = Vec::new();
        for case in cases {
            let inputs = strip_subnormals(&case.inputs);
            let expected = reference(&case.plan, &inputs);
            without_subnormals.push(Case {
                operation: case.operation,
                plan: case.plan,
                inputs,
                expected,
            });
        }

        let report =
            probe(&SoftwareDevice::new(1), &probe_cases(&without_subnormals)).expect("report");

        assert_eq!(report.modes, OperationModes::EXACT);
    }

    #[test]
    fn an_operation_that_carries_no_floating_point_operation_is_exact_in_both_forms() {
        let cases = cases();

        let report = probe(&SoftwareDevice::new(1), &probe_cases(&cases)).expect("report");

        let found = report
            .operations
            .iter()
            .find(|outcome| outcome.operation == PlanOp::CopySign)
            .map(|outcome| outcome.mode);
        assert_eq!(found, Some(OperationMode::ExactInBothForms));
    }

    #[test]
    fn an_operation_with_no_case_is_never_native() {
        let cases = cases();
        let without_rounding: Vec<ProbeCase<'_>> = probe_cases(&cases)
            .into_iter()
            .filter(|case| case.operation != Some(PlanOp::Floor))
            .collect();

        let report = probe(&SoftwareDevice::new(1), &without_rounding).expect("report");

        assert!(!report.modes.is_native(PlanOp::Floor));
    }

    #[test]
    fn a_device_that_flushes_subnormals_natively_keeps_those_operations_exact() {
        let cases = cases();
        let device = SoftwareDevice::with_fault(1, SoftwareFault::FlushesSubnormalsNatively);

        let report = probe(&device, &probe_cases(&cases)).expect("report");

        let eligible_and_exact = report
            .exact_operations()
            .into_iter()
            .any(OperationModes::eligible);
        assert!(eligible_and_exact);
    }

    #[test]
    fn a_device_that_fails_an_exact_case_is_refused() {
        let cases = cases();
        let device = SoftwareDevice::with_fault(1, SoftwareFault::WrongResultAtDispatch);

        let refusal = probe(&device, &probe_cases(&cases));

        assert!(matches!(refusal, Err(ProbeRefusal::ExactCaseFailed(_))));
    }

    #[test]
    fn a_device_that_cannot_run_a_case_is_refused() {
        let cases = cases();
        let device = SoftwareDevice::with_fault(1, SoftwareFault::LostAtDispatch);

        let refusal = probe(&device, &probe_cases(&cases));

        assert!(matches!(refusal, Err(ProbeRefusal::CaseNotRun(_))));
    }
}
