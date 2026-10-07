use crate::domain::{Constant, Domain};
use crate::iteration::{BodyGate, BodyGateId, Iteration, SlotId};
use crate::operation::PlanOp;
use crate::plan::{ChannelId, Gate, GateId, Plan, PlanError};

const ESCAPE_RADIUS_SQUARED_FLOOR: f32 = 4.0;

#[derive(Clone, Copy, Debug)]
pub enum EscapeTimePlanForm {
    Parameter,
    Initial {
        c_real: Constant,
        c_imaginary: Constant,
    },
}

struct Circuit {
    domain: Domain,
    gates: Vec<Gate>,
}

impl Circuit {
    fn push(&mut self, gate: Gate) -> GateId {
        self.gates.push(gate);
        GateId(u32::try_from(self.gates.len() - 1).unwrap_or(u32::MAX))
    }

    fn constant(&mut self, value: f32) -> GateId {
        let constant = match self.domain {
            Domain::F32 => Constant::F32(value),
            Domain::F64 => Constant::F64(f64::from(value)),
        };
        self.push(Gate::Constant(constant))
    }

    fn operation(&mut self, operation: PlanOp, arguments: &[GateId]) -> GateId {
        self.push(Gate::Operation {
            operation,
            arguments: arguments.to_vec(),
        })
    }
}

pub fn escape_time_plan(
    domain: Domain,
    form: EscapeTimePlanForm,
    iterations: u32,
) -> Result<Plan, PlanError> {
    let mut circuit = Circuit {
        domain,
        gates: vec![Gate::Input(ChannelId(0)), Gate::Input(ChannelId(1))],
    };
    let first_input = GateId(0);
    let second_input = GateId(1);
    let four = circuit.constant(ESCAPE_RADIUS_SQUARED_FLOOR);
    let zero = circuit.constant(0.0);
    let one = circuit.constant(1.0);
    let (c_real, c_imaginary, mut x, mut y) = match form {
        EscapeTimePlanForm::Parameter => (first_input, second_input, zero, zero),
        EscapeTimePlanForm::Initial {
            c_real,
            c_imaginary,
        } => {
            let real = circuit.push(Gate::Constant(c_real));
            let imaginary = circuit.push(Gate::Constant(c_imaginary));
            (real, imaginary, first_input, second_input)
        }
    };
    let c_real_squared = circuit.operation(PlanOp::Mul, &[c_real, c_real]);
    let c_imaginary_squared = circuit.operation(PlanOp::Mul, &[c_imaginary, c_imaginary]);
    let c_modulus_squared = circuit.operation(PlanOp::Add, &[c_real_squared, c_imaginary_squared]);
    let radius_squared = circuit.operation(PlanOp::Max, &[four, c_modulus_squared]);
    let mut escaped: Option<GateId> = None;
    let mut count = zero;
    for step in 0..=iterations {
        let x_squared = circuit.operation(PlanOp::Mul, &[x, x]);
        let y_squared = circuit.operation(PlanOp::Mul, &[y, y]);
        let modulus_squared = circuit.operation(PlanOp::Add, &[x_squared, y_squared]);
        let is_outside = circuit.operation(PlanOp::Greater, &[modulus_squared, radius_squared]);
        let sticky = match escaped {
            Some(previous) => circuit.operation(PlanOp::Or, &[previous, is_outside]),
            None => is_outside,
        };
        escaped = Some(sticky);
        let incremented = circuit.operation(PlanOp::Add, &[count, one]);
        count = circuit.operation(PlanOp::Select, &[sticky, count, incremented]);
        if step < iterations {
            let difference = circuit.operation(PlanOp::Sub, &[x_squared, y_squared]);
            let next_x = circuit.operation(PlanOp::Add, &[difference, c_real]);
            let doubled = circuit.operation(PlanOp::Add, &[x, x]);
            let product = circuit.operation(PlanOp::Mul, &[doubled, y]);
            let next_y = circuit.operation(PlanOp::Add, &[product, c_imaginary]);
            x = next_x;
            y = next_y;
        }
    }
    let Some(final_flag) = escaped else {
        return Err(PlanError::NoOutputs);
    };
    let escaped_scalar = circuit.operation(PlanOp::Select, &[final_flag, one, zero]);
    let input_count = 2;
    Plan::new(
        domain,
        input_count,
        circuit.gates,
        vec![count, escaped_scalar],
        None,
    )
}

struct Body {
    gates: Vec<BodyGate>,
}

impl Body {
    fn push(&mut self, gate: BodyGate) -> BodyGateId {
        self.gates.push(gate);
        BodyGateId(u32::try_from(self.gates.len() - 1).unwrap_or(u32::MAX))
    }

    fn operation(&mut self, operation: PlanOp, arguments: &[BodyGateId]) -> BodyGateId {
        self.push(BodyGate::Operation {
            operation,
            arguments: arguments.to_vec(),
        })
    }
}

const X_SLOT: SlotId = SlotId(0);
const Y_SLOT: SlotId = SlotId(1);
const COUNT_SLOT: SlotId = SlotId(2);

pub fn escape_time_iteration_plan(
    domain: Domain,
    form: EscapeTimePlanForm,
    iterations: u32,
) -> Result<Plan, PlanError> {
    let mut circuit = Circuit {
        domain,
        gates: vec![Gate::Input(ChannelId(0)), Gate::Input(ChannelId(1))],
    };
    let first_input = GateId(0);
    let second_input = GateId(1);
    let four = circuit.constant(ESCAPE_RADIUS_SQUARED_FLOOR);
    let zero = circuit.constant(0.0);
    let one = circuit.constant(1.0);
    let (c_real, c_imaginary, x, y) = match form {
        EscapeTimePlanForm::Parameter => (first_input, second_input, zero, zero),
        EscapeTimePlanForm::Initial {
            c_real,
            c_imaginary,
        } => {
            let real = circuit.push(Gate::Constant(c_real));
            let imaginary = circuit.push(Gate::Constant(c_imaginary));
            (real, imaginary, first_input, second_input)
        }
    };
    let c_real_squared = circuit.operation(PlanOp::Mul, &[c_real, c_real]);
    let c_imaginary_squared = circuit.operation(PlanOp::Mul, &[c_imaginary, c_imaginary]);
    let c_modulus_squared = circuit.operation(PlanOp::Add, &[c_real_squared, c_imaginary_squared]);
    let radius_squared = circuit.operation(PlanOp::Max, &[four, c_modulus_squared]);

    let mut body = Body { gates: Vec::new() };
    let body_x = body.push(BodyGate::State(X_SLOT));
    let body_y = body.push(BodyGate::State(Y_SLOT));
    let body_count = body.push(BodyGate::State(COUNT_SLOT));
    let body_radius_squared = body.push(BodyGate::Outer(radius_squared));
    let body_c_real = body.push(BodyGate::Outer(c_real));
    let body_c_imaginary = body.push(BodyGate::Outer(c_imaginary));
    let body_one = body.push(BodyGate::Outer(one));
    let x_squared = body.operation(PlanOp::Mul, &[body_x, body_x]);
    let y_squared = body.operation(PlanOp::Mul, &[body_y, body_y]);
    let modulus_squared = body.operation(PlanOp::Add, &[x_squared, y_squared]);
    let is_outside = body.operation(PlanOp::Greater, &[modulus_squared, body_radius_squared]);
    let incremented = body.operation(PlanOp::Add, &[body_count, body_one]);
    let difference = body.operation(PlanOp::Sub, &[x_squared, y_squared]);
    let next_x = body.operation(PlanOp::Add, &[difference, body_c_real]);
    let doubled = body.operation(PlanOp::Add, &[body_x, body_x]);
    let product = body.operation(PlanOp::Mul, &[doubled, body_y]);
    let next_y = body.operation(PlanOp::Add, &[product, body_c_imaginary]);

    let iterate = circuit.push(Gate::Iterate(Box::new(Iteration {
        initial: vec![x, y, zero],
        body: body.gates,
        next: vec![next_x, next_y, incremented],
        exit: is_outside,
        maximum_count: iterations,
    })));
    let final_x = circuit.push(Gate::IterationState {
        iteration: iterate,
        slot: X_SLOT,
    });
    let final_y = circuit.push(Gate::IterationState {
        iteration: iterate,
        slot: Y_SLOT,
    });
    let final_count = circuit.push(Gate::IterationState {
        iteration: iterate,
        slot: COUNT_SLOT,
    });
    let final_x_squared = circuit.operation(PlanOp::Mul, &[final_x, final_x]);
    let final_y_squared = circuit.operation(PlanOp::Mul, &[final_y, final_y]);
    let final_modulus_squared = circuit.operation(PlanOp::Add, &[final_x_squared, final_y_squared]);
    let escaped = circuit.operation(PlanOp::Greater, &[final_modulus_squared, radius_squared]);
    let final_incremented = circuit.operation(PlanOp::Add, &[final_count, one]);
    let count = circuit.operation(PlanOp::Select, &[escaped, final_count, final_incremented]);
    let escaped_scalar = circuit.operation(PlanOp::Select, &[escaped, one, zero]);
    let input_count = 2;
    Plan::new(
        domain,
        input_count,
        circuit.gates,
        vec![count, escaped_scalar],
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iteration_plan_keeps_one_body_whatever_the_limit() {
        let small =
            escape_time_iteration_plan(Domain::F64, EscapeTimePlanForm::Parameter, 4).unwrap();
        let large =
            escape_time_iteration_plan(Domain::F64, EscapeTimePlanForm::Parameter, 65536).unwrap();

        let counts = (small.gate_count(), large.gate_count());

        assert_eq!(counts.0, counts.1);
    }

    #[test]
    fn iteration_plan_estimates_the_body_at_its_limit() {
        let plan =
            escape_time_iteration_plan(Domain::F64, EscapeTimePlanForm::Parameter, 10).unwrap();

        let evaluations = plan.worst_case_gate_evaluations_per_element();

        assert_eq!(evaluations, 9 + 17 * 10 + 7);
    }

    #[test]
    fn plan_has_twelve_gates_per_iteration_and_a_fixed_frame() {
        let plan = escape_time_plan(Domain::F64, EscapeTimePlanForm::Parameter, 10).unwrap();

        assert_eq!(plan.gate_count(), 2 + 3 + 4 + 7 * 11 - 1 + 5 * 10 + 1);
    }

    #[test]
    fn plan_outputs_the_count_and_the_escaped_flag() {
        let plan = escape_time_plan(Domain::F32, EscapeTimePlanForm::Parameter, 3).unwrap();

        assert_eq!(
            (plan.output_channel_count(), plan.input_channel_count()),
            (2, 2)
        );
    }

    #[test]
    fn initial_form_takes_its_parameter_as_constants() {
        let form = EscapeTimePlanForm::Initial {
            c_real: Constant::F64(-1.0),
            c_imaginary: Constant::F64(0.0),
        };

        let plan = escape_time_plan(Domain::F64, form, 2).unwrap();

        let constants = plan
            .gates()
            .iter()
            .filter(|gate| matches!(gate, Gate::Constant(_)))
            .count();
        assert_eq!(constants, 5);
    }

    #[test]
    fn constants_of_the_other_domain_are_a_plan_error() {
        let form = EscapeTimePlanForm::Initial {
            c_real: Constant::F32(-1.0),
            c_imaginary: Constant::F32(0.0),
        };

        let result = escape_time_plan(Domain::F64, form, 2);

        assert!(matches!(
            result,
            Err(PlanError::ConstantDomainMismatch { .. })
        ));
    }
}
