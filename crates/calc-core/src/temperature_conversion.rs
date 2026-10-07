use calc_expr::{BuildError, ExprId, ExprPool, Head, Operator};
use calc_numbers::{ExactArithmeticError, Integer, Number};
use calc_units::TemperatureScale;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemperatureDirection {
    FromReading,
    ToReading,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TemperatureConversion {
    pub scale: TemperatureScale,
    pub direction: TemperatureDirection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AffineMap {
    pub factor: Number,
    pub addend: Number,
}

impl TemperatureConversion {
    pub fn of_operator(operator: Operator) -> Option<Self> {
        let (scale, direction) = match operator {
            Operator::FromCelsius => (TemperatureScale::Celsius, TemperatureDirection::FromReading),
            Operator::FromFahrenheit => (
                TemperatureScale::Fahrenheit,
                TemperatureDirection::FromReading,
            ),
            Operator::ToCelsius => (TemperatureScale::Celsius, TemperatureDirection::ToReading),
            Operator::ToFahrenheit => (
                TemperatureScale::Fahrenheit,
                TemperatureDirection::ToReading,
            ),
            _ => return None,
        };
        Some(Self { scale, direction })
    }

    pub fn affine_map(self) -> AffineMap {
        let factor = self.scale.factor_ratio();
        let offset = self.scale.offset_ratio();
        let factor_numerator = Integer::from(factor.numerator.get());
        let factor_denominator = Integer::from(factor.denominator.get());
        let offset_numerator = Integer::from(offset.numerator);
        let offset_denominator = Integer::from(offset.denominator.get());
        let nonzero_fraction = |numerator: &Integer, denominator: &Integer| match Number::fraction(
            numerator,
            denominator,
        ) {
            Ok(number) => number,
            Err(ExactArithmeticError::DivisionByZero | ExactArithmeticError::MachineOperand) => {
                unreachable!()
            }
        };
        match self.direction {
            TemperatureDirection::FromReading => AffineMap {
                factor: factor.to_number(),
                addend: nonzero_fraction(
                    &(&offset_numerator * &factor_numerator),
                    &(&offset_denominator * &factor_denominator),
                ),
            },
            TemperatureDirection::ToReading => AffineMap {
                factor: nonzero_fraction(&factor_denominator, &factor_numerator),
                addend: nonzero_fraction(&offset_numerator.negated(), &offset_denominator),
            },
        }
    }
}

pub fn affine_map_expression(
    pool: &mut ExprPool,
    conversion: TemperatureConversion,
    argument: ExprId,
) -> Result<ExprId, BuildError> {
    let map = conversion.affine_map();
    let factor = pool.number(map.factor)?;
    let addend = pool.number(map.addend)?;
    let product = pool.apply(Head::Operator(Operator::Mul), &[factor, argument])?;
    pool.apply(Head::Operator(Operator::Add), &[product, addend])
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_conformance::{ExpectedCase, check_expected};
    use calc_exec::{
        Batch, ChannelId, Constant, Domain, Gate, GateId, LowerInput, LowerSpec, PlanOp,
        Preference, ValueForm, lower,
    };
    use calc_exec_cpu::CpuBackend;
    use calc_expr::SymbolKind;
    use calc_numbers::Integer;
    use calc_units::Dimension;

    use crate::computed_result::{ResultValue, RoundingError};
    use crate::exact_evaluation::{ExactEvaluationError, evaluate_exact};
    use crate::machine_evaluation::evaluate_f64;
    use crate::quantities::{QuantityError, to_coherent_units};
    use crate::uncertainty::{central_expression, with_propagated_uncertainty};

    fn fraction(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    fn apply(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> ExprId {
        pool.apply(Head::Operator(operator), arguments).unwrap()
    }

    fn number(pool: &mut ExprPool, value: Number) -> ExprId {
        pool.number(value).unwrap()
    }

    fn quantity(pool: &mut ExprPool, value: Number, unit: &str) -> ExprId {
        let value = number(pool, value);
        let unit = pool.units_mut().lookup(unit).unwrap();
        pool.quantity(value, unit).unwrap()
    }

    fn exact(pool: &mut ExprPool, root: ExprId) -> Number {
        evaluate_exact(pool, root)
            .unwrap()
            .rational_value()
            .unwrap()
            .clone()
    }

    fn variable(pool: &mut ExprPool) -> (calc_expr::SymbolId, ExprId) {
        let symbol = pool.intern_symbol("x", SymbolKind::Variable).unwrap();
        (symbol, pool.symbol(symbol).unwrap())
    }

    #[test]
    fn from_celsius_twenty_is_two_hundred_ninety_three_point_one_five_kelvin() {
        let mut pool = ExprPool::new();
        let reading = number(&mut pool, Number::from(20_i64));
        let root = apply(&mut pool, Operator::FromCelsius, &[reading]);

        assert_eq!(exact(&mut pool, root), fraction(29315, 100));
    }

    #[test]
    fn from_fahrenheit_ninety_eight_point_six_is_thirty_seven_degrees_celsius_in_kelvin() {
        let mut pool = ExprPool::new();
        let reading = number(&mut pool, fraction(986, 10));
        let root = apply(&mut pool, Operator::FromFahrenheit, &[reading]);

        assert_eq!(exact(&mut pool, root), fraction(31015, 100));
    }

    #[test]
    fn to_celsius_of_three_hundred_ten_kelvin_is_thirty_six_point_eight_five() {
        let mut pool = ExprPool::new();
        let temperature = quantity(&mut pool, Number::from(310_i64), "K");
        let root = apply(&mut pool, Operator::ToCelsius, &[temperature]);

        assert_eq!(exact(&mut pool, root), fraction(3685, 100));
    }

    #[test]
    fn to_fahrenheit_takes_millikelvin_to_kelvin_first() {
        let mut pool = ExprPool::new();
        let temperature = quantity(&mut pool, Number::from(300_000_i64), "mK");
        let root = apply(&mut pool, Operator::ToFahrenheit, &[temperature]);

        assert_eq!(exact(&mut pool, root), fraction(8033, 100));
    }

    #[test]
    fn to_celsius_refuses_a_celsius_difference() {
        let mut pool = ExprPool::new();
        let difference = quantity(&mut pool, Number::from(8_i64), "degC");
        let root = apply(&mut pool, Operator::ToCelsius, &[difference]);

        assert!(matches!(
            evaluate_exact(&mut pool, root),
            Err(ExactEvaluationError::Quantity(
                QuantityError::DifferenceAsReading { .. }
            ))
        ));
    }

    #[test]
    fn a_celsius_reading_below_absolute_zero_is_refused() {
        let mut pool = ExprPool::new();
        let reading = number(&mut pool, Number::from(-300_i64));
        let root = apply(&mut pool, Operator::FromCelsius, &[reading]);

        assert!(matches!(
            evaluate_exact(&mut pool, root),
            Err(ExactEvaluationError::Quantity(
                QuantityError::BelowAbsoluteZero(_)
            ))
        ));
    }

    #[test]
    fn difference_of_two_celsius_readings_is_a_kelvin_difference() {
        let mut pool = ExprPool::new();
        let twenty = number(&mut pool, Number::from(20_i64));
        let twelve = number(&mut pool, Number::from(12_i64));
        let warm = apply(&mut pool, Operator::FromCelsius, &[twenty]);
        let cold = apply(&mut pool, Operator::FromCelsius, &[twelve]);
        let root = apply(&mut pool, Operator::Sub, &[warm, cold]);

        let coherent = to_coherent_units(&mut pool, root).unwrap();

        assert_eq!(
            (exact(&mut pool, root), coherent.dimension),
            (
                Number::from(8_i64),
                Dimension::from_exponents([0, 0, 0, 0, 1, 0, 0, 0])
            )
        );
    }

    #[test]
    fn to_reading_inverts_from_reading() {
        let mut pool = ExprPool::new();
        let reading = number(&mut pool, fraction(-1013, 7));
        let kelvin = apply(&mut pool, Operator::FromFahrenheit, &[reading]);
        let root = apply(&mut pool, Operator::ToFahrenheit, &[kelvin]);

        assert_eq!(exact(&mut pool, root), fraction(-1013, 7));
    }

    #[test]
    fn from_celsius_of_a_quantity_is_a_dimension_error() {
        let mut pool = ExprPool::new();
        let difference = quantity(&mut pool, Number::from(20_i64), "degC");
        let root = apply(&mut pool, Operator::FromCelsius, &[difference]);

        let result = evaluate_exact(&mut pool, root);

        assert!(matches!(
            result,
            Err(ExactEvaluationError::Quantity(
                QuantityError::DimensionedArgument { .. }
            ))
        ));
    }

    #[test]
    fn to_fahrenheit_of_a_number_is_a_dimension_error() {
        let mut pool = ExprPool::new();
        let bare = number(&mut pool, Number::from(20_i64));
        let root = apply(&mut pool, Operator::ToFahrenheit, &[bare]);

        let result = evaluate_exact(&mut pool, root);

        assert!(matches!(
            result,
            Err(ExactEvaluationError::Quantity(
                QuantityError::DimensionMismatch { .. }
            ))
        ));
    }

    #[test]
    fn to_celsius_of_a_length_is_a_dimension_error() {
        let mut pool = ExprPool::new();
        let length = quantity(&mut pool, Number::from(20_i64), "m");
        let root = apply(&mut pool, Operator::ToCelsius, &[length]);

        let result = evaluate_exact(&mut pool, root);

        assert!(matches!(
            result,
            Err(ExactEvaluationError::Quantity(
                QuantityError::DimensionMismatch { .. }
            ))
        ));
    }

    #[test]
    fn expansion_keeps_the_written_expression() {
        let mut pool = ExprPool::new();
        let reading = number(&mut pool, Number::from(20_i64));
        let root = apply(&mut pool, Operator::FromCelsius, &[reading]);

        to_coherent_units(&mut pool, root).unwrap();

        assert!(matches!(
            pool.node(root).unwrap(),
            calc_expr::NodeView::Apply {
                head: Head::Operator(Operator::FromCelsius),
                ..
            }
        ));
    }

    #[test]
    fn fahrenheit_reading_uncertainty_is_five_eighteenths_kelvin() {
        let mut pool = ExprPool::new();
        let value = number(&mut pool, Number::from(68_i64));
        let half = number(&mut pool, fraction(1, 2));
        let mark = pool.measurement().unwrap();
        let measured = apply(&mut pool, Operator::Uncertain, &[value, half, mark]);
        let root = apply(&mut pool, Operator::FromFahrenheit, &[measured]);
        let central = central_expression(&mut pool, root).unwrap();
        let backend = CpuBackend::new();
        let evaluation =
            evaluate_f64(&mut pool, central, &[&backend], Preference::Automatic).unwrap();

        let result =
            with_propagated_uncertainty(&mut pool, root, evaluation.result().clone()).unwrap();

        assert_eq!(
            result.uncertainty().unwrap().standard(),
            &ResultValue::Number(Number::F64(0.5 * (5.0 / 9.0)))
        );
    }

    #[test]
    fn machine_rounding_bound_encloses_the_exact_fahrenheit_value() {
        let mut pool = ExprPool::new();
        let kelvin = quantity(&mut pool, fraction(31015, 100), "K");
        let root = apply(&mut pool, Operator::ToFahrenheit, &[kelvin]);
        let backend = CpuBackend::new();

        let evaluation = evaluate_f64(&mut pool, root, &[&backend], Preference::Automatic).unwrap();

        let ResultValue::Number(Number::F64(value)) = evaluation.result().value() else {
            panic!("{:?}", evaluation.result().value());
        };
        let RoundingError::Bound(ResultValue::Number(bound)) = evaluation.result().rounding_error()
        else {
            panic!("{:?}", evaluation.result().rounding_error());
        };
        let exact_value = fraction(986, 10);
        let lower = exact_value.sub_exact(&bound.to_exact().unwrap()).unwrap();
        let upper = exact_value.add_exact(&bound.to_exact().unwrap()).unwrap();
        let computed = Number::F64(*value).to_exact().unwrap();
        assert!(is_non_negative(&computed.sub_exact(&lower).unwrap()));
        assert!(is_non_negative(&upper.sub_exact(&computed).unwrap()));
    }

    fn is_non_negative(number: &Number) -> bool {
        match number {
            Number::Integer(integer) => !integer.is_negative(),
            Number::Rational(rational) => !rational.numerator().is_negative(),
            Number::F32(_) | Number::F64(_) => false,
        }
    }

    fn lowered_plan(operator: Operator) -> calc_exec::Plan {
        let mut pool = ExprPool::new();
        let (symbol, x) = variable(&mut pool);
        let argument = match TemperatureConversion::of_operator(operator).map(|c| c.direction) {
            Some(TemperatureDirection::ToReading) => {
                let kelvin = pool.units_mut().lookup("K").unwrap();
                pool.quantity(x, kelvin).unwrap()
            }
            _ => x,
        };
        let root = apply(&mut pool, operator, &[argument]);
        let coherent = to_coherent_units(&mut pool, root).unwrap();
        let spec = LowerSpec {
            domain: Domain::F64,
            inputs: vec![LowerInput {
                symbol,
                form: ValueForm::Real,
            }],
            output: ValueForm::Real,
        };
        lower(&pool, coherent.expression, &spec).unwrap()
    }

    fn constant_bits(gate: &Gate) -> Option<u64> {
        match gate {
            Gate::Constant(Constant::F64(value)) => Some(value.to_bits()),
            _ => None,
        }
    }

    #[test]
    fn from_fahrenheit_lowers_to_mul_then_add_with_rounded_constants() {
        let plan = lowered_plan(Operator::FromFahrenheit);

        let gates = plan.gates();

        assert_eq!(gates.len(), 5);
        assert!(matches!(gates[0], Gate::Input(ChannelId(0))));
        assert_eq!(constant_bits(&gates[1]), Some((5.0_f64 / 9.0).to_bits()));
        assert!(matches!(
            &gates[2],
            Gate::Operation { operation: PlanOp::Mul, arguments } if arguments == &[GateId(1), GateId(0)]
        ));
        assert_eq!(
            constant_bits(&gates[3]),
            Some((45967.0_f64 / 180.0).to_bits())
        );
        assert!(matches!(
            &gates[4],
            Gate::Operation { operation: PlanOp::Add, arguments } if arguments == &[GateId(2), GateId(3)]
        ));
    }

    #[test]
    fn no_temperature_operator_lowers_to_a_fused_multiply_add() {
        let fused: Vec<Operator> = [
            Operator::FromCelsius,
            Operator::FromFahrenheit,
            Operator::ToCelsius,
            Operator::ToFahrenheit,
        ]
        .into_iter()
        .filter(|operator| {
            lowered_plan(*operator).gates().iter().any(|gate| {
                matches!(
                    gate,
                    Gate::Operation {
                        operation: PlanOp::MulAdd,
                        ..
                    }
                )
            })
        })
        .collect();

        assert!(fused.is_empty(), "{fused:?}");
    }

    fn conformance_case(operator: Operator, factor: f64, addend: f64) -> ExpectedCase {
        let readings = vec![
            -459.67,
            -273.15,
            -40.0,
            0.0,
            -0.0,
            20.5,
            98.6,
            100.0,
            310.15,
            1e300,
            -1e300,
            f64::MAX,
        ];
        let expected = readings
            .iter()
            .map(|reading| factor * reading + addend)
            .collect::<Vec<f64>>();
        ExpectedCase {
            plan: lowered_plan(operator),
            inputs: Batch::from_f64_columns(readings.len(), vec![readings.clone()]).unwrap(),
            expected: Batch::from_f64_columns(expected.len(), vec![expected]).unwrap(),
        }
    }

    #[test]
    fn from_celsius_conforms_to_one_times_reading_plus_273_15() {
        let case = conformance_case(Operator::FromCelsius, 1.0, 273.15);

        assert_eq!(check_expected(&CpuBackend::new(), &case), Ok(()));
    }

    #[test]
    fn from_fahrenheit_conforms_to_five_ninths_times_reading_plus_45967_over_180() {
        let case = conformance_case(Operator::FromFahrenheit, 5.0 / 9.0, 45967.0 / 180.0);

        assert_eq!(check_expected(&CpuBackend::new(), &case), Ok(()));
    }

    #[test]
    fn to_celsius_conforms_to_one_times_kelvin_minus_273_15() {
        let case = conformance_case(Operator::ToCelsius, 1.0, -273.15);

        assert_eq!(check_expected(&CpuBackend::new(), &case), Ok(()));
    }

    #[test]
    fn to_fahrenheit_conforms_to_nine_fifths_times_kelvin_minus_459_67() {
        let case = conformance_case(Operator::ToFahrenheit, 1.8, -459.67);

        assert_eq!(check_expected(&CpuBackend::new(), &case), Ok(()));
    }
}
