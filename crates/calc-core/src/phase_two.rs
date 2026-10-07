use std::collections::{BTreeSet, HashMap};

use calc_expr::{BuiltinConstant, ExprId, ExprPool, Head, NodeView, Operator, SymbolKind};
use calc_numbers::{Interval, Number, Truth};

use std::cmp::Ordering;

use calc_expr::substitute_symbols;

use crate::exact_evaluation::{evaluate_exact, exact_sign};
use crate::machine_evaluation::constant_enclosure;
use crate::rule_search::{
    RuleSet, SearchError, Way, derivation_expression, derivation_expressions, instantiate,
    staged_derivation,
};
use crate::temperature_conversion::TemperatureConversion;
use crate::way_ranking::EvaluatedScores;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GivenValue {
    pub quantity: String,
    pub value: Number,
    pub standard_uncertainty: Option<Number>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundKind {
    AtMost,
    AtLeast,
    StrictlyBetween,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundRule {
    pub identifier: String,
    pub role: String,
    pub inputs: Vec<String>,
    pub kind: BoundKind,
    pub formulas: Vec<ExprId>,
    pub conditions: Vec<ExprId>,
    pub sources: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundEnd {
    pub value: Number,
    pub inclusive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub lower: Option<BoundEnd>,
    pub upper: Option<BoundEnd>,
    pub sources: Vec<String>,
    pub from: Vec<BoundOrigin>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoundSide {
    Lower,
    Upper,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundOrigin {
    pub bound: String,
    pub kind: BoundKind,
    pub inputs: Vec<String>,
    pub ends: Vec<BoundSide>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FurtherInputEntry {
    pub add: Vec<String>,
    pub rule: String,
    pub derivation: Vec<String>,
    pub conditions: Vec<ConditionOutcome>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PhaseTwoError {
    Search(SearchError),
    MissingValue(String),
    NegativeUncertainty(String),
    BoundFormulaCount(String),
    BoundFormulaArity(String),
    FurtherInputDoesNotReach(Vec<String>),
}

#[derive(Clone, Debug)]
enum Enclosed {
    Real {
        value: Interval,
        gradient: Vec<Interval>,
        depends: bool,
    },
    Truth(Truth),
}

struct Encloser<'inputs> {
    leaves: &'inputs HashMap<String, (usize, Interval)>,
    width: usize,
}

fn zero() -> Option<Interval> {
    Interval::point(0.0)
}

fn constant(value: Interval, width: usize) -> Option<Enclosed> {
    Some(Enclosed::Real {
        value,
        gradient: vec![zero()?; width],
        depends: false,
    })
}

fn scaled(gradient: &[Interval], factor: &Interval) -> Option<Vec<Interval>> {
    gradient.iter().map(|entry| entry.mul(factor)).collect()
}

fn combined(
    left: &[Interval],
    right: &[Interval],
    operation: impl Fn(&Interval, &Interval) -> Option<Interval>,
) -> Option<Vec<Interval>> {
    left.iter()
        .zip(right)
        .map(|(left, right)| operation(left, right))
        .collect()
}

impl Encloser<'_> {
    fn enclose(&self, pool: &ExprPool, expression: ExprId) -> Option<Enclosed> {
        match pool.node(expression).ok()? {
            NodeView::Number(number) => constant(
                Interval::from_exact(pool.number_value(number).ok()?)?,
                self.width,
            ),
            NodeView::Symbol(symbol) if symbol == BuiltinConstant::Pi.symbol() => {
                constant(constant_enclosure(std::f64::consts::PI)?, self.width)
            }
            NodeView::Symbol(symbol) if symbol == BuiltinConstant::E.symbol() => {
                constant(constant_enclosure(std::f64::consts::E)?, self.width)
            }
            NodeView::Symbol(symbol) => {
                let name = pool.symbol_name(symbol).ok()?;
                let (position, value) = self.leaves.get(name)?;
                let mut gradient = vec![zero()?; self.width];
                *gradient.get_mut(*position)? = Interval::point(1.0)?;
                Some(Enclosed::Real {
                    value: *value,
                    gradient,
                    depends: true,
                })
            }
            NodeView::Apply {
                head: Head::Operator(operator),
                arguments,
            } => {
                let arguments = arguments.to_vec();
                let values = arguments
                    .iter()
                    .map(|argument| self.enclose(pool, *argument))
                    .collect::<Option<Vec<Enclosed>>>()?;
                self.apply(operator, &values)
            }
            _ => None,
        }
    }

    fn apply(&self, operator: Operator, values: &[Enclosed]) -> Option<Enclosed> {
        let reals: Vec<(Interval, Vec<Interval>, bool)> = values
            .iter()
            .filter_map(|value| match value {
                Enclosed::Real {
                    value,
                    gradient,
                    depends,
                } => Some((*value, gradient.clone(), *depends)),
                Enclosed::Truth(_) => None,
            })
            .collect();
        let truths: Vec<Truth> = values
            .iter()
            .filter_map(|value| match value {
                Enclosed::Truth(truth) => Some(*truth),
                Enclosed::Real { .. } => None,
            })
            .collect();
        let real = |value: Interval, gradient: Vec<Interval>, depends: bool| {
            Some(Enclosed::Real {
                value,
                gradient,
                depends,
            })
        };
        let one = Interval::point(1.0)?;
        match (operator, reals.as_slice(), truths.as_slice()) {
            (Operator::Add, [(a, ga, da), (b, gb, db)], []) => {
                real(a.add(b)?, combined(ga, gb, Interval::add)?, *da || *db)
            }
            (Operator::Sub, [(a, ga, da), (b, gb, db)], []) => {
                real(a.sub(b)?, combined(ga, gb, Interval::sub)?, *da || *db)
            }
            (Operator::Mul, [(a, ga, da), (b, gb, db)], []) => {
                let left = scaled(ga, b)?;
                let right = scaled(gb, a)?;
                real(
                    a.mul(b)?,
                    combined(&left, &right, Interval::add)?,
                    *da || *db,
                )
            }
            (Operator::Div, [(a, ga, da), (b, gb, db)], []) => {
                if b.contains_zero() {
                    return None;
                }
                let quotient = a.div(b)?;
                let inner = combined(ga, &scaled(gb, &quotient)?, Interval::sub)?;
                let gradient = inner
                    .iter()
                    .map(|entry| entry.div(b))
                    .collect::<Option<Vec<_>>>()?;
                real(quotient, gradient, *da || *db)
            }
            (Operator::Neg, [(a, ga, da)], []) => {
                real(a.neg(), ga.iter().map(Interval::neg).collect(), *da)
            }
            (Operator::MulAdd, [(a, ga, da), (b, gb, db), (c, gc, dc)], []) => {
                let product = combined(&scaled(ga, b)?, &scaled(gb, a)?, Interval::add)?;
                real(
                    a.mul_add(b, c)?,
                    combined(&product, gc, Interval::add)?,
                    *da || *db || *dc,
                )
            }
            (Operator::Sqrt, [(a, ga, da)], []) => {
                let root = a.sqrt()?;
                if root.contains_zero() {
                    return None;
                }
                let twice = root.add(&root)?;
                let gradient = ga
                    .iter()
                    .map(|entry| entry.div(&twice))
                    .collect::<Option<Vec<_>>>()?;
                real(root, gradient, *da)
            }
            (Operator::Pow, [(base, gb, db), (exponent, ge, de)], []) => {
                let power = Interval::pow(base, exponent)?;
                let mut gradient = vec![zero()?; self.width];
                if *db {
                    let reduced = exponent.sub(&one)?;
                    let factor = exponent.mul(&Interval::pow(base, &reduced)?)?;
                    gradient = combined(&gradient, &scaled(gb, &factor)?, Interval::add)?;
                }
                if *de {
                    let factor = base.ln()?.mul(&power)?;
                    gradient = combined(&gradient, &scaled(ge, &factor)?, Interval::add)?;
                }
                real(power, gradient, *db || *de)
            }
            (Operator::Exp, [(a, ga, da)], []) => {
                let value = a.exp();
                real(value, scaled(ga, &value)?, *da)
            }
            (Operator::Ln, [(a, ga, da)], []) => {
                if a.contains_zero() {
                    return None;
                }
                let gradient = ga
                    .iter()
                    .map(|entry| entry.div(a))
                    .collect::<Option<Vec<_>>>()?;
                real(a.ln()?, gradient, *da)
            }
            (Operator::Sin, [(a, ga, da)], []) => real(a.sin()?, scaled(ga, &a.cos()?)?, *da),
            (Operator::Cos, [(a, ga, da)], []) => real(a.cos()?, scaled(ga, &a.sin()?.neg())?, *da),
            (Operator::Tan, [(a, ga, da)], []) => {
                let value = a.tan()?;
                let magnitude = value.abs();
                let factor = one.add(&magnitude.mul(&magnitude)?)?;
                real(value, scaled(ga, &factor)?, *da)
            }
            (Operator::Abs, [(a, ga, da)], []) => {
                if a.contains_zero() {
                    return None;
                }
                let is_negative = a.upper() < 0.0;
                let gradient = if is_negative {
                    ga.iter().map(Interval::neg).collect()
                } else {
                    ga.clone()
                };
                real(a.abs(), gradient, *da)
            }
            (Operator::Less, [(a, _, _), (b, _, _)], []) => Some(Enclosed::Truth(a.less(b))),
            (Operator::LessOrEqual, [(a, _, _), (b, _, _)], []) => {
                Some(Enclosed::Truth(a.less_or_equal(b)))
            }
            (Operator::Greater, [(a, _, _), (b, _, _)], []) => Some(Enclosed::Truth(b.less(a))),
            (Operator::GreaterOrEqual, [(a, _, _), (b, _, _)], []) => {
                Some(Enclosed::Truth(b.less_or_equal(a)))
            }
            (Operator::Equal, [(a, _, _), (b, _, _)], []) => Some(Enclosed::Truth(a.equal(b))),
            (Operator::NotEqual, [(a, _, _), (b, _, _)], []) => {
                Some(Enclosed::Truth(a.equal(b).negated()))
            }
            (Operator::And, [], [left, right]) => Some(Enclosed::Truth(left.and(*right))),
            (Operator::Or, [], [left, right]) => Some(Enclosed::Truth(left.or(*right))),
            (Operator::Not, [], [truth]) => Some(Enclosed::Truth(truth.negated())),
            (_, [(a, ga, da)], [])
                if let Some(conversion) = TemperatureConversion::of_operator(operator) =>
            {
                let map = conversion.affine_map();
                let factor = Interval::from_exact(&map.factor)?;
                let addend = Interval::from_exact(&map.addend)?;
                real(factor.mul(a)?.add(&addend)?, scaled(ga, &factor)?, *da)
            }
            _ => None,
        }
    }
}

fn upper_end(interval: &Interval) -> Option<Number> {
    Number::F64(interval.upper()).to_exact().ok()
}

fn lower_end(interval: &Interval) -> Option<Number> {
    Number::F64(interval.lower()).to_exact().ok()
}

fn given_by_quantity(given: &[GivenValue]) -> HashMap<&str, &GivenValue> {
    given
        .iter()
        .map(|value| (value.quantity.as_str(), value))
        .collect()
}

fn leaf_intervals(
    leaves: &[String],
    given: &[GivenValue],
) -> Result<HashMap<String, (usize, Interval)>, PhaseTwoError> {
    let by_quantity = given_by_quantity(given);
    leaves
        .iter()
        .enumerate()
        .map(|(position, leaf)| {
            let value = by_quantity
                .get(leaf.as_str())
                .and_then(|given| Interval::from_exact(&given.value))
                .ok_or_else(|| PhaseTwoError::MissingValue(leaf.clone()))?;
            Ok((leaf.clone(), (position, value)))
        })
        .collect()
}

pub fn evaluated_scores(
    pool: &mut ExprPool,
    rule_set: &RuleSet,
    wanted: &str,
    way: &Way,
    given: &[GivenValue],
) -> Result<EvaluatedScores, PhaseTwoError> {
    let derivation = derivation_expression(pool, rule_set, wanted, &way.derivation)
        .map_err(PhaseTwoError::Search)?;
    let leaves = leaf_intervals(&derivation.leaves, given)?;
    let by_quantity = given_by_quantity(given);
    let mut uncertainties = Vec::new();
    for leaf in &derivation.leaves {
        let uncertainty = match by_quantity
            .get(leaf.as_str())
            .and_then(|given| given.standard_uncertainty.as_ref())
        {
            Some(uncertainty) => {
                let interval = Interval::from_exact(uncertainty)
                    .ok_or_else(|| PhaseTwoError::MissingValue(leaf.clone()))?;
                if interval.upper() < 0.0 {
                    return Err(PhaseTwoError::NegativeUncertainty(leaf.clone()));
                }
                interval
            }
            None => {
                Interval::point(0.0).ok_or_else(|| PhaseTwoError::MissingValue(leaf.clone()))?
            }
        };
        uncertainties.push(uncertainty);
    }
    let encloser = Encloser {
        leaves: &leaves,
        width: derivation.leaves.len(),
    };
    let Some(Enclosed::Real {
        value, gradient, ..
    }) = encloser.enclose(pool, derivation.expression)
    else {
        return Ok(EvaluatedScores::default());
    };
    Ok(EvaluatedScores {
        smallest_uncertainty: propagated_uncertainty(&gradient, &uncertainties)
            .as_ref()
            .and_then(upper_end),
        conditioning: relative_condition(&value, &gradient, &derivation.leaves, &leaves)
            .as_ref()
            .and_then(upper_end),
    })
}

fn propagated_uncertainty(gradient: &[Interval], uncertainties: &[Interval]) -> Option<Interval> {
    let mut sum = Interval::point(0.0)?;
    for (sensitivity, uncertainty) in gradient.iter().zip(uncertainties) {
        let is_certain = uncertainty.lower() == 0.0 && uncertainty.upper() == 0.0;
        if is_certain {
            continue;
        }
        let contribution = sensitivity.mul(uncertainty)?.abs();
        sum = sum.add(&contribution.mul(&contribution)?)?;
    }
    if sum.upper() <= 0.0 {
        return Interval::point(0.0);
    }
    Interval::point(0.0)?
        .hull(&Interval::point(sum.upper())?)
        .sqrt()
}

fn relative_condition(
    value: &Interval,
    gradient: &[Interval],
    names: &[String],
    leaves: &HashMap<String, (usize, Interval)>,
) -> Option<Interval> {
    if value.contains_zero() {
        return None;
    }
    let mut sum = Interval::point(0.0)?;
    for (sensitivity, name) in gradient.iter().zip(names) {
        let (_, input) = leaves.get(name)?;
        sum = sum.add(&sensitivity.abs().mul(&input.abs())?)?;
    }
    sum.div(&value.abs())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConditionOutcome {
    pub rule: String,
    pub condition: ExprId,
    pub holds: Option<bool>,
}

fn three_valued(truth: Truth) -> Option<bool> {
    match truth {
        Truth::True => Some(true),
        Truth::False => Some(false),
        Truth::Unknown => None,
    }
}

fn relation_from_sign(operator: Operator, sign: Ordering) -> Option<bool> {
    Some(match operator {
        Operator::Equal => sign == Ordering::Equal,
        Operator::NotEqual => sign != Ordering::Equal,
        Operator::Less => sign == Ordering::Less,
        Operator::LessOrEqual => sign != Ordering::Greater,
        Operator::Greater => sign == Ordering::Greater,
        Operator::GreaterOrEqual => sign != Ordering::Less,
        _ => return None,
    })
}

struct ConditionDecider<'values> {
    leaves: &'values HashMap<String, (usize, Interval)>,
    width: usize,
    exact_values: Option<HashMap<calc_expr::SymbolId, ExprId>>,
}

impl ConditionDecider<'_> {
    fn decide(&self, pool: &mut ExprPool, condition: ExprId) -> Option<bool> {
        let NodeView::Apply {
            head: Head::Operator(operator),
            arguments,
        } = pool.node(condition).ok()?
        else {
            return None;
        };
        let arguments = arguments.to_vec();
        match (operator, arguments.as_slice()) {
            (Operator::Not, [inner]) => self.decide(pool, *inner).map(|holds| !holds),
            (Operator::And, [left, right]) => {
                match (self.decide(pool, *left), self.decide(pool, *right)) {
                    (Some(false), _) | (_, Some(false)) => Some(false),
                    (Some(true), Some(true)) => Some(true),
                    _ => None,
                }
            }
            (Operator::Or, [left, right]) => {
                match (self.decide(pool, *left), self.decide(pool, *right)) {
                    (Some(true), _) | (_, Some(true)) => Some(true),
                    (Some(false), Some(false)) => Some(false),
                    _ => None,
                }
            }
            (
                Operator::Equal
                | Operator::NotEqual
                | Operator::Less
                | Operator::LessOrEqual
                | Operator::Greater
                | Operator::GreaterOrEqual,
                [left, right],
            ) => self
                .decide_exactly(pool, operator, *left, *right)
                .or_else(|| self.decide_by_enclosure(pool, condition)),
            _ => None,
        }
    }

    fn decide_exactly(
        &self,
        pool: &mut ExprPool,
        operator: Operator,
        left: ExprId,
        right: ExprId,
    ) -> Option<bool> {
        let values = self.exact_values.as_ref()?;
        let difference = pool
            .apply(Head::Operator(Operator::Sub), &[left, right])
            .ok()?;
        let substituted = substitute_symbols(pool, difference, values).ok()?;
        relation_from_sign(operator, exact_sign(pool, substituted)?)
    }

    fn decide_by_enclosure(&self, pool: &ExprPool, condition: ExprId) -> Option<bool> {
        let encloser = Encloser {
            leaves: self.leaves,
            width: self.width,
        };
        match encloser.enclose(pool, condition)? {
            Enclosed::Truth(truth) => three_valued(truth),
            Enclosed::Real { .. } => None,
        }
    }
}

pub fn decide_conditions(
    pool: &mut ExprPool,
    rule_set: &RuleSet,
    wanted: &str,
    way: &Way,
    given: &[GivenValue],
) -> Result<Vec<ConditionOutcome>, PhaseTwoError> {
    decide_derivation_conditions(pool, rule_set, wanted, &way.derivation, given)
}

pub fn decide_derivation_conditions(
    pool: &mut ExprPool,
    rule_set: &RuleSet,
    wanted: &str,
    derivation: &[String],
    given: &[GivenValue],
) -> Result<Vec<ConditionOutcome>, PhaseTwoError> {
    let (expressions, leaves) = derivation_expressions(pool, rule_set, wanted, derivation)
        .map_err(PhaseTwoError::Search)?;
    let by_quantity = given_by_quantity(given);
    let has_every_value = leaves
        .iter()
        .all(|leaf| by_quantity.contains_key(leaf.as_str()));
    let leaf_names: Vec<String> = leaves.iter().cloned().collect();
    let mut intervals: HashMap<String, (usize, Interval)> = HashMap::new();
    for (position, leaf) in leaf_names.iter().enumerate() {
        if let Some(value) = by_quantity
            .get(leaf.as_str())
            .and_then(|given| Interval::from_exact(&given.value))
        {
            intervals.insert(leaf.clone(), (position, value));
        }
    }
    let exact_values = if has_every_value
        && leaf_names.iter().all(|leaf| {
            by_quantity
                .get(leaf.as_str())
                .is_some_and(|given| given.value.is_exact())
        }) {
        let mut values = HashMap::new();
        for leaf in &leaf_names {
            let symbol = pool
                .lookup_symbol(leaf)
                .ok_or_else(|| PhaseTwoError::MissingValue(leaf.clone()))?;
            let number = by_quantity
                .get(leaf.as_str())
                .map(|given| given.value.clone())
                .ok_or_else(|| PhaseTwoError::MissingValue(leaf.clone()))?;
            let node = pool
                .number(number)
                .map_err(|_| PhaseTwoError::MissingValue(leaf.clone()))?;
            values.insert(symbol, node);
        }
        Some(values)
    } else {
        None
    };
    let decider = ConditionDecider {
        leaves: &intervals,
        width: leaf_names.len(),
        exact_values,
    };
    let mut outcomes = Vec::new();
    for identifier in derivation {
        let rule = rule_set
            .rules
            .iter()
            .find(|rule| &rule.identifier == identifier)
            .ok_or_else(|| PhaseTwoError::Search(SearchError::UnknownRule(identifier.clone())))?;
        let arguments = rule
            .inputs
            .iter()
            .map(|input| {
                expressions
                    .get(input)
                    .copied()
                    .ok_or_else(|| PhaseTwoError::MissingValue(input.clone()))
            })
            .collect::<Result<Vec<ExprId>, PhaseTwoError>>()?;
        for condition in &rule.conditions {
            let instance = instantiate(pool, *condition, &arguments).ok_or_else(|| {
                PhaseTwoError::Search(SearchError::FormulaArityMismatch(rule.identifier.clone()))
            })?;
            let holds = decider.decide(pool, instance);
            outcomes.push(ConditionOutcome {
                rule: rule.identifier.clone(),
                condition: *condition,
                holds,
            });
        }
    }
    Ok(outcomes)
}

pub fn further_input_entries(
    pool: &mut ExprPool,
    rule_set: &RuleSet,
    wanted: &str,
    given: &[GivenValue],
    obtainable: &[String],
    missing_any_of: &[Vec<String>],
) -> Result<Vec<FurtherInputEntry>, PhaseTwoError> {
    let given_roles: Vec<String> = given.iter().map(|value| value.quantity.clone()).collect();
    missing_any_of
        .iter()
        .map(|add| {
            let stages = [given_roles.clone(), obtainable.to_vec(), add.clone()];
            let staged = staged_derivation(rule_set, wanted, &stages)
                .map_err(PhaseTwoError::Search)?
                .ok_or_else(|| PhaseTwoError::FurtherInputDoesNotReach(add.clone()))?;
            let conditions =
                decide_derivation_conditions(pool, rule_set, wanted, &staged.derivation, given)?;
            Ok(FurtherInputEntry {
                add: add.clone(),
                rule: staged.rule,
                derivation: staged.derivation,
                conditions,
            })
        })
        .collect()
}

struct BoundEnds {
    lower: Option<BoundEnd>,
    upper: Option<BoundEnd>,
}

fn formula_end(
    pool: &mut ExprPool,
    rule: &BoundRule,
    formula: ExprId,
    given: &[GivenValue],
    upper: bool,
) -> Result<Option<Number>, PhaseTwoError> {
    let by_quantity = given_by_quantity(given);
    let mut numbers = Vec::new();
    for input in &rule.inputs {
        let value = by_quantity
            .get(input.as_str())
            .map(|given| given.value.to_exact())
            .and_then(Result::ok)
            .ok_or_else(|| PhaseTwoError::MissingValue(input.clone()))?;
        numbers.push(
            pool.number(value)
                .map_err(|_| PhaseTwoError::MissingValue(input.clone()))?,
        );
    }
    let instance = instantiate(pool, formula, &numbers)
        .ok_or_else(|| PhaseTwoError::BoundFormulaArity(rule.identifier.clone()))?;
    if let Some(exact) = evaluate_exact(pool, instance)
        .ok()
        .and_then(|evaluation| evaluation.rational_value().cloned())
    {
        return Ok(Some(exact));
    }
    let leaves = HashMap::new();
    let encloser = Encloser {
        leaves: &leaves,
        width: 0,
    };
    Ok(match encloser.enclose(pool, instance) {
        Some(Enclosed::Real { value, .. }) if upper => upper_end(&value),
        Some(Enclosed::Real { value, .. }) => lower_end(&value),
        _ => None,
    })
}

fn conditions_hold(
    pool: &mut ExprPool,
    rule: &BoundRule,
    given: &[GivenValue],
) -> Result<bool, PhaseTwoError> {
    let mut symbols = Vec::new();
    for input in &rule.inputs {
        let symbol = pool
            .intern_symbol(input, SymbolKind::Variable)
            .map_err(|_| PhaseTwoError::MissingValue(input.clone()))?;
        symbols.push(
            pool.symbol(symbol)
                .map_err(|_| PhaseTwoError::MissingValue(input.clone()))?,
        );
    }
    let leaves = leaf_intervals(&rule.inputs, given)?;
    let encloser = Encloser {
        leaves: &leaves,
        width: rule.inputs.len(),
    };
    for condition in &rule.conditions {
        let instance = instantiate(pool, *condition, &symbols)
            .ok_or_else(|| PhaseTwoError::BoundFormulaArity(rule.identifier.clone()))?;
        if !matches!(
            encloser.enclose(pool, instance),
            Some(Enclosed::Truth(Truth::True))
        ) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn rule_ends(
    pool: &mut ExprPool,
    rule: &BoundRule,
    given: &[GivenValue],
) -> Result<BoundEnds, PhaseTwoError> {
    let expected = match rule.kind {
        BoundKind::AtMost | BoundKind::AtLeast => 1,
        BoundKind::StrictlyBetween => 2,
    };
    if rule.formulas.len() != expected {
        return Err(PhaseTwoError::BoundFormulaCount(rule.identifier.clone()));
    }
    let end =
        |value: Option<Number>, inclusive: bool| value.map(|value| BoundEnd { value, inclusive });
    Ok(match rule.kind {
        BoundKind::AtMost => BoundEnds {
            lower: None,
            upper: end(
                formula_end(pool, rule, rule.formulas[0], given, true)?,
                true,
            ),
        },
        BoundKind::AtLeast => BoundEnds {
            lower: end(
                formula_end(pool, rule, rule.formulas[0], given, false)?,
                true,
            ),
            upper: None,
        },
        BoundKind::StrictlyBetween => BoundEnds {
            lower: end(
                formula_end(pool, rule, rule.formulas[0], given, false)?,
                false,
            ),
            upper: end(
                formula_end(pool, rule, rule.formulas[1], given, true)?,
                false,
            ),
        },
    })
}

fn exact_compare(left: &Number, right: &Number) -> std::cmp::Ordering {
    match left.sub_exact(right) {
        Ok(Number::Integer(difference)) if difference.is_zero() => std::cmp::Ordering::Equal,
        Ok(Number::Integer(difference)) if difference.is_negative() => std::cmp::Ordering::Less,
        Ok(Number::Rational(difference)) if difference.numerator().is_negative() => {
            std::cmp::Ordering::Less
        }
        _ => std::cmp::Ordering::Greater,
    }
}

fn tighter(candidate: &BoundEnd, current: &BoundEnd, upper: bool) -> std::cmp::Ordering {
    let by_value = exact_compare(&candidate.value, &current.value);
    let by_value = if upper { by_value } else { by_value.reverse() };
    by_value.then(candidate.inclusive.cmp(&current.inclusive))
}

pub fn not_reached_bounds(
    pool: &mut ExprPool,
    wanted: &str,
    bound_rules: &[BoundRule],
    given: &[GivenValue],
) -> Result<Option<Bounds>, PhaseTwoError> {
    let given_quantities: BTreeSet<&str> =
        given.iter().map(|value| value.quantity.as_str()).collect();
    let mut ordered: Vec<&BoundRule> = bound_rules
        .iter()
        .filter(|rule| rule.role == wanted)
        .filter(|rule| {
            rule.inputs
                .iter()
                .all(|input| given_quantities.contains(input.as_str()))
        })
        .collect();
    ordered.sort_by(|left, right| left.identifier.cmp(&right.identifier));
    let mut contributions: Vec<(&BoundRule, BoundEnds)> = Vec::new();
    for rule in ordered {
        if conditions_hold(pool, rule, given)? {
            let ends = rule_ends(pool, rule, given)?;
            contributions.push((rule, ends));
        }
    }
    let pick = |upper: bool| -> Option<BoundEnd> {
        contributions
            .iter()
            .filter_map(|(_, ends)| {
                if upper {
                    ends.upper.clone()
                } else {
                    ends.lower.clone()
                }
            })
            .min_by(|left, right| tighter(left, right, upper))
    };
    let lower = pick(false);
    let upper = pick(true);
    if lower.is_none() && upper.is_none() {
        return Ok(None);
    }
    let from: Vec<BoundOrigin> = contributions
        .iter()
        .filter_map(|(rule, ends)| {
            let mut sides = Vec::new();
            if ends.lower.is_some() && ends.lower == lower {
                sides.push(BoundSide::Lower);
            }
            if ends.upper.is_some() && ends.upper == upper {
                sides.push(BoundSide::Upper);
            }
            if sides.is_empty() {
                return None;
            }
            let mut inputs = rule.inputs.clone();
            inputs.sort();
            Some(BoundOrigin {
                bound: rule.identifier.clone(),
                kind: rule.kind,
                inputs,
                ends: sides,
            })
        })
        .collect();
    let sources: BTreeSet<String> = contributions
        .iter()
        .filter(|(rule, _)| from.iter().any(|origin| origin.bound == rule.identifier))
        .flat_map(|(rule, _)| rule.sources.iter().cloned())
        .collect();
    Ok(Some(Bounds {
        lower,
        upper,
        sources: sources.into_iter().collect(),
        from,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_search::{ErrorBound, Exactness, Rule};
    use calc_expr::BinderKind;
    use calc_numbers::Integer;

    fn apply(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> ExprId {
        pool.apply(Head::Operator(operator), arguments).unwrap()
    }

    fn integer(pool: &mut ExprPool, value: i64) -> ExprId {
        pool.number(Number::from(value)).unwrap()
    }

    fn lambda(pool: &mut ExprPool, parameters: usize, body: ExprId) -> ExprId {
        (0..parameters).fold(body, |inner, _| {
            pool.bind(BinderKind::Lambda, &[], inner).unwrap()
        })
    }

    fn decimal(digits: i64, scale: u32) -> Number {
        Number::fraction(&Integer::from(digits), &Integer::from(10_i64).pow(scale)).unwrap()
    }

    fn at_most(value: &Number, limit: &Number) -> bool {
        !limit
            .sub_exact(value)
            .unwrap()
            .to_exact()
            .is_ok_and(|difference| match difference {
                Number::Integer(integer) => integer.is_negative(),
                Number::Rational(rational) => rational.numerator().is_negative(),
                _ => true,
            })
    }

    fn within(value: &Number, lower: &Number, upper: &Number) -> bool {
        at_most(lower, value) && at_most(value, upper)
    }

    fn formula_rule(identifier: &str, output: &str, inputs: &[&str], formula: ExprId) -> Rule {
        Rule {
            identifier: identifier.to_string(),
            output: output.to_string(),
            inputs: inputs.iter().map(|input| input.to_string()).collect(),
            formula,
            conditions: Vec::new(),
            exact: false,
            sources: vec!["source".to_string()],
            corpus_references: Vec::new(),
        }
    }

    fn way(derivation: &[&str], inputs: &[&str]) -> Way {
        Way {
            inputs: inputs.iter().map(|input| input.to_string()).collect(),
            steps: 1,
            derivation: derivation.iter().map(|rule| rule.to_string()).collect(),
            last_rule: derivation.last().map(|rule| rule.to_string()),
            obtainable: true,
            exactness: Exactness::Machine,
            error_bound: ErrorBound::Documented,
            gate_count: None,
            conditions: Vec::new(),
            sources: Vec::new(),
        }
    }

    fn given(quantity: &str, value: Number, uncertainty: Option<Number>) -> GivenValue {
        GivenValue {
            quantity: quantity.to_string(),
            value,
            standard_uncertainty: uncertainty,
        }
    }

    fn single_rule(
        pool: &mut ExprPool,
        parameters: usize,
        build: impl FnOnce(&mut ExprPool) -> ExprId,
    ) -> RuleSet {
        let body = build(pool);
        let formula = lambda(pool, parameters, body);
        let inputs: Vec<&str> = ["x", "y"].into_iter().take(parameters).collect();
        RuleSet {
            version: 1,
            quantities: vec!["x".to_string(), "y".to_string(), "f".to_string()],
            rules: vec![formula_rule("f/rule", "f", &inputs, formula)],
        }
    }

    fn scores(
        pool: &mut ExprPool,
        rules: &RuleSet,
        inputs: &[&str],
        values: &[GivenValue],
    ) -> EvaluatedScores {
        evaluated_scores(pool, rules, "f", &way(&["f/rule"], inputs), values).unwrap()
    }

    fn circumference(pool: &mut ExprPool) -> RuleSet {
        single_rule(pool, 1, |pool| {
            let pi = pool.symbol(BuiltinConstant::Pi.symbol()).unwrap();
            let x = pool.bound(0).unwrap();
            apply(pool, Operator::Mul, &[pi, x])
        })
    }

    #[test]
    fn uncertainty_of_the_circumference_encloses_pi_times_the_diameter_uncertainty() {
        let mut pool = ExprPool::new();
        let rules = circumference(&mut pool);
        let values = [given("x", Number::from(10_i64), Some(decimal(1, 1)))];

        let found = scores(&mut pool, &rules, &["x"], &values);

        let score = found.smallest_uncertainty.unwrap();
        assert!(within(
            &score,
            &decimal(3_141_592_653_589_793, 16),
            &decimal(3_141_592_653_589_800, 16)
        ));
    }

    #[test]
    fn uncertainties_of_independent_inputs_add_in_quadrature() {
        let mut pool = ExprPool::new();
        let rules = single_rule(&mut pool, 2, |pool| {
            let x = pool.bound(1).unwrap();
            let y = pool.bound(0).unwrap();
            apply(pool, Operator::Mul, &[x, y])
        });
        let values = [
            given("x", Number::from(3_i64), Some(decimal(1, 1))),
            given("y", Number::from(4_i64), Some(decimal(2, 1))),
        ];

        let found = scores(&mut pool, &rules, &["x", "y"], &values);

        assert!(within(
            &found.smallest_uncertainty.unwrap(),
            &decimal(7_211_102_550_927_978, 16),
            &decimal(7_211_102_550_928_000, 16)
        ));
    }

    #[test]
    fn fahrenheit_reading_uncertainty_scales_by_five_ninths() {
        let mut pool = ExprPool::new();
        let rules = single_rule(&mut pool, 1, |pool| {
            let x = pool.bound(0).unwrap();
            apply(pool, Operator::FromFahrenheit, &[x])
        });
        let values = [given("x", Number::from(68_i64), Some(decimal(5, 1)))];

        let found = scores(&mut pool, &rules, &["x"], &values);

        let five_eighteenths =
            Number::fraction(&Integer::from(5_i64), &Integer::from(18_i64)).unwrap();
        assert!(within(
            &found.smallest_uncertainty.unwrap(),
            &five_eighteenths,
            &decimal(2_777_777_777_778, 13)
        ));
    }

    #[test]
    fn celsius_reading_conditioning_is_kelvin_over_the_reading() {
        let mut pool = ExprPool::new();
        let rules = single_rule(&mut pool, 1, |pool| {
            let x = pool.bound(0).unwrap();
            apply(pool, Operator::ToCelsius, &[x])
        });
        let values = [given("x", Number::from(300_i64), None)];

        let found = scores(&mut pool, &rules, &["x"], &values);

        let ratio = Number::fraction(&Integer::from(6000_i64), &Integer::from(537_i64)).unwrap();
        assert!(within(
            &found.conditioning.unwrap(),
            &ratio,
            &decimal(11_173_184_357_543, 12)
        ));
    }

    #[test]
    fn inputs_without_uncertainty_give_zero_uncertainty() {
        let mut pool = ExprPool::new();
        let rules = circumference(&mut pool);
        let values = [given("x", Number::from(10_i64), None)];

        let found = scores(&mut pool, &rules, &["x"], &values);

        assert_eq!(found.smallest_uncertainty, Some(Number::from(0_i64)));
    }

    #[test]
    fn square_root_sensitivity_is_half_the_reciprocal_root() {
        let mut pool = ExprPool::new();
        let rules = single_rule(&mut pool, 1, |pool| {
            let x = pool.bound(0).unwrap();
            apply(pool, Operator::Sqrt, &[x])
        });
        let values = [given("x", Number::from(4_i64), Some(Number::from(1_i64)))];

        let found = scores(&mut pool, &rules, &["x"], &values);

        assert!(within(
            &found.smallest_uncertainty.unwrap(),
            &decimal(25, 2),
            &decimal(2_500_000_001, 10)
        ));
    }

    #[test]
    fn quotient_rule_cancels_a_variable_that_appears_above_and_below() {
        let mut pool = ExprPool::new();
        let rules = single_rule(&mut pool, 1, |pool| {
            let x = pool.bound(0).unwrap();
            let two = integer(pool, 2);
            let doubled = apply(pool, Operator::Mul, &[two, x]);
            apply(pool, Operator::Div, &[x, doubled])
        });
        let values = [given("x", Number::from(4_i64), Some(Number::from(1_i64)))];

        let found = scores(&mut pool, &rules, &["x"], &values);

        assert!(within(
            &found.smallest_uncertainty.unwrap(),
            &Number::from(0_i64),
            &decimal(1, 10)
        ));
    }

    #[test]
    fn condition_number_of_a_sum_is_the_one_of_the_finite_sum() {
        let mut pool = ExprPool::new();
        let rules = single_rule(&mut pool, 2, |pool| {
            let x = pool.bound(1).unwrap();
            let y = pool.bound(0).unwrap();
            apply(pool, Operator::Add, &[x, y])
        });
        let values = [
            given("x", Number::from(3_i64), None),
            given("y", Number::from(-2_i64), None),
        ];

        let found = scores(&mut pool, &rules, &["x", "y"], &values);

        assert!(within(
            &found.conditioning.unwrap(),
            &Number::from(5_i64),
            &decimal(5_000_000_001, 9)
        ));
    }

    #[test]
    fn condition_number_of_a_product_is_two() {
        let mut pool = ExprPool::new();
        let rules = single_rule(&mut pool, 2, |pool| {
            let x = pool.bound(1).unwrap();
            let y = pool.bound(0).unwrap();
            apply(pool, Operator::Mul, &[x, y])
        });
        let values = [
            given("x", decimal(17, 1), None),
            given("y", Number::from(-9_i64), None),
        ];

        let found = scores(&mut pool, &rules, &["x", "y"], &values);

        assert!(within(
            &found.conditioning.unwrap(),
            &Number::from(2_i64),
            &decimal(2_000_000_001, 9)
        ));
    }

    #[test]
    fn condition_number_is_unknown_where_the_value_may_be_zero() {
        let mut pool = ExprPool::new();
        let rules = single_rule(&mut pool, 2, |pool| {
            let x = pool.bound(1).unwrap();
            let y = pool.bound(0).unwrap();
            apply(pool, Operator::Sub, &[x, y])
        });
        let values = [
            given("x", Number::from(1_i64), None),
            given("y", Number::from(1_i64), None),
        ];

        let found = scores(&mut pool, &rules, &["x", "y"], &values);

        assert_eq!(found.conditioning, None);
    }

    #[test]
    fn missing_input_value_is_an_error() {
        let mut pool = ExprPool::new();
        let rules = circumference(&mut pool);

        assert_eq!(
            evaluated_scores(&mut pool, &rules, "f", &way(&["f/rule"], &["x"]), &[]),
            Err(PhaseTwoError::MissingValue("x".to_string()))
        );
    }

    #[test]
    fn negative_uncertainty_is_an_error() {
        let mut pool = ExprPool::new();
        let rules = circumference(&mut pool);
        let values = [given("x", Number::from(10_i64), Some(Number::from(-1_i64)))];

        assert_eq!(
            evaluated_scores(&mut pool, &rules, "f", &way(&["f/rule"], &["x"]), &values),
            Err(PhaseTwoError::NegativeUncertainty("x".to_string()))
        );
    }

    fn condition_rules(
        pool: &mut ExprPool,
        build: impl FnOnce(&mut ExprPool, ExprId) -> ExprId,
    ) -> RuleSet {
        let x = pool.bound(0).unwrap();
        let formula = lambda(pool, 1, x);
        let body = build(pool, x);
        let condition = lambda(pool, 1, body);
        let mut rule = formula_rule("f/rule", "f", &["x"], formula);
        rule.conditions.push(condition);
        RuleSet {
            version: 1,
            quantities: vec!["x".to_string(), "f".to_string()],
            rules: vec![rule],
        }
    }

    fn holds_at(
        value: Number,
        build: impl FnOnce(&mut ExprPool, ExprId) -> ExprId,
    ) -> Option<bool> {
        let mut pool = ExprPool::new();
        let rules = condition_rules(&mut pool, build);
        let values = [given("x", value, None)];
        let outcomes =
            decide_conditions(&mut pool, &rules, "f", &way(&["f/rule"], &["x"]), &values).unwrap();
        outcomes[0].holds
    }

    fn compare_with(
        operator: Operator,
        limit: i64,
    ) -> impl FnOnce(&mut ExprPool, ExprId) -> ExprId {
        move |pool, x| {
            let limit = integer(pool, limit);
            apply(pool, operator, &[x, limit])
        }
    }

    #[test]
    fn greater_holds_for_a_positive_value() {
        assert_eq!(
            holds_at(Number::from(3_i64), compare_with(Operator::Greater, 0)),
            Some(true)
        );
    }

    #[test]
    fn greater_or_equal_holds_at_its_boundary() {
        assert_eq!(
            holds_at(
                Number::from(3_i64),
                compare_with(Operator::GreaterOrEqual, 3)
            ),
            Some(true)
        );
    }

    #[test]
    fn less_fails_at_its_boundary() {
        assert_eq!(
            holds_at(Number::from(3_i64), compare_with(Operator::Less, 3)),
            Some(false)
        );
    }

    #[test]
    fn less_or_equal_holds_at_its_boundary() {
        assert_eq!(
            holds_at(Number::from(3_i64), compare_with(Operator::LessOrEqual, 3)),
            Some(true)
        );
    }

    #[test]
    fn equal_holds_for_equal_values() {
        assert_eq!(
            holds_at(Number::from(3_i64), compare_with(Operator::Equal, 3)),
            Some(true)
        );
    }

    #[test]
    fn not_equal_fails_for_equal_values() {
        assert_eq!(
            holds_at(Number::from(3_i64), compare_with(Operator::NotEqual, 3)),
            Some(false)
        );
    }

    #[test]
    fn conjunction_fails_when_one_side_fails() {
        let holds = holds_at(Number::from(3_i64), |pool, x| {
            let zero = integer(pool, 0);
            let ten = integer(pool, 10);
            let positive = apply(pool, Operator::Greater, &[x, zero]);
            let large = apply(pool, Operator::Greater, &[x, ten]);
            apply(pool, Operator::And, &[positive, large])
        });

        assert_eq!(holds, Some(false));
    }

    #[test]
    fn disjunction_holds_when_one_side_holds() {
        let holds = holds_at(Number::from(3_i64), |pool, x| {
            let zero = integer(pool, 0);
            let ten = integer(pool, 10);
            let positive = apply(pool, Operator::Greater, &[x, zero]);
            let large = apply(pool, Operator::Greater, &[x, ten]);
            apply(pool, Operator::Or, &[large, positive])
        });

        assert_eq!(holds, Some(true));
    }

    #[test]
    fn negation_inverts_the_relation() {
        let holds = holds_at(Number::from(3_i64), |pool, x| {
            let relation = compare_with(Operator::Less, 0)(pool, x);
            apply(pool, Operator::Not, &[relation])
        });

        assert_eq!(holds, Some(true));
    }

    #[test]
    fn exact_values_decide_an_equality_that_intervals_cannot() {
        let holds = holds_at(Number::from(1_i64), |pool, x| {
            let three = integer(pool, 3);
            let third = pool
                .number(Number::fraction(&Integer::from(1_i64), &Integer::from(3_i64)).unwrap())
                .unwrap();
            let quotient = apply(pool, Operator::Div, &[x, three]);
            apply(pool, Operator::Equal, &[quotient, third])
        });

        assert_eq!(holds, Some(true));
    }

    #[test]
    fn exact_values_decide_a_relation_between_algebraic_numbers() {
        let holds = holds_at(Number::from(2_i64), |pool, x| {
            let root = apply(pool, Operator::Sqrt, &[x]);
            let close = pool.number(decimal(14_142, 4)).unwrap();
            apply(pool, Operator::Greater, &[root, close])
        });

        assert_eq!(holds, Some(true));
    }

    #[test]
    fn transcendental_relation_falls_back_to_the_enclosure() {
        let holds = holds_at(Number::from(1_i64), |pool, x| {
            let sine = apply(pool, Operator::Sin, &[x]);
            let zero = integer(pool, 0);
            apply(pool, Operator::Greater, &[sine, zero])
        });

        assert_eq!(holds, Some(true));
    }

    #[test]
    fn machine_value_is_decided_by_its_enclosure() {
        assert_eq!(
            holds_at(Number::F64(0.25), compare_with(Operator::Greater, 0)),
            Some(true)
        );
    }

    #[test]
    fn machine_value_that_the_enclosure_cannot_separate_is_undecided() {
        let holds = holds_at(Number::F64(0.1), |pool, x| {
            let tenth = pool.number(decimal(1, 1)).unwrap();
            apply(pool, Operator::Equal, &[x, tenth])
        });

        assert_eq!(holds, None);
    }

    #[test]
    fn missing_value_leaves_the_condition_undecided() {
        let mut pool = ExprPool::new();
        let rules = condition_rules(&mut pool, compare_with(Operator::Greater, 0));

        let outcomes =
            decide_conditions(&mut pool, &rules, "f", &way(&["f/rule"], &["x"]), &[]).unwrap();

        assert_eq!(outcomes[0].holds, None);
    }

    #[test]
    fn condition_on_a_derived_input_uses_its_derivation() {
        let mut pool = ExprPool::new();
        let x = pool.bound(0).unwrap();
        let two = integer(&mut pool, 2);
        let zero = integer(&mut pool, 0);
        let half = apply(&mut pool, Operator::Div, &[x, two]);
        let halve = lambda(&mut pool, 1, half);
        let identity = lambda(&mut pool, 1, x);
        let small = apply(&mut pool, Operator::Less, &[x, two]);
        let is_small = lambda(&mut pool, 1, small);
        let positive = apply(&mut pool, Operator::Greater, &[x, zero]);
        let is_positive = lambda(&mut pool, 1, positive);
        let mut first = formula_rule("r/from-d", "r", &["d"], halve);
        first.conditions.push(is_positive);
        let mut second = formula_rule("f/from-r", "f", &["r"], identity);
        second.conditions.push(is_small);
        let rules = RuleSet {
            version: 1,
            quantities: vec!["d".to_string(), "r".to_string(), "f".to_string()],
            rules: vec![first, second],
        };
        let values = [given("d", Number::from(6_i64), None)];

        let outcomes = decide_conditions(
            &mut pool,
            &rules,
            "f",
            &way(&["r/from-d", "f/from-r"], &["d"]),
            &values,
        )
        .unwrap();

        let decided: Vec<(String, Option<bool>)> = outcomes
            .into_iter()
            .map(|outcome| (outcome.rule, outcome.holds))
            .collect();
        assert_eq!(
            decided,
            vec![
                ("r/from-d".to_string(), Some(true)),
                ("f/from-r".to_string(), Some(false))
            ]
        );
    }

    #[test]
    fn conditions_of_a_derivation_are_decided_without_a_way() {
        let mut pool = ExprPool::new();
        let rules = condition_rules(&mut pool, compare_with(Operator::Greater, 0));
        let values = [given("x", Number::from(2_i64), None)];

        let outcomes =
            decide_derivation_conditions(&mut pool, &rules, "f", &["f/rule".to_string()], &values)
                .unwrap();

        assert_eq!(outcomes[0].holds, Some(true));
    }

    #[test]
    fn condition_with_the_wrong_number_of_parameters_is_an_error() {
        let mut pool = ExprPool::new();
        let mut rules = condition_rules(&mut pool, compare_with(Operator::Greater, 0));
        let x = pool.bound(0).unwrap();
        let two_parameters = lambda(&mut pool, 2, x);
        rules.rules[0].conditions[0] = two_parameters;
        let values = [given("x", Number::from(1_i64), None)];

        assert_eq!(
            decide_conditions(&mut pool, &rules, "f", &way(&["f/rule"], &["x"]), &values),
            Err(PhaseTwoError::Search(SearchError::FormulaArityMismatch(
                "f/rule".to_string()
            )))
        );
    }

    fn triangle_bound(pool: &mut ExprPool) -> BoundRule {
        let a = pool.bound(1).unwrap();
        let b = pool.bound(0).unwrap();
        let two = integer(pool, 2);
        let zero = integer(pool, 0);
        let product = apply(pool, Operator::Mul, &[a, b]);
        let half = apply(pool, Operator::Div, &[product, two]);
        let formula = lambda(pool, 2, half);
        let a_positive = apply(pool, Operator::Greater, &[a, zero]);
        let b_positive = apply(pool, Operator::Greater, &[b, zero]);
        let both = apply(pool, Operator::And, &[a_positive, b_positive]);
        let condition = lambda(pool, 2, both);
        BoundRule {
            identifier: "triangle-area/two-sides".to_string(),
            role: "triangle.area".to_string(),
            inputs: vec!["triangle.side-a".to_string(), "triangle.side-b".to_string()],
            kind: BoundKind::AtMost,
            formulas: vec![formula],
            conditions: vec![condition],
            sources: vec!["openstax-algebra-and-trigonometry-2e".to_string()],
        }
    }

    fn sides(a: i64, b: i64) -> Vec<GivenValue> {
        vec![
            given("triangle.side-a", Number::from(a), None),
            given("triangle.side-b", Number::from(b), None),
        ]
    }

    #[test]
    fn two_sides_bound_the_triangle_area_by_half_their_product() {
        let mut pool = ExprPool::new();
        let bound = triangle_bound(&mut pool);

        let found = not_reached_bounds(&mut pool, "triangle.area", &[bound], &sides(5, 7)).unwrap();

        assert_eq!(
            found,
            Some(Bounds {
                lower: None,
                upper: Some(BoundEnd {
                    value: Number::fraction(&Integer::from(35_i64), &Integer::from(2_i64)).unwrap(),
                    inclusive: true
                }),
                sources: vec!["openstax-algebra-and-trigonometry-2e".to_string()],
                from: vec![BoundOrigin {
                    bound: "triangle-area/two-sides".to_string(),
                    kind: BoundKind::AtMost,
                    inputs: vec!["triangle.side-a".to_string(), "triangle.side-b".to_string()],
                    ends: vec![BoundSide::Upper]
                }]
            })
        );
    }

    #[test]
    fn bound_holds_for_the_stated_values_and_ignores_their_uncertainty() {
        let mut pool = ExprPool::new();
        let bound = triangle_bound(&mut pool);
        let values = vec![
            given("triangle.side-a", Number::from(5_i64), Some(decimal(1, 1))),
            given("triangle.side-b", Number::from(7_i64), Some(decimal(1, 1))),
        ];

        let found = not_reached_bounds(&mut pool, "triangle.area", &[bound], &values)
            .unwrap()
            .unwrap();

        assert_eq!(
            found.upper.unwrap().value,
            Number::fraction(&Integer::from(35_i64), &Integer::from(2_i64)).unwrap()
        );
    }

    #[test]
    fn bound_whose_condition_fails_is_not_used() {
        let mut pool = ExprPool::new();
        let bound = triangle_bound(&mut pool);

        let found =
            not_reached_bounds(&mut pool, "triangle.area", &[bound], &sides(-5, 7)).unwrap();

        assert_eq!(found, None);
    }

    #[test]
    fn bound_with_an_input_without_value_is_not_used() {
        let mut pool = ExprPool::new();
        let bound = triangle_bound(&mut pool);
        let values = vec![given("triangle.side-a", Number::from(5_i64), None)];

        let found = not_reached_bounds(&mut pool, "triangle.area", &[bound], &values).unwrap();

        assert_eq!(found, None);
    }

    #[test]
    fn bound_for_another_role_is_not_used() {
        let mut pool = ExprPool::new();
        let bound = triangle_bound(&mut pool);

        let found =
            not_reached_bounds(&mut pool, "triangle.perimeter", &[bound], &sides(5, 7)).unwrap();

        assert_eq!(found, None);
    }

    #[test]
    fn irrational_bound_end_is_the_outward_end_of_its_enclosure() {
        let mut pool = ExprPool::new();
        let pi = pool.symbol(BuiltinConstant::Pi.symbol()).unwrap();
        let x = pool.bound(0).unwrap();
        let product = apply(&mut pool, Operator::Mul, &[pi, x]);
        let formula = lambda(&mut pool, 1, product);
        let bound = BoundRule {
            identifier: "q/at-most-pi-x".to_string(),
            role: "q".to_string(),
            inputs: vec!["x".to_string()],
            kind: BoundKind::AtMost,
            formulas: vec![formula],
            conditions: Vec::new(),
            sources: Vec::new(),
        };
        let values = vec![given("x", Number::from(1_i64), None)];

        let found = not_reached_bounds(&mut pool, "q", &[bound], &values)
            .unwrap()
            .unwrap();

        let upper = found.upper.unwrap();
        assert!(upper.inclusive);
        assert!(within(
            &upper.value,
            &decimal(31_415_926_535_897_933, 16),
            &decimal(3_141_592_653_589_800, 15)
        ));
    }

    fn constant_bound(
        identifier: &str,
        kind: BoundKind,
        ends: &[i64],
        pool: &mut ExprPool,
    ) -> BoundRule {
        let formulas = ends
            .iter()
            .map(|end| {
                let value = integer(pool, *end);
                lambda(pool, 1, value)
            })
            .collect();
        BoundRule {
            identifier: identifier.to_string(),
            role: "q".to_string(),
            inputs: vec!["x".to_string()],
            kind,
            formulas,
            conditions: Vec::new(),
            sources: vec![identifier.to_string()],
        }
    }

    #[test]
    fn tightest_ends_are_kept_and_exclusive_wins_a_tie() {
        let mut pool = ExprPool::new();
        let rules = vec![
            constant_bound("b/at-most-ten", BoundKind::AtMost, &[10], &mut pool),
            constant_bound("a/between", BoundKind::StrictlyBetween, &[1, 10], &mut pool),
            constant_bound("c/at-least-zero", BoundKind::AtLeast, &[0], &mut pool),
        ];
        let values = vec![given("x", Number::from(1_i64), None)];

        let found = not_reached_bounds(&mut pool, "q", &rules, &values)
            .unwrap()
            .unwrap();

        assert_eq!(
            found,
            Bounds {
                lower: Some(BoundEnd {
                    value: Number::from(1_i64),
                    inclusive: false
                }),
                upper: Some(BoundEnd {
                    value: Number::from(10_i64),
                    inclusive: false
                }),
                sources: vec!["a/between".to_string()],
                from: vec![BoundOrigin {
                    bound: "a/between".to_string(),
                    kind: BoundKind::StrictlyBetween,
                    inputs: vec!["x".to_string()],
                    ends: vec![BoundSide::Lower, BoundSide::Upper]
                }]
            }
        );
    }

    #[test]
    fn two_bounds_giving_the_same_kept_end_are_both_listed() {
        let mut pool = ExprPool::new();
        let rules = vec![
            constant_bound("b/at-most-ten", BoundKind::AtMost, &[10], &mut pool),
            constant_bound("a/at-most-ten", BoundKind::AtMost, &[10], &mut pool),
        ];
        let values = vec![given("x", Number::from(1_i64), None)];

        let found = not_reached_bounds(&mut pool, "q", &rules, &values)
            .unwrap()
            .unwrap();

        assert_eq!(
            (
                found
                    .from
                    .iter()
                    .map(|origin| origin.bound.clone())
                    .collect::<Vec<_>>(),
                found.sources
            ),
            (
                vec!["a/at-most-ten".to_string(), "b/at-most-ten".to_string()],
                vec!["a/at-most-ten".to_string(), "b/at-most-ten".to_string()]
            )
        );
    }

    #[test]
    fn bound_giving_no_kept_end_is_not_listed_in_from() {
        let mut pool = ExprPool::new();
        let rules = vec![
            constant_bound("a/at-most-ten", BoundKind::AtMost, &[10], &mut pool),
            constant_bound("b/at-most-twenty", BoundKind::AtMost, &[20], &mut pool),
        ];
        let values = vec![given("x", Number::from(1_i64), None)];

        let found = not_reached_bounds(&mut pool, "q", &rules, &values)
            .unwrap()
            .unwrap();

        assert_eq!(
            found
                .from
                .iter()
                .map(|origin| origin.bound.clone())
                .collect::<Vec<_>>(),
            vec!["a/at-most-ten".to_string()]
        );
    }

    fn triangle_rules(pool: &mut ExprPool) -> RuleSet {
        let a = pool.bound(2).unwrap();
        let b = pool.bound(1).unwrap();
        let g = pool.bound(0).unwrap();
        let zero = integer(pool, 0);
        let pi = pool.symbol(BuiltinConstant::Pi.symbol()).unwrap();
        let product = apply(pool, Operator::Mul, &[a, b]);
        let sine = apply(pool, Operator::Sin, &[g]);
        let area = apply(pool, Operator::Mul, &[product, sine]);
        let formula = lambda(pool, 3, area);
        let a_positive = apply(pool, Operator::Greater, &[a, zero]);
        let b_positive = apply(pool, Operator::Greater, &[b, zero]);
        let g_positive = apply(pool, Operator::Greater, &[g, zero]);
        let g_below_pi = apply(pool, Operator::Less, &[g, pi]);
        let sides = apply(pool, Operator::And, &[a_positive, b_positive]);
        let with_angle = apply(pool, Operator::And, &[sides, g_positive]);
        let all = apply(pool, Operator::And, &[with_angle, g_below_pi]);
        let condition = lambda(pool, 3, all);
        let x = pool.bound(0).unwrap();
        let two = integer(pool, 2);
        let doubled = apply(pool, Operator::Mul, &[two, x]);
        let double = lambda(pool, 1, doubled);
        let mut area_rule = formula_rule(
            "triangle-area/two-sides-angle",
            "area",
            &["side-a", "side-b", "angle-gamma"],
            formula,
        );
        area_rule.conditions.push(condition);
        RuleSet {
            version: 40,
            quantities: ["side-a", "side-b", "angle-gamma", "area", "half-angle"]
                .iter()
                .map(|name| name.to_string())
                .collect(),
            rules: vec![
                area_rule,
                formula_rule(
                    "triangle-angle/from-half",
                    "angle-gamma",
                    &["half-angle"],
                    double,
                ),
            ],
        }
    }

    fn further(
        pool: &mut ExprPool,
        rules: &RuleSet,
        a: i64,
        obtainable: &[&str],
        add: &[&str],
    ) -> FurtherInputEntry {
        let values = vec![
            given("side-a", Number::from(a), None),
            given("side-b", Number::from(4_i64), None),
        ];
        let obtainable: Vec<String> = obtainable.iter().map(|name| name.to_string()).collect();
        let add: Vec<String> = add.iter().map(|name| name.to_string()).collect();
        further_input_entries(pool, rules, "area", &values, &obtainable, &[add])
            .unwrap()
            .remove(0)
    }

    #[test]
    fn further_input_condition_with_an_unknown_angle_is_undecided() {
        let mut pool = ExprPool::new();
        let rules = triangle_rules(&mut pool);

        let entry = further(&mut pool, &rules, 3, &[], &["angle-gamma"]);

        assert_eq!(
            (
                entry.rule,
                entry.derivation,
                entry
                    .conditions
                    .iter()
                    .map(|condition| condition.holds)
                    .collect::<Vec<_>>()
            ),
            (
                "triangle-area/two-sides-angle".to_string(),
                vec!["triangle-area/two-sides-angle".to_string()],
                vec![None]
            )
        );
    }

    #[test]
    fn further_input_condition_that_the_given_already_breaks_is_false() {
        let mut pool = ExprPool::new();
        let rules = triangle_rules(&mut pool);

        let entry = further(&mut pool, &rules, -3, &[], &["angle-gamma"]);

        assert_eq!(entry.conditions[0].holds, Some(false));
    }

    #[test]
    fn further_input_derivation_is_staged_after_the_obtainable_roles() {
        let mut pool = ExprPool::new();
        let rules = triangle_rules(&mut pool);

        let entry = further(&mut pool, &rules, 3, &["half-angle"], &["angle-gamma"]);

        assert_eq!(
            entry.derivation,
            vec![
                "triangle-angle/from-half".to_string(),
                "triangle-area/two-sides-angle".to_string()
            ]
        );
    }

    #[test]
    fn further_input_that_does_not_reach_the_wanted_is_an_error() {
        let mut pool = ExprPool::new();
        let rules = triangle_rules(&mut pool);
        let values = vec![given("side-a", Number::from(3_i64), None)];

        assert_eq!(
            further_input_entries(
                &mut pool,
                &rules,
                "area",
                &values,
                &[],
                &[vec!["angle-gamma".to_string()]]
            ),
            Err(PhaseTwoError::FurtherInputDoesNotReach(vec![
                "angle-gamma".to_string()
            ]))
        );
    }

    #[test]
    fn conjunction_with_a_failing_known_part_is_false_despite_an_unknown_part() {
        let mut pool = ExprPool::new();
        let rules = condition_rules(&mut pool, |pool, x| {
            let zero = integer(pool, 0);
            let known = apply(pool, Operator::Less, &[x, zero]);
            let unknown_symbol = pool
                .intern_symbol("unknown", calc_expr::SymbolKind::Variable)
                .unwrap();
            let unknown = pool.symbol(unknown_symbol).unwrap();
            let open = apply(pool, Operator::Greater, &[unknown, zero]);
            apply(pool, Operator::And, &[open, known])
        });
        let values = [given("x", Number::from(3_i64), None)];

        let outcomes =
            decide_conditions(&mut pool, &rules, "f", &way(&["f/rule"], &["x"]), &values).unwrap();

        assert_eq!(outcomes[0].holds, Some(false));
    }

    #[test]
    fn bound_with_the_wrong_number_of_formulas_is_an_error() {
        let mut pool = ExprPool::new();
        let rule = constant_bound("q/between", BoundKind::StrictlyBetween, &[1], &mut pool);
        let values = vec![given("x", Number::from(1_i64), None)];

        assert_eq!(
            not_reached_bounds(&mut pool, "q", &[rule], &values),
            Err(PhaseTwoError::BoundFormulaCount("q/between".to_string()))
        );
    }
}
