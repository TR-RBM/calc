use calc_exec::{
    Batch, BodyGate, BodyGateId, ChannelId, Constant, Domain, EscapeTimePlanForm, Gate, GateId,
    Iteration, Plan, PlanOp, Reduce, ReduceOperation, ReduceShape, SlotId, ValueKind,
    escape_time_iteration_plan, escape_time_plan,
};

pub const REDUCE_LENGTHS: [usize; 7] = [1, 2, 3, 5, 1023, 1024, 1025];

const LARGE_ADDEND_F64: f64 = 1e16;
const LARGE_ADDEND_F32: f32 = 16_777_216.0;

#[derive(Clone, Debug)]
pub struct ExpectedCase {
    pub plan: Plan,
    pub inputs: Batch,
    pub expected: Batch,
}

#[derive(Clone, Debug)]
pub struct Case {
    pub plan: Plan,
    pub inputs: Batch,
}

pub fn special_values_f64() -> Vec<f64> {
    vec![
        0.0,
        -0.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        -f64::NAN,
        f64::from_bits(1),
        f64::MIN_POSITIVE.next_down(),
        f64::MIN_POSITIVE,
        f64::MAX,
        f64::MIN,
        1.0,
        1.0_f64.next_down(),
        1.0_f64.next_up(),
        -1.0,
        f64::EPSILON / 2.0,
        0.5,
        1.5,
        2.5,
        -2.5,
        3.0,
    ]
}

pub fn special_values_f32() -> Vec<f32> {
    vec![
        0.0,
        -0.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        -f32::NAN,
        f32::from_bits(1),
        f32::MIN_POSITIVE.next_down(),
        f32::MIN_POSITIVE,
        f32::MAX,
        f32::MIN,
        1.0,
        1.0_f32.next_down(),
        1.0_f32.next_up(),
        -1.0,
        f32::EPSILON / 2.0,
        0.5,
        1.5,
        2.5,
        -2.5,
        3.0,
    ]
}

enum Values {
    F32(Vec<f32>),
    F64(Vec<f64>),
}

impl Values {
    fn special(domain: Domain) -> Values {
        match domain {
            Domain::F32 => Values::F32(special_values_f32()),
            Domain::F64 => Values::F64(special_values_f64()),
        }
    }

    fn cross_product_batch(&self, channel_count: usize) -> Option<Batch> {
        match self {
            Values::F32(values) => {
                let (length, columns) = cross_product(values, channel_count);
                Batch::from_f32_columns(length, columns).ok()
            }
            Values::F64(values) => {
                let (length, columns) = cross_product(values, channel_count);
                Batch::from_f64_columns(length, columns).ok()
            }
        }
    }
}

fn cross_product<T: Copy>(values: &[T], channel_count: usize) -> (usize, Vec<Vec<T>>) {
    let length = (0..channel_count).fold(1, |length, _| length * values.len());
    let columns = (0..channel_count)
        .map(|channel| {
            let period = (channel + 1..channel_count).fold(1, |period, _| period * values.len());
            (0..length)
                .map(|element| values[(element / period) % values.len()])
                .collect()
        })
        .collect();
    (length, columns)
}

fn constant(domain: Domain, value: f32) -> Constant {
    match domain {
        Domain::F32 => Constant::F32(value),
        Domain::F64 => Constant::F64(f64::from(value)),
    }
}

fn gate_id(index: usize) -> GateId {
    GateId(u32::try_from(index).unwrap_or(u32::MAX))
}

struct CircuitBuilder {
    domain: Domain,
    gates: Vec<Gate>,
}

impl CircuitBuilder {
    fn with_inputs(domain: Domain, input_count: usize) -> CircuitBuilder {
        let gates = (0..input_count)
            .map(|channel| Gate::Input(ChannelId(u32::try_from(channel).unwrap_or(u32::MAX))))
            .collect();
        CircuitBuilder { domain, gates }
    }

    fn push(&mut self, gate: Gate) -> usize {
        self.gates.push(gate);
        self.gates.len() - 1
    }

    fn constant(&mut self, value: f32) -> usize {
        let domain = self.domain;
        self.push(Gate::Constant(constant(domain, value)))
    }

    fn operation(&mut self, operation: PlanOp, arguments: &[usize]) -> usize {
        self.push(Gate::Operation {
            operation,
            arguments: arguments.iter().copied().map(gate_id).collect(),
        })
    }

    fn below_one(&mut self, input: usize) -> usize {
        let one = self.constant(1.0);
        self.operation(PlanOp::Less, &[input, one])
    }

    fn as_scalar(&mut self, condition: usize) -> usize {
        let one = self.constant(1.0);
        let zero = self.constant(0.0);
        self.operation(PlanOp::Select, &[condition, one, zero])
    }

    fn finish(self, input_count: usize, output: usize, reduce: Option<Reduce>) -> Option<Plan> {
        Plan::new(
            self.domain,
            input_count,
            self.gates,
            vec![gate_id(output)],
            reduce,
        )
        .ok()
    }
}

pub fn operation_case(operation: PlanOp, domain: Domain) -> Option<Case> {
    let signature = operation.signature();
    let input_count = signature.arguments.len();
    let mut builder = CircuitBuilder::with_inputs(domain, input_count);
    let inputs: Vec<usize> = (0..input_count).collect();
    let arguments: Vec<usize> = signature
        .arguments
        .iter()
        .zip(&inputs)
        .map(|(kind, input)| match kind {
            ValueKind::Scalar => *input,
            ValueKind::Boolean => builder.below_one(*input),
        })
        .collect();
    let result = builder.operation(operation, &arguments);
    let output = match signature.result {
        ValueKind::Scalar => result,
        ValueKind::Boolean => builder.as_scalar(result),
    };
    let plan = builder.finish(input_count, output, None)?;
    let inputs = Values::special(domain).cross_product_batch(input_count)?;
    Some(Case { plan, inputs })
}

pub fn gpu_probe_values_f32() -> Vec<f32> {
    vec![
        f32::from_bits(1),
        -f32::from_bits(1),
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
        0.0,
        -0.0,
        0.5,
        -0.5,
        1.5,
        -1.5,
        2.5,
        1.0_f32.next_down(),
        2.0_f32.next_up(),
        4_194_304.5,
        8_388_608.0,
        8_388_609.0,
    ]
}

fn cross_product_f32(values: &[f32], channel_count: usize) -> Option<Batch> {
    let length = values
        .len()
        .checked_pow(u32::try_from(channel_count).ok()?)?;
    let mut columns = Vec::new();
    let mut repeat = 1;
    for _ in 0..channel_count {
        let column = (0..length)
            .map(|position| values[(position / repeat) % values.len()])
            .collect();
        columns.push(column);
        repeat *= values.len();
    }
    Batch::from_f32_columns(length, columns).ok()
}

pub fn gpu_probe_case(operation: PlanOp) -> Option<Case> {
    let case = operation_case(operation, Domain::F32)?;
    let inputs = cross_product_f32(&gpu_probe_values_f32(), case.plan.input_channel_count())?;
    Some(Case {
        plan: case.plan,
        inputs,
    })
}

pub fn contraction_case(domain: Domain) -> Option<Case> {
    let mut builder = CircuitBuilder::with_inputs(domain, 3);
    let product = builder.operation(PlanOp::Mul, &[0, 1]);
    let sum = builder.operation(PlanOp::Add, &[product, 2]);
    let plan = builder.finish(3, sum, None)?;
    let inputs = match domain {
        Domain::F32 => Batch::from_f32_columns(1, vec![vec![0.1], vec![10.0], vec![-1.0]]).ok()?,
        Domain::F64 => Batch::from_f64_columns(1, vec![vec![0.1], vec![10.0], vec![-1.0]]).ok()?,
    };
    Some(Case { plan, inputs })
}

fn reduce_values_f64(operation: ReduceOperation) -> Vec<f64> {
    match operation {
        ReduceOperation::Add => vec![
            LARGE_ADDEND_F64,
            1.0,
            -LARGE_ADDEND_F64,
            3.0,
            0.1,
            -2.5,
            1.0,
        ],
        ReduceOperation::Mul => vec![1.1, 0.9, 1.3, 0.7, 1.7, 0.6],
        ReduceOperation::Min | ReduceOperation::Max => vec![-0.0, 0.0, 1.0, -1.0, 0.0, -0.0, 2.5],
    }
}

fn reduce_values_f32(operation: ReduceOperation) -> Vec<f32> {
    match operation {
        ReduceOperation::Add => vec![
            LARGE_ADDEND_F32,
            1.0,
            -LARGE_ADDEND_F32,
            3.0,
            0.1,
            -2.5,
            1.0,
        ],
        ReduceOperation::Mul => vec![1.1, 0.9, 1.3, 0.7, 1.7, 0.6],
        ReduceOperation::Min | ReduceOperation::Max => vec![-0.0, 0.0, 1.0, -1.0, 0.0, -0.0, 2.5],
    }
}

fn cycled<T: Copy>(pattern: &[T], length: usize) -> Vec<T> {
    pattern.iter().copied().cycle().take(length).collect()
}

pub fn reduce_case(
    operation: ReduceOperation,
    shape: ReduceShape,
    length: usize,
    domain: Domain,
) -> Option<Case> {
    let builder = CircuitBuilder::with_inputs(domain, 1);
    let plan = builder.finish(1, 0, Some(Reduce { operation, shape }))?;
    let inputs = match domain {
        Domain::F32 => {
            Batch::from_f32_columns(length, vec![cycled(&reduce_values_f32(operation), length)])
                .ok()?
        }
        Domain::F64 => {
            Batch::from_f64_columns(length, vec![cycled(&reduce_values_f64(operation), length)])
                .ok()?
        }
    };
    Some(Case { plan, inputs })
}

fn binary_plan(operation: PlanOp, domain: Domain) -> Option<Plan> {
    let mut builder = CircuitBuilder::with_inputs(domain, 2);
    let result = builder.operation(operation, &[0, 1]);
    builder.finish(2, result, None)
}

fn atan2_corners_f64() -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let smallest_subnormal = f64::from_bits(1);
    let rows = [
        (f64::MAX, smallest_subnormal, std::f64::consts::FRAC_PI_2),
        (-f64::MAX, smallest_subnormal, -std::f64::consts::FRAC_PI_2),
        (smallest_subnormal, -f64::MAX, std::f64::consts::PI),
        (-smallest_subnormal, -f64::MAX, -std::f64::consts::PI),
        (f64::MAX, -smallest_subnormal, std::f64::consts::FRAC_PI_2),
        (smallest_subnormal, f64::MAX, 0.0),
    ];
    (
        rows.iter().map(|row| row.0).collect(),
        rows.iter().map(|row| row.1).collect(),
        rows.iter().map(|row| row.2).collect(),
    )
}

fn atan2_corners_f32() -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let smallest_subnormal = f32::from_bits(1);
    let rows = [
        (f32::MAX, smallest_subnormal, std::f32::consts::FRAC_PI_2),
        (-f32::MAX, smallest_subnormal, -std::f32::consts::FRAC_PI_2),
        (smallest_subnormal, -f32::MAX, std::f32::consts::PI),
        (-smallest_subnormal, -f32::MAX, -std::f32::consts::PI),
        (f32::MAX, -smallest_subnormal, std::f32::consts::FRAC_PI_2),
        (smallest_subnormal, f32::MAX, 0.0),
    ];
    (
        rows.iter().map(|row| row.0).collect(),
        rows.iter().map(|row| row.1).collect(),
        rows.iter().map(|row| row.2).collect(),
    )
}

pub fn corner_cases(domain: Domain) -> Vec<(PlanOp, ExpectedCase)> {
    let mut cases = Vec::new();
    let (atan2_inputs, pow_inputs) = match domain {
        Domain::F64 => {
            let (numerators, denominators, expected) = atan2_corners_f64();
            let pow = (
                vec![-1.0, -1.0, -1.0],
                vec![f64::MAX, -f64::MAX, 3.0],
                vec![1.0, 1.0, -1.0],
            );
            (
                Batch::from_f64_columns(numerators.len(), vec![numerators, denominators])
                    .ok()
                    .zip(Batch::from_f64_columns(expected.len(), vec![expected]).ok()),
                Batch::from_f64_columns(pow.0.len(), vec![pow.0, pow.1])
                    .ok()
                    .zip(Batch::from_f64_columns(pow.2.len(), vec![pow.2]).ok()),
            )
        }
        Domain::F32 => {
            let (numerators, denominators, expected) = atan2_corners_f32();
            let pow = (
                vec![-1.0, -1.0, -1.0],
                vec![f32::MAX, -f32::MAX, 3.0],
                vec![1.0, 1.0, -1.0],
            );
            (
                Batch::from_f32_columns(numerators.len(), vec![numerators, denominators])
                    .ok()
                    .zip(Batch::from_f32_columns(expected.len(), vec![expected]).ok()),
                Batch::from_f32_columns(pow.0.len(), vec![pow.0, pow.1])
                    .ok()
                    .zip(Batch::from_f32_columns(pow.2.len(), vec![pow.2]).ok()),
            )
        }
    };
    for (operation, batches) in [(PlanOp::Atan2, atan2_inputs), (PlanOp::Pow, pow_inputs)] {
        if let (Some(plan), Some((inputs, expected))) = (binary_plan(operation, domain), batches) {
            cases.push((
                operation,
                ExpectedCase {
                    plan,
                    inputs,
                    expected,
                },
            ));
        }
    }
    cases
}

pub const ESCAPE_TIME_CASE_ITERATIONS: u32 = 8;

const ESCAPE_TIME_NEVER_ESCAPED: f32 = 9.0;

fn escape_time_batches(
    domain: Domain,
    first: Vec<f32>,
    second: Vec<f32>,
    counts: Vec<f32>,
    flags: Vec<f32>,
) -> Option<(Batch, Batch)> {
    let length = first.len();
    match domain {
        Domain::F32 => Some((
            Batch::from_f32_columns(length, vec![first, second]).ok()?,
            Batch::from_f32_columns(length, vec![counts, flags]).ok()?,
        )),
        Domain::F64 => {
            let widen = |column: Vec<f32>| column.into_iter().map(f64::from).collect();
            Some((
                Batch::from_f64_columns(length, vec![widen(first), widen(second)]).ok()?,
                Batch::from_f64_columns(length, vec![widen(counts), widen(flags)]).ok()?,
            ))
        }
    }
}

pub fn escape_time_cases(domain: Domain) -> Vec<ExpectedCase> {
    escape_time_case_list(domain).unwrap_or_default()
}

fn escape_time_case_list(domain: Domain) -> Option<Vec<ExpectedCase>> {
    let never = ESCAPE_TIME_NEVER_ESCAPED;
    let parameter = escape_time_batches(
        domain,
        vec![0.0, 1.0, -2.0, 0.0, f32::NAN, f32::INFINITY],
        vec![0.0, 0.0, 0.0, 3.0, 0.0, 0.0],
        vec![never, 3.0, never, 2.0, never, never],
        vec![0.0, 1.0, 0.0, 1.0, 0.0, 0.0],
    );
    let (minus_one, zero) = match domain {
        Domain::F64 => (Constant::F64(-1.0), Constant::F64(0.0)),
        Domain::F32 => (Constant::F32(-1.0), Constant::F32(0.0)),
    };
    let initial_form = EscapeTimePlanForm::Initial {
        c_real: minus_one,
        c_imaginary: zero,
    };
    let initial = escape_time_batches(
        domain,
        vec![2.0, 0.0, -1.0],
        vec![0.0, 0.0, 0.0],
        vec![1.0, never, never],
        vec![1.0, 0.0, 0.0],
    );
    let overflowing = match domain {
        Domain::F32 => escape_time_batches(domain, vec![1.0e10], vec![0.0], vec![0.0], vec![1.0]),
        Domain::F64 => Some((
            Batch::from_f64_columns(1, vec![vec![1.0e200], vec![0.0]]).ok()?,
            Batch::from_f64_columns(1, vec![vec![0.0], vec![1.0]]).ok()?,
        )),
    };
    let mut cases = Vec::new();
    for (form, batches) in [
        (EscapeTimePlanForm::Parameter, parameter),
        (initial_form, initial),
        (initial_form, overflowing),
    ] {
        if let (Ok(plan), Some((inputs, expected))) = (
            escape_time_plan(domain, form, ESCAPE_TIME_CASE_ITERATIONS),
            batches,
        ) {
            cases.push(ExpectedCase {
                plan,
                inputs,
                expected,
            });
        }
    }
    Some(cases)
}

pub const ITERATION_CASE_MAXIMUM_COUNT: u32 = 8;

const DOUBLING_EXIT_ABOVE: f32 = 100.0;
const SQUARING_EXIT_ABOVE: f32 = 4.0;

fn domain_constant(domain: Domain, value: f32) -> Constant {
    match domain {
        Domain::F32 => Constant::F32(value),
        Domain::F64 => Constant::F64(f64::from(value)),
    }
}

fn single_column(domain: Domain, values: Vec<f32>) -> Option<Batch> {
    let length = values.len();
    match domain {
        Domain::F32 => Batch::from_f32_columns(length, vec![values]).ok(),
        Domain::F64 => {
            Batch::from_f64_columns(length, vec![values.into_iter().map(f64::from).collect()]).ok()
        }
    }
}

fn iteration_inputs() -> Vec<f32> {
    vec![
        3.0,
        50.0,
        101.0,
        -1.0,
        0.0,
        -0.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MAX,
        f32::from_bits(1),
        1.0e30,
    ]
}

#[derive(Clone, Copy)]
enum Growth {
    Doubling,
    Squaring,
    Sine,
}

fn scalar_iteration_plan(
    domain: Domain,
    growth: Growth,
    exit_above: f32,
    maximum_count: u32,
) -> Option<Plan> {
    let factor = match growth {
        Growth::Doubling | Growth::Sine => BodyGate::Constant(domain_constant(domain, 2.0)),
        Growth::Squaring => BodyGate::State(SlotId(0)),
    };
    let step = match growth {
        Growth::Sine => BodyGate::Operation {
            operation: PlanOp::Sin,
            arguments: vec![BodyGateId(0)],
        },
        Growth::Doubling | Growth::Squaring => BodyGate::Operation {
            operation: PlanOp::Mul,
            arguments: vec![BodyGateId(0), BodyGateId(2)],
        },
    };
    let iteration = Iteration {
        initial: vec![GateId(0)],
        body: vec![
            BodyGate::State(SlotId(0)),
            BodyGate::Outer(GateId(1)),
            factor,
            step,
            BodyGate::Operation {
                operation: PlanOp::Greater,
                arguments: vec![BodyGateId(0), BodyGateId(1)],
            },
        ],
        next: vec![BodyGateId(3)],
        exit: BodyGateId(4),
        maximum_count,
    };
    let gates = vec![
        Gate::Input(ChannelId(0)),
        Gate::Constant(domain_constant(domain, exit_above)),
        Gate::Iterate(Box::new(iteration)),
        Gate::IterationState {
            iteration: GateId(2),
            slot: SlotId(0),
        },
    ];
    Plan::new(domain, 1, gates, vec![GateId(3)], None).ok()
}

fn boolean_iteration_plan(domain: Domain, maximum_count: u32) -> Option<Plan> {
    let iteration = Iteration {
        initial: vec![GateId(0), GateId(3)],
        body: vec![
            BodyGate::State(SlotId(0)),
            BodyGate::State(SlotId(1)),
            BodyGate::Outer(GateId(1)),
            BodyGate::Outer(GateId(2)),
            BodyGate::Operation {
                operation: PlanOp::Add,
                arguments: vec![BodyGateId(0), BodyGateId(2)],
            },
            BodyGate::Operation {
                operation: PlanOp::Not,
                arguments: vec![BodyGateId(1)],
            },
            BodyGate::Operation {
                operation: PlanOp::GreaterOrEqual,
                arguments: vec![BodyGateId(0), BodyGateId(3)],
            },
        ],
        next: vec![BodyGateId(4), BodyGateId(5)],
        exit: BodyGateId(6),
        maximum_count,
    };
    let gates = vec![
        Gate::Input(ChannelId(0)),
        Gate::Constant(domain_constant(domain, 1.0)),
        Gate::Constant(domain_constant(domain, 5.0)),
        Gate::Operation {
            operation: PlanOp::Less,
            arguments: vec![GateId(0), GateId(1)],
        },
        Gate::Iterate(Box::new(iteration)),
        Gate::IterationState {
            iteration: GateId(4),
            slot: SlotId(0),
        },
        Gate::IterationState {
            iteration: GateId(4),
            slot: SlotId(1),
        },
        Gate::Constant(domain_constant(domain, 0.0)),
        Gate::Operation {
            operation: PlanOp::Select,
            arguments: vec![GateId(6), GateId(1), GateId(7)],
        },
    ];
    Plan::new(domain, 1, gates, vec![GateId(5), GateId(8)], None).ok()
}

pub fn iteration_cases(domain: Domain) -> Vec<Case> {
    let plans = [
        scalar_iteration_plan(
            domain,
            Growth::Doubling,
            DOUBLING_EXIT_ABOVE,
            ITERATION_CASE_MAXIMUM_COUNT,
        ),
        scalar_iteration_plan(domain, Growth::Doubling, DOUBLING_EXIT_ABOVE, 0),
        scalar_iteration_plan(
            domain,
            Growth::Squaring,
            SQUARING_EXIT_ABOVE,
            ITERATION_CASE_MAXIMUM_COUNT,
        ),
        boolean_iteration_plan(domain, ITERATION_CASE_MAXIMUM_COUNT),
        scalar_iteration_plan(
            domain,
            Growth::Sine,
            DOUBLING_EXIT_ABOVE,
            ITERATION_CASE_MAXIMUM_COUNT,
        ),
    ];
    plans
        .into_iter()
        .filter_map(|plan| {
            Some(Case {
                plan: plan?,
                inputs: single_column(domain, iteration_inputs())?,
            })
        })
        .collect()
}

pub struct EquivalentCase {
    pub case: Case,
    pub reference_plan: Plan,
}

pub fn escape_time_equivalent_cases(domain: Domain, iterations: u32) -> Vec<EquivalentCase> {
    let (c_real, c_imaginary) = (domain_constant(domain, -1.0), domain_constant(domain, 0.0));
    let forms = [
        EscapeTimePlanForm::Parameter,
        EscapeTimePlanForm::Initial {
            c_real,
            c_imaginary,
        },
    ];
    let first = vec![
        0.0,
        1.0,
        -2.0,
        0.0,
        0.25,
        -0.75,
        2.0,
        1.0e10,
        f32::NAN,
        f32::INFINITY,
    ];
    let second = vec![0.0, 0.0, 0.0, 3.0, 0.5, 0.1, 0.0, 0.0, 0.0, 0.0];
    forms
        .into_iter()
        .filter_map(|form| {
            let length = first.len();
            let inputs = match domain {
                Domain::F32 => {
                    Batch::from_f32_columns(length, vec![first.clone(), second.clone()]).ok()?
                }
                Domain::F64 => Batch::from_f64_columns(
                    length,
                    vec![
                        first.iter().copied().map(f64::from).collect(),
                        second.iter().copied().map(f64::from).collect(),
                    ],
                )
                .ok()?,
            };
            Some(EquivalentCase {
                case: Case {
                    plan: escape_time_iteration_plan(domain, form, iterations).ok()?,
                    inputs,
                },
                reference_plan: escape_time_plan(domain, form, iterations).ok()?,
            })
        })
        .collect()
}

pub fn constant_chain_case(domain: Domain) -> Option<Case> {
    let mut builder = CircuitBuilder::with_inputs(domain, 1);
    let tenth = builder.constant(0.1);
    let square = builder.operation(PlanOp::Mul, &[tenth, tenth]);
    let hundred = builder.constant(100.0);
    let scaled = builder.operation(PlanOp::Mul, &[square, hundred]);
    let plan = builder.finish(1, scaled, None)?;
    let inputs = match domain {
        Domain::F32 => Batch::from_f32_columns(1, vec![vec![0.0]]).ok()?,
        Domain::F64 => Batch::from_f64_columns(1, vec![vec![0.0]]).ok()?,
    };
    Some(Case { plan, inputs })
}
