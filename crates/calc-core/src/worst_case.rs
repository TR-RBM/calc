use std::cmp::Ordering;
use std::collections::BTreeMap;

use calc_expr::{BuiltinConstant, ExprId, ExprPool, Head, NodeView, Operator, SymbolKind};
use calc_numbers::{Integer, Number};
use calc_units::Dimension;

use crate::derivative::derivative;
use crate::exact_evaluation::evaluate_exact;
use crate::exact_rational::ExactRational;
use crate::polynomial::{AtomTable, rational_of};
use crate::quantities::to_coherent_units;
use crate::real_roots::{sign_on_interval, univariate_coefficients, vanishes_on_closed_interval};

pub const TOLERANCE_LIMIT: usize = 8;
const PI_LOW: (i64, i64) = (314_159_265_358_979, 100_000_000_000_000);
const PI_HIGH: (i64, i64) = (314_159_265_358_980, 100_000_000_000_000);
const E_LOW: (i64, i64) = (271_828_182_845_904, 100_000_000_000_000);
const E_HIGH: (i64, i64) = (271_828_182_845_905, 100_000_000_000_000);
const TOLERANCE_SYMBOL_PREFIX: &str = "$tolerance";
const NAMED_RANGE_PREFIX: &str = "$range";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorstCaseRefusal {
    TooMany,
    EndpointNotExact(ExprId),
    EndpointNotEnclosed(ExprId),
    EndsOfTwoDimensions(ExprId),
    EndpointsReversed(ExprId),
    NegativeTolerance(ExprId),
    NotMonotone(ExprId),
    DivisorReachesZero(ExprId),
    NotASubstance(ExprId),
    MolarMass(ExprId, crate::chemistry::MolarMassRefusal),
    QValue(ExprId, crate::nuclear::QValueRefusal),
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorstCase {
    pub low: ExprId,
    pub high: ExprId,
    pub low_corner: Vec<CornerEnd>,
    pub high_corner: Vec<CornerEnd>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CornerEnd {
    pub range: ExprId,
    pub name: Option<String>,
    pub end: ExprId,
}

#[derive(Clone, Debug)]
struct Range {
    low: ExactRational,
    high: ExactRational,
}

impl Range {
    fn point(value: ExactRational) -> Self {
        Self {
            low: value.clone(),
            high: value,
        }
    }

    fn from_pair(low: (i64, i64), high: (i64, i64)) -> Option<Self> {
        Some(Self {
            low: ExactRational::fraction(&Integer::from(low.0), &Integer::from(low.1))?,
            high: ExactRational::fraction(&Integer::from(high.0), &Integer::from(high.1))?,
        })
    }

    fn plus(&self, other: &Self) -> Self {
        Self {
            low: self.low.plus(&other.low),
            high: self.high.plus(&other.high),
        }
    }

    fn negated(&self) -> Self {
        Self {
            low: self.high.negated(),
            high: self.low.negated(),
        }
    }

    fn times(&self, other: &Self) -> Self {
        let products = [
            self.low.multiply(&other.low),
            self.low.multiply(&other.high),
            self.high.multiply(&other.low),
            self.high.multiply(&other.high),
        ];
        let mut low = products[0].clone();
        let mut high = products[0].clone();
        for product in &products[1..] {
            if product.compare(&low) == Ordering::Less {
                low = product.clone();
            }
            if product.compare(&high) == Ordering::Greater {
                high = product.clone();
            }
        }
        Self { low, high }
    }

    fn reciprocal(&self) -> Option<Self> {
        let positive = self.low.sign() == Ordering::Greater;
        let negative = self.high.sign() == Ordering::Less;
        if !positive && !negative {
            return None;
        }
        Some(Self {
            low: self.high.reciprocal()?,
            high: self.low.reciprocal()?,
        })
    }

    fn power(&self, exponent: i64) -> Option<Self> {
        if exponent < 0 {
            return self.power(-exponent)?.reciprocal();
        }
        let mut result = Self::point(ExactRational::one());
        for _ in 0..exponent {
            result = result.times(self);
        }
        if exponent % 2 == 0
            && self.low.sign() == Ordering::Less
            && self.high.sign() == Ordering::Greater
        {
            result.low = ExactRational::zero();
        }
        Some(result)
    }

    fn sign(&self) -> Option<Ordering> {
        if self.low.sign() == Ordering::Greater {
            Some(Ordering::Greater)
        } else if self.high.sign() == Ordering::Less {
            Some(Ordering::Less)
        } else if self.low.is_zero() && self.high.is_zero() {
            Some(Ordering::Equal)
        } else {
            None
        }
    }
}

fn apply(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> Option<ExprId> {
    pool.apply(Head::Operator(operator), arguments).ok()
}

fn exact_value(pool: &mut ExprPool, expression: ExprId) -> Option<ExactRational> {
    let coherent = to_coherent_units(pool, expression).ok()?;
    let evaluation = evaluate_exact(pool, coherent.expression).ok()?;
    ExactRational::from_number(evaluation.rational_value()?)
}

fn tolerances(pool: &ExprPool, root: ExprId) -> Vec<ExprId> {
    let mut found = Vec::new();
    let mut pending = vec![root];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(expression) = pending.pop() {
        if !seen.insert(expression) {
            continue;
        }
        match pool.node(expression) {
            Ok(NodeView::Apply {
                head:
                    Head::Operator(
                        Operator::Between
                        | Operator::Tolerance
                        | Operator::MolarMass
                        | Operator::QValue,
                    ),
                ..
            }) => found.push(expression),
            Ok(NodeView::Apply { arguments, .. }) => pending.extend(arguments.iter().copied()),
            Ok(NodeView::Quantity { value, .. }) => pending.push(value),
            Ok(NodeView::Bind {
                arguments, body, ..
            }) => {
                pending.extend(arguments.iter().copied());
                pending.push(body);
            }
            Ok(NodeView::Array { elements, .. }) => pending.extend(elements.iter().copied()),
            _ => {}
        }
    }
    found
}

pub fn has_tolerance(pool: &ExprPool, expression: ExprId) -> bool {
    !tolerances(pool, expression).is_empty()
}

struct Enclosed {
    range: Range,
    dimension: Dimension,
}

fn enclosed(pool: &mut ExprPool, endpoint: ExprId) -> Option<Enclosed> {
    let coherent = to_coherent_units(pool, endpoint).ok()?;
    let range = match exact_value(pool, endpoint) {
        Some(value) => Range::point(value),
        None => interval(
            pool,
            coherent.expression,
            &Range::point(ExactRational::zero()),
            &BTreeMap::new(),
        )?,
    };
    Some(Enclosed {
        range,
        dimension: coherent.dimension,
    })
}

struct Ends {
    low: ExprId,
    high: ExprId,
    range: Range,
}

fn molar_mass_endpoints(
    pool: &mut ExprPool,
    node: ExprId,
    substance: ExprId,
) -> Result<(ExprId, ExprId), WorstCaseRefusal> {
    let composition = crate::chemistry::composition_of(pool, substance)
        .ok_or(WorstCaseRefusal::NotASubstance(node))?;
    let range = crate::chemistry::molar_mass(&composition)
        .map_err(|refusal| WorstCaseRefusal::MolarMass(node, refusal))?;
    let unsupported = || WorstCaseRefusal::Unsupported;
    let unit = crate::chemistry::molar_mass_unit(pool).ok_or_else(unsupported)?;
    let one = pool
        .number(Number::from(1_i64))
        .map_err(|_| unsupported())?;
    let target = pool.quantity(one, unit).map_err(|_| unsupported())?;
    let mut end = |value: Number| -> Result<ExprId, WorstCaseRefusal> {
        let value = pool.number(value).map_err(|_| unsupported())?;
        let value = pool.quantity(value, unit).map_err(|_| unsupported())?;
        apply(pool, Operator::ConvertUnit, &[value, target]).ok_or_else(unsupported)
    };
    Ok((end(range.low)?, end(range.high)?))
}

fn q_value_endpoints(
    pool: &mut ExprPool,
    node: ExprId,
    reaction: ExprId,
) -> Result<(ExprId, ExprId), WorstCaseRefusal> {
    let refused = |refusal| WorstCaseRefusal::QValue(node, refusal);
    let species = crate::nuclear::nuclear_reaction_of(pool, reaction)
        .ok_or_else(|| refused(crate::nuclear::QValueRefusal::NotANuclearReaction))?;
    let coefficients = match crate::nuclear::balance_nuclear(&species) {
        Ok(crate::nuclear::NuclearBalancing::Checked { coefficients, .. }) => coefficients,
        _ => return Err(refused(crate::nuclear::QValueRefusal::DoesNotBalance)),
    };
    let range = crate::nuclear::q_value(&species, &coefficients).map_err(refused)?;
    let unsupported = || WorstCaseRefusal::Unsupported;
    let unit = crate::nuclear::q_value_unit(pool).ok_or_else(unsupported)?;
    let one = pool
        .number(Number::from(1_i64))
        .map_err(|_| unsupported())?;
    let target = pool.quantity(one, unit).map_err(|_| unsupported())?;
    let mut end = |value: Number| -> Result<ExprId, WorstCaseRefusal> {
        let value = pool.number(value).map_err(|_| unsupported())?;
        let value = pool.quantity(value, unit).map_err(|_| unsupported())?;
        apply(pool, Operator::ConvertUnit, &[value, target]).ok_or_else(unsupported)
    };
    Ok((end(range.low)?, end(range.high)?))
}

fn endpoints(pool: &mut ExprPool, node: ExprId) -> Result<Ends, WorstCaseRefusal> {
    let (low, high) = match pool.node(node) {
        Ok(NodeView::Apply {
            head: Head::Operator(Operator::MolarMass),
            arguments: [substance],
        }) => {
            let substance = *substance;
            molar_mass_endpoints(pool, node, substance)?
        }
        Ok(NodeView::Apply {
            head: Head::Operator(Operator::QValue),
            arguments: [reaction],
        }) => {
            let reaction = *reaction;
            q_value_endpoints(pool, node, reaction)?
        }
        _ => two_argument_endpoints(pool, node)?,
    };
    let low_end = enclosed(pool, low).ok_or(WorstCaseRefusal::EndpointNotEnclosed(node))?;
    let high_end = enclosed(pool, high).ok_or(WorstCaseRefusal::EndpointNotEnclosed(node))?;
    if low_end.dimension != high_end.dimension {
        return Err(WorstCaseRefusal::EndsOfTwoDimensions(node));
    }
    if low_end.range.low.compare(&high_end.range.high) == Ordering::Greater {
        return Err(WorstCaseRefusal::EndpointsReversed(node));
    }
    if low_end.range.high.compare(&high_end.range.low) == Ordering::Greater && low != high {
        return Err(WorstCaseRefusal::EndpointNotEnclosed(node));
    }
    Ok(Ends {
        low,
        high,
        range: Range {
            low: low_end.range.low,
            high: high_end.range.high,
        },
    })
}

fn two_argument_endpoints(
    pool: &mut ExprPool,
    node: ExprId,
) -> Result<(ExprId, ExprId), WorstCaseRefusal> {
    let (operator, arguments) = match pool.node(node) {
        Ok(NodeView::Apply {
            head: Head::Operator(operator),
            arguments: [left, right],
        }) => (operator, [*left, *right]),
        _ => return Err(WorstCaseRefusal::Unsupported),
    };
    let unsupported = || WorstCaseRefusal::Unsupported;
    Ok(match operator {
        Operator::Between => (arguments[0], arguments[1]),
        _ => {
            let [value, share] = arguments;
            let share_value =
                exact_value(pool, share).ok_or(WorstCaseRefusal::EndpointNotExact(node))?;
            if share_value.sign() == Ordering::Less {
                return Err(WorstCaseRefusal::NegativeTolerance(node));
            }
            let value_sign = enclosed(pool, value)
                .ok_or(WorstCaseRefusal::EndpointNotEnclosed(node))?
                .range
                .sign();
            let one = pool
                .number(Number::from(1_i64))
                .map_err(|_| unsupported())?;
            let below = apply(pool, Operator::Sub, &[one, share]).ok_or_else(unsupported)?;
            let above = apply(pool, Operator::Add, &[one, share]).ok_or_else(unsupported)?;
            let low = apply(pool, Operator::Mul, &[value, below]).ok_or_else(unsupported)?;
            let high = apply(pool, Operator::Mul, &[value, above]).ok_or_else(unsupported)?;
            if value_sign == Some(Ordering::Less) {
                (high, low)
            } else {
                (low, high)
            }
        }
    })
}

fn replaced(
    pool: &mut ExprPool,
    expression: ExprId,
    map: &BTreeMap<ExprId, ExprId>,
) -> Option<ExprId> {
    if let Some(replacement) = map.get(&expression) {
        return Some(*replacement);
    }
    match pool.node(expression).ok()? {
        NodeView::Apply { head, arguments } => {
            let arguments = arguments.to_vec();
            let mut changed = Vec::with_capacity(arguments.len());
            for argument in arguments {
                changed.push(replaced(pool, argument, map)?);
            }
            pool.apply(head, &changed).ok()
        }
        NodeView::Quantity { value, unit } => {
            let value = replaced(pool, value, map)?;
            pool.quantity(value, unit).ok()
        }
        _ => Some(expression),
    }
}

fn unit_quantity(pool: &mut ExprPool, endpoint: ExprId) -> Option<ExprId> {
    let coherent = to_coherent_units(pool, endpoint).ok()?;
    let one = pool.number(Number::from(1_i64)).ok()?;
    if coherent.dimension.is_dimensionless() {
        return Some(one);
    }
    let unit = pool.units_mut().coherent_unit(&coherent.dimension).ok()?;
    pool.quantity(one, unit).ok()
}

fn interval(
    pool: &ExprPool,
    expression: ExprId,
    bound: &Range,
    symbols: &BTreeMap<String, Range>,
) -> Option<Range> {
    match pool.node(expression).ok()? {
        NodeView::Number(number) => Some(Range::point(ExactRational::from_number(
            pool.number_value(number).ok()?,
        )?)),
        NodeView::Bound(0) => Some(bound.clone()),
        NodeView::Symbol(symbol) => {
            if symbol == BuiltinConstant::Pi.symbol() {
                return Range::from_pair(PI_LOW, PI_HIGH);
            }
            if symbol == BuiltinConstant::E.symbol() {
                return Range::from_pair(E_LOW, E_HIGH);
            }
            symbols.get(pool.symbol_name(symbol).ok()?).cloned()
        }
        NodeView::Quantity { value, unit } => {
            let value = interval(pool, value, bound, symbols)?;
            let scale = pool.units().scale_factor(unit).ok()?;
            let rational = ExactRational::from_number(&scale.rational_part())?;
            let mut factor = Range::point(rational);
            let pi = Range::from_pair(PI_LOW, PI_HIGH)?;
            let pi_exponent = scale.pi_exponent();
            factor = factor.times(&pi.power(i64::from(pi_exponent))?);
            Some(value.times(&factor))
        }
        NodeView::Apply {
            head: Head::Operator(operator),
            arguments,
        } => {
            let values: Vec<Range> = arguments
                .iter()
                .map(|argument| interval(pool, *argument, bound, symbols))
                .collect::<Option<_>>()?;
            match (operator, values.as_slice()) {
                (Operator::Add, [left, right]) => Some(left.plus(right)),
                (Operator::Sub, [left, right]) => Some(left.plus(&right.negated())),
                (Operator::Mul, [left, right]) => Some(left.times(right)),
                (Operator::Div, [left, right]) => Some(left.times(&right.reciprocal()?)),
                (Operator::Neg, [single]) => Some(single.negated()),
                (Operator::Percent, [share]) => Some(share.times(&Range::point(
                    ExactRational::fraction(&Integer::from(1_i64), &Integer::from(100_i64))?,
                ))),
                (Operator::Pow, [base, exponent])
                    if exponent.low.compare(&exponent.high) == Ordering::Equal
                        && exponent.low.is_integer() =>
                {
                    base.power(exponent.low.numerator().to_i64()?)
                }
                (Operator::ConvertUnit, [value, _]) => Some(value.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

const BISECTION_DEPTH: u32 = 10;

fn constant_sign(
    pool: &ExprPool,
    expression: ExprId,
    bound: &Range,
    symbols: &BTreeMap<String, Range>,
    depth: u32,
) -> Option<Ordering> {
    if let Some(sign) = interval(pool, expression, bound, symbols).and_then(|range| range.sign()) {
        return Some(sign);
    }
    if depth == 0 {
        return None;
    }
    let middle = bound
        .low
        .plus(&bound.high)
        .multiply(&ExactRational::one_half());
    let left = Range {
        low: bound.low.clone(),
        high: middle.clone(),
    };
    let right = Range {
        low: middle,
        high: bound.high.clone(),
    };
    let left_sign = constant_sign(pool, expression, &left, symbols, depth - 1)?;
    let right_sign = constant_sign(pool, expression, &right, symbols, depth - 1)?;
    match (left_sign, right_sign) {
        (left, right) if left == right => Some(left),
        (Ordering::Equal, other) | (other, Ordering::Equal) => Some(other),
        _ => None,
    }
}

fn product_sign(left: Ordering, right: Ordering) -> Ordering {
    match (left, right) {
        (Ordering::Equal, _) | (_, Ordering::Equal) => Ordering::Equal,
        (left, right) if left == right => Ordering::Greater,
        _ => Ordering::Less,
    }
}

fn exact_sign(pool: &mut ExprPool, expression: ExprId, bound: &Range) -> Option<Ordering> {
    let variable = pool.bound(0).ok()?;
    let mut atoms = AtomTable::default();
    let function = rational_of(pool, expression, &mut atoms)?;
    if atoms.expressions().iter().any(|atom| *atom != variable) {
        return None;
    }
    let numerator = univariate_coefficients(function.numerator())?;
    let denominator = univariate_coefficients(function.denominator())?;
    if vanishes_on_closed_interval(&denominator, &bound.low, &bound.high)? {
        return None;
    }
    Some(product_sign(
        sign_on_interval(&numerator, &bound.low, &bound.high)?,
        sign_on_interval(&denominator, &bound.low, &bound.high)?,
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedRange {
    pub key: ExprId,
    pub range: ExprId,
    pub name: String,
}

pub fn named_ranges(
    pool: &mut ExprPool,
    expression: ExprId,
    name: &str,
) -> Option<(ExprId, Vec<NamedRange>)> {
    let nodes = tolerances(pool, expression);
    let mut map = BTreeMap::new();
    let mut named = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        let label = if nodes.len() == 1 {
            name.to_owned()
        } else {
            format!("{name}#{}", index + 1)
        };
        let symbol = pool
            .intern_symbol(
                &format!("{NAMED_RANGE_PREFIX}{label}"),
                SymbolKind::Variable,
            )
            .ok()?;
        let key = pool.symbol(symbol).ok()?;
        map.insert(*node, key);
        named.push(NamedRange {
            key,
            range: *node,
            name: label,
        });
    }
    Some((replaced(pool, expression, &map)?, named))
}

pub fn worst_case(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Option<Result<WorstCase, WorstCaseRefusal>> {
    worst_case_with(pool, expression, &[])
}

struct Component {
    key: ExprId,
    range: ExprId,
    name: Option<String>,
}

pub fn worst_case_with(
    pool: &mut ExprPool,
    expression: ExprId,
    named: &[NamedRange],
) -> Option<Result<WorstCase, WorstCaseRefusal>> {
    let present = symbols_in(pool, expression);
    let mut components: Vec<Component> = named
        .iter()
        .filter(|named| present.contains(&named.key))
        .map(|named| Component {
            key: named.key,
            range: named.range,
            name: Some(named.name.clone()),
        })
        .collect();
    components.extend(
        tolerances(pool, expression)
            .into_iter()
            .map(|node| Component {
                key: node,
                range: node,
                name: None,
            }),
    );
    if components.is_empty() {
        return None;
    }
    Some(solve(pool, expression, &components))
}

fn symbols_in(pool: &ExprPool, root: ExprId) -> std::collections::BTreeSet<ExprId> {
    let mut found = std::collections::BTreeSet::new();
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if !found.insert(expression) {
            continue;
        }
        match pool.node(expression) {
            Ok(NodeView::Apply { arguments, .. }) => pending.extend(arguments.iter().copied()),
            Ok(NodeView::Quantity { value, .. }) => pending.push(value),
            _ => {}
        }
    }
    found
}

fn divisor_reaches_zero(
    pool: &ExprPool,
    expression: ExprId,
    symbols: &BTreeMap<String, Range>,
) -> bool {
    let nowhere = Range::point(ExactRational::zero());
    let stays_away_from_zero = |divisor: ExprId| {
        matches!(
            interval(pool, divisor, &nowhere, symbols).and_then(|range| range.sign()),
            Some(Ordering::Less | Ordering::Greater)
        )
    };
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        match pool.node(node) {
            Ok(NodeView::Apply {
                head: Head::Operator(Operator::Div),
                arguments: [numerator, denominator],
            }) => {
                if !stays_away_from_zero(*denominator) {
                    return true;
                }
                pending.extend([*numerator, *denominator]);
            }
            Ok(NodeView::Apply {
                head: Head::Operator(Operator::Pow),
                arguments: [base, exponent],
            }) => {
                let negative = interval(pool, *exponent, &nowhere, symbols)
                    .is_some_and(|range| range.high.sign() == Ordering::Less);
                if negative && !stays_away_from_zero(*base) {
                    return true;
                }
                pending.extend([*base, *exponent]);
            }
            Ok(NodeView::Apply { arguments, .. }) => pending.extend(arguments.iter().copied()),
            Ok(NodeView::Quantity { value, .. }) => pending.push(value),
            _ => {}
        }
    }
    false
}

fn solve(
    pool: &mut ExprPool,
    expression: ExprId,
    components: &[Component],
) -> Result<WorstCase, WorstCaseRefusal> {
    if components.len() > TOLERANCE_LIMIT {
        return Err(WorstCaseRefusal::TooMany);
    }
    let unsupported = || WorstCaseRefusal::Unsupported;
    let nodes: Vec<ExprId> = components.iter().map(|component| component.key).collect();
    let mut ends = Vec::new();
    let mut ranges = Vec::new();
    let mut units = Vec::new();
    for component in components {
        let found = endpoints(pool, component.range)?;
        ranges.push(found.range);
        units.push(unit_quantity(pool, found.low).ok_or_else(unsupported)?);
        ends.push((found.low, found.high));
    }
    let names: Vec<String> = (0..nodes.len())
        .map(|index| format!("{TOLERANCE_SYMBOL_PREFIX}{index}"))
        .collect();
    let mut symbol_ranges = BTreeMap::new();
    let mut placeholders = Vec::new();
    for (index, name) in names.iter().enumerate() {
        let symbol = pool
            .intern_symbol(name, SymbolKind::Variable)
            .map_err(|_| unsupported())?;
        let symbol = pool.symbol(symbol).map_err(|_| unsupported())?;
        placeholders
            .push(apply(pool, Operator::Mul, &[symbol, units[index]]).ok_or_else(unsupported)?);
        symbol_ranges.insert(name.clone(), ranges[index].clone());
    }
    let everywhere: BTreeMap<ExprId, ExprId> = nodes
        .iter()
        .copied()
        .zip(placeholders.iter().copied())
        .collect();
    let whole = replaced(pool, expression, &everywhere).ok_or_else(unsupported)?;
    let whole = to_coherent_units(pool, whole).map_err(|_| unsupported())?;
    if divisor_reaches_zero(pool, whole.expression, &symbol_ranges) {
        return Err(WorstCaseRefusal::DivisorReachesZero(expression));
    }
    let mut directions = Vec::new();
    for index in 0..nodes.len() {
        let shown = components[index].range;
        let bound = pool.bound(0).map_err(|_| unsupported())?;
        let variable =
            apply(pool, Operator::Mul, &[bound, units[index]]).ok_or_else(unsupported)?;
        let mut map = BTreeMap::new();
        for (other, other_node) in nodes.iter().enumerate() {
            map.insert(
                *other_node,
                if other == index {
                    variable
                } else {
                    placeholders[other]
                },
            );
        }
        let isolated = replaced(pool, expression, &map).ok_or_else(unsupported)?;
        let coherent = to_coherent_units(pool, isolated).map_err(|_| unsupported())?;
        let slope =
            derivative(pool, coherent.expression, 0).ok_or(WorstCaseRefusal::NotMonotone(shown))?;
        let slope = crate::claim::expanded(pool, slope).unwrap_or(slope);
        let sign = constant_sign(pool, slope, &ranges[index], &symbol_ranges, BISECTION_DEPTH)
            .or_else(|| exact_sign(pool, slope, &ranges[index]))
            .ok_or(WorstCaseRefusal::NotMonotone(shown))?;
        directions.push(sign);
    }
    let mut low_corner = Vec::new();
    let mut high_corner = Vec::new();
    let mut low_map = BTreeMap::new();
    let mut high_map = BTreeMap::new();
    for (index, component) in components.iter().enumerate() {
        let (low, high) = ends[index];
        let (for_low, for_high) = match directions[index] {
            Ordering::Less => (high, low),
            Ordering::Greater | Ordering::Equal => (low, high),
        };
        low_map.insert(component.key, for_low);
        high_map.insert(component.key, for_high);
        let corner = |end| CornerEnd {
            range: component.range,
            name: component.name.clone(),
            end,
        };
        low_corner.push(corner(for_low));
        high_corner.push(corner(for_high));
    }
    let low = replaced(pool, expression, &low_map).ok_or_else(unsupported)?;
    let high = replaced(pool, expression, &high_map).ok_or_else(unsupported)?;
    Ok(WorstCase {
        low,
        high,
        low_corner,
        high_corner,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(text: &str) -> Result<(ExactRational, ExactRational), WorstCaseRefusal> {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, text).unwrap();
        let found = worst_case(&mut pool, expression).unwrap()?;
        let low = exact_value(&mut pool, found.low).unwrap();
        let high = exact_value(&mut pool, found.high).unwrap();
        Ok((low, high))
    }

    fn fraction(numerator: i64, denominator: i64) -> ExactRational {
        ExactRational::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    #[test]
    fn a_tolerance_alone_is_its_range() {
        assert_eq!(
            case("tolerance(25, 10%)"),
            Ok((fraction(45, 2), fraction(55, 2)))
        );
    }

    #[test]
    fn a_voltage_divider_is_lowest_at_the_smallest_coil() {
        let (low, high) = case("33/10 * tolerance(25, 10%) / (tolerance(25, 10%) + 2)").unwrap();
        assert_eq!(low, fraction(33 * 45, 10 * 49));
        assert_eq!(high, fraction(33 * 55, 10 * 59));
    }

    #[test]
    fn a_difference_of_two_independent_tolerances_spans_both() {
        assert_eq!(
            case("between(1, 2) - between(3, 5)"),
            Ok((fraction(-4, 1), fraction(-1, 1)))
        );
    }

    #[test]
    fn ends_of_two_dimensions_are_refused() {
        assert!(matches!(
            case("between(1 m, 2 s)"),
            Err(WorstCaseRefusal::EndsOfTwoDimensions(_))
        ));
    }

    #[test]
    fn a_divisor_whose_range_holds_zero_is_refused() {
        assert!(matches!(
            case("0 / between(-1, 1)"),
            Err(WorstCaseRefusal::DivisorReachesZero(_))
        ));
    }

    #[test]
    fn a_negative_power_of_a_range_holding_zero_is_refused() {
        assert!(matches!(
            case("(between(-1, 1))^-1"),
            Err(WorstCaseRefusal::DivisorReachesZero(_))
        ));
    }

    #[test]
    fn a_divisor_whose_range_stays_positive_is_taken() {
        assert_eq!(
            case("1 / between(1, 2)"),
            Ok((fraction(1, 2), fraction(1, 1)))
        );
    }

    #[test]
    fn the_limit_is_taken_and_one_more_is_refused() {
        let ranges = |count: usize| {
            (1..=count)
                .map(|index| format!("between({index}, {})", index + 1))
                .collect::<Vec<_>>()
                .join(" + ")
        };
        assert!(case(&ranges(TOLERANCE_LIMIT)).is_ok());
        assert_eq!(
            case(&ranges(TOLERANCE_LIMIT + 1)),
            Err(WorstCaseRefusal::TooMany)
        );
    }

    #[test]
    fn two_named_ranges_with_one_text_are_two_components() {
        let mut pool = ExprPool::new();
        let first = calc_syntax::parse_expression(&mut pool, "between(1, 2)").unwrap();
        let (first, mut named) = named_ranges(&mut pool, first, "a").unwrap();
        let second = calc_syntax::parse_expression(&mut pool, "between(1, 2)").unwrap();
        let (second, more) = named_ranges(&mut pool, second, "b").unwrap();
        named.extend(more);
        let difference = pool
            .apply(Head::Operator(Operator::Sub), &[first, second])
            .unwrap();
        let found = worst_case_with(&mut pool, difference, &named)
            .unwrap()
            .unwrap();
        assert_eq!(exact_value(&mut pool, found.low), Some(fraction(-1, 1)));
        assert_eq!(exact_value(&mut pool, found.high), Some(fraction(1, 1)));
    }

    fn named_case(
        range: &str,
        line: &str,
    ) -> Result<(ExactRational, ExactRational), WorstCaseRefusal> {
        let mut pool = ExprPool::new();
        let range = calc_syntax::parse_expression(&mut pool, range).unwrap();
        let (key, named) = named_ranges(&mut pool, range, "r").unwrap();
        let placeholder = calc_syntax::parse_expression(&mut pool, "r").unwrap();
        let line = calc_syntax::parse_expression(&mut pool, line).unwrap();
        let mut map = BTreeMap::new();
        map.insert(placeholder, key);
        let line = replaced(&mut pool, line, &map).unwrap();
        let found = worst_case_with(&mut pool, line, &named).unwrap()?;
        let low = exact_value(&mut pool, found.low).unwrap();
        let high = exact_value(&mut pool, found.high).unwrap();
        Ok((low, high))
    }

    #[test]
    fn a_slope_that_touches_zero_inside_is_still_monotone() {
        assert_eq!(
            named_case("between(0, 2)", "r^3 - 3*r^2 + 3*r"),
            Ok((fraction(0, 1), fraction(2, 1)))
        );
    }

    #[test]
    fn a_quotient_whose_slope_is_zero_at_an_end_is_monotone() {
        assert_eq!(
            named_case("between(1, 3)", "r + 1/r"),
            Ok((fraction(2, 1), fraction(10, 3)))
        );
    }

    #[test]
    fn a_slope_that_touches_zero_at_an_irrational_point_is_still_monotone() {
        assert_eq!(
            named_case("between(0, 2)", "r^5/5 - 4*r^3/3 + 4*r"),
            Ok((fraction(0, 1), fraction(56, 15)))
        );
    }

    #[test]
    fn a_line_that_turns_inside_its_named_range_is_refused() {
        assert!(matches!(
            named_case("between(0, 1)", "r*(1 - r)"),
            Err(WorstCaseRefusal::NotMonotone(_))
        ));
    }

    #[test]
    fn a_function_that_turns_inside_its_range_is_refused() {
        assert!(matches!(
            case("(between(-1, 1))^2"),
            Err(WorstCaseRefusal::NotMonotone(_))
        ));
    }
}
