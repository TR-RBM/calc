use std::collections::HashMap;

use calc_exec::{Backend, BackendKind, Preference, SkippedBackend};
use calc_expr::{
    AccessError, BuildError, BuiltinConstant, ExprId, ExprPool, Head, NodeView, Operator, SymbolId,
    substitute_symbols,
};
use calc_numbers::{
    Integer, Number, acos_f64, asin_f64, atan_f64, atan2_f64, cos_f64, exp_f64, ln_f64,
    maximum_f64, minimum_f64, pow_f64, sin_f64, tan_f64,
};

use crate::computed_result::{
    ComputedResult, ComputedResultError, EXACT_EVALUATION_METHOD, Method, ParameterValue,
    ResultKind, ResultValue, Uncertainty, WAY_EVALUATION_METHOD,
};
use crate::exact_evaluation::evaluate_exact;
use crate::machine_evaluation::{MachineEvaluationError, evaluate_f64};
use crate::phase_two::GivenValue;
use crate::quantities::{QuantityError, to_coherent_units};
use crate::rule_search::{RuleSet, SearchError, Way, derivation_expression};

const PROPAGATION_PARAMETER: &str = "uncertainty_propagation";
const FIRST_ORDER: &str = "first_order_taylor";
const CORRELATION_PARAMETER: &str = "input_correlation";
const UNCORRELATED: &str = "uncorrelated_distinct_inputs";
const LINEARITY_PARAMETER: &str = "assumes_linear_within_uncertainty";
const LINEARITY_VERIFIED_PARAMETER: &str = "linearity_verified";
const INPUT_COUNT_PARAMETER: &str = "uncertain_input_count";
const RULE_PARAMETER: &str = "rule";
const CONCEPT_SET_PARAMETER: &str = "concept_set_version";
const VALUE_METHOD_PARAMETER: &str = "value_method";
const STANDARD_UNCERTAINTY_BLOCK: &str = "P-GRD-D-038";
const EXPANDED_UNCERTAINTY_BLOCK: &str = "P-GRD-D-039";

#[derive(Clone, Debug, PartialEq)]
pub enum UncertaintyError {
    Access(AccessError),
    Build(BuildError),
    Quantity(QuantityError),
    NestedUncertainty(ExprId),
    NegativeUncertainty(ExprId),
    CoverageFactorBelowOne(ExprId),
    UnboundSymbol(SymbolId),
    UnsupportedNode(ExprId),
    UnsupportedOperator(Operator),
    NotFinite(ExprId),
    Result(ComputedResultError),
}

#[derive(Clone, Debug, PartialEq)]
pub enum WayEvaluationError {
    Search(SearchError),
    MissingValue(String),
    Build(BuildError),
    Substitution(ExprId),
    Machine(MachineEvaluationError),
    Uncertainty(UncertaintyError),
    Result(ComputedResultError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WayEvaluation {
    pub result: ComputedResult,
    pub backend: Option<BackendKind>,
    pub skipped: Vec<SkippedBackend>,
    pub approximate_operations: bool,
}

#[derive(Clone, Debug)]
struct Sensitive {
    value: f64,
    gradient: Vec<f64>,
}

#[derive(Clone, Debug)]
enum Evaluated {
    Real(Sensitive),
    Truth(bool),
}

struct UncertainInput {
    node: ExprId,
    standard: f64,
}

struct Propagation<'pool> {
    pool: &'pool ExprPool,
    inputs: Vec<UncertainInput>,
    positions: HashMap<ExprId, usize>,
    memo: HashMap<ExprId, Evaluated>,
}

fn post_order(pool: &ExprPool, root: ExprId) -> Result<Vec<ExprId>, AccessError> {
    let mut order = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![(root, false)];
    while let Some((node, expanded)) = stack.pop() {
        if expanded {
            order.push(node);
            continue;
        }
        if !visited.insert(node) {
            continue;
        }
        stack.push((node, true));
        let children: Vec<ExprId> = match pool.node(node)? {
            NodeView::Apply {
                head: Head::Operator(Operator::Uncertain | Operator::UncertainExpanded),
                arguments,
            } => arguments
                .split_last()
                .map_or_else(Vec::new, |(_, written)| written.to_vec()),
            NodeView::Apply { arguments, .. } => arguments.to_vec(),
            NodeView::Bind {
                arguments, body, ..
            } => arguments.iter().copied().chain([body]).collect(),
            NodeView::Quantity { value, .. } => vec![value],
            NodeView::Array { elements, .. } => elements.to_vec(),
            NodeView::Number(_) | NodeView::Symbol(_) | NodeView::Bound(_) => Vec::new(),
        };
        for child in children.into_iter().rev() {
            stack.push((child, false));
        }
    }
    Ok(order)
}

fn is_uncertain(pool: &ExprPool, node: ExprId) -> Result<Option<Measured>, AccessError> {
    Ok(match pool.node(node)? {
        NodeView::Apply {
            head: Head::Operator(operator @ (Operator::Uncertain | Operator::UncertainExpanded)),
            arguments,
        } => arguments.split_last().map(|(_, written)| Measured {
            operator,
            arguments: written.to_vec(),
        }),
        _ => None,
    })
}

struct Measured {
    operator: Operator,
    arguments: Vec<ExprId>,
}

impl Measured {
    fn value(&self) -> Option<ExprId> {
        self.arguments.first().copied()
    }

    fn uncertainty(&self) -> Option<ExprId> {
        self.arguments.get(1).copied()
    }

    fn coverage_factor(&self) -> Option<ExprId> {
        match self.operator {
            Operator::UncertainExpanded => self.arguments.get(2).copied(),
            _ => None,
        }
    }
}

pub fn has_uncertain_inputs(pool: &ExprPool, expression: ExprId) -> Result<bool, AccessError> {
    for node in post_order(pool, expression)? {
        if is_uncertain(pool, node)?.is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn central_expression(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Result<ExprId, UncertaintyError> {
    let order = post_order(pool, expression).map_err(UncertaintyError::Access)?;
    let mut rebuilt: HashMap<ExprId, ExprId> = HashMap::new();
    for node in order {
        let mapped = |child: &ExprId, rebuilt: &HashMap<ExprId, ExprId>| {
            rebuilt.get(child).copied().unwrap_or(*child)
        };
        let view = pool.node(node).map_err(UncertaintyError::Access)?;
        let new_node = match view {
            NodeView::Apply {
                head: Head::Operator(Operator::Uncertain | Operator::UncertainExpanded),
                ..
            } => {
                let value = is_uncertain(pool, node)
                    .map_err(UncertaintyError::Access)?
                    .and_then(|measured| measured.value())
                    .ok_or(UncertaintyError::UnsupportedNode(node))?;
                mapped(&value, &rebuilt)
            }
            NodeView::Apply { head, arguments } => {
                let arguments: Vec<ExprId> = arguments
                    .iter()
                    .map(|child| mapped(child, &rebuilt))
                    .collect();
                pool.apply(head, &arguments)
                    .map_err(UncertaintyError::Build)?
            }
            NodeView::Quantity { value, unit } => pool
                .quantity(mapped(&value, &rebuilt), unit)
                .map_err(UncertaintyError::Build)?,
            NodeView::Array { shape, elements } => {
                let shape = shape.to_vec();
                let elements: Vec<ExprId> = elements
                    .iter()
                    .map(|child| mapped(child, &rebuilt))
                    .collect();
                pool.array(&shape, &elements)
                    .map_err(UncertaintyError::Build)?
            }
            NodeView::Bind { .. } => {
                if has_uncertain_inputs(pool, node).map_err(UncertaintyError::Access)? {
                    return Err(UncertaintyError::UnsupportedNode(node));
                }
                node
            }
            NodeView::Number(_) | NodeView::Symbol(_) | NodeView::Bound(_) => node,
        };
        rebuilt.insert(node, new_node);
    }
    Ok(rebuilt.get(&expression).copied().unwrap_or(expression))
}

fn real(value: f64, gradient: Vec<f64>) -> Evaluated {
    Evaluated::Real(Sensitive { value, gradient })
}

fn combine(left: &[f64], right: &[f64], operation: impl Fn(f64, f64) -> f64) -> Vec<f64> {
    left.iter()
        .zip(right)
        .map(|(left, right)| operation(*left, *right))
        .collect()
}

fn scale(gradient: &[f64], factor: f64) -> Vec<f64> {
    gradient.iter().map(|entry| entry * factor).collect()
}

fn sign(value: f64) -> f64 {
    if value.is_sign_negative() { -1.0 } else { 1.0 }
}

impl Propagation<'_> {
    fn width(&self) -> usize {
        self.inputs.len()
    }

    fn zero_gradient(&self) -> Vec<f64> {
        vec![0.0; self.width()]
    }

    fn register_inputs(&mut self, root: ExprId) -> Result<(), UncertaintyError> {
        for node in post_order(self.pool, root).map_err(UncertaintyError::Access)? {
            let Some(measured) = is_uncertain(self.pool, node).map_err(UncertaintyError::Access)?
            else {
                continue;
            };
            for argument in &measured.arguments {
                if has_uncertain_inputs(self.pool, *argument).map_err(UncertaintyError::Access)? {
                    return Err(UncertaintyError::NestedUncertainty(node));
                }
            }
            let position = self.inputs.len();
            self.positions.insert(node, position);
            self.inputs.push(UncertainInput {
                node,
                standard: 0.0,
            });
        }
        let nodes: Vec<(usize, ExprId)> = self
            .inputs
            .iter()
            .enumerate()
            .map(|(position, input)| (position, input.node))
            .collect();
        for (position, node) in nodes {
            let measured = is_uncertain(self.pool, node)
                .map_err(UncertaintyError::Access)?
                .ok_or(UncertaintyError::UnsupportedNode(node))?;
            let uncertainty = match measured.uncertainty() {
                Some(argument) => self.plain_value(argument)?,
                None => return Err(UncertaintyError::UnsupportedNode(node)),
            };
            if uncertainty < 0.0 {
                return Err(UncertaintyError::NegativeUncertainty(node));
            }
            let coverage_factor = match measured.coverage_factor() {
                Some(argument) => self.plain_value(argument)?,
                None => 1.0,
            };
            if coverage_factor < 1.0 || coverage_factor.is_nan() {
                return Err(UncertaintyError::CoverageFactorBelowOne(node));
            }
            if let Some(input) = self.inputs.get_mut(position) {
                input.standard = uncertainty / coverage_factor;
            }
        }
        Ok(())
    }

    fn plain_value(&mut self, expression: ExprId) -> Result<f64, UncertaintyError> {
        match self.evaluate(expression)? {
            Evaluated::Real(sensitive) => Ok(sensitive.value),
            Evaluated::Truth(_) => Err(UncertaintyError::UnsupportedNode(expression)),
        }
    }

    fn evaluate(&mut self, root: ExprId) -> Result<Evaluated, UncertaintyError> {
        for node in post_order(self.pool, root).map_err(UncertaintyError::Access)? {
            if self.memo.contains_key(&node) {
                continue;
            }
            let evaluated = self.evaluate_node(node)?;
            self.memo.insert(node, evaluated);
        }
        self.memo
            .get(&root)
            .cloned()
            .ok_or(UncertaintyError::UnsupportedNode(root))
    }

    fn child(&self, node: ExprId) -> Result<Evaluated, UncertaintyError> {
        self.memo
            .get(&node)
            .cloned()
            .ok_or(UncertaintyError::UnsupportedNode(node))
    }

    fn evaluate_node(&mut self, node: ExprId) -> Result<Evaluated, UncertaintyError> {
        match self.pool.node(node).map_err(UncertaintyError::Access)? {
            NodeView::Number(number) => {
                let value = self
                    .pool
                    .number_value(number)
                    .map_err(UncertaintyError::Access)?
                    .round_to_f64_ties_even();
                Ok(real(value, self.zero_gradient()))
            }
            NodeView::Symbol(symbol) => {
                let value = if symbol == BuiltinConstant::Pi.symbol() {
                    std::f64::consts::PI
                } else if symbol == BuiltinConstant::E.symbol() {
                    std::f64::consts::E
                } else {
                    return Err(UncertaintyError::UnboundSymbol(symbol));
                };
                Ok(real(value, self.zero_gradient()))
            }
            NodeView::Apply {
                head: Head::Operator(operator),
                arguments,
            } => {
                let arguments = arguments.to_vec();
                if let Some(position) = self.positions.get(&node).copied() {
                    let value = match arguments.first() {
                        Some(argument) => match self.child(*argument)? {
                            Evaluated::Real(sensitive) => sensitive.value,
                            Evaluated::Truth(_) => {
                                return Err(UncertaintyError::UnsupportedNode(node));
                            }
                        },
                        None => return Err(UncertaintyError::UnsupportedNode(node)),
                    };
                    let mut gradient = self.zero_gradient();
                    if let Some(entry) = gradient.get_mut(position) {
                        *entry = 1.0;
                    }
                    return Ok(real(value, gradient));
                }
                let values = arguments
                    .iter()
                    .map(|argument| self.child(*argument))
                    .collect::<Result<Vec<_>, _>>()?;
                self.apply(operator, &values)
            }
            _ => Err(UncertaintyError::UnsupportedNode(node)),
        }
    }

    fn apply(
        &self,
        operator: Operator,
        values: &[Evaluated],
    ) -> Result<Evaluated, UncertaintyError> {
        let reals: Vec<&Sensitive> = values
            .iter()
            .filter_map(|value| match value {
                Evaluated::Real(sensitive) => Some(sensitive),
                Evaluated::Truth(_) => None,
            })
            .collect();
        let truths: Vec<bool> = values
            .iter()
            .filter_map(|value| match value {
                Evaluated::Truth(truth) => Some(*truth),
                Evaluated::Real(_) => None,
            })
            .collect();
        let unsupported = Err(UncertaintyError::UnsupportedOperator(operator));
        let zero = self.zero_gradient();
        Ok(match (operator, reals.as_slice(), truths.as_slice()) {
            (Operator::Add, [a, b], []) => real(
                a.value + b.value,
                combine(&a.gradient, &b.gradient, |x, y| x + y),
            ),
            (Operator::Sub, [a, b], []) => real(
                a.value - b.value,
                combine(&a.gradient, &b.gradient, |x, y| x - y),
            ),
            (Operator::Mul, [a, b], []) => real(
                a.value * b.value,
                combine(
                    &scale(&a.gradient, b.value),
                    &scale(&b.gradient, a.value),
                    |x, y| x + y,
                ),
            ),
            (Operator::Div, [a, b], []) => {
                let quotient = a.value / b.value;
                real(
                    quotient,
                    combine(&a.gradient, &b.gradient, |x, y| {
                        (x - y * quotient) / b.value
                    }),
                )
            }
            (Operator::Neg, [a], []) => real(-a.value, scale(&a.gradient, -1.0)),
            (Operator::MulAdd, [a, b, c], []) => {
                let product = combine(
                    &scale(&a.gradient, b.value),
                    &scale(&b.gradient, a.value),
                    |x, y| x + y,
                );
                real(
                    a.value.mul_add(b.value, c.value),
                    combine(&product, &c.gradient, |x, y| x + y),
                )
            }
            (Operator::Sqrt, [a], []) => {
                let root = a.value.sqrt();
                real(root, scale(&a.gradient, 1.0 / (2.0 * root)))
            }
            (Operator::Pow, [base, exponent], []) => {
                let power = pow_f64(base.value, exponent.value);
                let base_factor = exponent.value * pow_f64(base.value, exponent.value - 1.0);
                let has_exponent_gradient = exponent.gradient.iter().any(|entry| *entry != 0.0);
                let exponent_factor = if has_exponent_gradient {
                    ln_f64(base.value) * power
                } else {
                    0.0
                };
                real(
                    power,
                    combine(
                        &scale(&base.gradient, base_factor),
                        &scale(&exponent.gradient, exponent_factor),
                        |x, y| x + y,
                    ),
                )
            }
            (Operator::Exp, [a], []) => {
                let value = exp_f64(a.value);
                real(value, scale(&a.gradient, value))
            }
            (Operator::Ln, [a], []) => real(ln_f64(a.value), scale(&a.gradient, 1.0 / a.value)),
            (Operator::Sin, [a], []) => {
                real(sin_f64(a.value), scale(&a.gradient, cos_f64(a.value)))
            }
            (Operator::Cos, [a], []) => {
                real(cos_f64(a.value), scale(&a.gradient, -sin_f64(a.value)))
            }
            (Operator::Tan, [a], []) => {
                let value = tan_f64(a.value);
                real(value, scale(&a.gradient, 1.0 + value * value))
            }
            (Operator::Asin, [a], []) => real(
                asin_f64(a.value),
                scale(&a.gradient, 1.0 / (1.0 - a.value * a.value).sqrt()),
            ),
            (Operator::Acos, [a], []) => real(
                acos_f64(a.value),
                scale(&a.gradient, -1.0 / (1.0 - a.value * a.value).sqrt()),
            ),
            (Operator::Atan, [a], []) => real(
                atan_f64(a.value),
                scale(&a.gradient, 1.0 / (1.0 + a.value * a.value)),
            ),
            (Operator::Atan2, [y, x], []) => {
                let squared = x.value * x.value + y.value * y.value;
                real(
                    atan2_f64(y.value, x.value),
                    combine(&y.gradient, &x.gradient, |dy, dx| {
                        (x.value * dy - y.value * dx) / squared
                    }),
                )
            }
            (Operator::Abs, [a], []) => real(a.value.abs(), scale(&a.gradient, sign(a.value))),
            (Operator::Min, [a, b], []) => {
                let value = minimum_f64(a.value, b.value);
                if value.to_bits() == a.value.to_bits() {
                    real(value, a.gradient.clone())
                } else {
                    real(value, b.gradient.clone())
                }
            }
            (Operator::Max, [a, b], []) => {
                let value = maximum_f64(a.value, b.value);
                if value.to_bits() == a.value.to_bits() {
                    real(value, a.gradient.clone())
                } else {
                    real(value, b.gradient.clone())
                }
            }
            (Operator::Floor, [a], []) => real(a.value.floor(), zero),
            (Operator::Ceil, [a], []) => real(a.value.ceil(), zero),
            (Operator::Trunc, [a], []) => real(a.value.trunc(), zero),
            (Operator::RoundTiesEven, [a], []) => real(a.value.round_ties_even(), zero),
            (Operator::CopySign, [magnitude, sign_source], []) => {
                let value = magnitude.value.copysign(sign_source.value);
                let factor = sign(magnitude.value) * sign(sign_source.value);
                real(value, scale(&magnitude.gradient, factor))
            }
            (Operator::ToF64 | Operator::ToF32 | Operator::ToExact, [a], []) => {
                real(a.value, a.gradient.clone())
            }
            (Operator::Less, [a, b], []) => Evaluated::Truth(a.value < b.value),
            (Operator::LessOrEqual, [a, b], []) => Evaluated::Truth(a.value <= b.value),
            (Operator::Greater, [a, b], []) => Evaluated::Truth(a.value > b.value),
            (Operator::GreaterOrEqual, [a, b], []) => Evaluated::Truth(a.value >= b.value),
            (Operator::Equal, [a, b], []) => Evaluated::Truth(a.value == b.value),
            (Operator::NotEqual, [a, b], []) => Evaluated::Truth(a.value != b.value),
            (Operator::And, [], [left, right]) => Evaluated::Truth(*left && *right),
            (Operator::Or, [], [left, right]) => Evaluated::Truth(*left || *right),
            (Operator::Not, [], [truth]) => Evaluated::Truth(!truth),
            (Operator::Select, [when_true, when_false], [condition]) => {
                let chosen = if *condition { when_true } else { when_false };
                real(chosen.value, chosen.gradient.clone())
            }
            _ => return unsupported,
        })
    }
}

fn propagated(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Result<Option<(f64, usize)>, UncertaintyError> {
    let coherent = to_coherent_units(pool, expression).map_err(UncertaintyError::Quantity)?;
    let mut propagation = Propagation {
        pool,
        inputs: Vec::new(),
        positions: HashMap::new(),
        memo: HashMap::new(),
    };
    propagation.register_inputs(coherent.expression)?;
    if propagation.inputs.is_empty() {
        return Ok(None);
    }
    propagation.memo.clear();
    let Evaluated::Real(result) = propagation.evaluate(coherent.expression)? else {
        return Err(UncertaintyError::UnsupportedNode(coherent.expression));
    };
    let sum_of_squares = result
        .gradient
        .iter()
        .zip(&propagation.inputs)
        .filter(|(_, input)| input.standard != 0.0)
        .map(|(sensitivity, input)| {
            let contribution = sensitivity * input.standard;
            contribution * contribution
        })
        .fold(0.0, |total, square| total + square);
    let standard = sum_of_squares.sqrt();
    if !standard.is_finite() {
        return Err(UncertaintyError::NotFinite(coherent.expression));
    }
    Ok(Some((standard, propagation.inputs.len())))
}

pub fn with_propagated_uncertainty(
    pool: &mut ExprPool,
    expression: ExprId,
    computed: ComputedResult,
) -> Result<ComputedResult, UncertaintyError> {
    let Some((standard, input_count)) = propagated(pool, expression)? else {
        return Ok(computed);
    };
    let uncertainty = Uncertainty::new(
        ResultValue::Number(Number::F64(standard)),
        Number::Integer(Integer::one()),
    )
    .map_err(UncertaintyError::Result)?;
    let mut method = computed.method().clone();
    method.parameters.insert(
        PROPAGATION_PARAMETER.to_string(),
        ParameterValue::Identifier(FIRST_ORDER.to_string()),
    );
    method.parameters.insert(
        CORRELATION_PARAMETER.to_string(),
        ParameterValue::Identifier(UNCORRELATED.to_string()),
    );
    method.parameters.insert(
        LINEARITY_PARAMETER.to_string(),
        ParameterValue::Boolean(true),
    );
    method.parameters.insert(
        LINEARITY_VERIFIED_PARAMETER.to_string(),
        ParameterValue::Boolean(false),
    );
    method.parameters.insert(
        INPUT_COUNT_PARAMETER.to_string(),
        ParameterValue::Value(ResultValue::Number(Number::from(
            i64::try_from(input_count).unwrap_or(i64::MAX),
        ))),
    );
    Ok(computed
        .with_method(method)
        .with_uncertainty(uncertainty)
        .with_corpus_reference(STANDARD_UNCERTAINTY_BLOCK)
        .with_corpus_reference(EXPANDED_UNCERTAINTY_BLOCK))
}

fn given_expression(pool: &mut ExprPool, given: &GivenValue) -> Result<ExprId, BuildError> {
    let value = pool.number(given.value.clone())?;
    match &given.standard_uncertainty {
        Some(uncertainty) => {
            let uncertainty = pool.number(uncertainty.clone())?;
            let mark = pool.measurement()?;
            pool.apply(
                Head::Operator(Operator::Uncertain),
                &[value, uncertainty, mark],
            )
        }
        None => Ok(value),
    }
}

pub fn evaluate_way(
    pool: &mut ExprPool,
    rule_set: &RuleSet,
    wanted: &str,
    way: &Way,
    given: &[GivenValue],
    backends: &[&dyn Backend],
    preference: Preference,
) -> Result<WayEvaluation, WayEvaluationError> {
    let derivation = derivation_expression(pool, rule_set, wanted, &way.derivation)
        .map_err(WayEvaluationError::Search)?;
    let mut replacements = HashMap::new();
    for leaf in &derivation.leaves {
        let value = given
            .iter()
            .find(|value| &value.quantity == leaf)
            .ok_or_else(|| WayEvaluationError::MissingValue(leaf.clone()))?;
        let symbol = pool
            .lookup_symbol(leaf)
            .ok_or_else(|| WayEvaluationError::MissingValue(leaf.clone()))?;
        let replacement = given_expression(pool, value).map_err(WayEvaluationError::Build)?;
        replacements.insert(symbol, replacement);
    }
    let expression = substitute_symbols(pool, derivation.expression, &replacements)
        .map_err(|_| WayEvaluationError::Substitution(derivation.expression))?;
    let central = central_expression(pool, expression).map_err(WayEvaluationError::Uncertainty)?;

    let mut method = Method::named(WAY_EVALUATION_METHOD);
    if let Some(rule) = &way.last_rule {
        method.parameters.insert(
            RULE_PARAMETER.to_string(),
            ParameterValue::Identifier(rule.clone()),
        );
    }
    method.parameters.insert(
        CONCEPT_SET_PARAMETER.to_string(),
        ParameterValue::Identifier(format!("{:016x}", rule_set.version)),
    );

    let exact = evaluate_exact(pool, central).ok().and_then(|evaluation| {
        evaluation
            .rational_value()
            .cloned()
            .map(|value| (value, evaluation.unit()))
    });
    let (computed, backend, skipped, approximate_operations) = match exact {
        Some((value, unit)) => {
            let mut exact_method = method.clone();
            exact_method.parameters.insert(
                VALUE_METHOD_PARAMETER.to_string(),
                ParameterValue::Identifier(EXACT_EVALUATION_METHOD.to_string()),
            );
            let computed = ComputedResult::new(
                ResultKind::ExactRational,
                ResultValue::Number(value),
                unit,
                exact_method,
            )
            .map_err(WayEvaluationError::Result)?;
            (computed, None, Vec::new(), false)
        }
        None => {
            let evaluation = evaluate_f64(pool, central, backends, preference)
                .map_err(WayEvaluationError::Machine)?;
            let mut machine_method = evaluation.result().method().clone();
            machine_method.name = WAY_EVALUATION_METHOD.to_string();
            machine_method.parameters.extend(method.parameters.clone());
            let computed = evaluation.result().clone().with_method(machine_method);
            (
                computed,
                Some(evaluation.backend()),
                evaluation.skipped().to_vec(),
                evaluation.has_approximate_operations(),
            )
        }
    };
    let computed = way
        .sources
        .iter()
        .fold(computed, |result, source| result.with_source(source));
    let result = with_propagated_uncertainty(pool, expression, computed)
        .map_err(WayEvaluationError::Uncertainty)?;
    Ok(WayEvaluation {
        result,
        backend,
        skipped,
        approximate_operations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computed_result::RoundingError;
    use crate::phase_two::evaluated_scores;
    use crate::rule_search::{ErrorBound, Exactness, Rule};
    use calc_exec_cpu::CpuBackend;
    use calc_expr::BinderKind;

    fn apply(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> ExprId {
        pool.apply(Head::Operator(operator), arguments).unwrap()
    }

    fn decimal(digits: i64, scale: u32) -> Number {
        Number::fraction(&Integer::from(digits), &Integer::from(10_i64).pow(scale)).unwrap()
    }

    fn number(pool: &mut ExprPool, value: Number) -> ExprId {
        pool.number(value).unwrap()
    }

    fn measured(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> ExprId {
        let mark = pool.measurement().unwrap();
        let mut arguments = arguments.to_vec();
        arguments.push(mark);
        apply(pool, operator, &arguments)
    }

    fn uncertain(pool: &mut ExprPool, value: Number, uncertainty: Number) -> ExprId {
        let value = number(pool, value);
        let uncertainty = number(pool, uncertainty);
        measured(pool, Operator::Uncertain, &[value, uncertainty])
    }

    fn exact_result(value: Number) -> ComputedResult {
        ComputedResult::new(
            ResultKind::ExactRational,
            ResultValue::Number(value),
            None,
            Method::named(EXACT_EVALUATION_METHOD),
        )
        .unwrap()
    }

    fn standard_uncertainty(result: &ComputedResult) -> f64 {
        match result.uncertainty().unwrap().standard() {
            ResultValue::Number(Number::F64(value)) => *value,
            other => panic!("{other:?}"),
        }
    }

    fn lambda(pool: &mut ExprPool, parameters: usize, body: ExprId) -> ExprId {
        (0..parameters).fold(body, |inner, _| {
            pool.bind(BinderKind::Lambda, &[], inner).unwrap()
        })
    }

    fn rule_set(pool: &mut ExprPool, operator: Operator, with_pi: bool) -> RuleSet {
        let left = if with_pi {
            pool.symbol(BuiltinConstant::Pi.symbol()).unwrap()
        } else {
            pool.bound(1).unwrap()
        };
        let right = pool.bound(0).unwrap();
        let body = apply(pool, operator, &[left, right]);
        let parameters = if with_pi { 1 } else { 2 };
        let formula = lambda(pool, parameters, body);
        let inputs: Vec<String> = if with_pi {
            vec!["x".to_string()]
        } else {
            vec!["x".to_string(), "y".to_string()]
        };
        RuleSet {
            version: 0x0123_4567_89ab_cdef,
            quantities: vec!["x".to_string(), "y".to_string(), "f".to_string()],
            rules: vec![Rule {
                identifier: "f/rule".to_string(),
                output: "f".to_string(),
                inputs,
                formula,
                conditions: Vec::new(),
                exact: !with_pi,
                sources: vec!["some-source".to_string()],
                corpus_references: Vec::new(),
            }],
        }
    }

    fn way(inputs: &[&str]) -> Way {
        Way {
            inputs: inputs.iter().map(|input| input.to_string()).collect(),
            steps: 1,
            derivation: vec!["f/rule".to_string()],
            last_rule: Some("f/rule".to_string()),
            obtainable: true,
            exactness: Exactness::Exact,
            error_bound: ErrorBound::Zero,
            gate_count: None,
            conditions: Vec::new(),
            sources: vec!["some-source".to_string()],
        }
    }

    fn given(quantity: &str, value: Number, uncertainty: Option<Number>) -> GivenValue {
        GivenValue {
            quantity: quantity.to_string(),
            value,
            standard_uncertainty: uncertainty,
        }
    }

    fn evaluate(
        pool: &mut ExprPool,
        rules: &RuleSet,
        inputs: &[&str],
        values: &[GivenValue],
    ) -> WayEvaluation {
        let backend = CpuBackend::new();
        evaluate_way(
            pool,
            rules,
            "f",
            &way(inputs),
            values,
            &[&backend],
            Preference::Automatic,
        )
        .unwrap()
    }

    #[test]
    fn central_expression_replaces_an_uncertain_value_by_its_value() {
        let mut pool = ExprPool::new();
        let measured = uncertain(&mut pool, decimal(25, 1), decimal(1, 2));

        let central = central_expression(&mut pool, measured).unwrap();

        assert_eq!(central, pool.number(decimal(25, 1)).unwrap());
    }

    #[test]
    fn expression_without_uncertain_input_keeps_its_result() {
        let mut pool = ExprPool::new();
        let two = number(&mut pool, Number::from(2_i64));
        let computed = exact_result(Number::from(2_i64));

        let result = with_propagated_uncertainty(&mut pool, two, computed.clone()).unwrap();

        assert_eq!(result, computed);
    }

    #[test]
    fn single_uncertain_value_keeps_its_standard_uncertainty() {
        let mut pool = ExprPool::new();
        let measured = uncertain(&mut pool, decimal(25, 1), decimal(1, 2));

        let result =
            with_propagated_uncertainty(&mut pool, measured, exact_result(decimal(25, 1))).unwrap();

        assert_eq!(standard_uncertainty(&result).to_bits(), 0.01_f64.to_bits());
    }

    #[test]
    fn expanded_uncertainty_is_divided_by_its_coverage_factor() {
        let mut pool = ExprPool::new();
        let value = number(&mut pool, Number::from(4_i64));
        let expanded = number(&mut pool, decimal(2, 1));
        let coverage = number(&mut pool, Number::from(2_i64));
        let measured = measured(
            &mut pool,
            Operator::UncertainExpanded,
            &[value, expanded, coverage],
        );

        let result =
            with_propagated_uncertainty(&mut pool, measured, exact_result(Number::from(4_i64)))
                .unwrap();

        assert_eq!(standard_uncertainty(&result).to_bits(), 0.1_f64.to_bits());
    }

    #[test]
    fn result_carries_standard_uncertainty_with_coverage_factor_one() {
        let mut pool = ExprPool::new();
        let measured = uncertain(&mut pool, decimal(25, 1), decimal(1, 2));

        let result =
            with_propagated_uncertainty(&mut pool, measured, exact_result(decimal(25, 1))).unwrap();

        assert_eq!(
            result.uncertainty().unwrap().coverage_factor(),
            &Number::from(1_i64)
        );
    }

    #[test]
    fn independent_uncertainties_add_in_quadrature() {
        let mut pool = ExprPool::new();
        let mass = uncertain(&mut pool, decimal(25, 1), decimal(1, 2));
        let acceleration = uncertain(&mut pool, decimal(981, 2), decimal(2, 2));
        let force = apply(&mut pool, Operator::Mul, &[mass, acceleration]);

        let result =
            with_propagated_uncertainty(&mut pool, force, exact_result(decimal(24_525, 3)))
                .unwrap();

        let first = 9.81_f64 * 0.01;
        let second = 2.5_f64 * 0.02;
        let expected = (first * first + second * second).sqrt();
        assert_eq!(standard_uncertainty(&result).to_bits(), expected.to_bits());
    }

    #[test]
    fn one_uncertain_value_used_twice_is_one_input() {
        let mut pool = ExprPool::new();
        let measured = uncertain(&mut pool, Number::from(2_i64), decimal(1, 1));
        let square = apply(&mut pool, Operator::Mul, &[measured, measured]);

        let result =
            with_propagated_uncertainty(&mut pool, square, exact_result(Number::from(4_i64)))
                .unwrap();

        assert_eq!(
            standard_uncertainty(&result).to_bits(),
            (4.0_f64 * 0.1).to_bits()
        );
    }

    #[test]
    fn minimum_follows_the_sensitivity_of_the_smaller_argument() {
        let mut pool = ExprPool::new();
        let small = uncertain(&mut pool, Number::from(1_i64), decimal(1, 1));
        let large = uncertain(&mut pool, Number::from(5_i64), decimal(3, 1));
        let minimum = apply(&mut pool, Operator::Min, &[large, small]);

        let result =
            with_propagated_uncertainty(&mut pool, minimum, exact_result(Number::from(1_i64)))
                .unwrap();

        assert_eq!(standard_uncertainty(&result).to_bits(), 0.1_f64.to_bits());
    }

    #[test]
    fn method_parameters_state_the_propagation_and_its_conditions() {
        let mut pool = ExprPool::new();
        let measured = uncertain(&mut pool, decimal(25, 1), decimal(1, 2));

        let result =
            with_propagated_uncertainty(&mut pool, measured, exact_result(decimal(25, 1))).unwrap();

        let parameters = &result.method().parameters;
        assert_eq!(
            parameters.get(PROPAGATION_PARAMETER),
            Some(&ParameterValue::Identifier(FIRST_ORDER.to_string()))
        );
        assert_eq!(
            parameters.get(CORRELATION_PARAMETER),
            Some(&ParameterValue::Identifier(UNCORRELATED.to_string()))
        );
        assert_eq!(
            parameters.get(LINEARITY_PARAMETER),
            Some(&ParameterValue::Boolean(true))
        );
        assert_eq!(
            parameters.get(LINEARITY_VERIFIED_PARAMETER),
            Some(&ParameterValue::Boolean(false))
        );
        assert_eq!(
            parameters.get(INPUT_COUNT_PARAMETER),
            Some(&ParameterValue::Value(ResultValue::Number(Number::from(
                1_i64
            ))))
        );
    }

    #[test]
    fn result_cites_the_uncertainty_blocks() {
        let mut pool = ExprPool::new();
        let measured = uncertain(&mut pool, decimal(25, 1), decimal(1, 2));

        let result =
            with_propagated_uncertainty(&mut pool, measured, exact_result(decimal(25, 1))).unwrap();

        let references: Vec<&str> = result.corpus_references().collect();
        assert_eq!(
            references,
            vec![STANDARD_UNCERTAINTY_BLOCK, EXPANDED_UNCERTAINTY_BLOCK]
        );
    }

    #[test]
    fn negative_uncertainty_is_an_error() {
        let mut pool = ExprPool::new();
        let measured = uncertain(&mut pool, Number::from(2_i64), decimal(-1, 1));

        let result =
            with_propagated_uncertainty(&mut pool, measured, exact_result(Number::from(2_i64)));

        assert_eq!(result, Err(UncertaintyError::NegativeUncertainty(measured)));
    }

    #[test]
    fn coverage_factor_below_one_is_an_error() {
        let mut pool = ExprPool::new();
        let value = number(&mut pool, Number::from(4_i64));
        let expanded = number(&mut pool, decimal(2, 1));
        let coverage = number(&mut pool, decimal(5, 1));
        let measured = measured(
            &mut pool,
            Operator::UncertainExpanded,
            &[value, expanded, coverage],
        );

        let result =
            with_propagated_uncertainty(&mut pool, measured, exact_result(Number::from(4_i64)));

        assert_eq!(
            result,
            Err(UncertaintyError::CoverageFactorBelowOne(measured))
        );
    }

    #[test]
    fn uncertainty_of_an_uncertainty_is_an_error() {
        let mut pool = ExprPool::new();
        let inner = uncertain(&mut pool, decimal(1, 1), decimal(1, 2));
        let value = number(&mut pool, Number::from(2_i64));
        let measured = measured(&mut pool, Operator::Uncertain, &[value, inner]);

        let result =
            with_propagated_uncertainty(&mut pool, measured, exact_result(Number::from(2_i64)));

        assert_eq!(result, Err(UncertaintyError::NestedUncertainty(measured)));
    }

    #[test]
    fn operator_without_derivative_rule_is_an_error() {
        let mut pool = ExprPool::new();
        let measured = uncertain(&mut pool, Number::from(3_i64), decimal(1, 1));
        let factorial = apply(&mut pool, Operator::Factorial, &[measured]);

        let result =
            with_propagated_uncertainty(&mut pool, factorial, exact_result(Number::from(6_i64)));

        assert_eq!(
            result,
            Err(UncertaintyError::UnsupportedOperator(Operator::Factorial))
        );
    }

    #[test]
    fn exact_way_is_evaluated_with_propagated_uncertainty_and_its_method() {
        let mut pool = ExprPool::new();
        let rules = rule_set(&mut pool, Operator::Mul, false);
        let values = [
            given("x", Number::from(3_i64), Some(decimal(1, 1))),
            given("y", Number::from(4_i64), Some(decimal(2, 1))),
        ];

        let evaluation = evaluate(&mut pool, &rules, &["x", "y"], &values);

        let result = &evaluation.result;
        assert_eq!(result.value(), &ResultValue::Number(Number::from(12_i64)));
        assert_eq!(result.method().name, WAY_EVALUATION_METHOD);
        assert_eq!(
            result.method().parameters.get(RULE_PARAMETER),
            Some(&ParameterValue::Identifier("f/rule".to_string()))
        );
        assert_eq!(
            result.method().parameters.get(CONCEPT_SET_PARAMETER),
            Some(&ParameterValue::Identifier("0123456789abcdef".to_string()))
        );
        let expected = ((4.0_f64 * 0.1) * (4.0 * 0.1) + (3.0 * 0.2) * (3.0 * 0.2)).sqrt();
        assert_eq!(standard_uncertainty(result).to_bits(), expected.to_bits());
        assert_eq!(evaluation.backend, None);
    }

    #[test]
    fn way_result_lists_the_sources_of_its_rules() {
        let mut pool = ExprPool::new();
        let rules = rule_set(&mut pool, Operator::Mul, false);
        let values = [
            given("x", Number::from(3_i64), None),
            given("y", Number::from(4_i64), None),
        ];

        let evaluation = evaluate(&mut pool, &rules, &["x", "y"], &values);

        assert_eq!(
            evaluation.result.sources().collect::<Vec<_>>(),
            vec!["some-source"]
        );
        assert_eq!(evaluation.result.uncertainty(), None);
    }

    #[test]
    fn machine_way_runs_on_a_backend_and_keeps_its_rounding_bound() {
        let mut pool = ExprPool::new();
        let rules = rule_set(&mut pool, Operator::Mul, true);
        let values = [given("x", Number::from(12_i64), Some(decimal(1, 1)))];

        let evaluation = evaluate(&mut pool, &rules, &["x"], &values);

        assert_eq!(evaluation.backend, Some(BackendKind::Cpu));
        assert!(matches!(
            evaluation.result.rounding_error(),
            RoundingError::Bound(_)
        ));
        assert_eq!(
            standard_uncertainty(&evaluation.result).to_bits(),
            (std::f64::consts::PI * 0.1).to_bits()
        );
    }

    #[test]
    fn way_uncertainty_is_the_estimate_and_not_the_upper_end_of_its_enclosure() {
        let mut pool = ExprPool::new();
        let rules = rule_set(&mut pool, Operator::Mul, true);
        let values = [given("x", Number::from(12_i64), Some(decimal(1, 1)))];

        let evaluation = evaluate(&mut pool, &rules, &["x"], &values);
        let scores = evaluated_scores(&mut pool, &rules, "f", &way(&["x"]), &values).unwrap();

        let estimate = Number::F64(standard_uncertainty(&evaluation.result))
            .to_exact()
            .unwrap();
        let upper_end = scores.smallest_uncertainty.unwrap();
        let difference = upper_end.sub_exact(&estimate).unwrap();
        assert!(
            matches!(difference, Number::Rational(ref fraction) if !fraction.numerator().is_negative())
        );
    }

    #[test]
    fn way_without_a_given_input_is_an_error() {
        let mut pool = ExprPool::new();
        let rules = rule_set(&mut pool, Operator::Mul, false);
        let values = [given("x", Number::from(3_i64), None)];
        let backend = CpuBackend::new();

        let result = evaluate_way(
            &mut pool,
            &rules,
            "f",
            &way(&["x", "y"]),
            &values,
            &[&backend],
            Preference::Automatic,
        );

        assert_eq!(
            result.err(),
            Some(WayEvaluationError::MissingValue("y".to_string()))
        );
    }
}
