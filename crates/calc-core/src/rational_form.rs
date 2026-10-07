use calc_expr::{
    BinderKind, BuiltinConstant, ExprId, ExprPool, Head, IntegerTypeSpelling, NodeView, Operator,
};
use calc_numbers::{Integer, Number};
use calc_units::{ScaleFactor, UnitId};

use crate::exact_evaluation::evaluate_exact;

const KELVIN_SYMBOL: &str = "K";
const DECIMAL_PRIMES: [u64; 2] = [2, 5];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RationalForm {
    Decimal,
    Fraction,
    Radix(RadixForm),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadixForm {
    pub base: u32,
    pub bits: u32,
    pub signed: bool,
    pub big_endian: Option<bool>,
}

fn small_literal(pool: &ExprPool, expression: ExprId) -> Option<u32> {
    let NodeView::Number(number) = pool.node(expression).ok()? else {
        return None;
    };
    match pool.number_value(number).ok()? {
        Number::Integer(integer) => u32::try_from(integer.to_i64()?).ok(),
        _ => None,
    }
}

fn is_signed_type(pool: &ExprPool, spelling: ExprId) -> Option<bool> {
    let code = i64::from(small_literal(pool, spelling)?);
    Some(IntegerTypeSpelling::from_code(code).is_some_and(IntegerTypeSpelling::is_signed))
}

pub fn radix_form(pool: &ExprPool, expression: ExprId) -> Option<RadixForm> {
    match pool.node(expression).ok()? {
        NodeView::Apply {
            head: Head::Operator(Operator::Radix),
            arguments: [_, base, bits, spelling],
        } => Some(RadixForm {
            base: small_literal(pool, *base)?,
            bits: small_literal(pool, *bits)?,
            signed: is_signed_type(pool, *spelling)?,
            big_endian: None,
        }),
        NodeView::Apply {
            head: Head::Operator(Operator::Bytes),
            arguments: [_, bits, spelling, order],
        } => Some(RadixForm {
            base: 16,
            bits: small_literal(pool, *bits)?,
            signed: is_signed_type(pool, *spelling)?,
            big_endian: Some(small_literal(pool, *order)? != 0),
        }),
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct Written {
    unit: Option<UnitId>,
    has_decimal: bool,
    has_division: bool,
    has_decimal_factor: bool,
}

impl Written {
    fn whole(unit: Option<UnitId>) -> Self {
        Self {
            unit,
            has_decimal: false,
            has_division: false,
            has_decimal_factor: false,
        }
    }

    fn with_decimal_factor(self, has_decimal_factor: bool) -> Self {
        Self {
            has_decimal_factor: self.has_decimal_factor || has_decimal_factor,
            ..self
        }
    }

    fn is_decimal(&self) -> bool {
        self.has_decimal || (self.has_decimal_factor && !self.has_division)
    }
}

fn has_only_decimal_primes(denominator: &Integer) -> bool {
    let mut rest = denominator.absolute();
    for prime in DECIMAL_PRIMES {
        let prime = Integer::from(prime);
        while let Ok((quotient, remainder)) = rest.div_rem_euclid(&prime) {
            if !remainder.is_zero() || rest.is_zero() {
                break;
            }
            rest = quotient;
        }
    }
    rest.is_one()
}

fn is_terminating_factor(factor: &ScaleFactor) -> bool {
    factor.pi_exponent() == 0 && has_only_decimal_primes(factor.denominator())
}

fn decimal_conversion(pool: &ExprPool, from: Option<UnitId>, to: Option<UnitId>) -> Option<bool> {
    let units = pool.units();
    let from = from.unwrap_or_else(|| units.dimensionless());
    let to = to.unwrap_or_else(|| units.dimensionless());
    let factor = units.conversion_factor(from, to).ok()?;
    let is_binary = units.has_binary_prefix(from).ok()? || units.has_binary_prefix(to).ok()?;
    is_terminating_factor(&factor).then(|| factor != ScaleFactor::one() && !is_binary)
}

fn is_decimal_literal(number: &Number) -> Option<bool> {
    match number {
        Number::Integer(_) => Some(false),
        Number::Rational(rational) => {
            has_only_decimal_primes(rational.denominator()).then_some(true)
        }
        Number::F32(_) | Number::F64(_) => None,
    }
}

pub fn literal_form(pool: &ExprPool, expression: ExprId) -> RationalForm {
    let mut pending = vec![expression];
    let mut seen = std::collections::HashSet::new();
    while let Some(expression) = pending.pop() {
        if !seen.insert(expression) {
            continue;
        }
        match pool.node(expression) {
            Ok(NodeView::Number(number)) => {
                let is_decimal = pool.number_value(number).ok().and_then(is_decimal_literal);
                if is_decimal == Some(true) {
                    return RationalForm::Decimal;
                }
            }
            Ok(NodeView::Apply { arguments, .. }) => pending.extend(arguments.iter().copied()),
            Ok(NodeView::Bind {
                arguments, body, ..
            }) => {
                pending.extend(arguments.iter().copied());
                pending.push(body);
            }
            Ok(NodeView::Quantity { value, .. }) => pending.push(value),
            Ok(NodeView::Array { elements, .. }) => pending.extend(elements.iter().copied()),
            Ok(NodeView::Symbol(_) | NodeView::Bound(_)) | Err(_) => {}
        }
    }
    RationalForm::Fraction
}

fn non_negative_rational_literal(pool: &ExprPool, expression: ExprId) -> Option<Number> {
    let NodeView::Number(number) = pool.node(expression).ok()? else {
        return None;
    };
    let value = pool.number_value(number).ok()?;
    let is_non_negative = match value {
        Number::Integer(integer) => !integer.is_negative(),
        Number::Rational(rational) => !rational.numerator().is_negative(),
        Number::F32(_) | Number::F64(_) => false,
    };
    is_non_negative.then(|| value.clone())
}

fn is_rational_result(pool: &mut ExprPool, expression: ExprId) -> bool {
    evaluate_exact(pool, expression).is_ok_and(|evaluation| evaluation.rational_value().is_some())
}

fn rational_unit_power(
    pool: &mut ExprPool,
    unit: Option<UnitId>,
    numerator: i64,
    denominator: i64,
) -> Option<Option<UnitId>> {
    let Some(unit) = unit else {
        return Some(None);
    };
    let factors: Vec<(String, i64)> = {
        let units = pool.units();
        units
            .factors(unit)
            .ok()?
            .iter()
            .map(|factor| {
                Some((
                    units.symbol(factor.named_unit()).ok()?.to_owned(),
                    i64::from(factor.exponent()),
                ))
            })
            .collect::<Option<_>>()?
    };
    let mut result = pool.units().dimensionless();
    for (symbol, exponent) in factors {
        let scaled = exponent.checked_mul(numerator)?;
        if scaled % denominator != 0 {
            return None;
        }
        let units = pool.units_mut();
        let named = units.lookup(&symbol).ok()?;
        let powered = units
            .power(named, i8::try_from(scaled / denominator).ok()?)
            .ok()?;
        result = units.multiply(result, powered).ok()?;
    }
    Some(Some(result))
}

fn combined(children: &[Written], unit: Option<UnitId>) -> Written {
    Written {
        unit,
        has_decimal: children.iter().any(|child| child.has_decimal),
        has_division: children.iter().any(|child| child.has_division),
        has_decimal_factor: children.iter().any(|child| child.has_decimal_factor),
    }
}

fn converted(pool: &ExprPool, child: Written, unit: Option<UnitId>) -> Option<Written> {
    let has_decimal_factor = decimal_conversion(pool, child.unit, unit)?;
    Some(Written { unit, ..child }.with_decimal_factor(has_decimal_factor))
}

fn written(pool: &mut ExprPool, expression: ExprId) -> Option<Written> {
    let view = pool.node(expression).ok()?;
    match view {
        NodeView::Number(number) => {
            let has_decimal = is_decimal_literal(pool.number_value(number).ok()?)?;
            Some(Written {
                has_decimal,
                ..Written::whole(None)
            })
        }
        NodeView::Quantity { value, unit } => {
            let inner = written(pool, value)?;
            if inner.unit.is_some() {
                return None;
            }
            Some(Written {
                unit: Some(unit),
                ..inner
            })
        }
        NodeView::Array { elements, .. } => {
            let elements = elements.to_vec();
            same_unit_operands(pool, &elements)
        }
        NodeView::Apply {
            head: Head::Operator(operator),
            arguments,
        } => {
            let arguments = arguments.to_vec();
            operation(pool, expression, operator, &arguments)
        }
        NodeView::Symbol(symbol) if symbol == BuiltinConstant::ImaginaryUnit.symbol() => {
            Some(Written::whole(None))
        }
        NodeView::Bound(_) => Some(Written::whole(None)),
        NodeView::Bind {
            binder: BinderKind::Sum(_) | BinderKind::Product(_),
            body,
            ..
        } => written(pool, body),
        NodeView::Bind {
            binder: BinderKind::Sort(_),
            arguments: [list],
            ..
        } => written(pool, *list),
        NodeView::Symbol(_) | NodeView::Bind { .. } | NodeView::Apply { .. } => None,
    }
}

fn same_unit_operands(pool: &mut ExprPool, operands: &[ExprId]) -> Option<Written> {
    let children = operands
        .iter()
        .map(|operand| written(pool, *operand))
        .collect::<Option<Vec<_>>>()?;
    let first = children.first()?.unit;
    if let Some(in_first) = all_converted(pool, &children, first) {
        return Some(combined(&in_first, first));
    }
    let coherent = coherent_unit_of(pool, first)?;
    let in_coherent = all_converted(pool, &children, coherent)?;
    Some(combined(&in_coherent, coherent))
}

fn all_converted(
    pool: &ExprPool,
    children: &[Written],
    unit: Option<UnitId>,
) -> Option<Vec<Written>> {
    children
        .iter()
        .map(|child| converted(pool, *child, unit))
        .collect()
}

fn coherent_unit_of(pool: &mut ExprPool, unit: Option<UnitId>) -> Option<Option<UnitId>> {
    let dimension = pool.units().dimension(unit?).ok()?;
    pool.units_mut().coherent_unit(&dimension).ok().map(Some)
}

fn picked_operand(pool: &mut ExprPool, expression: ExprId, operands: &[ExprId]) -> Option<Written> {
    let first = written(pool, *operands.first()?)?;
    let picked_value = evaluate_exact(pool, expression)
        .ok()?
        .rational_value()?
        .clone();
    for operand in operands {
        let is_picked = evaluate_exact(pool, *operand)
            .ok()
            .is_some_and(|evaluation| evaluation.rational_value() == Some(&picked_value));
        if is_picked {
            let picked = written(pool, *operand)?;
            return converted(pool, picked, first.unit);
        }
    }
    None
}

fn operation(
    pool: &mut ExprPool,
    expression: ExprId,
    operator: Operator,
    arguments: &[ExprId],
) -> Option<Written> {
    match (operator, arguments) {
        (Operator::Min | Operator::Max, _) => picked_operand(pool, expression, arguments)
            .or_else(|| same_unit_operands(pool, arguments)),
        (Operator::Add | Operator::Sub | Operator::Complex, _) => {
            same_unit_operands(pool, arguments)
        }
        (
            Operator::Neg
            | Operator::Abs
            | Operator::Total
            | Operator::Mean
            | Operator::Sorted
            | Operator::Smallest
            | Operator::Largest
            | Operator::Median,
            [argument],
        ) => written(pool, *argument),
        (Operator::Percent, [argument]) => {
            Some(written(pool, *argument)?.with_decimal_factor(true))
        }
        (Operator::ToExact, [argument]) => {
            let unit = match pool.node(*argument).ok()? {
                NodeView::Quantity { unit, .. } => Some(unit),
                _ => written(pool, *argument).and_then(|child| child.unit),
            };
            Some(Written {
                has_decimal: true,
                ..Written::whole(unit)
            })
        }
        (Operator::Mul, [left, right]) => {
            let left = written(pool, *left)?;
            let right = written(pool, *right)?;
            let unit = match (left.unit, right.unit) {
                (Some(left_unit), Some(right_unit)) => {
                    Some(pool.units_mut().multiply(left_unit, right_unit).ok()?)
                }
                (unit, None) | (None, unit) => unit,
            };
            Some(combined(&[left, right], unit))
        }
        (
            Operator::Floor | Operator::Ceil | Operator::Trunc | Operator::RoundTiesEven,
            [argument],
        ) => {
            let child = written(pool, *argument)?;
            converted(pool, child, None)
        }
        (Operator::Pow, [base, exponent]) => {
            let base_written = written(pool, *base)?;
            let exponent_value = non_negative_rational_literal(pool, *exponent)?;
            match exponent_value {
                Number::Integer(integer) => {
                    let unit = match base_written.unit {
                        Some(unit) => Some(
                            pool.units_mut()
                                .power(unit, i8::try_from(integer.to_i64()?).ok()?)
                                .ok()?,
                        ),
                        None => None,
                    };
                    Some(combined(&[base_written], unit))
                }
                Number::Rational(rational) => {
                    let unit = rational_unit_power(
                        pool,
                        base_written.unit,
                        rational.numerator().to_i64()?,
                        rational.denominator().to_i64()?,
                    )?;
                    is_rational_result(pool, expression).then(|| combined(&[base_written], unit))
                }
                Number::F32(_) | Number::F64(_) => None,
            }
        }
        (Operator::Sqrt, [argument]) => {
            let child = written(pool, *argument)?;
            let unit = rational_unit_power(pool, child.unit, 1, 2)?;
            is_rational_result(pool, expression).then(|| combined(&[child], unit))
        }
        (Operator::Div, [left, right]) => {
            let left_written = written(pool, *left)?;
            let right_written = written(pool, *right)?;
            let inverse = match right_written.unit {
                Some(unit) => Some(pool.units_mut().power(unit, -1).ok()?),
                None => None,
            };
            let unit = match (left_written.unit, inverse) {
                (Some(numerator), Some(denominator)) => {
                    Some(pool.units_mut().multiply(numerator, denominator).ok()?)
                }
                (unit, None) | (None, unit) => unit,
            };
            Some(Written {
                has_division: true,
                ..combined(&[left_written, right_written], unit)
            })
        }
        (Operator::ConvertUnit, [value, _]) => written(pool, *value),
        (Operator::FromCelsius, [argument]) => {
            let child = written(pool, *argument)?;
            let kelvin = pool.units_mut().lookup(KELVIN_SYMBOL).ok()?;
            Some(combined(&[child], Some(kelvin)).with_decimal_factor(true))
        }
        (Operator::ToCelsius | Operator::ToFahrenheit, [argument]) => {
            let child = written(pool, *argument)?;
            let kelvin = pool.units_mut().lookup(KELVIN_SYMBOL).ok()?;
            let in_kelvin = converted(pool, child, Some(kelvin))?;
            Some(Written {
                unit: None,
                ..in_kelvin.with_decimal_factor(true)
            })
        }
        (Operator::Mod, [left, right]) => {
            let children = [written(pool, *left)?, written(pool, *right)?];
            let dimensionless = children.iter().all(|child| child.unit.is_none());
            dimensionless.then(|| combined(&children, None))
        }
        (Operator::Log, _) => {
            let children = arguments
                .iter()
                .map(|argument| written(pool, *argument))
                .collect::<Option<Vec<_>>>()?;
            let dimensionless = children.iter().all(|child| child.unit.is_none());
            (dimensionless && is_rational_result(pool, expression))
                .then(|| combined(&children, None))
        }
        _ => None,
    }
}

pub fn rational_form(
    pool: &mut ExprPool,
    expression: ExprId,
    display_unit: Option<UnitId>,
) -> RationalForm {
    if let Some(form) = radix_form(pool, expression) {
        return RationalForm::Radix(form);
    }
    match written(pool, expression) {
        Some(root) => match converted(pool, root, display_unit) {
            Some(displayed) if displayed.is_decimal() => RationalForm::Decimal,
            _ => RationalForm::Fraction,
        },
        _ => RationalForm::Fraction,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form_of(text: &str) -> RationalForm {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, text).unwrap();
        let display = crate::to_coherent_units(&mut pool, expression)
            .ok()
            .and_then(|coherent| coherent.unit);
        rational_form(&mut pool, expression, display)
    }

    fn form_in(text: &str, display: &str) -> RationalForm {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, text).unwrap();
        let unit_expression =
            calc_syntax::parse_expression(&mut pool, &format!("1 {display}")).unwrap();
        let display = match pool.node(unit_expression).unwrap() {
            NodeView::Quantity { unit, .. } => Some(unit),
            _ => None,
        };
        rational_form(&mut pool, expression, display)
    }

    #[test]
    fn sum_of_decimal_literals_is_a_decimal() {
        assert_eq!(form_of("0.5 + 0.25"), RationalForm::Decimal);
    }

    #[test]
    fn product_of_decimal_literals_is_a_decimal() {
        assert_eq!(form_of("1.5 * 2.5"), RationalForm::Decimal);
    }

    #[test]
    fn division_is_a_fraction() {
        assert_eq!(form_of("1/4"), RationalForm::Fraction);
    }

    #[test]
    fn dividing_a_decimal_by_a_terminating_divisor_is_a_decimal() {
        assert_eq!(form_of("0.5/2"), RationalForm::Decimal);
    }

    #[test]
    fn dividing_a_decimal_by_a_divisor_with_other_primes_is_still_a_decimal() {
        assert_eq!(form_of("47.76/12"), RationalForm::Decimal);
    }

    #[test]
    fn dividing_by_a_decimal_literal_is_a_decimal() {
        assert_eq!(form_of("1.99/0.5"), RationalForm::Decimal);
    }

    #[test]
    fn mixed_input_with_a_terminating_division_is_a_decimal() {
        assert_eq!(form_of("1/4 + 0.5"), RationalForm::Decimal);
    }

    #[test]
    fn mixed_input_with_a_division_of_whole_numbers_is_a_decimal() {
        assert_eq!(form_of("1/3 + 0.5"), RationalForm::Decimal);
    }

    #[test]
    fn dividing_by_a_symbol_is_a_fraction() {
        assert_eq!(form_of("0.5/x"), RationalForm::Fraction);
    }

    #[test]
    fn integers_alone_give_no_decimal() {
        assert_eq!(form_of("3 + 4"), RationalForm::Fraction);
    }

    #[test]
    fn negative_integer_exponent_is_a_fraction() {
        assert_eq!(form_of("0.5^-1"), RationalForm::Fraction);
    }

    #[test]
    fn non_negative_integer_exponent_keeps_a_decimal() {
        assert_eq!(form_of("0.5^3"), RationalForm::Decimal);
    }

    #[test]
    fn rational_power_with_a_rational_result_keeps_a_decimal() {
        assert_eq!(form_of("0.25^0.5"), RationalForm::Decimal);
    }

    #[test]
    fn square_root_with_a_rational_result_keeps_a_decimal() {
        assert_eq!(form_of("sqrt(0.25)"), RationalForm::Decimal);
    }

    #[test]
    fn square_root_of_a_quantity_with_a_rational_result_keeps_a_decimal() {
        assert_eq!(form_in("sqrt(0.25 m^2)", "m"), RationalForm::Decimal);
    }

    #[test]
    fn rational_power_of_a_quantity_with_a_rational_result_keeps_a_decimal() {
        assert_eq!(form_in("(0.0625 m^4)^0.5", "m^2"), RationalForm::Decimal);
    }

    #[test]
    fn square_root_of_a_unit_with_an_odd_exponent_is_a_fraction() {
        assert_eq!(form_in("sqrt(0.25 m^3)", "m"), RationalForm::Fraction);
    }

    #[test]
    fn exact_machine_quantity_keeps_its_unit() {
        assert_eq!(form_in("exact(f64'0.5' m)", "m"), RationalForm::Decimal);
    }

    #[test]
    fn floor_keeps_a_decimal() {
        assert_eq!(form_of("floor(2.5) + 0.5"), RationalForm::Decimal);
    }

    #[test]
    fn conversion_to_exact_counts_as_a_decimal() {
        assert_eq!(form_of("exact(f64'0.5') + 1"), RationalForm::Decimal);
    }

    #[test]
    fn non_decimal_literal_is_a_fraction() {
        assert_eq!(form_of("q'1/3' + 0.5"), RationalForm::Fraction);
    }

    #[test]
    fn quantity_displayed_in_its_written_unit_is_a_decimal() {
        assert_eq!(form_in("1.5 km/h", "km/h"), RationalForm::Decimal);
    }

    #[test]
    fn quantity_displayed_with_a_non_decimal_ratio_is_a_fraction() {
        assert_eq!(form_of("1.5 km/h"), RationalForm::Fraction);
    }

    #[test]
    fn sum_of_units_with_a_non_decimal_ratio_is_a_fraction() {
        assert_eq!(form_in("0.5 h + 1.5 min", "min"), RationalForm::Fraction);
    }

    #[test]
    fn product_of_prefixed_units_displayed_coherently_is_a_decimal() {
        assert_eq!(form_in("0.3 km * 0.2 km", "m^2"), RationalForm::Decimal);
    }

    #[test]
    fn celsius_reading_to_kelvin_is_a_decimal() {
        assert_eq!(form_in("from_celsius(20.5)", "K"), RationalForm::Decimal);
    }

    #[test]
    fn fahrenheit_reading_is_a_fraction() {
        assert_eq!(
            form_in("from_fahrenheit(68.5)", "K"),
            RationalForm::Fraction
        );
    }

    #[test]
    fn an_imaginary_part_written_as_a_decimal_stays_a_decimal() {
        assert_eq!(form_of("i * 0.5"), RationalForm::Decimal);
    }

    #[test]
    fn a_complex_call_written_as_fractions_stays_a_fraction() {
        assert_eq!(form_of("complex(1/2, 1/4)"), RationalForm::Fraction);
    }

    #[test]
    fn a_whole_number_through_a_decimal_factor_is_a_decimal() {
        assert_eq!(form_in("1 lb", "kg"), RationalForm::Decimal);
    }

    #[test]
    fn a_fraction_through_a_decimal_factor_stays_a_fraction() {
        assert_eq!(form_in("(1/4) mi", "m"), RationalForm::Fraction);
    }

    #[test]
    fn a_binary_prefix_is_no_decimal_factor() {
        assert_eq!(form_in("1 TB", "GiB"), RationalForm::Fraction);
    }

    #[test]
    fn a_sum_whose_units_meet_only_in_the_coherent_unit_is_a_decimal() {
        assert_eq!(form_in("1 lb + 1 kg", "g"), RationalForm::Decimal);
        assert_eq!(form_in("5 ft + 3 in", "cm"), RationalForm::Decimal);
    }

    #[test]
    fn a_sum_shown_in_a_unit_the_coherent_one_does_not_reach_stays_a_fraction() {
        assert_eq!(form_in("1 yd + 1 ft", "yd"), RationalForm::Fraction);
    }

    #[test]
    fn a_sum_over_a_prefix_is_a_decimal() {
        assert_eq!(form_in("2 m + 30 cm", "m"), RationalForm::Decimal);
    }

    #[test]
    fn a_whole_celsius_reading_to_kelvin_is_a_decimal() {
        assert_eq!(form_in("from_celsius(20)", "K"), RationalForm::Decimal);
    }

    #[test]
    fn a_whole_kelvin_value_to_celsius_is_a_decimal() {
        assert_eq!(form_of("to_celsius(300 K)"), RationalForm::Decimal);
    }

    #[test]
    fn a_percent_of_a_whole_number_is_a_decimal() {
        assert_eq!(form_of("19 %"), RationalForm::Decimal);
    }

    #[test]
    fn a_percent_of_a_fraction_is_a_fraction() {
        assert_eq!(form_of("(1/2) %"), RationalForm::Fraction);
    }

    #[test]
    fn a_maximum_takes_the_form_of_the_fraction_it_picks() {
        assert_eq!(form_of("max(1/4, 0.1)"), RationalForm::Fraction);
    }

    #[test]
    fn a_maximum_takes_the_form_of_the_decimal_it_picks() {
        assert_eq!(form_of("max(0.5, 1/3)"), RationalForm::Decimal);
    }

    #[test]
    fn a_mean_of_decimals_is_a_decimal() {
        assert_eq!(form_of("mean([0.2, 0.4])"), RationalForm::Decimal);
    }

    #[test]
    fn a_sum_of_a_decimal_term_is_a_decimal() {
        assert_eq!(form_of("sum(0.1, k, 1, 3)"), RationalForm::Decimal);
    }

    #[test]
    fn a_sum_of_a_fraction_term_is_a_fraction() {
        assert_eq!(form_of("sum(1/k, k, 1, 3)"), RationalForm::Fraction);
    }

    #[test]
    fn a_rational_logarithm_of_a_decimal_is_a_decimal() {
        assert_eq!(form_of("log(0.5, 4)"), RationalForm::Decimal);
    }
}
