use calc_exec::Domain;
use calc_expr::{BuiltinConstant, ExprId, ExprPool, Head, Operator};
use calc_numbers::{Number, pow_f32, pow_f64};

use crate::view::AxisUnit;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ConversionNotBuilt;

fn apply(
    pool: &mut ExprPool,
    operator: Operator,
    arguments: &[ExprId],
) -> Result<ExprId, ConversionNotBuilt> {
    pool.apply(Head::Operator(operator), arguments)
        .map_err(|_| ConversionNotBuilt)
}

fn number(pool: &mut ExprPool, value: &Number) -> Result<ExprId, ConversionNotBuilt> {
    pool.number(value.clone()).map_err(|_| ConversionNotBuilt)
}

fn unit_factor(
    pool: &mut ExprPool,
    factor: &Number,
    pi_exponent: i8,
) -> Result<ExprId, ConversionNotBuilt> {
    let factor = number(pool, factor)?;
    if pi_exponent == 0 {
        return Ok(factor);
    }
    let pi = pool
        .symbol(BuiltinConstant::Pi.symbol())
        .map_err(|_| ConversionNotBuilt)?;
    let exponent = number(pool, &Number::from(i64::from(pi_exponent)))?;
    let power = apply(pool, Operator::Pow, &[pi, exponent])?;
    apply(pool, Operator::Mul, &[factor, power])
}

pub(crate) fn shown_to_coherent(
    pool: &mut ExprPool,
    shown: ExprId,
    unit: &AxisUnit,
) -> Result<ExprId, ConversionNotBuilt> {
    match unit {
        AxisUnit::Coherent { .. } => Ok(shown),
        AxisUnit::Unit {
            factor,
            pi_exponent,
            ..
        } => {
            let factor = unit_factor(pool, factor, *pi_exponent)?;
            apply(pool, Operator::Mul, &[shown, factor])
        }
        AxisUnit::TemperatureScale { factor, offset, .. } => {
            let factor = number(pool, factor)?;
            let offset = number(pool, offset)?;
            let scaled = apply(pool, Operator::Mul, &[factor, shown])?;
            apply(pool, Operator::Add, &[scaled, offset])
        }
    }
}

pub(crate) fn coherent_to_shown(
    pool: &mut ExprPool,
    coherent: ExprId,
    unit: &AxisUnit,
) -> Result<ExprId, ConversionNotBuilt> {
    match unit {
        AxisUnit::Coherent { .. } => Ok(coherent),
        AxisUnit::Unit {
            factor,
            pi_exponent,
            ..
        } => {
            let factor = unit_factor(pool, factor, *pi_exponent)?;
            apply(pool, Operator::Div, &[coherent, factor])
        }
        AxisUnit::TemperatureScale { factor, offset, .. } => {
            let factor = number(pool, factor)?;
            let offset = number(pool, offset)?;
            let shifted = apply(pool, Operator::Sub, &[coherent, offset])?;
            apply(pool, Operator::Div, &[shifted, factor])
        }
    }
}

pub(crate) fn coherent_positions_merge(
    positions: &[Number],
    unit: &AxisUnit,
    domain: Domain,
) -> bool {
    match domain {
        Domain::F64 => {
            let convert = |shown: f64| -> f64 {
                match unit {
                    AxisUnit::Coherent { .. } => shown,
                    AxisUnit::Unit {
                        factor,
                        pi_exponent,
                        ..
                    } => {
                        let rounded = factor.round_to_f64_ties_even();
                        let scale = if *pi_exponent == 0 {
                            rounded
                        } else {
                            rounded * pow_f64(std::f64::consts::PI, f64::from(*pi_exponent))
                        };
                        shown * scale
                    }
                    AxisUnit::TemperatureScale { factor, offset, .. } => {
                        factor.round_to_f64_ties_even() * shown + offset.round_to_f64_ties_even()
                    }
                }
            };
            let converted: Vec<f64> = positions
                .iter()
                .map(|position| convert(position.round_to_f64_ties_even()))
                .collect();
            converted
                .windows(2)
                .any(|pair| matches!(pair, [first, second] if first.to_bits() == second.to_bits()))
        }
        Domain::F32 => {
            let convert = |shown: f32| -> f32 {
                match unit {
                    AxisUnit::Coherent { .. } => shown,
                    AxisUnit::Unit {
                        factor,
                        pi_exponent,
                        ..
                    } => {
                        let rounded = factor.round_to_f32_ties_even();
                        let scale = if *pi_exponent == 0 {
                            rounded
                        } else {
                            rounded * pow_f32(std::f32::consts::PI, f32::from(*pi_exponent))
                        };
                        shown * scale
                    }
                    AxisUnit::TemperatureScale { factor, offset, .. } => {
                        factor.round_to_f32_ties_even() * shown + offset.round_to_f32_ties_even()
                    }
                }
            };
            let converted: Vec<f32> = positions
                .iter()
                .map(|position| convert(position.round_to_f32_ties_even()))
                .collect();
            converted
                .windows(2)
                .any(|pair| matches!(pair, [first, second] if first.to_bits() == second.to_bits()))
        }
    }
}
