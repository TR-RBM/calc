use std::collections::{HashMap, HashSet};

use calc_expr::{
    AccessError, BinderKind, BuildError, BuiltinConstant, ExprId, ExprPool, Head, NodeView,
    Operator,
};
use calc_numbers::{Integer, Number};
use calc_units::{
    BaseDimension, Dimension, DimensionError, ScaleFactor, UnitAccessError, UnitId,
    UnitProductError,
};

use crate::exact_evaluation::evaluate_exact;

const TEMPERATURE_DIFFERENCE_SYMBOLS: [&str; 2] = ["degC", "degF"];
use crate::temperature_conversion::{
    TemperatureConversion, TemperatureDirection, affine_map_expression,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuantityError {
    Access(AccessError),
    Build(BuildError),
    UnitAccess(UnitAccessError),
    UnitProduct(UnitProductError),
    DimensionMismatch {
        expression: ExprId,
        left: Dimension,
        right: Dimension,
    },
    DimensionedArgument {
        expression: ExprId,
        dimension: Dimension,
    },
    ExponentNotConstant(ExprId),
    FractionalDimension(ExprId),
    DimensionOutOfRange(ExprId),
    DifferenceAsReading {
        expression: ExprId,
        difference: ExprId,
    },
    BelowAbsoluteZero(ExprId),
    KindMismatch {
        expression: ExprId,
        left: &'static str,
        right: &'static str,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoherentExpression {
    pub expression: ExprId,
    pub dimension: Dimension,
    pub unit: Option<UnitId>,
}

type Checked<T> = Result<T, QuantityError>;

#[derive(Clone, Copy)]
struct Converted {
    expression: ExprId,
    dimension: Dimension,
}

fn children(view: &NodeView<'_>) -> Vec<ExprId> {
    match view {
        NodeView::Number(_) | NodeView::Symbol(_) | NodeView::Bound(_) => Vec::new(),
        NodeView::Apply { arguments, .. } => arguments.to_vec(),
        NodeView::Bind {
            arguments, body, ..
        } => arguments.iter().copied().chain([*body]).collect(),
        NodeView::Quantity { value, .. } => vec![*value],
        NodeView::Array { elements, .. } => elements.to_vec(),
    }
}

fn post_order(pool: &ExprPool, root: ExprId) -> Checked<Vec<ExprId>> {
    let mut order = Vec::new();
    let mut visited = HashSet::new();
    let mut pending = vec![(root, false)];
    while let Some((expression, children_are_done)) = pending.pop() {
        if children_are_done {
            order.push(expression);
            continue;
        }
        if !visited.insert(expression) {
            continue;
        }
        let view = pool.node(expression).map_err(QuantityError::Access)?;
        pending.push((expression, true));
        pending.extend(
            children(&view)
                .into_iter()
                .rev()
                .filter(|child| !visited.contains(child))
                .map(|child| (child, false)),
        );
    }
    Ok(order)
}

fn same_dimension(expression: ExprId, dimensions: &[Dimension]) -> Checked<Dimension> {
    let first = dimensions
        .first()
        .copied()
        .unwrap_or(Dimension::DIMENSIONLESS);
    match dimensions.iter().find(|dimension| **dimension != first) {
        Some(other) => Err(QuantityError::DimensionMismatch {
            expression,
            left: first,
            right: *other,
        }),
        None => Ok(first),
    }
}

fn dimensionless(expression: ExprId, dimensions: &[Dimension]) -> Checked<Dimension> {
    match dimensions
        .iter()
        .find(|dimension| !dimension.is_dimensionless())
    {
        Some(dimension) => Err(QuantityError::DimensionedArgument {
            expression,
            dimension: *dimension,
        }),
        None => Ok(Dimension::DIMENSIONLESS),
    }
}

fn out_of_range(expression: ExprId) -> impl Fn(DimensionError) -> QuantityError {
    move |_| QuantityError::DimensionOutOfRange(expression)
}

fn product(expression: ExprId, left: Dimension, right: Dimension) -> Checked<Dimension> {
    left.multiply(&right).map_err(out_of_range(expression))
}

fn quotient(expression: ExprId, left: Dimension, right: Dimension) -> Checked<Dimension> {
    let inverse = right.power(-1).map_err(out_of_range(expression))?;
    product(expression, left, inverse)
}

fn rational_power(
    expression: ExprId,
    base: Dimension,
    numerator: &Integer,
    denominator: &Integer,
) -> Checked<Dimension> {
    let small = |value: &Integer| {
        value
            .to_i64()
            .and_then(|value| i8::try_from(value).ok())
            .ok_or(QuantityError::DimensionOutOfRange(expression))
    };
    let numerator = small(numerator)?;
    let denominator = small(denominator)?;
    let mut exponents = base.exponents();
    for exponent in &mut exponents {
        if *exponent % denominator != 0 {
            return Err(QuantityError::FractionalDimension(expression));
        }
        *exponent /= denominator;
    }
    Dimension::from_exponents(exponents)
        .power(numerator)
        .map_err(out_of_range(expression))
}

fn scale_expression(pool: &mut ExprPool, scale: &ScaleFactor) -> Checked<Option<ExprId>> {
    if scale.is_one() {
        return Ok(None);
    }
    let rational = pool
        .number(scale.rational_part())
        .map_err(QuantityError::Build)?;
    if scale.pi_exponent() == 0 {
        return Ok(Some(rational));
    }
    let pi = pool
        .symbol(BuiltinConstant::Pi.symbol())
        .map_err(QuantityError::Build)?;
    let exponent = pool
        .number(Number::from(i64::from(scale.pi_exponent())))
        .map_err(QuantityError::Build)?;
    let power = build(pool, Operator::Pow, &[pi, exponent])?;
    build(pool, Operator::Mul, &[rational, power]).map(Some)
}

fn build(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> Checked<ExprId> {
    pool.apply(Head::Operator(operator), arguments)
        .map_err(QuantityError::Build)
}

fn unit_value(pool: &mut ExprPool, value: ExprId, unit: UnitId) -> Checked<Converted> {
    let dimension = pool
        .units()
        .dimension(unit)
        .map_err(QuantityError::UnitAccess)?;
    let scale = pool
        .units()
        .scale_factor(unit)
        .map_err(QuantityError::UnitAccess)?
        .clone();
    let expression = match scale_expression(pool, &scale)? {
        Some(factor) => build(pool, Operator::Mul, &[value, factor])?,
        None => value,
    };
    Ok(Converted {
        expression,
        dimension,
    })
}

fn is_a_plain_number(pool: &ExprPool, expression: ExprId) -> bool {
    matches!(pool.node(expression), Ok(NodeView::Number(_)))
}

fn joined_kind(
    expression: ExprId,
    kinds: &[Option<&'static str>],
) -> Checked<Option<&'static str>> {
    let mut known = kinds.iter().flatten();
    let Some(first) = known.next() else {
        return Ok(None);
    };
    match known.find(|kind| *kind != first) {
        Some(other) => Err(QuantityError::KindMismatch {
            expression,
            left: first,
            right: other,
        }),
        None => Ok(Some(first)),
    }
}

fn check_kinds(pool: &ExprPool, root: ExprId) -> Checked<()> {
    let mut kinds: HashMap<ExprId, Option<&'static str>> = HashMap::new();
    for expression in post_order(pool, root)? {
        let view = pool.node(expression).map_err(QuantityError::Access)?;
        let of = |child: &ExprId| kinds.get(child).copied().flatten();
        let kind = match view {
            NodeView::Quantity { unit, .. } => pool.units().kind_sharing_the_unit(unit),
            NodeView::Apply {
                head: Head::Operator(Operator::ConvertUnit),
                arguments: [value, target],
            } => {
                let target_kind = match pool.node(*target).map_err(QuantityError::Access)? {
                    NodeView::Quantity { unit, .. } => pool.units().kind_sharing_the_unit(unit),
                    _ => None,
                };
                joined_kind(expression, &[of(value), target_kind])?
            }
            NodeView::Apply {
                head:
                    Head::Operator(
                        Operator::Add
                        | Operator::Sub
                        | Operator::Min
                        | Operator::Max
                        | Operator::Equal
                        | Operator::NotEqual
                        | Operator::Less
                        | Operator::LessOrEqual
                        | Operator::Greater
                        | Operator::GreaterOrEqual,
                    ),
                arguments,
            } => {
                let children: Vec<Option<&'static str>> = arguments.iter().map(of).collect();
                joined_kind(expression, &children)?
            }
            NodeView::Apply {
                head: Head::Operator(Operator::Neg | Operator::Abs),
                arguments: [argument],
            } => of(argument),
            NodeView::Apply {
                head: Head::Operator(Operator::Mul | Operator::Div),
                arguments: [left, right],
            } => match (of(left), of(right)) {
                (Some(kind), None) if is_a_plain_number(pool, *right) => Some(kind),
                (None, Some(kind))
                    if is_a_plain_number(pool, *left)
                        && pool.node(expression).is_ok_and(|node| {
                            matches!(
                                node,
                                NodeView::Apply {
                                    head: Head::Operator(Operator::Mul),
                                    ..
                                }
                            )
                        }) =>
                {
                    Some(kind)
                }
                _ => None,
            },
            _ => None,
        };
        kinds.insert(expression, kind);
    }
    Ok(())
}

fn target_unit(pool: &ExprPool, target: ExprId) -> Checked<Option<UnitId>> {
    let NodeView::Quantity { value, unit } = pool.node(target).map_err(QuantityError::Access)?
    else {
        return Ok(None);
    };
    let is_one = matches!(
        pool.node(value).map_err(QuantityError::Access)?,
        NodeView::Number(number)
            if pool.number_value(number).map_err(QuantityError::Access)? == &Number::from(1_i64)
    );
    Ok(is_one.then_some(unit))
}

fn convert_operator(
    pool: &mut ExprPool,
    expression: ExprId,
    operator: Operator,
    arguments: &[Converted],
) -> Checked<Converted> {
    let expressions: Vec<ExprId> = arguments
        .iter()
        .map(|argument| argument.expression)
        .collect();
    let dimensions: Vec<Dimension> = arguments
        .iter()
        .map(|argument| argument.dimension)
        .collect();
    let rebuilt = |pool: &mut ExprPool, dimension: Dimension| -> Checked<Converted> {
        Ok(Converted {
            expression: build(pool, operator, &expressions)?,
            dimension,
        })
    };
    match (operator, dimensions.as_slice()) {
        (Operator::Add | Operator::Sub | Operator::Min | Operator::Max | Operator::Complex, _) => {
            let dimension = same_dimension(expression, &dimensions)?;
            rebuilt(pool, dimension)
        }
        (Operator::Mul, [left, right]) => {
            let dimension = product(expression, *left, *right)?;
            rebuilt(pool, dimension)
        }
        (Operator::Div, [left, right]) => {
            let dimension = quotient(expression, *left, *right)?;
            rebuilt(pool, dimension)
        }
        (Operator::MulAdd, [left, right, addend]) => {
            let dimension = product(expression, *left, *right)?;
            let dimension = same_dimension(expression, &[dimension, *addend])?;
            rebuilt(pool, dimension)
        }
        (
            Operator::Neg | Operator::Abs | Operator::ToF32 | Operator::ToF64 | Operator::ToExact,
            [argument],
        ) => rebuilt(pool, *argument),
        (Operator::EnclosureLower | Operator::EnclosureUpper, [value, digits]) => {
            dimensionless(expression, &[*digits])?;
            rebuilt(pool, *value)
        }
        (Operator::CopySign, [magnitude, _]) => rebuilt(pool, *magnitude),
        (Operator::Pow, [base, exponent]) => {
            dimensionless(expression, &[*exponent])?;
            if base.is_dimensionless() {
                return rebuilt(pool, Dimension::DIMENSIONLESS);
            }
            let exponent_expression = expressions[1];
            let constant = evaluate_exact(pool, exponent_expression)
                .ok()
                .and_then(|evaluation| evaluation.rational_value().cloned())
                .ok_or(QuantityError::ExponentNotConstant(expression))?;
            let dimension = match &constant {
                Number::Integer(integer) => {
                    rational_power(expression, *base, integer, &Integer::one())?
                }
                Number::Rational(rational) => rational_power(
                    expression,
                    *base,
                    rational.numerator(),
                    rational.denominator(),
                )?,
                Number::F32(_) | Number::F64(_) => {
                    return Err(QuantityError::ExponentNotConstant(expression));
                }
            };
            rebuilt(pool, dimension)
        }
        (Operator::Sqrt, [argument]) => {
            let dimension = rational_power(
                expression,
                *argument,
                &Integer::one(),
                &Integer::from(2_i64),
            )?;
            rebuilt(pool, dimension)
        }
        (Operator::Atan2, _) => {
            same_dimension(expression, &dimensions)?;
            rebuilt(pool, Dimension::DIMENSIONLESS)
        }
        (
            Operator::Equal
            | Operator::NotEqual
            | Operator::Less
            | Operator::LessOrEqual
            | Operator::Greater
            | Operator::GreaterOrEqual,
            _,
        ) => {
            same_dimension(expression, &dimensions)?;
            rebuilt(pool, Dimension::DIMENSIONLESS)
        }
        (Operator::Select, [condition, when_true, when_false]) => {
            dimensionless(expression, &[*condition])?;
            let dimension = same_dimension(expression, &[*when_true, *when_false])?;
            rebuilt(pool, dimension)
        }
        (Operator::Uncertain, [value, uncertainty, _mark]) => {
            let dimension = same_dimension(expression, &[*value, *uncertainty])?;
            rebuilt(pool, dimension)
        }
        (Operator::UncertainExpanded, [value, uncertainty, coverage, _mark]) => {
            dimensionless(expression, &[*coverage])?;
            let dimension = same_dimension(expression, &[*value, *uncertainty])?;
            rebuilt(pool, dimension)
        }
        (_, [argument]) if let Some(conversion) = TemperatureConversion::of_operator(operator) => {
            temperature_operator(pool, expression, conversion, expressions[0], *argument)
        }
        (Operator::Between, [low, high]) => {
            let dimension = same_dimension(expression, &[*low, *high])?;
            rebuilt(pool, dimension)
        }
        (Operator::Tolerance, [value, share]) => {
            dimensionless(expression, &[*share])?;
            rebuilt(pool, *value)
        }
        (Operator::ConvertUnit, [value, target]) => {
            let dimension = same_dimension(expression, &[*value, *target])?;
            Ok(Converted {
                expression: expressions[0],
                dimension,
            })
        }
        _ => {
            dimensionless(expression, &dimensions)?;
            rebuilt(pool, Dimension::DIMENSIONLESS)
        }
    }
}

fn temperature_operator(
    pool: &mut ExprPool,
    expression: ExprId,
    conversion: TemperatureConversion,
    argument: ExprId,
    dimension: Dimension,
) -> Checked<Converted> {
    let temperature = Dimension::of_base(BaseDimension::ThermodynamicTemperature);
    let result_dimension = match conversion.direction {
        TemperatureDirection::FromReading => {
            dimensionless(expression, &[dimension])?;
            temperature
        }
        TemperatureDirection::ToReading => {
            same_dimension(expression, &[dimension, temperature])?;
            if let Some(difference) = written_difference(pool, expression) {
                return Err(QuantityError::DifferenceAsReading {
                    expression,
                    difference,
                });
            }
            Dimension::DIMENSIONLESS
        }
    };
    if is_below_absolute_zero(pool, conversion, argument) {
        return Err(QuantityError::BelowAbsoluteZero(expression));
    }
    Ok(Converted {
        expression: affine_map_expression(pool, conversion, argument)
            .map_err(QuantityError::Build)?,
        dimension: result_dimension,
    })
}

fn written_difference(pool: &ExprPool, expression: ExprId) -> Option<ExprId> {
    let Ok(NodeView::Apply {
        arguments: [argument],
        ..
    }) = pool.node(expression)
    else {
        return None;
    };
    let Ok(NodeView::Quantity { unit, .. }) = pool.node(*argument) else {
        return None;
    };
    let units = pool.units();
    let [factor] = units.factors(unit).ok()? else {
        return None;
    };
    let symbol = units.symbol(factor.named_unit()).ok()?;
    (factor.exponent() == 1 && TEMPERATURE_DIFFERENCE_SYMBOLS.contains(&symbol))
        .then_some(*argument)
}

fn is_below_absolute_zero(
    pool: &mut ExprPool,
    conversion: TemperatureConversion,
    argument: ExprId,
) -> bool {
    let Some(value) = evaluate_exact(pool, argument)
        .ok()
        .and_then(|evaluation| evaluation.rational_value().cloned())
    else {
        return false;
    };
    let kelvin = match conversion.direction {
        TemperatureDirection::ToReading => value,
        TemperatureDirection::FromReading => {
            let map = conversion.affine_map();
            let Ok(scaled) = map.factor.mul_exact(&value) else {
                return false;
            };
            let Ok(kelvin) = scaled.add_exact(&map.addend) else {
                return false;
            };
            kelvin
        }
    };
    number_is_negative(&kelvin)
}

fn number_is_negative(number: &Number) -> bool {
    match number {
        Number::Integer(integer) => integer.is_negative(),
        Number::Rational(rational) => rational.numerator().is_negative(),
        Number::F32(value) => *value < 0.0,
        Number::F64(value) => *value < 0.0,
    }
}

fn convert_node(
    pool: &mut ExprPool,
    expression: ExprId,
    converted: &HashMap<ExprId, Converted>,
) -> Checked<Converted> {
    let lookup = |child: &ExprId| {
        converted
            .get(child)
            .copied()
            .ok_or(QuantityError::Access(AccessError::UnknownExprId(*child)))
    };
    let view = pool.node(expression).map_err(QuantityError::Access)?;
    let unchanged = Converted {
        expression,
        dimension: Dimension::DIMENSIONLESS,
    };
    match view {
        NodeView::Number(_) | NodeView::Symbol(_) | NodeView::Bound(_) => Ok(unchanged),
        NodeView::Quantity { value, unit } => {
            let value = lookup(&value)?;
            let scaled = unit_value(pool, value.expression, unit)?;
            Ok(Converted {
                expression: scaled.expression,
                dimension: product(expression, value.dimension, scaled.dimension)?,
            })
        }
        NodeView::Apply { head, arguments } => {
            let arguments = arguments
                .iter()
                .map(lookup)
                .collect::<Checked<Vec<Converted>>>()?;
            match head {
                Head::Operator(operator) => {
                    convert_operator(pool, expression, operator, &arguments)
                }
                Head::Function(_) => {
                    let dimensions: Vec<Dimension> = arguments
                        .iter()
                        .map(|argument| argument.dimension)
                        .collect();
                    dimensionless(expression, &dimensions)?;
                    let expressions: Vec<ExprId> = arguments
                        .iter()
                        .map(|argument| argument.expression)
                        .collect();
                    Ok(Converted {
                        expression: pool
                            .apply(head, &expressions)
                            .map_err(QuantityError::Build)?,
                        dimension: Dimension::DIMENSIONLESS,
                    })
                }
            }
        }
        NodeView::Bind {
            binder,
            arguments,
            body,
        } => {
            let arguments = arguments
                .iter()
                .map(lookup)
                .collect::<Checked<Vec<Converted>>>()?;
            let body = lookup(&body)?;
            let argument_dimensions: Vec<Dimension> = arguments
                .iter()
                .map(|argument| argument.dimension)
                .collect();
            dimensionless(expression, &argument_dimensions)?;
            let dimension = match binder {
                BinderKind::Product(_) => dimensionless(expression, &[body.dimension])?,
                _ => body.dimension,
            };
            let expressions: Vec<ExprId> = arguments
                .iter()
                .map(|argument| argument.expression)
                .collect();
            let bound_name = pool.bound_name(expression).map(str::to_string);
            let rebuilt = pool
                .bind(binder, &expressions, body.expression)
                .map_err(QuantityError::Build)?;
            if let Some(name) = bound_name {
                pool.record_bound_name(rebuilt, &name)
                    .map_err(QuantityError::Access)?;
            }
            Ok(Converted {
                expression: rebuilt,
                dimension,
            })
        }
        NodeView::Array { shape, elements } => {
            let shape = shape.to_vec();
            let elements = elements
                .iter()
                .map(lookup)
                .collect::<Checked<Vec<Converted>>>()?;
            let dimensions: Vec<Dimension> =
                elements.iter().map(|element| element.dimension).collect();
            let dimension = same_dimension(expression, &dimensions)?;
            let expressions: Vec<ExprId> =
                elements.iter().map(|element| element.expression).collect();
            Ok(Converted {
                expression: pool
                    .array(&shape, &expressions)
                    .map_err(QuantityError::Build)?,
                dimension,
            })
        }
    }
}

fn unit_of_operator(
    pool: &mut ExprPool,
    operator: Operator,
    units: &[WrittenUnit],
    exponent: Option<i8>,
) -> Option<UnitId> {
    let scaled = |carried: &WrittenUnit, other: &WrittenUnit| -> Option<UnitId> {
        (!other.has_quantity).then_some(carried.unit).flatten()
    };
    match (operator, units) {
        (
            Operator::Add
            | Operator::Sub
            | Operator::Min
            | Operator::Max
            | Operator::Uncertain
            | Operator::CopySign,
            [left, right],
        ) => (left.unit == right.unit).then_some(left.unit).flatten(),
        (Operator::Mul, [left, right]) => match (left.unit, right.unit) {
            (Some(left_unit), Some(right_unit)) => {
                pool.units_mut().multiply(left_unit, right_unit).ok()
            }
            (Some(_), None) => scaled(left, right),
            (None, Some(_)) => scaled(right, left),
            (None, None) => None,
        },
        (Operator::Div, [left, right]) => match (left.unit, right.unit) {
            (Some(left_unit), Some(right_unit)) => {
                pool.units_mut().divide(left_unit, right_unit).ok()
            }
            (Some(_), None) => scaled(left, right),
            (None, Some(right_unit)) if !left.has_quantity => {
                let dimensionless = pool.units().dimensionless();
                pool.units_mut().divide(dimensionless, right_unit).ok()
            }
            _ => None,
        },
        (
            Operator::Neg | Operator::Abs | Operator::ToF32 | Operator::ToF64 | Operator::ToExact,
            [single],
        ) => single.unit,
        (Operator::EnclosureLower | Operator::EnclosureUpper, [value, _]) => value.unit,
        (Operator::Pow, [base, _]) => pool.units_mut().power(base.unit?, exponent?).ok(),
        _ => None,
    }
}

#[derive(Clone, Copy, Default)]
struct WrittenUnit {
    unit: Option<UnitId>,
    has_quantity: bool,
}

pub fn expression_unit(pool: &mut ExprPool, root: ExprId) -> Checked<Option<UnitId>> {
    let mut units: HashMap<ExprId, WrittenUnit> = HashMap::new();
    for expression in post_order(pool, root)? {
        let view = pool.node(expression).map_err(QuantityError::Access)?;
        let written = match view {
            NodeView::Quantity { unit, .. } => WrittenUnit {
                unit: Some(unit),
                has_quantity: true,
            },
            NodeView::Apply {
                head: Head::Operator(operator),
                arguments,
            } => {
                let arguments = arguments.to_vec();
                let found: Vec<WrittenUnit> = arguments
                    .iter()
                    .map(|argument| units.get(argument).copied().unwrap_or_default())
                    .collect();
                let exponent = arguments
                    .get(1)
                    .and_then(|argument| integer_exponent(pool, *argument));
                WrittenUnit {
                    unit: unit_of_operator(pool, operator, &found, exponent),
                    has_quantity: found.iter().any(|found| found.has_quantity),
                }
            }
            other => {
                let carried: Vec<WrittenUnit> = children(&other)
                    .iter()
                    .map(|child| units.get(child).copied().unwrap_or_default())
                    .collect();
                WrittenUnit {
                    unit: None,
                    has_quantity: carried.iter().any(|child| child.has_quantity),
                }
            }
        };
        units.insert(expression, written);
    }
    Ok(units.get(&root).copied().unwrap_or_default().unit)
}

fn integer_exponent(pool: &ExprPool, expression: ExprId) -> Option<i8> {
    match pool.node(expression).ok()? {
        NodeView::Number(number) => {
            let value = pool.number_value(number).ok()?;
            match value {
                calc_numbers::Number::Integer(integer) => {
                    integer.to_i64().and_then(|value| i8::try_from(value).ok())
                }
                _ => None,
            }
        }
        _ => None,
    }
}

pub fn to_coherent_units(
    pool: &mut ExprPool,
    root: ExprId,
) -> Result<CoherentExpression, QuantityError> {
    check_kinds(pool, root)?;
    let mut converted: HashMap<ExprId, Converted> = HashMap::new();
    for expression in post_order(pool, root)? {
        let result = convert_node(pool, expression, &converted)?;
        converted.insert(expression, result);
    }
    let result = converted
        .get(&root)
        .copied()
        .ok_or(QuantityError::Access(AccessError::UnknownExprId(root)))?;
    let conversion_target = match pool.node(root).map_err(QuantityError::Access)? {
        NodeView::Apply {
            head: Head::Operator(Operator::ConvertUnit),
            arguments: [_, target],
        } => target_unit(pool, *target)?,
        _ => None,
    };
    let unit = match conversion_target {
        Some(unit) => {
            let scale = pool
                .units()
                .scale_factor(unit)
                .map_err(QuantityError::UnitAccess)?
                .clone();
            let expression = match scale_expression(pool, &scale)? {
                Some(factor) => build(pool, Operator::Div, &[result.expression, factor])?,
                None => result.expression,
            };
            return Ok(CoherentExpression {
                expression,
                dimension: result.dimension,
                unit: Some(unit),
            });
        }
        None if result.dimension.is_dimensionless() => None,
        None => {
            let ambiguous =
                !calc_units::UnitTable::named_coherent_unit_names_one_quantity(&result.dimension);
            let written = expression_unit(pool, root)?.filter(|unit| {
                ambiguous
                    && pool
                        .units()
                        .scale_factor(*unit)
                        .is_ok_and(ScaleFactor::is_one)
            });
            match written {
                Some(unit) => Some(unit),
                None => Some(
                    pool.units_mut()
                        .named_coherent_unit(&result.dimension)
                        .map_err(QuantityError::UnitProduct)?,
                ),
            }
        }
    };
    Ok(CoherentExpression {
        expression: result.expression,
        dimension: result.dimension,
        unit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_expr::SymbolKind;

    fn integer(pool: &mut ExprPool, value: i64) -> ExprId {
        pool.number(Number::from(value)).unwrap()
    }

    fn fraction(pool: &mut ExprPool, numerator: i64, denominator: i64) -> ExprId {
        let value =
            Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap();
        pool.number(value).unwrap()
    }

    fn quantity(pool: &mut ExprPool, value: i64, unit: &str) -> ExprId {
        let value = integer(pool, value);
        let unit = pool.units_mut().lookup(unit).unwrap();
        pool.quantity(value, unit).unwrap()
    }

    fn apply(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> ExprId {
        pool.apply(Head::Operator(operator), arguments).unwrap()
    }

    fn named_unit(pool: &mut ExprPool, name: &str) -> UnitId {
        pool.units_mut().lookup(name).unwrap()
    }

    fn exponents(pool: &ExprPool, unit: UnitId) -> [i8; 8] {
        pool.units().dimension(unit).unwrap().exponents()
    }

    fn mismatch(result: Checked<CoherentExpression>) -> bool {
        matches!(result, Err(QuantityError::DimensionMismatch { .. }))
    }

    #[test]
    fn expression_without_quantities_is_unchanged_and_dimensionless() {
        let mut pool = ExprPool::new();
        let one = integer(&mut pool, 1);
        let two = integer(&mut pool, 2);
        let sum = apply(&mut pool, Operator::Add, &[one, two]);

        let coherent = to_coherent_units(&mut pool, sum).unwrap();

        assert_eq!(
            coherent,
            CoherentExpression {
                expression: sum,
                dimension: Dimension::DIMENSIONLESS,
                unit: None
            }
        );
    }

    #[test]
    fn coherent_unit_quantity_becomes_its_value() {
        let mut pool = ExprPool::new();
        let mass = quantity(&mut pool, 3, "kg");
        let three = integer(&mut pool, 3);

        let coherent = to_coherent_units(&mut pool, mass).unwrap();

        assert_eq!(coherent.expression, three);
    }

    #[test]
    fn prefixed_unit_multiplies_by_its_exact_scale_factor() {
        let mut pool = ExprPool::new();
        let length = quantity(&mut pool, 3, "km");
        let three = integer(&mut pool, 3);
        let thousand = integer(&mut pool, 1000);
        let expected = apply(&mut pool, Operator::Mul, &[three, thousand]);

        let coherent = to_coherent_units(&mut pool, length).unwrap();

        assert_eq!(coherent.expression, expected);
    }

    #[test]
    fn gram_scale_factor_is_an_exact_fraction() {
        let mut pool = ExprPool::new();
        let mass = quantity(&mut pool, 5, "g");
        let five = integer(&mut pool, 5);
        let thousandth = fraction(&mut pool, 1, 1000);
        let expected = apply(&mut pool, Operator::Mul, &[five, thousandth]);

        let coherent = to_coherent_units(&mut pool, mass).unwrap();

        assert_eq!(coherent.expression, expected);
    }

    #[test]
    fn product_of_quantities_multiplies_dimensions_and_names_the_newton() {
        let mut pool = ExprPool::new();
        let mass = quantity(&mut pool, 2, "kg");
        let acceleration_value = integer(&mut pool, 9);
        let metre = named_unit(&mut pool, "m");
        let second = named_unit(&mut pool, "s");
        let squared = pool.units_mut().power(second, 2).unwrap();
        let per_second_squared = pool.units_mut().divide(metre, squared).unwrap();
        let acceleration = pool
            .quantity(acceleration_value, per_second_squared)
            .unwrap();
        let force = apply(&mut pool, Operator::Mul, &[mass, acceleration]);
        let newton = named_unit(&mut pool, "N");

        let coherent = to_coherent_units(&mut pool, force).unwrap();

        assert_eq!(coherent.unit, Some(newton));
    }

    #[test]
    fn quotient_of_quantities_subtracts_dimensions() {
        let mut pool = ExprPool::new();
        let length = quantity(&mut pool, 100, "m");
        let time = quantity(&mut pool, 9, "s");
        let speed = apply(&mut pool, Operator::Div, &[length, time]);

        let coherent = to_coherent_units(&mut pool, speed).unwrap();

        assert_eq!(coherent.dimension.exponents(), [1, 0, -1, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn unnamed_dimension_gets_the_base_unit_product() {
        let mut pool = ExprPool::new();
        let length = quantity(&mut pool, 100, "m");
        let time = quantity(&mut pool, 9, "s");
        let speed = apply(&mut pool, Operator::Div, &[length, time]);

        let unit = to_coherent_units(&mut pool, speed).unwrap().unit.unwrap();

        assert_eq!(
            (
                exponents(&pool, unit),
                pool.units().factors(unit).unwrap().len()
            ),
            ([1, 0, -1, 0, 0, 0, 0, 0], 2)
        );
    }

    #[test]
    fn sum_of_equal_dimensions_is_allowed_across_prefixes() {
        let mut pool = ExprPool::new();
        let kilometres = quantity(&mut pool, 3, "km");
        let metres = quantity(&mut pool, 200, "m");
        let sum = apply(&mut pool, Operator::Add, &[kilometres, metres]);
        let metre = named_unit(&mut pool, "m");

        let coherent = to_coherent_units(&mut pool, sum).unwrap();

        assert_eq!(coherent.unit, Some(metre));
    }

    #[test]
    fn maximum_of_equal_dimensions_keeps_the_dimension() {
        let mut pool = ExprPool::new();
        let short = quantity(&mut pool, 2, "m");
        let long = quantity(&mut pool, 3, "km");
        let maximum = apply(&mut pool, Operator::Max, &[short, long]);
        let metre = named_unit(&mut pool, "m");

        let coherent = to_coherent_units(&mut pool, maximum).unwrap();

        assert_eq!(coherent.unit, Some(metre));
    }

    #[test]
    fn unit_applied_to_a_quantity_multiplies_the_units() {
        let mut pool = ExprPool::new();
        let length = quantity(&mut pool, 2, "m");
        let second = named_unit(&mut pool, "s");
        let product = pool.quantity(length, second).unwrap();

        let coherent = to_coherent_units(&mut pool, product).unwrap();

        assert_eq!(coherent.dimension.exponents(), [1, 0, 1, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn sum_of_different_dimensions_is_inhomogeneous() {
        let mut pool = ExprPool::new();
        let length = quantity(&mut pool, 3, "m");
        let time = quantity(&mut pool, 2, "s");
        let sum = apply(&mut pool, Operator::Add, &[length, time]);

        assert!(mismatch(to_coherent_units(&mut pool, sum)));
    }

    #[test]
    fn sum_of_quantity_and_pure_number_is_inhomogeneous() {
        let mut pool = ExprPool::new();
        let length = quantity(&mut pool, 3, "m");
        let one = integer(&mut pool, 1);
        let sum = apply(&mut pool, Operator::Add, &[length, one]);

        assert!(mismatch(to_coherent_units(&mut pool, sum)));
    }

    #[test]
    fn transcendental_function_of_a_quantity_is_rejected() {
        let mut pool = ExprPool::new();
        let length = quantity(&mut pool, 2, "m");
        let sine = apply(&mut pool, Operator::Sin, &[length]);

        assert!(matches!(
            to_coherent_units(&mut pool, sine),
            Err(QuantityError::DimensionedArgument { expression, .. }) if expression == sine
        ));
    }

    #[test]
    fn ratio_of_equal_dimensions_may_enter_a_function() {
        let mut pool = ExprPool::new();
        let metres = quantity(&mut pool, 2, "m");
        let kilometres = quantity(&mut pool, 1, "km");
        let ratio = apply(&mut pool, Operator::Div, &[metres, kilometres]);
        let logarithm = apply(&mut pool, Operator::Ln, &[ratio]);

        let coherent = to_coherent_units(&mut pool, logarithm).unwrap();

        assert_eq!(coherent.unit, None);
    }

    #[test]
    fn integer_power_multiplies_the_dimension() {
        let mut pool = ExprPool::new();
        let length = quantity(&mut pool, 2, "m");
        let three = integer(&mut pool, 3);
        let volume = apply(&mut pool, Operator::Pow, &[length, three]);

        let coherent = to_coherent_units(&mut pool, volume).unwrap();

        assert_eq!(coherent.dimension.exponents(), [3, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn square_root_halves_even_exponents() {
        let mut pool = ExprPool::new();
        let area = quantity(&mut pool, 4, "m");
        let two = integer(&mut pool, 2);
        let squared = apply(&mut pool, Operator::Pow, &[area, two]);
        let root = apply(&mut pool, Operator::Sqrt, &[squared]);
        let metre = named_unit(&mut pool, "m");

        let coherent = to_coherent_units(&mut pool, root).unwrap();

        assert_eq!(coherent.unit, Some(metre));
    }

    #[test]
    fn square_root_of_odd_exponent_is_a_fractional_dimension() {
        let mut pool = ExprPool::new();
        let length = quantity(&mut pool, 4, "m");
        let root = apply(&mut pool, Operator::Sqrt, &[length]);

        assert_eq!(
            to_coherent_units(&mut pool, root),
            Err(QuantityError::FractionalDimension(root))
        );
    }

    #[test]
    fn rational_power_divides_exponents() {
        let mut pool = ExprPool::new();
        let length = quantity(&mut pool, 8, "m");
        let three = integer(&mut pool, 3);
        let volume = apply(&mut pool, Operator::Pow, &[length, three]);
        let two_thirds = fraction(&mut pool, 2, 3);
        let power = apply(&mut pool, Operator::Pow, &[volume, two_thirds]);

        let coherent = to_coherent_units(&mut pool, power).unwrap();

        assert_eq!(coherent.dimension.exponents(), [2, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn power_of_a_quantity_with_a_variable_exponent_is_rejected() {
        let mut pool = ExprPool::new();
        let length = quantity(&mut pool, 2, "m");
        let symbol = pool.intern_symbol("n", SymbolKind::Variable).unwrap();
        let exponent = pool.symbol(symbol).unwrap();
        let power = apply(&mut pool, Operator::Pow, &[length, exponent]);

        assert_eq!(
            to_coherent_units(&mut pool, power),
            Err(QuantityError::ExponentNotConstant(power))
        );
    }

    #[test]
    fn dimensioned_exponent_is_rejected() {
        let mut pool = ExprPool::new();
        let two = integer(&mut pool, 2);
        let time = quantity(&mut pool, 3, "s");
        let power = apply(&mut pool, Operator::Pow, &[two, time]);

        assert!(matches!(
            to_coherent_units(&mut pool, power),
            Err(QuantityError::DimensionedArgument { .. })
        ));
    }

    #[test]
    fn conversion_at_the_root_divides_by_the_target_scale_and_keeps_its_unit() {
        let mut pool = ExprPool::new();
        let metres = quantity(&mut pool, 3200, "m");
        let one_kilometre = quantity(&mut pool, 1, "km");
        let conversion = apply(&mut pool, Operator::ConvertUnit, &[metres, one_kilometre]);
        let value = integer(&mut pool, 3200);
        let thousand = integer(&mut pool, 1000);
        let expected = apply(&mut pool, Operator::Div, &[value, thousand]);
        let kilometre = named_unit(&mut pool, "km");

        let coherent = to_coherent_units(&mut pool, conversion).unwrap();

        assert_eq!(
            (coherent.expression, coherent.unit),
            (expected, Some(kilometre))
        );
    }

    #[test]
    fn conversion_to_a_different_dimension_is_rejected() {
        let mut pool = ExprPool::new();
        let metres = quantity(&mut pool, 3, "m");
        let one_second = quantity(&mut pool, 1, "s");
        let conversion = apply(&mut pool, Operator::ConvertUnit, &[metres, one_second]);

        assert!(mismatch(to_coherent_units(&mut pool, conversion)));
    }

    #[test]
    fn nested_conversion_keeps_the_coherent_value() {
        let mut pool = ExprPool::new();
        let metres = quantity(&mut pool, 3200, "m");
        let one_kilometre = quantity(&mut pool, 1, "km");
        let conversion = apply(&mut pool, Operator::ConvertUnit, &[metres, one_kilometre]);
        let two = integer(&mut pool, 2);
        let doubled = apply(&mut pool, Operator::Mul, &[conversion, two]);
        let value = integer(&mut pool, 3200);
        let expected = apply(&mut pool, Operator::Mul, &[value, two]);

        let coherent = to_coherent_units(&mut pool, doubled).unwrap();

        assert_eq!(coherent.expression, expected);
    }

    #[test]
    fn array_elements_must_share_one_dimension() {
        let mut pool = ExprPool::new();
        let length = quantity(&mut pool, 1, "m");
        let time = quantity(&mut pool, 1, "s");
        let array = pool.array(&[2], &[length, time]).unwrap();

        assert!(mismatch(to_coherent_units(&mut pool, array)));
    }

    #[test]
    fn sum_binder_keeps_the_dimension_of_its_body() {
        let mut pool = ExprPool::new();
        let bound = pool.bound(0).unwrap();
        let unit = named_unit(&mut pool, "m");
        let body = pool.quantity(bound, unit).unwrap();
        let one = integer(&mut pool, 1);
        let three = integer(&mut pool, 3);
        let sum = pool
            .bind(
                BinderKind::Sum(calc_expr::ReductionShape::LeftFold),
                &[one, three],
                body,
            )
            .unwrap();

        let coherent = to_coherent_units(&mut pool, sum).unwrap();

        assert_eq!(coherent.unit, Some(unit));
    }

    #[test]
    fn product_binder_over_a_quantity_is_rejected() {
        let mut pool = ExprPool::new();
        let bound = pool.bound(0).unwrap();
        let unit = named_unit(&mut pool, "m");
        let body = pool.quantity(bound, unit).unwrap();
        let one = integer(&mut pool, 1);
        let three = integer(&mut pool, 3);
        let product = pool
            .bind(
                BinderKind::Product(calc_expr::ReductionShape::LeftFold),
                &[one, three],
                body,
            )
            .unwrap();

        assert!(matches!(
            to_coherent_units(&mut pool, product),
            Err(QuantityError::DimensionedArgument { .. })
        ));
    }

    #[test]
    fn dimension_exponent_beyond_i8_is_out_of_range() {
        let mut pool = ExprPool::new();
        let length = quantity(&mut pool, 2, "m");
        let large = integer(&mut pool, 1000);
        let power = apply(&mut pool, Operator::Pow, &[length, large]);

        assert_eq!(
            to_coherent_units(&mut pool, power),
            Err(QuantityError::DimensionOutOfRange(power))
        );
    }

    #[test]
    fn unknown_root_is_an_access_error() {
        let mut other = ExprPool::new();
        let mut last = integer(&mut other, 0);
        for value in 1..5 {
            last = integer(&mut other, value);
        }
        let mut pool = ExprPool::new();

        assert_eq!(
            to_coherent_units(&mut pool, last),
            Err(QuantityError::Access(AccessError::UnknownExprId(last)))
        );
    }
}
