use std::collections::{BTreeMap, HashMap};

use calc_core::{
    ConditionOutcome, GivenValue, Rule, RuleSet, decide_derivation_conditions,
    derivation_expression, derivation_expressions, evaluate_exact,
};
use calc_expr::{ExprId, ExprPool, SymbolKind};
use calc_numbers::Number;
use calc_units::{
    DisplayTarget, DisplayedNumber, DisplayedPiMultiple, UnitId, convert_for_display,
    convert_pi_multiple_for_display,
};

use crate::displayed_value::pi_power_text;
use crate::readable::exact_reading;
use crate::solve_answer::{AnswerBody, AnswerWay, ConditionParameter, SolveAnswer};
use crate::solve_form::{SolveVocabulary, person_text};
use crate::summary::value_text;
use crate::unit_display::display_text;

const ROLE_SEPARATOR: char = '.';

type ConditionTextOf<'text> =
    dyn Fn(&mut ExprPool, &str, ExprId) -> (String, Vec<ConditionParameter>) + 'text;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkingCondition {
    pub text: String,
    pub parameters: Vec<ConditionParameter>,
    pub holds: Option<bool>,
    pub could_have_failed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkingRule {
    pub rule: String,
    pub output: String,
    pub inputs: Vec<String>,
    pub values: Vec<(String, String)>,
    pub result: String,
    pub reading: Option<String>,
    pub conditions: Vec<WorkingCondition>,
    pub sources: Vec<String>,
    pub corpus_references: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedRule {
    pub output: String,
    pub inputs: Vec<String>,
    pub formula: String,
    pub substituted: Option<String>,
    pub result: String,
    pub reading: Option<String>,
    pub conditions: Vec<WorkingCondition>,
    pub sources: Vec<String>,
}

fn role_of(identifier: &str) -> &str {
    identifier
        .split_once(ROLE_SEPARATOR)
        .map_or(identifier, |(_, role)| role)
}

pub fn rendered_rule(
    rule: &WorkingRule,
    vocabulary: &SolveVocabulary,
    symbols: &BTreeMap<String, String>,
) -> Option<RenderedRule> {
    let formula = vocabulary.person_formula_text(&rule.rule, symbols)?;
    let values: HashMap<&str, &str> = rule
        .values
        .iter()
        .map(|(role, value)| (role.as_str(), value.as_str()))
        .collect();
    let body = formula.split_once(" = ").map(|(_, body)| body)?;
    let substituted = (values.len() == rule.inputs.len())
        .then(|| person_text_over(&rule.inputs, body, vocabulary, symbols, &values))
        .flatten();
    Some(RenderedRule {
        output: vocabulary.person_role_name(&rule.output, symbols),
        inputs: rule
            .inputs
            .iter()
            .map(|input| vocabulary.person_role_name(input, symbols))
            .collect(),
        formula,
        substituted,
        result: rule.result.clone(),
        reading: rule.reading.clone(),
        conditions: rule.conditions.clone(),
        sources: rule.sources.clone(),
    })
}

fn person_text_over(
    inputs: &[String],
    body: &str,
    vocabulary: &SolveVocabulary,
    symbols: &BTreeMap<String, String>,
    values: &HashMap<&str, &str>,
) -> Option<String> {
    let parameters: Vec<ConditionParameter> = inputs
        .iter()
        .map(|role| ConditionParameter {
            name: vocabulary.person_role_name(role, symbols),
            role: role.clone(),
        })
        .collect();
    let named = |role: &str| {
        values
            .get(role)
            .map(|value| (*value).to_string())
            .unwrap_or_else(|| role_of(role).to_string())
    };
    person_text(body, &parameters, named)
}

fn given_symbols(
    pool: &mut ExprPool,
    given: &[GivenValue],
) -> Option<HashMap<calc_expr::SymbolId, ExprId>> {
    let mut replacements = HashMap::new();
    for value in given {
        let symbol = pool
            .intern_symbol(&value.quantity, SymbolKind::Variable)
            .ok()?;
        let node = pool.number(value.value.clone()).ok()?;
        replacements.insert(symbol, node);
    }
    Some(replacements)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RoleValue {
    Rational(Number),
    PiPower { coefficient: Number, exponent: i32 },
}

fn value_of(
    pool: &mut ExprPool,
    expression: ExprId,
    replacements: &HashMap<calc_expr::SymbolId, ExprId>,
) -> Option<RoleValue> {
    let substituted = calc_expr::substitute_symbols(pool, expression, replacements).ok()?;
    let evaluation = evaluate_exact(pool, substituted).ok()?;
    if let Some(number) = evaluation.rational_value() {
        return Some(RoleValue::Rational(number.clone()));
    }
    evaluation
        .pi_power()
        .map(|(coefficient, exponent)| RoleValue::PiPower {
            coefficient: coefficient.clone(),
            exponent,
        })
}

fn value_and_reading(
    pool: &mut ExprPool,
    value: &RoleValue,
    units: Option<(UnitId, UnitId)>,
) -> Option<(String, Option<String>)> {
    let (coherent, target) = match units {
        Some(units) => units,
        None => {
            return Some(match value {
                RoleValue::Rational(number) => (
                    value_text(&calc_core::ResultValue::Number(number.clone())),
                    exact_reading(number),
                ),
                RoleValue::PiPower {
                    coefficient,
                    exponent,
                } => (person_pi_power_text(pool, coefficient, *exponent)?, None),
            });
        }
    };
    let unit = display_text(pool, target);
    let with_unit = |text: String| match &unit {
        Some(unit) => format!("{text} {unit}"),
        None => text,
    };
    match value {
        RoleValue::Rational(number) => {
            let shown = convert_for_display(
                pool.units(),
                number,
                coherent,
                DisplayTarget::Unit(target),
                None,
            )
            .ok()?;
            let number = match shown {
                DisplayedNumber::Exact(number)
                | DisplayedNumber::Rounded { value: number, .. }
                | DisplayedNumber::Unscaled(number) => number,
            };
            let reading = exact_reading(&number);
            Some((
                with_unit(value_text(&calc_core::ResultValue::Number(number))),
                reading.map(&with_unit),
            ))
        }
        RoleValue::PiPower {
            coefficient,
            exponent,
        } => {
            let shown = convert_pi_multiple_for_display(
                pool.units(),
                coefficient,
                *exponent,
                coherent,
                DisplayTarget::Unit(target),
            )
            .ok()?;
            match shown {
                DisplayedPiMultiple::Rational(number) => {
                    let reading = exact_reading(&number);
                    Some((
                        with_unit(value_text(&calc_core::ResultValue::Number(number))),
                        reading.map(&with_unit),
                    ))
                }
                DisplayedPiMultiple::PiPower {
                    coefficient,
                    exponent,
                } => {
                    let reading = pi_reading(pool, &coefficient, exponent);
                    Some((
                        with_unit(person_pi_power_text(pool, &coefficient, exponent)?),
                        reading.map(&with_unit),
                    ))
                }
            }
        }
    }
}

fn person_pi_power_text(
    pool: &mut ExprPool,
    coefficient: &Number,
    exponent: i32,
) -> Option<String> {
    let text = pi_power_text(pool, coefficient, exponent)?;
    let expression = calc_syntax::parse_expression(pool, &text).ok()?;
    calc_syntax::print_expression(pool, expression, calc_syntax::PrintMode::Unicode).ok()
}

fn pi_reading(pool: &mut ExprPool, coefficient: &Number, exponent: i32) -> Option<String> {
    let text = pi_power_text(pool, coefficient, exponent)?;
    let expression = calc_syntax::parse_expression(pool, &text).ok()?;
    crate::readable::expression_reading(pool, expression, &calc_units::ScaleFactor::one())
}

fn rule_of<'set>(rule_set: &'set RuleSet, identifier: &str) -> Option<&'set Rule> {
    rule_set
        .rules
        .iter()
        .find(|rule| rule.identifier == identifier)
}

pub(crate) fn evaluated_way(answer: &SolveAnswer) -> Option<&AnswerWay> {
    let ways = match &answer.body {
        AnswerBody::Solved { ways, .. } | AnswerBody::Ways { ways, .. } => ways,
        AnswerBody::Front { front, .. } => front,
        AnswerBody::NotReached { .. } => return None,
    };
    ways.iter()
        .find(|way| way.record.is_some())
        .or_else(|| ways.first())
}

pub(crate) fn rule_chain(
    pool: &mut ExprPool,
    rule_set: &RuleSet,
    wanted: &str,
    derivation: &[String],
    given: &[GivenValue],
    units: &HashMap<String, (UnitId, UnitId)>,
    condition_text: &ConditionTextOf<'_>,
) -> Option<Vec<WorkingRule>> {
    let (expressions, _) = derivation_expressions(pool, rule_set, wanted, derivation).ok()?;
    let replacements = given_symbols(pool, given)?;
    let mut values: HashMap<String, RoleValue> = HashMap::new();
    for (quantity, expression) in &expressions {
        if let Some(value) = value_of(pool, *expression, &replacements) {
            values.insert(quantity.clone(), value);
        }
    }
    let outcomes = decide_derivation_conditions(pool, rule_set, wanted, derivation, given).ok()?;
    let mut chain = Vec::new();
    for identifier in derivation {
        let rule = rule_of(rule_set, identifier)?;
        let conditions = conditions_of(pool, rule, &outcomes, given, condition_text);
        let mut shown = Vec::new();
        for input in &rule.inputs {
            let Some(value) = values.get(input).cloned() else {
                continue;
            };
            let Some((text, _)) = value_and_reading(pool, &value, units.get(input).copied()) else {
                continue;
            };
            shown.push((input.clone(), text));
        }
        let output = values.get(&rule.output).cloned();
        let (result, reading) = output
            .and_then(|value| value_and_reading(pool, &value, units.get(&rule.output).copied()))
            .unwrap_or_default();
        chain.push(WorkingRule {
            rule: rule.identifier.clone(),
            output: rule.output.clone(),
            inputs: rule.inputs.clone(),
            values: shown,
            result,
            reading,
            conditions,
            sources: rule.sources.clone(),
            corpus_references: rule.corpus_references.clone(),
        });
    }
    Some(chain)
}

fn conditions_of(
    pool: &mut ExprPool,
    rule: &Rule,
    outcomes: &[ConditionOutcome],
    given: &[GivenValue],
    condition_text: &ConditionTextOf<'_>,
) -> Vec<WorkingCondition> {
    let mut conditions = Vec::new();
    for condition in &rule.conditions {
        let holds = outcomes
            .iter()
            .find(|outcome| outcome.rule == rule.identifier && outcome.condition == *condition)
            .and_then(|outcome| outcome.holds);
        let (text, parameters) = condition_text(pool, &rule.identifier, *condition);
        let could_have_failed = parameters
            .iter()
            .any(|parameter| given.iter().any(|value| value.quantity == parameter.role));
        conditions.push(WorkingCondition {
            text,
            parameters,
            holds,
            could_have_failed,
        });
    }
    conditions
}

pub(crate) fn derivation_root(
    pool: &mut ExprPool,
    rule_set: &RuleSet,
    wanted: &str,
    derivation: &[String],
    given: &[GivenValue],
) -> Option<ExprId> {
    let expression = derivation_expression(pool, rule_set, wanted, derivation).ok()?;
    let replacements = given_symbols(pool, given)?;
    calc_expr::substitute_symbols(pool, expression.expression, &replacements).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::tests::session;
    use crate::solve_request::parse_request;
    use crate::views::{ResultView, ViewEvent, ViewOutcome, WorkingOutcome};
    use crate::{LineId, Session};
    use calc_i18n::{LanguageTag, Locale};
    use std::sync::mpsc::channel;

    const DIAMETER: &str = r#"{"phase": "evaluate", "object": "circle", "wanted": "circumference", "given": [{"name": "diameter", "value": "5", "unit": "cm"}]}"#;

    fn circumference(session: &mut Session) -> LineId {
        let request = parse_request(DIAMETER.as_bytes()).expect("a request");
        session.enter_solve(&request).expect("a solve line")
    }

    fn rules_of(session: &mut Session, id: LineId) -> Vec<WorkingRule> {
        let (sender, receiver) = channel();
        let mut job = session
            .view(
                id,
                ResultView::Working { path: Vec::new() },
                None,
                false,
                sender,
            )
            .expect("a working");
        crate::platform::Job::step(&mut job);
        match receiver.try_recv().expect("an event") {
            ViewEvent::Finished { outcome, .. } => match *outcome {
                ViewOutcome::Working(WorkingOutcome::Steps { rules, .. }) => rules,
                other => panic!("expected a working, found {other:?}"),
            },
            ViewEvent::Failed { error, .. } => panic!("expected a working, found {error:?}"),
        }
    }

    fn symbols() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("circle.circumference".to_owned(), "U".to_owned()),
            ("circle.diameter".to_owned(), "d".to_owned()),
        ])
    }

    fn vocabulary() -> SolveVocabulary {
        SolveVocabulary::load(&Locale::matching(&LanguageTag::parse("en").unwrap()))
            .expect("the concept set")
    }

    #[test]
    fn a_solve_line_names_the_rule_it_used() {
        let mut computing = session();
        let id = circumference(&mut computing);

        let rules = rules_of(&mut computing, id);

        assert_eq!(
            rules
                .iter()
                .map(|rule| rule.rule.as_str())
                .collect::<Vec<&str>>(),
            ["circle-circumference/from-diameter"]
        );
    }

    #[test]
    fn a_rule_carries_the_roles_it_relates() {
        let mut computing = session();
        let id = circumference(&mut computing);

        let rules = rules_of(&mut computing, id);

        assert_eq!(
            (rules[0].output.as_str(), rules[0].inputs.as_slice()),
            ("circle.circumference", &["circle.diameter".to_owned()][..])
        );
    }

    #[test]
    fn a_rule_reads_as_a_formula_with_its_values_and_its_result() {
        let mut computing = session();
        let id = circumference(&mut computing);
        let rules = rules_of(&mut computing, id);

        let rendered = rendered_rule(&rules[0], &vocabulary(), &symbols()).expect("a rendering");

        assert_eq!(
            (
                rendered.formula.as_str(),
                rendered.substituted.as_deref(),
                rendered.result.as_str()
            ),
            ("U = π · d", Some("π · 5 cm"), "5 · π cm")
        );
    }

    #[test]
    fn a_rule_carries_the_corpus_blocks_its_concept_rests_on() {
        let mut computing = session();
        let id = circumference(&mut computing);

        let rules = rules_of(&mut computing, id);

        assert_eq!(
            rules[0].corpus_references.is_empty(),
            concept_rests_on_nothing()
        );
    }

    fn concept_rests_on_nothing() -> bool {
        crate::solve::concept_set()
            .expect("the concept set")
            .concepts
            .iter()
            .find(|concept| concept.identifier == "circle-circumference")
            .expect("the concept")
            .rests_on
            .is_empty()
    }

    #[test]
    fn a_rule_whose_inputs_are_not_all_valued_shows_no_substitution() {
        let mut computing = session();
        let id = circumference(&mut computing);
        let mut rules = rules_of(&mut computing, id);
        rules[0].values.clear();

        let rendered = rendered_rule(&rules[0], &vocabulary(), &symbols()).expect("a rendering");

        assert_eq!(rendered.substituted, None);
    }

    #[test]
    fn the_result_of_a_rule_carries_its_reading() {
        let mut computing = session();
        let id = circumference(&mut computing);

        let rules = rules_of(&mut computing, id);

        assert_eq!(rules[0].reading.as_deref(), Some("15.71 cm"));
    }

    #[test]
    fn a_rule_carries_the_conditions_it_needed_with_whether_they_held() {
        let mut computing = session();
        let id = circumference(&mut computing);

        let rules = rules_of(&mut computing, id);

        let conditions: Vec<(Option<bool>, bool)> = rules[0]
            .conditions
            .iter()
            .map(|condition| (condition.holds, condition.could_have_failed))
            .collect();
        assert_eq!(conditions, [(Some(true), true)]);
    }

    #[test]
    fn a_rule_carries_the_sources_of_its_way() {
        let mut computing = session();
        let id = circumference(&mut computing);

        let rules = rules_of(&mut computing, id);

        assert!(!rules[0].sources.is_empty());
    }

    #[test]
    fn a_line_without_a_rule_has_no_rule_chain() {
        let mut computing = session();
        let id = computing.enter("(2 + 3) * 4").unwrap();

        let rules = rules_of(&mut computing, id);

        assert!(rules.is_empty());
    }
}
