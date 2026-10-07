mod cases;
mod check;
mod workers;

pub use cases::{
    Case, ESCAPE_TIME_CASE_ITERATIONS, EquivalentCase, ExpectedCase, ITERATION_CASE_MAXIMUM_COUNT,
    REDUCE_LENGTHS, constant_chain_case, contraction_case, corner_cases, escape_time_cases,
    escape_time_equivalent_cases, gpu_probe_case, gpu_probe_values_f32, iteration_cases,
    operation_case, reduce_case, special_values_f32, special_values_f64,
};
pub use check::{
    ConformanceError, bits_match_f32, bits_match_f64, check_bitwise, check_bound, check_equivalent,
    check_expected, check_unrolled,
};
pub use workers::{PermutedParallelism, WORKER_COUNTS, check_worker_counts};

#[cfg(test)]
mod tests {
    use super::*;
    use calc_exec::{
        Backend, BackendKind, Batch, BodyGate, BodyGateId, Capabilities, Domain, Gate,
        OperationClass, Plan, PlanOp, PrepareError, Prepared, Reduce, ReduceOperation, ReduceShape,
        RunError, RunReport, SlotId, Unsupported,
    };
    use calc_exec_cpu::CpuBackend;

    #[derive(Clone, Copy)]
    enum Fault {
        LeftFoldAsHalving,
        ContractedMultiplyAdd,
        PositiveZeroForNegativeZero,
        FlippedNanSign,
        RefusedPrepare,
        LostDevice,
        OneUlpAbove,
        NegatedResult,
        IgnoredExit,
        OneStepMore,
    }

    struct FaultyBackend {
        reference: CpuBackend,
        fault: Fault,
    }

    struct FaultyPrepared {
        inner: Box<dyn Prepared>,
        fault: Fault,
    }

    fn faulty(fault: Fault) -> FaultyBackend {
        FaultyBackend {
            reference: CpuBackend::new(),
            fault,
        }
    }

    fn contracted(plan: &Plan) -> Plan {
        let gates: Vec<Gate> = plan
            .gates()
            .iter()
            .map(|gate| match gate {
                Gate::Operation {
                    operation: PlanOp::Add,
                    arguments,
                } => match plan.gates().get(arguments[0].index()) {
                    Some(Gate::Operation {
                        operation: PlanOp::Mul,
                        arguments: factors,
                    }) => Gate::Operation {
                        operation: PlanOp::MulAdd,
                        arguments: vec![factors[0], factors[1], arguments[1]],
                    },
                    _ => gate.clone(),
                },
                _ => gate.clone(),
            })
            .collect();
        Plan::new(
            plan.domain(),
            plan.input_channel_count(),
            gates,
            plan.outputs().to_vec(),
            plan.reduce(),
        )
        .unwrap()
    }

    fn reshaped(plan: &Plan) -> Plan {
        let reduce = plan.reduce().map(|reduce| Reduce {
            operation: reduce.operation,
            shape: ReduceShape::Halving,
        });
        Plan::new(
            plan.domain(),
            plan.input_channel_count(),
            plan.gates().to_vec(),
            plan.outputs().to_vec(),
            reduce,
        )
        .unwrap()
    }

    fn rewritten_count(plan: &Plan, maximum_count: u32) -> Plan {
        let gates: Vec<Gate> = plan
            .gates()
            .iter()
            .map(|gate| match gate {
                Gate::Iterate(iteration) => {
                    let mut changed = (**iteration).clone();
                    changed.maximum_count = maximum_count;
                    Gate::Iterate(Box::new(changed))
                }
                _ => gate.clone(),
            })
            .collect();
        Plan::new(
            plan.domain(),
            plan.input_channel_count(),
            gates,
            plan.outputs().to_vec(),
            plan.reduce(),
        )
        .unwrap()
    }

    fn rewritten_iterations(plan: &Plan, fault: Fault) -> Plan {
        let gates: Vec<Gate> = plan
            .gates()
            .iter()
            .map(|gate| match gate {
                Gate::Iterate(iteration) => {
                    let mut changed = (**iteration).clone();
                    match fault {
                        Fault::OneStepMore => changed.maximum_count += 1,
                        _ => {
                            let body_id = |offset: usize| {
                                BodyGateId(u32::try_from(changed.body.len() + offset).unwrap())
                            };
                            let state = body_id(0);
                            let is_nan = body_id(1);
                            let is_not_nan = body_id(2);
                            let never = body_id(3);
                            changed.body.push(BodyGate::State(SlotId(0)));
                            changed.body.push(BodyGate::Operation {
                                operation: PlanOp::NotEqual,
                                arguments: vec![state, state],
                            });
                            changed.body.push(BodyGate::Operation {
                                operation: PlanOp::Not,
                                arguments: vec![is_nan],
                            });
                            changed.body.push(BodyGate::Operation {
                                operation: PlanOp::And,
                                arguments: vec![is_nan, is_not_nan],
                            });
                            changed.exit = never;
                        }
                    }
                    Gate::Iterate(Box::new(changed))
                }
                _ => gate.clone(),
            })
            .collect();
        Plan::new(
            plan.domain(),
            plan.input_channel_count(),
            gates,
            plan.outputs().to_vec(),
            plan.reduce(),
        )
        .unwrap()
    }

    impl Backend for FaultyBackend {
        fn kind(&self) -> BackendKind {
            BackendKind::Simd
        }

        fn capabilities(&self) -> &Capabilities {
            self.reference.capabilities()
        }

        fn prepare(&self, plan: &Plan) -> Result<Box<dyn Prepared>, PrepareError> {
            let rewritten = match self.fault {
                Fault::LeftFoldAsHalving => reshaped(plan),
                Fault::ContractedMultiplyAdd => contracted(plan),
                Fault::RefusedPrepare => {
                    return Err(PrepareError::Unsupported(Unsupported::Domain(
                        plan.domain(),
                    )));
                }
                Fault::PositiveZeroForNegativeZero
                | Fault::FlippedNanSign
                | Fault::LostDevice
                | Fault::OneUlpAbove
                | Fault::NegatedResult => plan.clone(),
                Fault::IgnoredExit | Fault::OneStepMore => rewritten_iterations(plan, self.fault),
            };
            Ok(Box::new(FaultyPrepared {
                inner: self.reference.prepare(&rewritten)?,
                fault: self.fault,
            }))
        }
    }

    impl Prepared for FaultyPrepared {
        fn run(&mut self, inputs: &Batch, outputs: &mut Batch) -> Result<RunReport, RunError> {
            if matches!(self.fault, Fault::LostDevice) {
                return Err(RunError::DeviceLost);
            }
            let report = self.inner.run(inputs, outputs)?;
            let replace = |value: f64| match self.fault {
                Fault::PositiveZeroForNegativeZero if value.to_bits() == (-0.0_f64).to_bits() => {
                    0.0
                }
                Fault::FlippedNanSign if value.is_nan() => -value,
                Fault::OneUlpAbove if value.is_finite() => value.next_up(),
                Fault::NegatedResult => -value,
                _ => value,
            };
            for channel in 0..outputs.channel_count() {
                if let Some(values) = outputs.f64_channel_mut(channel) {
                    values.iter_mut().for_each(|value| *value = replace(*value));
                }
            }
            Ok(report)
        }
    }

    #[test]
    fn reference_passes_its_own_operation_cases() {
        let failures: Vec<(PlanOp, Domain)> = PlanOp::ALL
            .into_iter()
            .flat_map(|operation| [(operation, Domain::F32), (operation, Domain::F64)])
            .filter(|(operation, domain)| {
                let case = operation_case(*operation, *domain).unwrap();
                match operation.class() {
                    OperationClass::CorrectlyRounded => check_bitwise(&CpuBackend::new(), &case),
                    OperationClass::Approximate => check_bound(&CpuBackend::new(), &case),
                }
                .is_err()
            })
            .collect();

        assert!(failures.is_empty(), "{failures:?}");
    }

    const SPLIT_CHUNK_LENGTH: usize = 7;

    fn split_cpu(parallelism: calc_exec::SharedParallelism) -> Box<dyn Backend> {
        Box::new(CpuBackend::with_chunk_length(
            parallelism,
            std::num::NonZeroUsize::new(SPLIT_CHUNK_LENGTH).unwrap(),
        ))
    }

    fn worker_count_failures(cases: &[Case]) -> usize {
        cases
            .iter()
            .flat_map(|case| [1_u64, 2].map(|seed| (case, seed)))
            .filter(|(case, seed)| check_worker_counts(&split_cpu, case, *seed).is_err())
            .count()
    }

    #[test]
    fn operation_cases_do_not_depend_on_the_worker_count() {
        let cases: Vec<Case> = PlanOp::ALL
            .into_iter()
            .flat_map(|operation| [(operation, Domain::F32), (operation, Domain::F64)])
            .filter_map(|(operation, domain)| operation_case(operation, domain))
            .collect();

        let failures = worker_count_failures(&cases);

        assert_eq!((cases.len(), failures), (2 * PlanOp::ALL.len(), 0));
    }

    #[test]
    fn reduce_cases_do_not_depend_on_the_worker_count() {
        let cases: Vec<Case> = [ReduceOperation::Add, ReduceOperation::Max]
            .into_iter()
            .flat_map(|operation| {
                [ReduceShape::LeftFold, ReduceShape::Halving].map(|shape| (operation, shape))
            })
            .filter_map(|(operation, shape)| reduce_case(operation, shape, 1025, Domain::F64))
            .collect();

        let failures = worker_count_failures(&cases);

        assert_eq!((cases.len(), failures), (4, 0));
    }

    #[test]
    fn iteration_cases_do_not_depend_on_the_worker_count() {
        let cases: Vec<Case> = [Domain::F32, Domain::F64]
            .into_iter()
            .flat_map(iteration_cases)
            .chain(
                [Domain::F32, Domain::F64]
                    .into_iter()
                    .flat_map(|domain| {
                        escape_time_equivalent_cases(domain, ESCAPE_TIME_CASE_ITERATIONS)
                    })
                    .map(|equivalent| equivalent.case),
            )
            .collect();

        let failures = worker_count_failures(&cases);

        assert_eq!((cases.len(), failures), (14, 0));
    }

    #[test]
    fn worker_count_check_notices_a_backend_that_changes_with_workers() {
        let case = operation_case(PlanOp::Add, Domain::F64).unwrap();
        let build = |parallelism: calc_exec::SharedParallelism| -> Box<dyn Backend> {
            if parallelism.worker_count().get() > 2 {
                Box::new(faulty(Fault::NegatedResult))
            } else {
                split_cpu(parallelism)
            }
        };

        let result = check_worker_counts(&build, &case, 1);

        assert!(matches!(result, Err(ConformanceError::Mismatch { .. })));
    }

    #[test]
    fn reference_passes_every_iteration_case_in_both_domains() {
        let cases: Vec<Case> = [Domain::F32, Domain::F64]
            .into_iter()
            .flat_map(iteration_cases)
            .collect();

        let failures = cases
            .iter()
            .filter(|case| check_unrolled(&CpuBackend::new(), case).is_err())
            .count();

        assert_eq!((cases.len(), failures), (10, 0));
    }

    #[test]
    fn iteration_form_of_escape_time_matches_the_unrolled_circuit() {
        let cases: Vec<EquivalentCase> = [Domain::F32, Domain::F64]
            .into_iter()
            .flat_map(|domain| escape_time_equivalent_cases(domain, ESCAPE_TIME_CASE_ITERATIONS))
            .collect();

        let failures = cases
            .iter()
            .filter(|case| check_equivalent(&CpuBackend::new(), case).is_err())
            .count();

        assert_eq!((cases.len(), failures), (4, 0));
    }

    #[test]
    fn iteration_form_of_escape_time_matches_with_zero_iterations() {
        let cases = escape_time_equivalent_cases(Domain::F64, 0);

        let failures = cases
            .iter()
            .filter(|case| check_equivalent(&CpuBackend::new(), case).is_err())
            .count();

        assert_eq!((cases.len(), failures), (2, 0));
    }

    #[test]
    fn unrolled_check_notices_a_backend_that_ignores_the_exit() {
        let case = iteration_cases(Domain::F64).remove(0);

        let result = check_unrolled(&faulty(Fault::IgnoredExit), &case);

        assert!(matches!(result, Err(ConformanceError::Mismatch { .. })));
    }

    #[test]
    fn unrolled_check_notices_a_backend_that_takes_one_step_more() {
        let case = iteration_cases(Domain::F64).remove(0);

        let result = check_unrolled(&faulty(Fault::OneStepMore), &case);

        assert!(matches!(result, Err(ConformanceError::Mismatch { .. })));
    }

    #[test]
    fn unrolled_check_notices_one_step_more_in_a_boolean_slot() {
        let case = iteration_cases(Domain::F64).remove(3);

        let result = check_unrolled(&faulty(Fault::OneStepMore), &case);

        assert!(matches!(result, Err(ConformanceError::Mismatch { .. })));
    }

    #[test]
    fn unrolled_check_is_bitwise_for_an_approximate_body() {
        let case = iteration_cases(Domain::F64).remove(4);

        let result = check_unrolled(&faulty(Fault::OneUlpAbove), &case);

        assert!(matches!(result, Err(ConformanceError::Mismatch { .. })));
    }

    #[test]
    fn unrolled_check_reports_a_plan_too_large_to_unroll() {
        let mut case = iteration_cases(Domain::F64).remove(0);
        case.plan = rewritten_count(&case.plan, u32::MAX);

        let result = check_unrolled(&CpuBackend::new(), &case);

        assert!(matches!(
            result,
            Err(ConformanceError::Unrolling(
                calc_exec::PlanError::UnrolledTooLarge { .. }
            ))
        ));
    }

    #[test]
    fn equivalence_check_notices_a_negated_iteration_result() {
        let case = escape_time_equivalent_cases(Domain::F64, ESCAPE_TIME_CASE_ITERATIONS).remove(0);

        let result = check_equivalent(&faulty(Fault::NegatedResult), &case);

        assert!(matches!(result, Err(ConformanceError::Mismatch { .. })));
    }

    #[test]
    fn reference_passes_the_escape_time_cases_in_both_domains() {
        let cases: Vec<ExpectedCase> = [Domain::F32, Domain::F64]
            .into_iter()
            .flat_map(escape_time_cases)
            .collect();

        let failures = cases
            .iter()
            .filter(|case| check_expected(&CpuBackend::new(), case).is_err())
            .count();

        assert_eq!((cases.len(), failures), (6, 0));
    }

    #[test]
    fn escape_time_case_notices_a_negated_result() {
        let case = escape_time_cases(Domain::F64).remove(0);

        let result = check_expected(&faulty(Fault::NegatedResult), &case);

        assert!(result.is_err());
    }

    #[test]
    fn reference_passes_the_f64_corner_cases() {
        let failures: Vec<PlanOp> = corner_cases(Domain::F64)
            .into_iter()
            .filter(|(_, case)| check_expected(&CpuBackend::new(), case).is_err())
            .map(|(operation, _)| operation)
            .collect();

        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn f32_corner_expectations_match_the_f32_references() {
        for (operation, case) in corner_cases(Domain::F32) {
            let numerators = case.inputs.f32_channel(0).unwrap();
            let second = case.inputs.f32_channel(1).unwrap();
            let expected = case.expected.f32_channel(0).unwrap();
            for ((left, right), expected) in numerators.iter().zip(second).zip(expected) {
                let reference = match operation {
                    PlanOp::Atan2 => calc_numbers::atan2_f32(*left, *right),
                    _ => calc_numbers::pow_f32(*left, *right),
                };
                assert_eq!(
                    reference.to_bits(),
                    expected.to_bits(),
                    "{operation:?} {left} {right}"
                );
            }
        }
    }

    #[test]
    fn corner_check_notices_a_flipped_sign() {
        let case = corner_cases(Domain::F64)
            .into_iter()
            .find(|(operation, _)| *operation == PlanOp::Atan2)
            .unwrap()
            .1;

        let result = check_expected(&faulty(Fault::NegatedResult), &case);

        assert!(matches!(result, Err(ConformanceError::OutsideBound { .. })));
    }

    #[test]
    fn every_operation_has_a_case_in_both_domains() {
        let missing = PlanOp::ALL
            .into_iter()
            .flat_map(|operation| [(operation, Domain::F32), (operation, Domain::F64)])
            .filter(|(operation, domain)| operation_case(*operation, *domain).is_none())
            .count();

        assert_eq!(missing, 0);
    }

    #[test]
    fn ternary_case_covers_every_combination_of_special_values() {
        let case = operation_case(PlanOp::MulAdd, Domain::F64).unwrap();

        let length = case.inputs.len();

        assert_eq!(length, special_values_f64().len().pow(3));
    }

    #[test]
    fn reduce_cases_exist_for_every_operation_shape_and_length() {
        let operations = [
            ReduceOperation::Add,
            ReduceOperation::Mul,
            ReduceOperation::Min,
            ReduceOperation::Max,
        ];
        let shapes = [ReduceShape::LeftFold, ReduceShape::Halving];

        let failures = operations
            .iter()
            .flat_map(|operation| shapes.iter().map(move |shape| (*operation, *shape)))
            .flat_map(|(operation, shape)| {
                REDUCE_LENGTHS.iter().flat_map(move |length| {
                    [Domain::F32, Domain::F64].map(|domain| (operation, shape, *length, domain))
                })
            })
            .filter(|(operation, shape, length, domain)| {
                reduce_case(*operation, *shape, *length, *domain)
                    .is_none_or(|case| check_bitwise(&CpuBackend::new(), &case).is_err())
            })
            .count();

        assert_eq!(failures, 0);
    }

    #[test]
    fn check_notices_left_fold_evaluated_as_halving() {
        let case =
            reduce_case(ReduceOperation::Add, ReduceShape::LeftFold, 5, Domain::F64).unwrap();

        let result = check_bitwise(&faulty(Fault::LeftFoldAsHalving), &case);

        assert!(matches!(result, Err(ConformanceError::Mismatch { .. })));
    }

    #[test]
    fn constant_chain_differs_when_folded_in_double_precision() {
        let case = constant_chain_case(Domain::F32).unwrap();
        let mut outputs = Batch::zeroed(Domain::F32, 1, 1);
        let mut prepared = CpuBackend::new().prepare(&case.plan).unwrap();

        prepared.run(&case.inputs, &mut outputs).unwrap();

        assert_eq!(
            outputs.f32_channel(0).unwrap()[0].to_bits(),
            1.0_f32.next_up().to_bits()
        );
    }

    #[test]
    fn check_notices_contracted_multiply_add() {
        let case = contraction_case(Domain::F64).unwrap();

        let result = check_bitwise(&faulty(Fault::ContractedMultiplyAdd), &case);

        assert!(matches!(result, Err(ConformanceError::Mismatch { .. })));
    }

    #[test]
    fn check_notices_positive_zero_returned_for_negative_zero() {
        let case = operation_case(PlanOp::Neg, Domain::F64).unwrap();

        let result = check_bitwise(&faulty(Fault::PositiveZeroForNegativeZero), &case);

        assert!(matches!(result, Err(ConformanceError::Mismatch { .. })));
    }

    #[test]
    fn check_accepts_nan_with_other_sign() {
        let case = operation_case(PlanOp::Neg, Domain::F64).unwrap();

        let result = check_bitwise(&faulty(Fault::FlippedNanSign), &case);

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn check_reports_tested_backend_that_cannot_prepare() {
        let case = operation_case(PlanOp::Neg, Domain::F64).unwrap();

        let result = check_bitwise(&faulty(Fault::RefusedPrepare), &case);

        assert_eq!(
            result,
            Err(ConformanceError::Prepare(PrepareError::Unsupported(
                Unsupported::Domain(Domain::F64)
            )))
        );
    }

    #[test]
    fn check_reports_tested_backend_that_fails_to_run() {
        let case = operation_case(PlanOp::Neg, Domain::F64).unwrap();

        let result = check_bitwise(&faulty(Fault::LostDevice), &case);

        assert_eq!(result, Err(ConformanceError::Run(RunError::DeviceLost)));
    }

    #[test]
    fn check_reports_reference_that_fails_to_run() {
        let case = reduce_case(ReduceOperation::Add, ReduceShape::Halving, 0, Domain::F64).unwrap();

        let result = check_bitwise(&CpuBackend::new(), &case);

        assert_eq!(
            result,
            Err(ConformanceError::ReferenceRun(RunError::EmptyReduction))
        );
    }

    #[test]
    fn bound_check_notices_result_one_ulp_away_from_reference() {
        let case = operation_case(PlanOp::Exp, Domain::F64).unwrap();

        let result = check_bound(&faulty(Fault::OneUlpAbove), &case);

        assert!(matches!(result, Err(ConformanceError::OutsideBound { .. })));
    }

    #[test]
    fn bits_match_rejects_opposite_zeros() {
        let matches = bits_match_f32(0.0, -0.0);

        assert!(!matches);
    }
}
