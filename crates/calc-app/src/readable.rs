use std::cmp::Ordering;

use calc_concepts::QuantityKind;
use calc_core::{round_scaled_expression, scaled_expression_at_least_one};
use calc_expr::{ExprId, ExprPool, Head, NodeView, Operator};
use calc_numbers::{
    DecimalRounding, ENCLOSURE_BUDGET_BITS, Integer, Number, POWER_OF_TEN_MARK, at_least_one,
    power_of_ten_text, round_half_away_to_significant_digits, round_to_decimal_places,
};
use calc_units::{ScaleFactor, UnitId};

const DECIMAL_PRIMES: [i64; 2] = [2, 5];
pub const READING_DIGITS: u32 = 4;
const UNCERTAINTY_DIGITS: u32 = 2;
pub(crate) const PLAIN_LOWEST_DECADE: i64 = -3;
pub(crate) const PLAIN_HIGHEST_DECADE: i64 = 5;
const DECIMAL_POINT: char = '.';
const MINUS: char = '-';

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickMagnitude {
    Exact(Number),
    PiPower { coefficient: Number, exponent: i32 },
    Expression(ExprId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadableValue {
    pub inside: Vec<UnitId>,
    pub composed: Option<UnitId>,
    pub magnitude: PickMagnitude,
    pub needs_termination: bool,
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

pub(crate) fn terminates(value: &Number) -> bool {
    match value {
        Number::Integer(_) => true,
        Number::Rational(rational) => has_only_decimal_primes(rational.denominator()),
        Number::F32(_) | Number::F64(_) => false,
    }
}

fn never_cancelled() -> bool {
    false
}

pub(crate) fn pi_scaled_at_least_one(coefficient: &Number, pi_exponent: i32) -> Option<bool> {
    let coefficient = coefficient.clone();
    at_least_one(ENCLOSURE_BUDGET_BITS, &never_cancelled, move |step| {
        let value = step.exact(&coefficient)?;
        if pi_exponent == 0 {
            return Ok(value.absolute());
        }
        let power = step.pi()?.power(i64::from(pi_exponent.unsigned_abs()))?;
        let scaled = if pi_exponent < 0 {
            value.div(&power)?
        } else {
            value.mul(&power)?
        };
        Ok(scaled.absolute())
    })
    .ok()
    .flatten()
}

fn magnitude_at_least_one(
    pool: &mut ExprPool,
    magnitude: &PickMagnitude,
    factor: &ScaleFactor,
) -> bool {
    let rational = factor.rational_part();
    match magnitude {
        PickMagnitude::Exact(value) if factor.pi_exponent() == 0 => value
            .mul_exact(&rational)
            .ok()
            .and_then(|scaled| scaled.to_exact().ok())
            .is_some_and(|scaled| is_at_least_one(&scaled)),
        PickMagnitude::Exact(value) => value
            .mul_exact(&rational)
            .ok()
            .and_then(|scaled| pi_scaled_at_least_one(&scaled, factor.pi_exponent()))
            .unwrap_or(false),
        PickMagnitude::PiPower {
            coefficient,
            exponent,
        } => {
            let Some(exponent) = exponent.checked_add(factor.pi_exponent()) else {
                return false;
            };
            let Ok(scaled) = coefficient.mul_exact(&rational) else {
                return false;
            };
            if exponent == 0 {
                return is_at_least_one(&scaled);
            }
            pi_scaled_at_least_one(&scaled, exponent).unwrap_or(false)
        }
        PickMagnitude::Expression(expression) => {
            scaled_expression_at_least_one(pool, *expression, &rational, factor.pi_exponent())
                .unwrap_or(false)
        }
    }
}

fn is_at_least_one(value: &Number) -> bool {
    let magnitude = match value {
        Number::Integer(integer) if integer.is_negative() => Number::Integer(integer.negated()),
        Number::Rational(rational) if rational.numerator().is_negative() => {
            match value.negate_exact() {
                Ok(negated) => negated,
                Err(_) => return false,
            }
        }
        other => other.clone(),
    };
    let one = Number::Integer(Integer::one());
    match magnitude.sub_exact(&one) {
        Ok(Number::Integer(difference)) => !difference.is_negative(),
        Ok(Number::Rational(difference)) => !difference.numerator().is_negative(),
        _ => false,
    }
}

fn is_eligible(magnitude: &PickMagnitude, factor: &ScaleFactor) -> bool {
    match magnitude {
        PickMagnitude::Exact(value) if factor.pi_exponent() == 0 => value
            .mul_exact(&factor.rational_part())
            .is_ok_and(|scaled| terminates(&scaled)),
        _ => false,
    }
}

pub(crate) fn pick_unit(
    pool: &mut ExprPool,
    candidates: &[UnitId],
    coherent: UnitId,
    readable: &ReadableValue,
) -> Option<UnitId> {
    let (unit, _) = picked_unit(pool, candidates, coherent, readable)?;
    Some(unit)
}

pub(crate) fn readable_pick(
    pool: &mut ExprPool,
    candidates: &[UnitId],
    coherent: UnitId,
    readable: &ReadableValue,
) -> Option<UnitId> {
    let (unit, at_least_one) = picked_unit(pool, candidates, coherent, readable)?;
    at_least_one.then_some(unit)
}

fn picked_unit(
    pool: &mut ExprPool,
    candidates: &[UnitId],
    coherent: UnitId,
    readable: &ReadableValue,
) -> Option<(UnitId, bool)> {
    let factors: Vec<(UnitId, ScaleFactor)> = candidates
        .iter()
        .filter_map(|candidate| {
            pool.units()
                .conversion_factor(coherent, *candidate)
                .ok()
                .map(|factor| (*candidate, factor))
        })
        .collect();
    let eligible: Vec<(UnitId, ScaleFactor)> = factors
        .into_iter()
        .filter(|(_, factor)| {
            !readable.needs_termination || is_eligible(&readable.magnitude, factor)
        })
        .collect();
    let smallest = eligible.first().map(|(unit, _)| *unit)?;
    let largest_at_least_one = eligible
        .iter()
        .rfind(|(_, factor)| magnitude_at_least_one(pool, &readable.magnitude, factor))
        .map(|(unit, _)| *unit);
    Some(match largest_at_least_one {
        Some(unit) => (unit, true),
        None => (smallest, false),
    })
}

pub(crate) fn value_is_one_unit(pool: &ExprPool, expression: ExprId, unit: UnitId) -> bool {
    unit_power(pool, expression, unit) == Some(1)
}

fn keeps_its_quantity(operator: Operator) -> bool {
    matches!(
        operator,
        Operator::Neg
            | Operator::Abs
            | Operator::Floor
            | Operator::Ceil
            | Operator::Trunc
            | Operator::Round
            | Operator::RoundTiesEven
            | Operator::Percent
            | Operator::CopySign
            | Operator::Total
            | Operator::Mean
            | Operator::Median
            | Operator::Smallest
            | Operator::Largest
            | Operator::Sorted
            | Operator::At
            | Operator::Transpose
            | Operator::ToF32
            | Operator::ToF64
            | Operator::ToExact
            | Operator::Uncertain
            | Operator::UncertainExpanded
            | Operator::ConvertUnit
    )
}

fn agreeing_quantity(operator: Operator) -> bool {
    matches!(
        operator,
        Operator::Add | Operator::Sub | Operator::Min | Operator::Max | Operator::Select
    )
}

fn whole_exponent(pool: &ExprPool, expression: ExprId) -> Option<i32> {
    let NodeView::Number(number) = pool.node(expression).ok()? else {
        return None;
    };
    match pool.number_value(number).ok()? {
        Number::Integer(integer) => integer.to_i64().and_then(|value| i32::try_from(value).ok()),
        _ => None,
    }
}

fn unit_power(pool: &ExprPool, expression: ExprId, unit: UnitId) -> Option<i32> {
    let power_of = |argument: &ExprId| unit_power(pool, *argument, unit);
    match pool.node(expression).ok()? {
        NodeView::Number(_) | NodeView::Symbol(_) | NodeView::Bound(_) => Some(0),
        NodeView::Quantity { value, unit: worn } => {
            let inside = unit_power(pool, value, unit)?;
            if worn == unit {
                return inside.checked_add(1);
            }
            let dimension = pool.units().dimension(worn).ok()?;
            (!dimension.is_dimensionless()).then_some(inside)
        }
        NodeView::Array { elements, .. } => agreed(elements.iter().map(power_of)),
        NodeView::Bind { body, .. } => unit_power(pool, body, unit).filter(|power| *power == 0),
        NodeView::Apply { head, arguments } => {
            let Head::Operator(operator) = head else {
                return agreed(arguments.iter().map(power_of)).filter(|power| *power == 0);
            };
            match (operator, arguments) {
                (Operator::Mul, [left, right]) => {
                    unit_power(pool, *left, unit)?.checked_add(unit_power(pool, *right, unit)?)
                }
                (Operator::Div, [left, right]) => {
                    unit_power(pool, *left, unit)?.checked_sub(unit_power(pool, *right, unit)?)
                }
                (Operator::Pow, [base, exponent]) => {
                    let base = unit_power(pool, *base, unit)?;
                    if base == 0 {
                        return Some(0);
                    }
                    base.checked_mul(whole_exponent(pool, *exponent)?)
                }
                (Operator::Sqrt, [argument]) => {
                    unit_power(pool, *argument, unit)?.eq(&0).then_some(0)
                }
                (operator, [first, ..]) if keeps_its_quantity(operator) => {
                    unit_power(pool, *first, unit)
                }
                (operator, arguments) if agreeing_quantity(operator) => {
                    agreed(arguments.iter().rev().take(2).map(power_of))
                }
                _ => Some(0),
            }
        }
    }
}

fn agreed(powers: impl Iterator<Item = Option<i32>>) -> Option<i32> {
    let mut agreed = None;
    for power in powers {
        let power = power?;
        if agreed.is_some_and(|agreed| agreed != power) {
            return None;
        }
        agreed = Some(power);
    }
    agreed.or(Some(0))
}

pub(crate) fn written_units(pool: &ExprPool, expression: ExprId) -> Vec<UnitId> {
    let mut units = Vec::new();
    let mut pending = vec![expression];
    let mut seen = Vec::new();
    while let Some(current) = pending.pop() {
        if seen.contains(&current) {
            continue;
        }
        seen.push(current);
        match pool.node(current) {
            Ok(NodeView::Quantity { value, unit }) => {
                if !units.contains(&unit) {
                    units.push(unit);
                }
                pending.push(value);
            }
            Ok(NodeView::Apply { arguments, .. }) => {
                for argument in arguments.iter().rev() {
                    pending.push(*argument);
                }
            }
            _ => {}
        }
    }
    units
}

enum Composed {
    PureNumber,
    Unit(UnitId),
}

fn composed(pool: &mut ExprPool, expression: ExprId) -> Option<Composed> {
    let node = pool.node(expression).ok()?;
    let times = |pool: &mut ExprPool, left: Composed, right: Composed, divide: bool| {
        Some(match (left, right) {
            (Composed::PureNumber, Composed::PureNumber) => Composed::PureNumber,
            (Composed::Unit(unit), Composed::PureNumber) => Composed::Unit(unit),
            (Composed::PureNumber, Composed::Unit(unit)) if divide => {
                Composed::Unit(pool.units_mut().power(unit, -1).ok()?)
            }
            (Composed::PureNumber, Composed::Unit(unit)) => Composed::Unit(unit),
            (Composed::Unit(left), Composed::Unit(right)) if divide => {
                Composed::Unit(pool.units_mut().divide(left, right).ok()?)
            }
            (Composed::Unit(left), Composed::Unit(right)) => {
                Composed::Unit(pool.units_mut().multiply(left, right).ok()?)
            }
        })
    };
    match node {
        NodeView::Number(_) | NodeView::Symbol(_) => Some(Composed::PureNumber),
        NodeView::Quantity { value, unit } => {
            let value = composed(pool, value)?;
            times(pool, value, Composed::Unit(unit), false)
        }
        NodeView::Apply {
            head: Head::Operator(operator),
            arguments,
        } => {
            let arguments = arguments.to_vec();
            match (operator, arguments.as_slice()) {
                (Operator::Mul | Operator::Div, [left, right]) => {
                    let left = composed(pool, *left)?;
                    let right = composed(pool, *right)?;
                    times(pool, left, right, operator == Operator::Div)
                }
                (Operator::Add | Operator::Sub, [left, right]) => {
                    match (composed(pool, *left)?, composed(pool, *right)?) {
                        (Composed::PureNumber, Composed::PureNumber) => Some(Composed::PureNumber),
                        (Composed::Unit(left), Composed::Unit(right)) if left == right => {
                            Some(Composed::Unit(left))
                        }
                        _ => None,
                    }
                }
                (Operator::Neg, [single]) => composed(pool, *single),
                (Operator::Pow, [base, exponent]) => {
                    let base = composed(pool, *base)?;
                    let power = match pool.node(*exponent).ok()? {
                        NodeView::Number(number) => {
                            let calc_numbers::Number::Integer(whole) =
                                pool.number_value(number).ok()?
                            else {
                                return None;
                            };
                            i8::try_from(whole.to_i64()?).ok()?
                        }
                        _ => return None,
                    };
                    match base {
                        Composed::PureNumber => Some(Composed::PureNumber),
                        Composed::Unit(unit) => {
                            Some(Composed::Unit(pool.units_mut().power(unit, power).ok()?))
                        }
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn quantities_repeat(pool: &mut ExprPool, unit: UnitId) -> bool {
    let Ok(factors) = pool.units().factors(unit) else {
        return true;
    };
    let symbols: Vec<String> = factors
        .iter()
        .filter_map(|factor| {
            pool.units()
                .symbol(factor.named_unit())
                .ok()
                .map(str::to_string)
        })
        .collect();
    let mut dimensions = Vec::new();
    for symbol in symbols {
        let Some(dimension) = pool
            .units_mut()
            .lookup(&symbol)
            .ok()
            .and_then(|named| pool.units().dimension(named).ok())
        else {
            return true;
        };
        if dimensions.contains(&dimension) {
            return true;
        }
        dimensions.push(dimension);
    }
    false
}

pub(crate) fn composed_written_unit(pool: &mut ExprPool, expression: ExprId) -> Option<UnitId> {
    let Composed::Unit(unit) = composed(pool, expression)? else {
        return None;
    };
    if quantities_repeat(pool, unit) {
        return None;
    }
    let dimension = pool.units().dimension(unit).ok()?;
    let coherent = pool.units_mut().coherent_unit(&dimension).ok()?;
    let factor = pool.units().conversion_factor(unit, coherent).ok()?;
    let same_as_coherent = factor.pi_exponent() == 0 && factor.numerator() == factor.denominator();
    let coherent_has_one_name = pool
        .units_mut()
        .named_coherent_unit(&dimension)
        .ok()
        .and_then(|named| pool.units().factors(named).ok().map(<[_]>::len))
        == Some(1);
    (!(same_as_coherent && coherent_has_one_name)).then_some(unit)
}

fn factor_units(pool: &mut ExprPool, unit: UnitId) -> Vec<UnitId> {
    let Ok(factors) = pool.units().factors(unit) else {
        return Vec::new();
    };
    let parts: Vec<(String, i8)> = factors
        .iter()
        .filter_map(|factor| {
            pool.units()
                .symbol(factor.named_unit())
                .ok()
                .map(|symbol| (symbol.to_string(), factor.exponent()))
        })
        .collect();
    parts
        .into_iter()
        .filter_map(|(symbol, exponent)| {
            let named = pool.units_mut().lookup(&symbol).ok()?;
            pool.units_mut().power(named, exponent).ok()
        })
        .collect()
}

fn family_of(pool: &ExprPool, unit: UnitId) -> Option<(calc_units::NamedUnitId, i8)> {
    let factors = pool.units().factors(unit).ok()?;
    let [single] = factors else {
        return None;
    };
    let family = pool.units().prefix_family(single.named_unit()).ok()?;
    Some((family, single.exponent()))
}

pub(crate) fn written_candidates(
    pool: &mut ExprPool,
    written: &[UnitId],
    dimension: calc_units::Dimension,
    kind: Option<QuantityKind>,
    coherent: UnitId,
    list: &[UnitId],
) -> Vec<UnitId> {
    if matches!(
        kind,
        Some(QuantityKind::Temperature | QuantityKind::TemperatureDifference)
    ) {
        return Vec::new();
    }
    let mut candidates: Vec<UnitId> = Vec::new();
    for written in written.iter().copied() {
        let mut parts = vec![written];
        parts.extend(factor_units(pool, written));
        for part in parts {
            if pool.units().dimension(part).ok() != Some(dimension) {
                continue;
            }
            if kind == Some(QuantityKind::Time)
                && !pool
                    .units()
                    .conversion_factor(part, coherent)
                    .is_ok_and(|factor| {
                        factor.pi_exponent() == 0
                            && has_only_decimal_primes(factor.denominator())
                            && has_only_decimal_primes(factor.numerator())
                    })
            {
                continue;
            }
            if !candidates.contains(&part) {
                candidates.push(part);
            }
        }
    }
    let families: Vec<(calc_units::NamedUnitId, i8)> = candidates
        .iter()
        .filter_map(|candidate| family_of(pool, *candidate))
        .collect();
    for unit in list {
        if candidates.contains(unit) {
            continue;
        }
        if family_of(pool, *unit).is_some_and(|family| families.contains(&family)) {
            candidates.push(*unit);
        }
    }
    sorted_by_scale(pool, candidates)
}

pub(crate) fn sorted_by_scale(pool: &ExprPool, units: Vec<UnitId>) -> Vec<UnitId> {
    let mut sorted: Vec<UnitId> = Vec::new();
    for unit in units {
        let mut orders = Vec::new();
        for kept in &sorted {
            match compare_scale(pool, unit, *kept) {
                Some(order) => orders.push(order),
                None => {
                    orders.clear();
                    break;
                }
            }
        }
        if orders.len() != sorted.len() || orders.contains(&Ordering::Equal) {
            continue;
        }
        let position = orders
            .iter()
            .position(|order| *order == Ordering::Less)
            .unwrap_or(sorted.len());
        sorted.insert(position, unit);
    }
    sorted
}

fn compare_scale(pool: &ExprPool, left: UnitId, right: UnitId) -> Option<Ordering> {
    let factor = pool.units().conversion_factor(left, right).ok()?;
    if factor.pi_exponent() != 0 {
        let at_least_one = pi_scaled_at_least_one(&factor.rational_part(), factor.pi_exponent())?;
        return Some(if at_least_one {
            Ordering::Greater
        } else {
            Ordering::Less
        });
    }
    let difference = factor.numerator() - factor.denominator();
    Some(if difference.is_zero() {
        Ordering::Equal
    } else if difference.is_negative() {
        Ordering::Less
    } else {
        Ordering::Greater
    })
}

fn power_of_ten(exponent: i64) -> Option<Number> {
    let magnitude = Integer::from(10_i64).pow(u32::try_from(exponent.unsigned_abs()).ok()?);
    if exponent >= 0 {
        Some(Number::Integer(magnitude))
    } else {
        Number::fraction(&Integer::one(), &magnitude).ok()
    }
}

fn decade(value: &Number) -> Option<i64> {
    let magnitude = match value {
        Number::Integer(integer) if integer.is_negative() => Number::Integer(integer.negated()),
        Number::Rational(rational) if rational.numerator().is_negative() => {
            value.negate_exact().ok()?
        }
        other => other.clone(),
    };
    if matches!(&magnitude, Number::Integer(integer) if integer.is_zero()) {
        return Some(0);
    }
    let mut exponent = 0_i64;
    while is_below(&magnitude, &power_of_ten(exponent)?) {
        exponent -= 1;
    }
    while !is_below(&magnitude, &power_of_ten(exponent + 1)?) {
        exponent += 1;
    }
    Some(exponent)
}

fn is_below(value: &Number, limit: &Number) -> bool {
    match limit.sub_exact(value) {
        Ok(Number::Integer(difference)) => !difference.is_negative() && !difference.is_zero(),
        Ok(Number::Rational(difference)) => !difference.numerator().is_negative(),
        _ => false,
    }
}

fn digits_text(value: &Number, decimals: u32) -> Option<String> {
    let scaled = value.mul_exact(&power_of_ten(i64::from(decimals))?).ok()?;
    let Number::Integer(scaled) = scaled else {
        return None;
    };
    let is_negative = scaled.is_negative();
    let digits = scaled.absolute().to_i128()?.to_string();
    let mut text = String::new();
    if is_negative {
        text.push(MINUS);
    }
    if decimals == 0 {
        text.push_str(&digits);
        return Some(text);
    }
    let decimals = usize::try_from(decimals).ok()?;
    let padded = format!("{digits:0>width$}", width = decimals + 1);
    let point = padded.len() - decimals;
    text.push_str(&padded[..point]);
    text.push(DECIMAL_POINT);
    text.push_str(&padded[point..]);
    Some(text)
}

pub(crate) fn reading_digits(value: &Number, significant_digits: u32) -> Option<String> {
    let rounded = round_half_away_to_significant_digits(value, significant_digits).ok()?;
    let decade = decade(&rounded)?;
    if (PLAIN_LOWEST_DECADE..=PLAIN_HIGHEST_DECADE).contains(&decade) {
        let decimals = u32::try_from((i64::from(significant_digits) - 1 - decade).max(0)).ok()?;
        return digits_text(&rounded, decimals);
    }
    let scaled = rounded
        .mul_exact(&power_of_ten(i64::from(significant_digits) - 1 - decade)?)
        .ok()?;
    let digits = digits_text(&scaled, 0)?;
    Some(power_of_ten_text(&digits, decade, DECIMAL_POINT))
}

pub(crate) fn measured_texts(value: &Number, uncertainty: &Number) -> Option<(String, String)> {
    let shown = round_half_away_to_significant_digits(uncertainty, UNCERTAINTY_DIGITS).ok()?;
    if is_zero(&shown) {
        return None;
    }
    let place = decade(&shown)? - i64::from(UNCERTAINTY_DIGITS - 1);
    let decade = decade(value)?;
    if is_zero(value) || (PLAIN_LOWEST_DECADE..=PLAIN_HIGHEST_DECADE).contains(&decade) {
        let decimals = decimals_of(place)?;
        return Some((
            digits_text(&rounded_to_place(value, place)?, decimals)?,
            digits_text(&shown, decimals)?,
        ));
    }
    let scale = power_of_ten(-decade)?;
    let decimals = u32::try_from(decade.checked_sub(place)?.max(0)).ok()?;
    let value = rounded_to_place(value, place)?.mul_exact(&scale).ok()?;
    let uncertainty = shown.mul_exact(&scale).ok()?;
    Some((
        with_power_of_ten(&digits_text(&value, decimals)?, decade),
        with_power_of_ten(&digits_text(&uncertainty, decimals)?, decade),
    ))
}

fn with_power_of_ten(digits: &str, exponent: i64) -> String {
    format!("{digits}{POWER_OF_TEN_MARK}{exponent}")
}

fn decimals_of(place: i64) -> Option<u32> {
    u32::try_from((-place).max(0)).ok()
}

fn rounded_to_place(value: &Number, place: i64) -> Option<Number> {
    if place <= 0 {
        let rounded =
            round_to_decimal_places(value, decimals_of(place)?, DecimalRounding::TiesToEven)
                .ok()?;
        return Some(rounded.rounded);
    }
    let down = value.mul_exact(&power_of_ten(-place)?).ok()?;
    let rounded = round_to_decimal_places(&down, 0, DecimalRounding::TiesToEven).ok()?;
    rounded.rounded.mul_exact(&power_of_ten(place)?).ok()
}

pub(crate) fn is_zero(value: &Number) -> bool {
    matches!(value.to_exact(), Ok(Number::Integer(integer)) if integer.is_zero())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadingDistance {
    Below(String),
    Above(String),
    AtMost(String),
}

pub(crate) fn reading_distance(reading: &str, exact: Option<&Number>) -> Option<ReadingDistance> {
    if let Some(exact @ Number::Rational(_)) = exact {
        let rounded = round_half_away_to_significant_digits(exact, READING_DIGITS).ok()?;
        let difference = rounded.sub_exact(exact).ok()?;
        let is_below = match &difference {
            Number::Integer(integer) => integer.is_negative(),
            Number::Rational(rational) => rational.numerator().is_negative(),
            Number::F32(_) | Number::F64(_) => return None,
        };
        return Some(if is_below {
            ReadingDistance::Below(crate::summary::digits_text(
                &difference.negate_exact().ok()?,
            ))
        } else {
            ReadingDistance::Above(crate::summary::digits_text(&difference))
        });
    }
    let (mantissa, exponent) = match reading.split_once(POWER_OF_TEN_MARK) {
        Some((mantissa, exponent)) => (mantissa, exponent.parse::<i64>().ok()?),
        None => (reading, 0),
    };
    let places = mantissa
        .split_once(DECIMAL_POINT)
        .map_or(0, |(_, fraction)| fraction.len());
    let places = i64::try_from(places).ok()?;
    let half = power_of_ten(exponent - places - 1)?
        .mul_exact(&Number::Integer(Integer::from(5_i64)))
        .ok()?;
    let written = if reading.contains(POWER_OF_TEN_MARK) {
        reading_digits(&half, 1)?
    } else {
        crate::summary::digits_text(&half)
    };
    Some(ReadingDistance::AtMost(written))
}

pub(crate) fn exact_reading(value: &Number) -> Option<String> {
    let rounded = round_half_away_to_significant_digits(value, READING_DIGITS).ok()?;
    if rounded.sub_exact(value).is_ok_and(
        |difference| matches!(&difference, Number::Integer(integer) if integer.is_zero()),
    ) {
        return None;
    }
    reading_digits(value, READING_DIGITS)
}

pub(crate) fn expression_reading(
    pool: &mut ExprPool,
    expression: ExprId,
    factor: &ScaleFactor,
) -> Option<String> {
    let rounded = round_scaled_expression(
        pool,
        expression,
        &factor.rational_part(),
        factor.pi_exponent(),
        READING_DIGITS,
    )?;
    reading_digits(&rounded, READING_DIGITS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(pool: &mut ExprPool, symbol: &str) -> UnitId {
        pool.units_mut().lookup(symbol).unwrap()
    }

    #[test]
    fn degrees_sort_below_radians() {
        let mut pool = ExprPool::new();
        let radian = pool.units().dimensionless();
        let degree = unit(&mut pool, "deg");

        let sorted = sorted_by_scale(&pool, vec![radian, degree]);

        assert_eq!(sorted, vec![degree, radian]);
    }

    #[test]
    fn metres_sort_below_kilometres() {
        let mut pool = ExprPool::new();
        let metre = unit(&mut pool, "m");
        let kilometre = unit(&mut pool, "km");

        let sorted = sorted_by_scale(&pool, vec![kilometre, metre]);

        assert_eq!(sorted, vec![metre, kilometre]);
    }

    #[test]
    fn a_repeated_size_keeps_the_first_unit() {
        let mut pool = ExprPool::new();
        let litre = unit(&mut pool, "l");
        let decimetre = unit(&mut pool, "dm");
        let cubic_decimetre = pool.units_mut().power(decimetre, 3).unwrap();

        let sorted = sorted_by_scale(&pool, vec![litre, cubic_decimetre]);

        assert_eq!(sorted, vec![litre]);
    }
}
