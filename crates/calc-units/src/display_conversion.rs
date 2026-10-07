use calc_numbers::{
    ENCLOSURE_BUDGET_BITS, EnclosureError, Number, correctly_rounded_to_significant_digits,
    round_to_significant_digits, smallest_f64_at_least,
};

use crate::dimension::{BaseDimension, Dimension};
use crate::scale_factor::ScaleFactor;
use crate::temperature_scale::TemperatureScale;
use crate::unit_table::{ConversionError, UnitId, UnitTable};

const DOUBLE_SIGNIFICANT_DIGITS: u32 = 17;
const SINGLE_SIGNIFICANT_DIGITS: u32 = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayTarget {
    Unit(UnitId),
    Scale(TemperatureScale),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisplayedNumber {
    Exact(Number),
    Rounded {
        value: Number,
        significant_digits: u32,
    },
    Unscaled(Number),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayConversionError {
    Conversion(ConversionError),
    NotATemperature,
    ExactValueWouldBeRounded,
    NotRounded(EnclosureError),
}

enum Map {
    Ratio(ScaleFactor),
    Affine(TemperatureScale),
}

fn never_cancelled() -> bool {
    false
}

fn map_to(
    table: &UnitTable,
    coherent: UnitId,
    target: DisplayTarget,
) -> Result<Map, DisplayConversionError> {
    match target {
        DisplayTarget::Unit(unit) => table
            .conversion_factor(coherent, unit)
            .map(Map::Ratio)
            .map_err(DisplayConversionError::Conversion),
        DisplayTarget::Scale(scale) => {
            let dimension = table.dimension(coherent).map_err(|_| {
                DisplayConversionError::Conversion(ConversionError::UnknownUnitId(coherent))
            })?;
            if dimension != Dimension::of_base(BaseDimension::ThermodynamicTemperature) {
                return Err(DisplayConversionError::NotATemperature);
            }
            Ok(Map::Affine(scale))
        }
    }
}

fn machine_digits(value: &Number) -> Option<u32> {
    match value {
        Number::F64(_) => Some(DOUBLE_SIGNIFICANT_DIGITS),
        Number::F32(_) => Some(SINGLE_SIGNIFICANT_DIGITS),
        Number::Integer(_) | Number::Rational(_) => None,
    }
}

fn is_special(value: &Number) -> bool {
    match value {
        Number::F64(double) => !double.is_finite(),
        Number::F32(single) => !single.is_finite(),
        Number::Integer(_) | Number::Rational(_) => false,
    }
}

fn is_negative_zero(value: &Number) -> bool {
    match value {
        Number::F64(double) => *double == 0.0 && double.is_sign_negative(),
        Number::F32(single) => *single == 0.0 && single.is_sign_negative(),
        Number::Integer(_) | Number::Rational(_) => false,
    }
}

fn exact_of(value: &Number) -> Number {
    match value.to_exact() {
        Ok(exact) => exact,
        Err(_) => value.clone(),
    }
}

fn ratio_product(exact: &Number, factor: &ScaleFactor) -> Option<Number> {
    exact.mul_exact(&factor.rational_part()).ok()
}

fn rounded(exact_result: &Number, digits: u32) -> Result<DisplayedNumber, DisplayConversionError> {
    round_to_significant_digits(exact_result, digits)
        .map(|value| DisplayedNumber::Rounded {
            value,
            significant_digits: digits,
        })
        .map_err(DisplayConversionError::NotRounded)
}

fn pi_product_rounded(
    exact: &Number,
    factor: &ScaleFactor,
    digits: u32,
) -> Result<DisplayedNumber, DisplayConversionError> {
    let rational = factor.rational_part();
    let pi_exponent = i64::from(factor.pi_exponent());
    correctly_rounded_to_significant_digits(
        digits,
        ENCLOSURE_BUDGET_BITS,
        &never_cancelled,
        |step| {
            let product = step.exact(exact)?.mul(&step.exact(&rational)?)?;
            let pi_power = step.pi()?.power(pi_exponent.abs())?;
            if pi_exponent < 0 {
                product.div(&pi_power)
            } else {
                product.mul(&pi_power)
            }
        },
    )
    .map(|value| DisplayedNumber::Rounded {
        value,
        significant_digits: digits,
    })
    .map_err(DisplayConversionError::NotRounded)
}

pub fn convert_for_display(
    table: &UnitTable,
    value: &Number,
    coherent: UnitId,
    target: DisplayTarget,
    significant_digits: Option<u32>,
) -> Result<DisplayedNumber, DisplayConversionError> {
    let map = map_to(table, coherent, target)?;
    let is_ratio = matches!(map, Map::Ratio(_));
    if is_special(value) || (is_ratio && is_negative_zero(value)) {
        return Ok(DisplayedNumber::Unscaled(value.clone()));
    }
    let exact = exact_of(value);
    let digits =
        machine_digits(value).map(|format_digits| significant_digits.unwrap_or(format_digits));
    match (map, digits) {
        (Map::Affine(scale), None) => scale
            .from_kelvin(&exact)
            .map(DisplayedNumber::Exact)
            .map_err(|_| DisplayConversionError::NotATemperature),
        (Map::Affine(scale), Some(digits)) => {
            let reading = scale
                .from_kelvin(&exact)
                .map_err(|_| DisplayConversionError::NotATemperature)?;
            rounded(&reading, digits)
        }
        (Map::Ratio(factor), None) if factor.pi_exponent() == 0 => ratio_product(&exact, &factor)
            .map(DisplayedNumber::Exact)
            .ok_or(DisplayConversionError::ExactValueWouldBeRounded),
        (Map::Ratio(_), None) => Err(DisplayConversionError::ExactValueWouldBeRounded),
        (Map::Ratio(factor), Some(digits)) if factor.pi_exponent() == 0 => {
            let product = ratio_product(&exact, &factor)
                .ok_or(DisplayConversionError::ExactValueWouldBeRounded)?;
            rounded(&product, digits)
        }
        (Map::Ratio(factor), Some(digits)) => pi_product_rounded(&exact, &factor, digits),
    }
}

fn spread_factor(map: &Map) -> ScaleFactor {
    match map {
        Map::Ratio(factor) => factor.clone(),
        Map::Affine(scale) => {
            let ratio = scale.factor_ratio();
            ScaleFactor::from_positive_ratio(ratio.denominator.get(), ratio.numerator.get())
        }
    }
}

fn upward_pi_product(
    spread: &Number,
    factor: &ScaleFactor,
) -> Result<Number, DisplayConversionError> {
    let rational = factor.rational_part();
    let pi_exponent = i64::from(factor.pi_exponent());
    let enclosure = calc_numbers::enclose_to_significant_digits(
        DOUBLE_SIGNIFICANT_DIGITS,
        ENCLOSURE_BUDGET_BITS,
        &never_cancelled,
        |step| {
            let product = step.exact(spread)?.mul(&step.exact(&rational)?)?;
            let pi_power = step.pi()?.power(pi_exponent.abs())?;
            if pi_exponent < 0 {
                product.div(&pi_power)
            } else {
                product.mul(&pi_power)
            }
        },
    )
    .map_err(DisplayConversionError::NotRounded)?;
    smallest_f64_at_least(&enclosure.upper)
        .map(Number::F64)
        .ok_or(DisplayConversionError::NotRounded(
            EnclosureError::NotFinite,
        ))
}

pub fn scale_spread_for_display(
    table: &UnitTable,
    spread: &Number,
    coherent: UnitId,
    target: DisplayTarget,
) -> Result<Number, DisplayConversionError> {
    let map = map_to(table, coherent, target)?;
    if is_special(spread) {
        return Ok(spread.clone());
    }
    let factor = spread_factor(&map);
    let exact = exact_of(spread);
    let is_exact_input = machine_digits(spread).is_none();
    if factor.pi_exponent() != 0 {
        return upward_pi_product(&exact, &factor);
    }
    let product = ratio_product(&exact, &factor).ok_or(DisplayConversionError::NotRounded(
        EnclosureError::NotFinite,
    ))?;
    if is_exact_input {
        return Ok(product);
    }
    smallest_f64_at_least(&product)
        .map(Number::F64)
        .ok_or(DisplayConversionError::NotRounded(
            EnclosureError::NotFinite,
        ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisplayedPiMultiple {
    Rational(Number),
    PiPower { coefficient: Number, exponent: i32 },
}

pub fn convert_pi_multiple_for_display(
    table: &UnitTable,
    coefficient: &Number,
    pi_exponent: i32,
    coherent: UnitId,
    target: DisplayTarget,
) -> Result<DisplayedPiMultiple, DisplayConversionError> {
    let Map::Ratio(factor) = map_to(table, coherent, target)? else {
        return Err(DisplayConversionError::ExactValueWouldBeRounded);
    };
    let product = coefficient
        .mul_exact(&factor.rational_part())
        .map_err(|_| DisplayConversionError::ExactValueWouldBeRounded)?;
    match pi_exponent.checked_add(factor.pi_exponent()) {
        Some(0) => Ok(DisplayedPiMultiple::Rational(product)),
        Some(exponent) => Ok(DisplayedPiMultiple::PiPower {
            coefficient: product,
            exponent,
        }),
        None => Err(DisplayConversionError::ExactValueWouldBeRounded),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_numbers::Integer;

    fn fraction(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    fn decimal(digits: i128, scale: u32) -> Number {
        Number::fraction(&Integer::from(digits), &Integer::from(10_i64).pow(scale)).unwrap()
    }

    fn unit(table: &mut UnitTable, symbol: &str) -> UnitId {
        table.lookup(symbol).unwrap()
    }

    fn is_at_most(left: &Number, right: &Number) -> bool {
        match right.sub_exact(left).unwrap() {
            Number::Integer(integer) => !integer.is_negative(),
            Number::Rational(rational) => !rational.numerator().is_negative(),
            Number::F32(_) | Number::F64(_) => false,
        }
    }

    #[test]
    fn five_kilometres_stored_in_metres_show_as_metres() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");

        let shown = convert_for_display(
            &table,
            &Number::from(5000_i64),
            metre,
            DisplayTarget::Unit(metre),
            None,
        );

        assert_eq!(shown, Ok(DisplayedNumber::Exact(Number::from(5000_i64))));
    }

    #[test]
    fn five_kilometres_stored_in_metres_show_as_kilometres() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");
        let kilometre = unit(&mut table, "km");

        let shown = convert_for_display(
            &table,
            &Number::from(5000_i64),
            metre,
            DisplayTarget::Unit(kilometre),
            None,
        );

        assert_eq!(shown, Ok(DisplayedNumber::Exact(Number::from(5_i64))));
    }

    #[test]
    fn machine_value_in_a_rational_unit_is_rounded_to_its_digits() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");
        let kilometre = unit(&mut table, "km");

        let shown = convert_for_display(
            &table,
            &Number::F64(5000.0),
            metre,
            DisplayTarget::Unit(kilometre),
            None,
        );

        assert_eq!(
            shown,
            Ok(DisplayedNumber::Rounded {
                value: Number::from(5_i64),
                significant_digits: 17
            })
        );
    }

    #[test]
    fn thirty_degrees_in_radians_are_correctly_rounded_back_to_degrees() {
        let mut table = UnitTable::new();
        let radian = table.dimensionless();
        let degree = unit(&mut table, "deg");

        let shown = convert_for_display(
            &table,
            &Number::F64(0.5235987755982988),
            radian,
            DisplayTarget::Unit(degree),
            None,
        );

        assert_eq!(
            shown,
            Ok(DisplayedNumber::Rounded {
                value: decimal(29_999_999_999_999_997, 15),
                significant_digits: 17
            })
        );
    }

    #[test]
    fn exact_value_is_not_rounded_into_a_unit_with_pi() {
        let mut table = UnitTable::new();
        let radian = table.dimensionless();
        let degree = unit(&mut table, "deg");

        let shown = convert_for_display(
            &table,
            &fraction(1, 2),
            radian,
            DisplayTarget::Unit(degree),
            None,
        );

        assert_eq!(shown, Err(DisplayConversionError::ExactValueWouldBeRounded));
    }

    #[test]
    fn kelvin_reading_is_shown_exactly_on_the_fahrenheit_scale() {
        let mut table = UnitTable::new();
        let kelvin = unit(&mut table, "K");

        let shown = convert_for_display(
            &table,
            &decimal(31_015, 2),
            kelvin,
            DisplayTarget::Scale(TemperatureScale::Fahrenheit),
            None,
        );

        assert_eq!(shown, Ok(DisplayedNumber::Exact(decimal(986, 1))));
    }

    #[test]
    fn machine_kelvin_reading_is_rounded_on_the_fahrenheit_scale() {
        let mut table = UnitTable::new();
        let kelvin = unit(&mut table, "K");

        let shown = convert_for_display(
            &table,
            &Number::F64(310.15),
            kelvin,
            DisplayTarget::Scale(TemperatureScale::Fahrenheit),
            None,
        );

        assert_eq!(
            shown,
            Ok(DisplayedNumber::Rounded {
                value: decimal(98_599_999_999_999_959, 15),
                significant_digits: 17
            })
        );
    }

    #[test]
    fn scale_is_refused_for_a_value_that_is_not_a_temperature() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");

        let shown = convert_for_display(
            &table,
            &Number::from(1_i64),
            metre,
            DisplayTarget::Scale(TemperatureScale::Celsius),
            None,
        );

        assert_eq!(shown, Err(DisplayConversionError::NotATemperature));
    }

    #[test]
    fn unit_of_another_dimension_is_refused() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");
        let second = unit(&mut table, "s");

        let shown = convert_for_display(
            &table,
            &Number::from(1_i64),
            metre,
            DisplayTarget::Unit(second),
            None,
        );

        assert!(matches!(shown, Err(DisplayConversionError::Conversion(_))));
    }

    #[test]
    fn not_a_number_is_shown_unscaled() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");
        let kilometre = unit(&mut table, "km");

        let shown = convert_for_display(
            &table,
            &Number::F64(f64::NAN),
            metre,
            DisplayTarget::Unit(kilometre),
            None,
        );

        assert!(
            matches!(shown, Ok(DisplayedNumber::Unscaled(Number::F64(value))) if value.is_nan())
        );
    }

    #[test]
    fn infinity_is_shown_unscaled() {
        let mut table = UnitTable::new();
        let kelvin = unit(&mut table, "K");

        let shown = convert_for_display(
            &table,
            &Number::F64(f64::INFINITY),
            kelvin,
            DisplayTarget::Scale(TemperatureScale::Celsius),
            None,
        );

        assert_eq!(
            shown,
            Ok(DisplayedNumber::Unscaled(Number::F64(f64::INFINITY)))
        );
    }

    #[test]
    fn machine_bound_is_scaled_and_rounded_upward() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");
        let kilometre = unit(&mut table, "km");

        let scaled = scale_spread_for_display(
            &table,
            &Number::F64(0.1),
            metre,
            DisplayTarget::Unit(kilometre),
        )
        .unwrap();

        let exact = Number::F64(0.1)
            .to_exact()
            .unwrap()
            .mul_exact(&fraction(1, 1000))
            .unwrap();
        let nearest = exact.round_to_f64_ties_even();
        assert_eq!(scaled, Number::F64(nearest.next_up()));
        assert!(is_at_most(&exact, &scaled.to_exact().unwrap()));
    }

    #[test]
    fn uncertainty_on_a_temperature_scale_is_scaled_without_offset() {
        let mut table = UnitTable::new();
        let kelvin = unit(&mut table, "K");

        let scaled = scale_spread_for_display(
            &table,
            &fraction(1, 2),
            kelvin,
            DisplayTarget::Scale(TemperatureScale::Fahrenheit),
        );

        assert_eq!(scaled, Ok(fraction(9, 10)));
    }

    #[test]
    fn bound_in_a_unit_with_pi_is_rounded_upward_above_the_exact_product() {
        let mut table = UnitTable::new();
        let radian = table.dimensionless();
        let degree = unit(&mut table, "deg");

        let scaled = scale_spread_for_display(
            &table,
            &Number::F64(4.440892098500627e-16),
            radian,
            DisplayTarget::Unit(degree),
        )
        .unwrap();

        let below = decimal(254_444_374_517_081_389, 31);
        let above = decimal(254_444_374_517_082, 28);
        let scaled = scaled.to_exact().unwrap();
        assert!(is_at_most(&below, &scaled) && is_at_most(&scaled, &above));
    }

    #[test]
    fn machine_value_is_rounded_once_at_the_digits_the_display_asks_for() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");
        let kilometre = unit(&mut table, "km");

        let shown = convert_for_display(
            &table,
            &Number::F64(1234.5),
            metre,
            DisplayTarget::Unit(kilometre),
            Some(4),
        );

        assert_eq!(
            shown,
            Ok(DisplayedNumber::Rounded {
                value: decimal(1234, 3),
                significant_digits: 4
            })
        );
    }

    #[test]
    fn negative_machine_zero_keeps_its_sign_in_a_ratio_unit() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");
        let kilometre = unit(&mut table, "km");

        let shown = convert_for_display(
            &table,
            &Number::F64(-0.0),
            metre,
            DisplayTarget::Unit(kilometre),
            None,
        );

        assert!(matches!(
            shown,
            Ok(DisplayedNumber::Unscaled(Number::F64(zero))) if zero.is_sign_negative()
        ));
    }

    #[test]
    fn sixth_of_pi_radians_is_exactly_thirty_degrees() {
        let mut table = UnitTable::new();
        let radian = table.dimensionless();
        let degree = unit(&mut table, "deg");

        let shown = convert_pi_multiple_for_display(
            &table,
            &fraction(1, 6),
            1,
            radian,
            DisplayTarget::Unit(degree),
        );

        assert_eq!(
            shown,
            Ok(DisplayedPiMultiple::Rational(Number::from(30_i64)))
        );
    }

    #[test]
    fn pi_multiple_in_a_rational_unit_stays_a_pi_multiple() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");
        let kilometre = unit(&mut table, "km");

        let shown = convert_pi_multiple_for_display(
            &table,
            &Number::from(2_i64),
            1,
            metre,
            DisplayTarget::Unit(kilometre),
        );

        assert_eq!(
            shown,
            Ok(DisplayedPiMultiple::PiPower {
                coefficient: fraction(1, 500),
                exponent: 1
            })
        );
    }

    #[test]
    fn pi_multiple_on_a_temperature_scale_would_be_rounded() {
        let mut table = UnitTable::new();
        let kelvin = unit(&mut table, "K");

        let shown = convert_pi_multiple_for_display(
            &table,
            &Number::from(100_i64),
            1,
            kelvin,
            DisplayTarget::Scale(TemperatureScale::Celsius),
        );

        assert_eq!(shown, Err(DisplayConversionError::ExactValueWouldBeRounded));
    }

    #[test]
    fn square_radian_value_of_one_square_degree_is_one_square_degree() {
        let mut table = UnitTable::new();
        let radian = table.dimensionless();
        let degree = unit(&mut table, "deg");
        let square_degree = table.power(degree, 2).unwrap();

        let shown = convert_pi_multiple_for_display(
            &table,
            &fraction(1, 32_400),
            2,
            radian,
            DisplayTarget::Unit(square_degree),
        );

        assert_eq!(
            shown,
            Ok(DisplayedPiMultiple::Rational(Number::from(1_i64)))
        );
    }

    #[test]
    fn reciprocal_of_pi_in_a_rational_unit_keeps_its_negative_power() {
        let table = UnitTable::new();
        let radian = table.dimensionless();

        let shown = convert_pi_multiple_for_display(
            &table,
            &Number::from(1_i64),
            -1,
            radian,
            DisplayTarget::Unit(radian),
        );

        assert_eq!(
            shown,
            Ok(DisplayedPiMultiple::PiPower {
                coefficient: Number::from(1_i64),
                exponent: -1
            })
        );
    }
}
