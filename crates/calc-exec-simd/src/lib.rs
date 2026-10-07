#![cfg(target_arch = "x86_64")]

mod element;
mod lanes;
mod program;

#[allow(unsafe_code)]
mod avx2;
#[allow(unsafe_code)]
mod sse2;

use calc_exec::{
    Backend, BackendKind, Batch, Capabilities, Domain, Plan, PrepareError, Prepared, RunError,
    RunReport, expected_transfers,
};

use crate::lanes::Lanes;
use crate::program::{Program, compile};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstructionSet {
    Sse2,
    Avx2,
}

impl InstructionSet {
    pub fn detected() -> InstructionSet {
        if is_x86_feature_detected!("avx2") {
            InstructionSet::Avx2
        } else {
            InstructionSet::Sse2
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            InstructionSet::Sse2 => "sse2",
            InstructionSet::Avx2 => "avx2",
        }
    }
}

pub struct SimdBackend {
    capabilities: Capabilities,
    instruction_set: InstructionSet,
}

impl Default for SimdBackend {
    fn default() -> SimdBackend {
        SimdBackend::new()
    }
}

impl SimdBackend {
    pub fn new() -> SimdBackend {
        SimdBackend::with_instruction_set(InstructionSet::detected())
    }

    pub fn with_instruction_set(instruction_set: InstructionSet) -> SimdBackend {
        SimdBackend {
            capabilities: Capabilities {
                domains: vec![Domain::F32, Domain::F64],
                operations: calc_exec::PlanOp::ALL.into_iter().collect(),
                largest_batch_length: usize::MAX,
                largest_iteration_count: u32::MAX,
                reference_approximate_operations: true,
                cost: calc_exec::cost::SIMD,
            },
            instruction_set,
        }
    }

    pub fn instruction_set(&self) -> InstructionSet {
        self.instruction_set
    }
}

enum Compiled {
    F32(Program<f32>),
    F64(Program<f64>),
}

pub struct SimdPrepared {
    plan: Plan,
    compiled: Compiled,
    instruction_set: InstructionSet,
}

impl Backend for SimdBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Simd
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn prepare(&self, plan: &Plan) -> Result<Box<dyn Prepared>, PrepareError> {
        self.capabilities
            .check_plan(plan)
            .map_err(PrepareError::Unsupported)?;
        let compiled = match plan.domain() {
            Domain::F32 => Compiled::F32(compile::<f32>(plan)?),
            Domain::F64 => Compiled::F64(compile::<f64>(plan)?),
        };
        Ok(Box::new(SimdPrepared {
            plan: plan.clone(),
            compiled,
            instruction_set: self.instruction_set,
        }))
    }
}

impl Prepared for SimdPrepared {
    fn run(&mut self, inputs: &Batch, outputs: &mut Batch) -> Result<RunReport, RunError> {
        let length = inputs.len();
        self.plan
            .check_batches(inputs, outputs)
            .map_err(RunError::BatchShape)?;
        match (&self.compiled, self.instruction_set) {
            (Compiled::F32(program), InstructionSet::Sse2) => {
                program.run::<sse2::F32x4>(inputs, outputs)?;
            }
            (Compiled::F64(program), InstructionSet::Sse2) => {
                program.run::<sse2::F64x2>(inputs, outputs)?;
            }
            (Compiled::F32(program), InstructionSet::Avx2) => {
                program.run::<avx2::F32x8>(inputs, outputs)?;
            }
            (Compiled::F64(program), InstructionSet::Avx2) => {
                program.run::<avx2::F64x4>(inputs, outputs)?;
            }
        }
        Ok(expected_transfers(&self.plan, length))
    }
}

pub fn lane_width(instruction_set: InstructionSet, domain: Domain) -> usize {
    match (instruction_set, domain) {
        (InstructionSet::Sse2, Domain::F32) => sse2::F32x4::WIDTH,
        (InstructionSet::Sse2, Domain::F64) => sse2::F64x2::WIDTH,
        (InstructionSet::Avx2, Domain::F32) => avx2::F32x8::WIDTH,
        (InstructionSet::Avx2, Domain::F64) => avx2::F64x4::WIDTH,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_conformance::{
        ESCAPE_TIME_CASE_ITERATIONS, REDUCE_LENGTHS, check_bitwise, check_expected,
        constant_chain_case, contraction_case, corner_cases, escape_time_cases, iteration_cases,
        operation_case, reduce_case,
    };
    use calc_exec::{OperationClass, PlanOp, ReduceOperation, ReduceShape};

    fn backend() -> SimdBackend {
        SimdBackend::new()
    }

    fn instruction_sets() -> Vec<InstructionSet> {
        let mut found = vec![InstructionSet::Sse2];
        if InstructionSet::detected() == InstructionSet::Avx2 {
            found.push(InstructionSet::Avx2);
        }
        found
    }

    #[test]
    fn every_operation_case_is_bit_identical_to_the_cpu() {
        for instruction_set in instruction_sets() {
            let backend = SimdBackend::with_instruction_set(instruction_set);
            for domain in [Domain::F32, Domain::F64] {
                for operation in PlanOp::ALL {
                    if let Some(case) = operation_case(operation, domain) {
                        assert_eq!(
                            check_bitwise(&backend, &case),
                            Ok(()),
                            "{operation:?} {domain:?} {}",
                            instruction_set.name()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_iteration_case_is_bit_identical_on_every_instruction_set() {
        for instruction_set in instruction_sets() {
            let backend = SimdBackend::with_instruction_set(instruction_set);
            for domain in [Domain::F32, Domain::F64] {
                for (number, case) in iteration_cases(domain).into_iter().enumerate() {
                    assert_eq!(
                        check_bitwise(&backend, &case),
                        Ok(()),
                        "iteration {number} {domain:?} {}",
                        instruction_set.name()
                    );
                }
            }
        }
    }

    #[test]
    fn every_reduce_case_is_bit_identical_on_every_instruction_set() {
        for instruction_set in instruction_sets() {
            let backend = SimdBackend::with_instruction_set(instruction_set);
            for shape in [ReduceShape::LeftFold, ReduceShape::Halving] {
                for length in REDUCE_LENGTHS {
                    if let Some(case) =
                        reduce_case(ReduceOperation::Add, shape, length, Domain::F64)
                    {
                        assert_eq!(
                            check_bitwise(&backend, &case),
                            Ok(()),
                            "{shape:?} {length} {}",
                            instruction_set.name()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_detected_instruction_set_names_itself() {
        let found = InstructionSet::detected();

        assert!(matches!(
            (found, found.name()),
            (InstructionSet::Sse2, "sse2") | (InstructionSet::Avx2, "avx2")
        ));
    }

    #[test]
    fn every_corner_case_holds() {
        for domain in [Domain::F32, Domain::F64] {
            for (operation, case) in corner_cases(domain) {
                assert_eq!(
                    check_expected(&backend(), &case),
                    Ok(()),
                    "{operation:?} {domain:?}"
                );
            }
        }
    }

    #[test]
    fn contraction_and_constant_chain_are_bit_identical_to_the_cpu() {
        for domain in [Domain::F32, Domain::F64] {
            for case in [contraction_case(domain), constant_chain_case(domain)]
                .into_iter()
                .flatten()
            {
                assert_eq!(check_bitwise(&backend(), &case), Ok(()), "{domain:?}");
            }
        }
    }

    #[test]
    fn every_reduce_case_is_bit_identical_to_the_cpu() {
        for domain in [Domain::F32, Domain::F64] {
            for operation in [
                ReduceOperation::Add,
                ReduceOperation::Mul,
                ReduceOperation::Min,
                ReduceOperation::Max,
            ] {
                for shape in [ReduceShape::LeftFold, ReduceShape::Halving] {
                    for length in REDUCE_LENGTHS {
                        if let Some(case) = reduce_case(operation, shape, length, domain) {
                            assert_eq!(
                                check_bitwise(&backend(), &case),
                                Ok(()),
                                "{operation:?} {shape:?} {length} {domain:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_iteration_case_is_bit_identical_to_the_cpu() {
        for domain in [Domain::F32, Domain::F64] {
            for (number, case) in iteration_cases(domain).into_iter().enumerate() {
                assert_eq!(
                    check_bitwise(&backend(), &case),
                    Ok(()),
                    "iteration {number} {domain:?}"
                );
            }
        }
    }

    #[test]
    fn every_escape_time_case_is_bit_identical_to_the_cpu() {
        for domain in [Domain::F32, Domain::F64] {
            for (number, case) in escape_time_cases(domain).into_iter().enumerate() {
                assert_eq!(
                    check_expected(&backend(), &case),
                    Ok(()),
                    "escape time {number} {domain:?} at {ESCAPE_TIME_CASE_ITERATIONS}"
                );
            }
        }
    }

    #[test]
    fn approximate_operations_use_the_scalar_reference() {
        let approximate = PlanOp::ALL
            .into_iter()
            .filter(|operation| operation.class() == OperationClass::Approximate);

        for operation in approximate {
            if let Some(case) = operation_case(operation, Domain::F64) {
                assert_eq!(check_bitwise(&backend(), &case), Ok(()), "{operation:?}");
            }
        }
    }

    #[test]
    fn lane_minimum_matches_the_reference() {
        use crate::lanes::Lanes;
        let values = calc_conformance::special_values_f32();
        for &a in &values {
            for &b in &values {
                let left = sse2::F32x4::splat(a);
                let right = sse2::F32x4::splat(b);
                let mut found = [0.0_f32; 4];
                left.minimum(right).store(&mut found);
                let wanted = calc_numbers::minimum_f32(a, b);
                assert!(
                    found[0].to_bits() == wanted.to_bits()
                        || (found[0].is_nan() && wanted.is_nan()),
                    "min({a}, {b}) gave {} wanted {wanted}",
                    found[0]
                );
            }
        }
    }

    #[test]
    fn the_backend_names_itself_simd() {
        assert_eq!(backend().kind(), BackendKind::Simd);
    }
}
