mod program;
mod scalar;

use std::num::NonZeroUsize;

use calc_exec::{
    Backend, BackendKind, Batch, Capabilities, Domain, Plan, PlanOp, PrepareError, Prepared,
    RunError, RunReport, SharedParallelism,
};

use crate::program::Program;

const MINIMUM_GATE_EVALUATIONS_PER_CHUNK: u64 = 1 << 16;

pub struct CpuBackend {
    capabilities: Capabilities,
    parallelism: SharedParallelism,
    minimum_chunk_length: Option<NonZeroUsize>,
}

#[derive(Clone, Debug)]
enum DomainProgram {
    F32(Program<f32>),
    F64(Program<f64>),
}

pub struct CpuPrepared {
    plan: Plan,
    program: DomainProgram,
    parallelism: SharedParallelism,
    minimum_chunk_length: NonZeroUsize,
}

impl CpuBackend {
    pub fn new() -> CpuBackend {
        CpuBackend::with_parallelism(SharedParallelism::sequential())
    }

    pub fn with_parallelism(parallelism: SharedParallelism) -> CpuBackend {
        CpuBackend::build(parallelism, None)
    }

    pub fn with_chunk_length(
        parallelism: SharedParallelism,
        minimum_chunk_length: NonZeroUsize,
    ) -> CpuBackend {
        CpuBackend::build(parallelism, Some(minimum_chunk_length))
    }

    fn build(
        parallelism: SharedParallelism,
        minimum_chunk_length: Option<NonZeroUsize>,
    ) -> CpuBackend {
        CpuBackend {
            parallelism,
            minimum_chunk_length,
            capabilities: Capabilities {
                domains: vec![Domain::F32, Domain::F64],
                operations: PlanOp::ALL.to_vec(),
                largest_batch_length: usize::MAX,
                largest_iteration_count: u32::MAX,
                reference_approximate_operations: true,
                cost: calc_exec::cost::CPU,
            },
        }
    }
}

fn minimum_chunk_length_for(plan: &Plan) -> NonZeroUsize {
    let per_element = plan.worst_case_gate_evaluations_per_element().max(1);
    let elements = MINIMUM_GATE_EVALUATIONS_PER_CHUNK.div_ceil(per_element);
    usize::try_from(elements)
        .ok()
        .and_then(NonZeroUsize::new)
        .unwrap_or(NonZeroUsize::MIN)
}

impl Default for CpuBackend {
    fn default() -> CpuBackend {
        CpuBackend::new()
    }
}

impl Backend for CpuBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Cpu
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn prepare(&self, plan: &Plan) -> Result<Box<dyn Prepared>, PrepareError> {
        self.capabilities
            .check_plan(plan)
            .map_err(PrepareError::Unsupported)?;
        let program = match plan.domain() {
            Domain::F32 => DomainProgram::F32(Program::compile(plan)?),
            Domain::F64 => DomainProgram::F64(Program::compile(plan)?),
        };
        Ok(Box::new(CpuPrepared {
            plan: plan.clone(),
            program,
            parallelism: self.parallelism.clone(),
            minimum_chunk_length: self
                .minimum_chunk_length
                .unwrap_or_else(|| minimum_chunk_length_for(plan)),
        }))
    }
}

impl Prepared for CpuPrepared {
    fn run(&mut self, inputs: &Batch, outputs: &mut Batch) -> Result<RunReport, RunError> {
        self.plan
            .check_batches(inputs, outputs)
            .map_err(RunError::BatchShape)?;
        if self.plan.reduce().is_some() && inputs.is_empty() {
            return Err(RunError::EmptyReduction);
        }
        match &self.program {
            DomainProgram::F32(program) => program.run(
                inputs,
                outputs,
                &self.parallelism,
                self.minimum_chunk_length,
            )?,
            DomainProgram::F64(program) => program.run(
                inputs,
                outputs,
                &self.parallelism,
                self.minimum_chunk_length,
            )?,
        }
        Ok(RunReport::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_exec::{
        BatchRole, BatchShapeError, ChannelId, Constant, Gate, GateId, PlanBatchError, Reduce,
        ReduceOperation, ReduceShape,
    };

    fn input_gates(count: u32) -> Vec<Gate> {
        (0..count)
            .map(|channel| Gate::Input(ChannelId(channel)))
            .collect()
    }

    fn scalar_plan(domain: Domain, operation: PlanOp) -> Plan {
        let arity = u32::try_from(operation.arity()).unwrap();
        let mut gates = input_gates(arity);
        gates.push(Gate::Operation {
            operation,
            arguments: (0..arity).map(GateId).collect(),
        });
        Plan::new(domain, operation.arity(), gates, vec![GateId(arity)], None).unwrap()
    }

    fn boolean_plan(domain: Domain, gates_after_inputs: Vec<Gate>, input_count: u32) -> Plan {
        let one = match domain {
            Domain::F32 => Constant::F32(1.0),
            Domain::F64 => Constant::F64(1.0),
        };
        let zero = match domain {
            Domain::F32 => Constant::F32(0.0),
            Domain::F64 => Constant::F64(0.0),
        };
        let mut gates = input_gates(input_count);
        gates.extend(gates_after_inputs);
        let condition = u32::try_from(gates.len() - 1).unwrap();
        gates.push(Gate::Constant(one));
        gates.push(Gate::Constant(zero));
        gates.push(Gate::Operation {
            operation: PlanOp::Select,
            arguments: vec![
                GateId(condition),
                GateId(condition + 1),
                GateId(condition + 2),
            ],
        });
        let output = u32::try_from(gates.len() - 1).unwrap();
        Plan::new(
            domain,
            usize::try_from(input_count).unwrap(),
            gates,
            vec![GateId(output)],
            None,
        )
        .unwrap()
    }

    fn run_f64(plan: &Plan, columns: Vec<Vec<f64>>) -> Vec<f64> {
        let length = columns.first().map_or(1, Vec::len);
        let inputs = Batch::from_f64_columns(length, columns).unwrap();
        let mut outputs = Batch::zeroed(Domain::F64, 1, plan.output_length(length));
        let mut prepared = CpuBackend::new().prepare(plan).unwrap();
        prepared.run(&inputs, &mut outputs).unwrap();
        outputs.f64_channel(0).unwrap().to_vec()
    }

    fn run_f32(plan: &Plan, columns: Vec<Vec<f32>>) -> Vec<f32> {
        let length = columns.first().map_or(1, Vec::len);
        let inputs = Batch::from_f32_columns(length, columns).unwrap();
        let mut outputs = Batch::zeroed(Domain::F32, 1, plan.output_length(length));
        let mut prepared = CpuBackend::new().prepare(plan).unwrap();
        prepared.run(&inputs, &mut outputs).unwrap();
        outputs.f32_channel(0).unwrap().to_vec()
    }

    fn operation_f64(operation: PlanOp, arguments: &[f64]) -> f64 {
        let plan = scalar_plan(Domain::F64, operation);
        let columns = arguments.iter().map(|argument| vec![*argument]).collect();
        run_f64(&plan, columns)[0]
    }

    fn operation_f32(operation: PlanOp, arguments: &[f32]) -> f32 {
        let plan = scalar_plan(Domain::F32, operation);
        let columns = arguments.iter().map(|argument| vec![*argument]).collect();
        run_f32(&plan, columns)[0]
    }

    fn comparison_f64(operation: PlanOp, left: f64, right: f64) -> f64 {
        let comparison = Gate::Operation {
            operation,
            arguments: vec![GateId(0), GateId(1)],
        };
        let plan = boolean_plan(Domain::F64, vec![comparison], 2);
        run_f64(&plan, vec![vec![left], vec![right]])[0]
    }

    fn logic_f64(operation: PlanOp, left: bool, right: bool) -> f64 {
        let truth = |value: bool| if value { 1.0 } else { 2.0 };
        let mut gates = vec![
            Gate::Constant(Constant::F64(1.0)),
            Gate::Operation {
                operation: PlanOp::Equal,
                arguments: vec![GateId(0), GateId(2)],
            },
            Gate::Operation {
                operation: PlanOp::Equal,
                arguments: vec![GateId(1), GateId(2)],
            },
        ];
        let logic = match operation {
            PlanOp::Not => Gate::Operation {
                operation,
                arguments: vec![GateId(3)],
            },
            _ => Gate::Operation {
                operation,
                arguments: vec![GateId(3), GateId(4)],
            },
        };
        gates.push(logic);
        let plan = boolean_plan(Domain::F64, gates, 2);
        run_f64(&plan, vec![vec![truth(left)], vec![truth(right)]])[0]
    }

    fn same_bits(actual: f64, expected: f64) -> bool {
        actual.to_bits() == expected.to_bits()
    }

    #[test]
    fn add_of_opposite_zeros_is_positive_zero() {
        let sum = operation_f64(PlanOp::Add, &[-0.0, 0.0]);

        assert!(same_bits(sum, 0.0));
    }

    #[test]
    fn add_rounds_tie_to_even() {
        let sum = operation_f64(PlanOp::Add, &[1.0, f64::EPSILON / 2.0]);

        assert!(same_bits(sum, 1.0));
    }

    #[test]
    fn sub_of_equal_infinities_is_nan() {
        let difference = operation_f64(PlanOp::Sub, &[f64::INFINITY, f64::INFINITY]);

        assert!(difference.is_nan());
    }

    #[test]
    fn mul_of_negative_zero_and_positive_value_is_negative_zero() {
        let product = operation_f64(PlanOp::Mul, &[-0.0, 3.0]);

        assert!(same_bits(product, -0.0));
    }

    #[test]
    fn div_by_negative_zero_is_negative_infinity() {
        let quotient = operation_f64(PlanOp::Div, &[1.0, -0.0]);

        assert!(same_bits(quotient, f64::NEG_INFINITY));
    }

    #[test]
    fn neg_of_positive_zero_is_negative_zero() {
        let negated = operation_f64(PlanOp::Neg, &[0.0]);

        assert!(same_bits(negated, -0.0));
    }

    #[test]
    fn abs_of_negative_subnormal_is_positive_subnormal() {
        let subnormal = f64::from_bits(1);

        let magnitude = operation_f64(PlanOp::Abs, &[-subnormal]);

        assert!(same_bits(magnitude, subnormal));
    }

    #[test]
    fn sqrt_of_negative_zero_is_negative_zero() {
        let root = operation_f64(PlanOp::Sqrt, &[-0.0]);

        assert!(same_bits(root, -0.0));
    }

    #[test]
    fn sqrt_of_negative_value_is_nan() {
        let root = operation_f64(PlanOp::Sqrt, &[-1.0]);

        assert!(root.is_nan());
    }

    #[test]
    fn mul_add_is_fused() {
        let fused = operation_f64(PlanOp::MulAdd, &[0.1, 10.0, -1.0]);

        assert!(same_bits(fused, 0.1_f64.mul_add(10.0, -1.0)));
    }

    #[test]
    fn mul_add_differs_from_separate_mul_and_add() {
        let fused = operation_f64(PlanOp::MulAdd, &[0.1, 10.0, -1.0]);

        assert!(!same_bits(fused, 0.1 * 10.0 - 1.0));
    }

    #[test]
    fn min_orders_negative_zero_below_positive_zero() {
        let minimum = operation_f64(PlanOp::Min, &[0.0, -0.0]);

        assert!(same_bits(minimum, -0.0));
    }

    #[test]
    fn min_propagates_nan() {
        let minimum = operation_f64(PlanOp::Min, &[1.0, f64::NAN]);

        assert!(minimum.is_nan());
    }

    #[test]
    fn max_orders_positive_zero_above_negative_zero() {
        let maximum = operation_f64(PlanOp::Max, &[-0.0, 0.0]);

        assert!(same_bits(maximum, 0.0));
    }

    #[test]
    fn max_propagates_nan() {
        let maximum = operation_f64(PlanOp::Max, &[f64::NAN, f64::INFINITY]);

        assert!(maximum.is_nan());
    }

    #[test]
    fn floor_of_negative_half_is_negative_one() {
        let floor = operation_f64(PlanOp::Floor, &[-0.5]);

        assert!(same_bits(floor, -1.0));
    }

    #[test]
    fn ceil_of_negative_half_is_negative_zero() {
        let ceil = operation_f64(PlanOp::Ceil, &[-0.5]);

        assert!(same_bits(ceil, -0.0));
    }

    #[test]
    fn trunc_of_negative_one_and_a_half_is_negative_one() {
        let truncated = operation_f64(PlanOp::Trunc, &[-1.5]);

        assert!(same_bits(truncated, -1.0));
    }

    #[test]
    fn round_ties_even_rounds_two_and_a_half_to_two() {
        let rounded = operation_f64(PlanOp::RoundTiesEven, &[2.5]);

        assert!(same_bits(rounded, 2.0));
    }

    #[test]
    fn copy_sign_takes_sign_of_negative_nan() {
        let signed = operation_f64(PlanOp::CopySign, &[1.0, -f64::NAN]);

        assert!(same_bits(signed, -1.0));
    }

    #[test]
    fn less_is_false_for_nan() {
        let result = comparison_f64(PlanOp::Less, f64::NAN, 1.0);

        assert!(same_bits(result, 0.0));
    }

    #[test]
    fn less_or_equal_holds_for_opposite_zeros() {
        let result = comparison_f64(PlanOp::LessOrEqual, 0.0, -0.0);

        assert!(same_bits(result, 1.0));
    }

    #[test]
    fn greater_holds_for_infinity_over_max() {
        let result = comparison_f64(PlanOp::Greater, f64::INFINITY, f64::MAX);

        assert!(same_bits(result, 1.0));
    }

    #[test]
    fn greater_or_equal_is_false_for_nan() {
        let result = comparison_f64(PlanOp::GreaterOrEqual, f64::NAN, f64::NAN);

        assert!(same_bits(result, 0.0));
    }

    #[test]
    fn equal_holds_for_opposite_zeros() {
        let result = comparison_f64(PlanOp::Equal, -0.0, 0.0);

        assert!(same_bits(result, 1.0));
    }

    #[test]
    fn not_equal_holds_for_nan_with_itself() {
        let result = comparison_f64(PlanOp::NotEqual, f64::NAN, f64::NAN);

        assert!(same_bits(result, 1.0));
    }

    #[test]
    fn and_is_false_when_one_side_is_false() {
        let result = logic_f64(PlanOp::And, true, false);

        assert!(same_bits(result, 0.0));
    }

    #[test]
    fn or_is_true_when_one_side_is_true() {
        let result = logic_f64(PlanOp::Or, false, true);

        assert!(same_bits(result, 1.0));
    }

    #[test]
    fn not_inverts_true() {
        let result = logic_f64(PlanOp::Not, true, true);

        assert!(same_bits(result, 0.0));
    }

    #[test]
    fn select_takes_second_argument_when_condition_holds() {
        let result = comparison_f64(PlanOp::Less, -1.0, 1.0);

        assert!(same_bits(result, 1.0));
    }

    #[test]
    fn f32_add_rounds_in_single_precision() {
        let sum = operation_f32(PlanOp::Add, &[16_777_216.0, 1.0]);

        assert_eq!(sum.to_bits(), 16_777_216.0_f32.to_bits());
    }

    #[test]
    fn f32_min_orders_negative_zero_below_positive_zero() {
        let minimum = operation_f32(PlanOp::Min, &[0.0, -0.0]);

        assert_eq!(minimum.to_bits(), (-0.0_f32).to_bits());
    }

    #[test]
    fn map_stage_evaluates_every_element() {
        let plan = scalar_plan(Domain::F64, PlanOp::Mul);

        let products = run_f64(&plan, vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, -0.0]]);

        assert_eq!(
            products
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            [4.0, 10.0, -0.0]
                .iter()
                .map(|value: &f64| value.to_bits())
                .collect::<Vec<_>>()
        );
    }

    fn reduce_plan(operation: ReduceOperation, shape: ReduceShape) -> Plan {
        Plan::new(
            Domain::F64,
            1,
            input_gates(1),
            vec![GateId(0)],
            Some(Reduce { operation, shape }),
        )
        .unwrap()
    }

    #[test]
    fn left_fold_sums_from_the_left() {
        let plan = reduce_plan(ReduceOperation::Add, ReduceShape::LeftFold);

        let sum = run_f64(&plan, vec![vec![1e16, 1.0, 1.0]]);

        assert!(same_bits(sum[0], (1e16 + 1.0) + 1.0));
    }

    #[test]
    fn halving_splits_three_values_into_one_and_two() {
        let plan = reduce_plan(ReduceOperation::Add, ReduceShape::Halving);

        let sum = run_f64(&plan, vec![vec![1e16, 1.0, 1.0]]);

        assert!(same_bits(sum[0], 1e16 + (1.0 + 1.0)));
    }

    #[test]
    fn halving_of_five_values_follows_the_recursive_split() {
        let plan = reduce_plan(ReduceOperation::Add, ReduceShape::Halving);
        let values = vec![1e16, 1.0, -1e16, 1.0, 1.0];

        let sum = run_f64(&plan, vec![values]);

        assert!(same_bits(sum[0], (1e16 + 1.0) + (-1e16 + (1.0 + 1.0))));
    }

    #[test]
    fn halving_product_of_one_value_is_that_value() {
        let plan = reduce_plan(ReduceOperation::Mul, ReduceShape::Halving);

        let product = run_f64(&plan, vec![vec![-0.0]]);

        assert!(same_bits(product[0], -0.0));
    }

    #[test]
    fn reduce_minimum_propagates_nan() {
        let plan = reduce_plan(ReduceOperation::Min, ReduceShape::LeftFold);

        let minimum = run_f64(&plan, vec![vec![1.0, f64::NAN, -1.0]]);

        assert!(minimum[0].is_nan());
    }

    #[test]
    fn reduce_maximum_prefers_positive_zero() {
        let plan = reduce_plan(ReduceOperation::Max, ReduceShape::Halving);

        let maximum = run_f64(&plan, vec![vec![-0.0, 0.0, -0.0]]);

        assert!(same_bits(maximum[0], 0.0));
    }

    #[test]
    fn empty_batch_on_reduced_plan_is_an_error() {
        let plan = reduce_plan(ReduceOperation::Add, ReduceShape::Halving);
        let inputs = Batch::from_f64_columns(0, vec![Vec::new()]).unwrap();
        let mut outputs = Batch::zeroed(Domain::F64, 1, 1);
        let mut prepared = CpuBackend::new().prepare(&plan).unwrap();

        let result = prepared.run(&inputs, &mut outputs);

        assert_eq!(result, Err(RunError::EmptyReduction));
    }

    #[test]
    fn wrong_output_shape_is_an_error() {
        let plan = scalar_plan(Domain::F64, PlanOp::Neg);
        let inputs = Batch::zeroed(Domain::F64, 1, 3);
        let mut outputs = Batch::zeroed(Domain::F64, 1, 2);
        let mut prepared = CpuBackend::new().prepare(&plan).unwrap();

        let result = prepared.run(&inputs, &mut outputs);

        assert_eq!(
            result,
            Err(RunError::BatchShape(PlanBatchError {
                role: BatchRole::Outputs,
                shape: BatchShapeError::LengthMismatch {
                    expected: 3,
                    actual: 2
                }
            }))
        );
    }

    #[test]
    fn sin_in_f32_plan_calls_the_f32_reference() {
        let value = operation_f32(PlanOp::Sin, &[0.5]);

        assert_eq!(value.to_bits(), calc_numbers::sin_f32(0.5).to_bits());
    }

    #[test]
    fn exp_in_f32_plan_calls_the_f32_reference() {
        let value = operation_f32(PlanOp::Exp, &[1.0]);

        assert_eq!(value.to_bits(), calc_numbers::exp_f32(1.0).to_bits());
    }

    #[test]
    fn pow_in_f32_plan_calls_the_f32_reference() {
        let value = operation_f32(PlanOp::Pow, &[2.0, 0.5]);

        assert_eq!(value.to_bits(), calc_numbers::pow_f32(2.0, 0.5).to_bits());
    }

    #[test]
    fn atan2_in_f32_plan_calls_the_f32_reference() {
        let value = operation_f32(PlanOp::Atan2, &[1.0, -1.0]);

        assert_eq!(
            value.to_bits(),
            calc_numbers::atan2_f32(1.0, -1.0).to_bits()
        );
    }

    #[test]
    fn asin_in_f32_plan_calls_the_f32_reference() {
        let value = operation_f32(PlanOp::Asin, &[0.3]);

        assert_eq!(value.to_bits(), calc_numbers::asin_f32(0.3).to_bits());
    }

    #[test]
    fn exp_of_zero_is_one() {
        let value = operation_f64(PlanOp::Exp, &[0.0]);

        assert!(same_bits(value, 1.0));
    }

    #[test]
    fn exp_of_one_is_correctly_rounded_e() {
        let value = operation_f64(PlanOp::Exp, &[1.0]);

        assert!(same_bits(value, std::f64::consts::E));
    }

    #[test]
    fn ln_of_negative_zero_is_negative_infinity() {
        let value = operation_f64(PlanOp::Ln, &[-0.0]);

        assert!(same_bits(value, f64::NEG_INFINITY));
    }

    #[test]
    fn sin_of_negative_zero_is_negative_zero() {
        let value = operation_f64(PlanOp::Sin, &[-0.0]);

        assert!(same_bits(value, -0.0));
    }

    #[test]
    fn cos_of_infinity_is_nan() {
        let value = operation_f64(PlanOp::Cos, &[f64::INFINITY]);

        assert!(value.is_nan());
    }

    #[test]
    fn tan_of_positive_zero_is_positive_zero() {
        let value = operation_f64(PlanOp::Tan, &[0.0]);

        assert!(same_bits(value, 0.0));
    }

    #[test]
    fn asin_of_one_is_half_pi() {
        let value = operation_f64(PlanOp::Asin, &[1.0]);

        assert!(same_bits(value, std::f64::consts::FRAC_PI_2));
    }

    #[test]
    fn acos_of_one_is_positive_zero() {
        let value = operation_f64(PlanOp::Acos, &[1.0]);

        assert!(same_bits(value, 0.0));
    }

    #[test]
    fn asin_outside_the_domain_is_nan() {
        let value = operation_f64(PlanOp::Asin, &[2.0]);

        assert!(value.is_nan());
    }

    #[test]
    fn atan_of_infinity_is_half_pi() {
        let value = operation_f64(PlanOp::Atan, &[f64::INFINITY]);

        assert!(same_bits(value, std::f64::consts::FRAC_PI_2));
    }

    #[test]
    fn atan2_of_positive_zero_over_negative_zero_is_pi() {
        let value = operation_f64(PlanOp::Atan2, &[0.0, -0.0]);

        assert!(same_bits(value, std::f64::consts::PI));
    }

    #[test]
    fn pow_of_two_to_ten_is_exact() {
        let value = operation_f64(PlanOp::Pow, &[2.0, 10.0]);

        assert!(same_bits(value, 1024.0));
    }

    #[test]
    fn host_backend_reports_no_transfers() {
        let plan = scalar_plan(Domain::F64, PlanOp::Neg);
        let inputs = Batch::zeroed(Domain::F64, 1, 3);
        let mut outputs = Batch::zeroed(Domain::F64, 1, 3);
        let mut prepared = CpuBackend::new().prepare(&plan).unwrap();

        let report = prepared.run(&inputs, &mut outputs);

        assert_eq!(report, Ok(RunReport::default()));
    }

    fn doubling_plan(maximum_count: u32) -> Plan {
        let iteration = calc_exec::Iteration {
            initial: vec![GateId(0)],
            body: vec![
                calc_exec::BodyGate::State(calc_exec::SlotId(0)),
                calc_exec::BodyGate::Outer(GateId(1)),
                calc_exec::BodyGate::Constant(Constant::F64(2.0)),
                calc_exec::BodyGate::Operation {
                    operation: PlanOp::Mul,
                    arguments: vec![calc_exec::BodyGateId(0), calc_exec::BodyGateId(2)],
                },
                calc_exec::BodyGate::Operation {
                    operation: PlanOp::Greater,
                    arguments: vec![calc_exec::BodyGateId(0), calc_exec::BodyGateId(1)],
                },
            ],
            next: vec![calc_exec::BodyGateId(3)],
            exit: calc_exec::BodyGateId(4),
            maximum_count,
        };
        let gates = vec![
            Gate::Input(ChannelId(0)),
            Gate::Constant(Constant::F64(100.0)),
            Gate::Iterate(Box::new(iteration)),
            Gate::IterationState {
                iteration: GateId(2),
                slot: calc_exec::SlotId(0),
            },
        ];
        Plan::new(Domain::F64, 1, gates, vec![GateId(3)], None).unwrap()
    }

    #[test]
    fn iteration_stops_at_the_first_state_that_meets_the_exit() {
        let plan = doubling_plan(8);

        let values = run_f64(&plan, vec![vec![3.0]]);

        assert!(same_bits(values[0], 192.0));
    }

    #[test]
    fn iteration_stops_at_the_maximum_count() {
        let plan = doubling_plan(3);

        let values = run_f64(&plan, vec![vec![3.0]]);

        assert!(same_bits(values[0], 24.0));
    }

    #[test]
    fn iteration_with_zero_count_keeps_the_initial_state() {
        let plan = doubling_plan(0);

        let values = run_f64(&plan, vec![vec![-0.0]]);

        assert!(same_bits(values[0], -0.0));
    }

    struct DroppingParallelism;

    impl calc_exec::Parallelism for DroppingParallelism {
        fn worker_count(&self) -> NonZeroUsize {
            NonZeroUsize::new(4).unwrap()
        }

        fn run_all<'scope>(&self, tasks: Vec<Box<dyn FnOnce() + Send + 'scope>>) {
            for task in tasks.into_iter().skip(1) {
                task();
            }
        }
    }

    struct ReversedParallelism;

    impl calc_exec::Parallelism for ReversedParallelism {
        fn worker_count(&self) -> NonZeroUsize {
            NonZeroUsize::new(3).unwrap()
        }

        fn run_all<'scope>(&self, tasks: Vec<Box<dyn FnOnce() + Send + 'scope>>) {
            for task in tasks.into_iter().rev() {
                task();
            }
        }
    }

    fn split_backend(parallelism: impl calc_exec::Parallelism + 'static) -> CpuBackend {
        CpuBackend::with_chunk_length(SharedParallelism::new(parallelism), NonZeroUsize::MIN)
    }

    #[test]
    fn chunks_cover_every_element_once_in_order() {
        let ranges = program::chunk_ranges(7, NonZeroUsize::new(3).unwrap(), NonZeroUsize::MIN);

        assert_eq!(ranges, vec![0..3, 3..6, 6..7]);
    }

    #[test]
    fn short_batch_stays_in_one_chunk() {
        let ranges = program::chunk_ranges(
            100,
            NonZeroUsize::new(8).unwrap(),
            NonZeroUsize::new(4096).unwrap(),
        );

        assert_eq!(ranges, vec![0..100]);
    }

    #[test]
    fn empty_batch_has_no_chunks() {
        let ranges = program::chunk_ranges(0, NonZeroUsize::new(8).unwrap(), NonZeroUsize::MIN);

        assert!(ranges.is_empty());
    }

    #[test]
    fn chunks_run_in_reverse_order_give_the_same_outputs() {
        let plan = scalar_plan(Domain::F64, PlanOp::Mul);
        let inputs = Batch::from_f64_columns(
            5,
            vec![
                vec![1.0, 2.0, 3.0, -0.0, f64::NAN],
                vec![4.0, 5.0, 6.0, 1.0, 1.0],
            ],
        )
        .unwrap();
        let mut outputs = Batch::zeroed(Domain::F64, 1, 5);
        let mut prepared = split_backend(ReversedParallelism).prepare(&plan).unwrap();

        prepared.run(&inputs, &mut outputs).unwrap();

        let bits: Vec<u64> = outputs
            .f64_channel(0)
            .unwrap()
            .iter()
            .map(|value| value.to_bits())
            .collect();
        assert_eq!(
            &bits[..4],
            &[
                4.0_f64.to_bits(),
                10.0_f64.to_bits(),
                18.0_f64.to_bits(),
                (-0.0_f64).to_bits()
            ]
        );
    }

    #[test]
    fn reduce_after_a_split_map_keeps_its_tree_shape() {
        let plan = reduce_plan(ReduceOperation::Add, ReduceShape::Halving);
        let inputs = Batch::from_f64_columns(3, vec![vec![1e16, 1.0, 1.0]]).unwrap();
        let mut outputs = Batch::zeroed(Domain::F64, 1, 1);
        let mut prepared = split_backend(ReversedParallelism).prepare(&plan).unwrap();

        prepared.run(&inputs, &mut outputs).unwrap();

        assert!(same_bits(
            outputs.f64_channel(0).unwrap()[0],
            1e16 + (1.0 + 1.0)
        ));
    }

    #[test]
    fn parallelism_that_skips_a_task_is_an_error() {
        let plan = scalar_plan(Domain::F64, PlanOp::Neg);
        let inputs = Batch::zeroed(Domain::F64, 1, 8);
        let mut outputs = Batch::zeroed(Domain::F64, 1, 8);
        let mut prepared = split_backend(DroppingParallelism).prepare(&plan).unwrap();

        let result = prepared.run(&inputs, &mut outputs);

        assert_eq!(result, Err(RunError::ParallelTaskNotRun));
    }

    #[test]
    fn cheap_plan_needs_many_elements_per_chunk() {
        let plan = scalar_plan(Domain::F64, PlanOp::Add);

        let length = minimum_chunk_length_for(&plan);

        assert_eq!(length.get(), 21_846);
    }

    #[test]
    fn costly_plan_splits_small_batches() {
        let plan =
            calc_exec::escape_time_plan(Domain::F64, calc_exec::EscapeTimePlanForm::Parameter, 256)
                .unwrap();

        let length = minimum_chunk_length_for(&plan);

        assert!(length.get() < 64);
    }
}
