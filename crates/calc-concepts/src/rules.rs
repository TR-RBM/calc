use std::collections::BTreeMap;

use calc_core::{Rule, RuleSet};
use calc_expr::{BuiltinConstant, ExprId, ExprPool, Head, NodeView, Operator};
use calc_numbers::Number;

use crate::error::LoadError;
use crate::identifier::{permuted_identifier, role_identifier, role_name, role_object};
use crate::load::parse_function;
use crate::model::{BoundDefinition, BoundRelation, ConceptSet, WayDefinition};

const EXACT_OPERATORS: [Operator; 10] = [
    Operator::Add,
    Operator::Sub,
    Operator::Mul,
    Operator::Div,
    Operator::Neg,
    Operator::Pow,
    Operator::Sqrt,
    Operator::Abs,
    Operator::Min,
    Operator::Max,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bound {
    pub identifier: String,
    pub role: String,
    pub inputs: Vec<String>,
    pub relation: BoundRelation,
    pub expressions: Vec<ExprId>,
    pub conditions: Vec<ExprId>,
    pub sources: Vec<String>,
}

pub fn is_exact_formula(pool: &ExprPool, formula: ExprId) -> bool {
    let mut pending = vec![formula];
    while let Some(expression) = pending.pop() {
        match pool.node(expression) {
            Ok(NodeView::Number(number)) => {
                if !pool.number_value(number).is_ok_and(Number::is_exact) {
                    return false;
                }
            }
            Ok(NodeView::Symbol(symbol)) => {
                let is_exact_constant =
                    symbol == BuiltinConstant::Pi.symbol() || symbol == BuiltinConstant::E.symbol();
                if !is_exact_constant {
                    return false;
                }
            }
            Ok(NodeView::Bound(_)) => {}
            Ok(NodeView::Bind { body, .. }) => pending.push(body),
            Ok(NodeView::Apply {
                head: Head::Operator(operator),
                arguments,
            }) if EXACT_OPERATORS.contains(&operator) => pending.extend_from_slice(arguments),
            Ok(_) | Err(_) => return false,
        }
    }
    true
}

fn file_of(way_identifier: &str) -> String {
    let concept = way_identifier
        .split_once('/')
        .map_or(way_identifier, |(concept, _)| concept);
    format!("concepts/{concept}/node.md")
}

fn renamed(role: &str, renaming: &BTreeMap<String, String>) -> String {
    match (role_object(role), role_name(role)) {
        (Some(object), Some(name)) => renaming.get(name).map_or_else(
            || role.to_string(),
            |target| role_identifier(object, target),
        ),
        _ => role.to_string(),
    }
}

fn variants(way: &WayDefinition) -> Vec<(String, String, Vec<String>)> {
    let mut variants = vec![(
        way.identifier.clone(),
        way.output.clone(),
        way.inputs.clone(),
    )];
    let Some((original, renamings)) = way.permutations.split_first() else {
        return variants;
    };
    for (index, renaming) in renamings.iter().enumerate() {
        let map: BTreeMap<String, String> = original
            .iter()
            .cloned()
            .zip(renaming.iter().cloned())
            .collect();
        variants.push((
            permuted_identifier(&way.identifier, index + 1),
            renamed(&way.output, &map),
            way.inputs
                .iter()
                .map(|input| renamed(input, &map))
                .collect(),
        ));
    }
    variants
}

impl ConceptSet {
    pub fn rule_set(&self, pool: &mut ExprPool) -> Result<RuleSet, LoadError> {
        let quantities = self
            .objects
            .iter()
            .flat_map(|object| object.roles.iter().map(|role| role.identifier.clone()))
            .collect();
        let mut rules = Vec::new();
        for (concept, way) in self
            .concepts
            .iter()
            .flat_map(|concept| concept.ways.iter().map(move |way| (concept, way)))
        {
            let file = file_of(&way.identifier);
            let formula = parse_function(&file, pool, &way.formula, way.inputs.len())?;
            let conditions = way
                .conditions
                .iter()
                .map(|condition| parse_function(&file, pool, condition, way.inputs.len()))
                .collect::<Result<Vec<_>, _>>()?;
            let exact = is_exact_formula(pool, formula);
            for (identifier, output, inputs) in variants(way) {
                rules.push(Rule {
                    identifier,
                    output,
                    inputs,
                    formula,
                    conditions: conditions.clone(),
                    exact,
                    sources: way.sources.clone(),
                    corpus_references: concept.rests_on.clone(),
                });
            }
        }
        rules.sort_by(|left, right| left.identifier.cmp(&right.identifier));
        Ok(RuleSet {
            version: self.version,
            quantities,
            rules,
        })
    }

    pub fn bounds(&self, pool: &mut ExprPool) -> Result<Vec<Bound>, LoadError> {
        let definitions: Vec<&BoundDefinition> = self
            .concepts
            .iter()
            .flat_map(|concept| concept.bounds.iter())
            .collect();
        definitions
            .into_iter()
            .map(|bound| {
                let file = file_of(&bound.identifier);
                let parse = |pool: &mut ExprPool, text: &String| {
                    parse_function(&file, pool, text, bound.inputs.len())
                };
                let expressions = bound
                    .expressions
                    .iter()
                    .map(|text| parse(pool, text))
                    .collect::<Result<Vec<_>, _>>()?;
                let conditions = bound
                    .conditions
                    .iter()
                    .map(|text| parse(pool, text))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Bound {
                    identifier: bound.identifier.clone(),
                    role: bound.role.clone(),
                    inputs: bound.inputs.clone(),
                    relation: bound.relation,
                    expressions,
                    conditions,
                    sources: bound.sources.clone(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load::load;

    const SOURCE: &str = "# book\n\nTitle: A Book\nAuthors: Ann Author\nYear: 2020\nIdentifier: https://example.org/book\nLicence: CC-BY-4.0\n";
    const TRIANGLE: &str = "# triangle\n\n## Role side-a\nDimension: m\nQuantity: length\n\n## Role side-b\nDimension: m\nQuantity: length\n\n## Role side-c\nDimension: m\nQuantity: length\n\n## Role perimeter\nDimension: m\nQuantity: length\n";
    const TRIANGLE_TEXT: &str = "# Triangle\nRunning: triangle\n\n## Role side-a\nName: a\nRunning: a\n\n## Role side-b\nName: b\nRunning: b\n\n## Role side-c\nName: c\nRunning: c\n\n## Role perimeter\nName: Perimeter\nRunning: perimeter\n";
    const TEXT: &str = "# Perimeter\n\nStatement: The perimeter is the sum of the sides.\n";

    fn node(way: &str) -> String {
        format!("# perimeter\n\nLevel: isced-1\nPrerequisites:\nStart: yes\nSources: book\n\n{way}")
    }

    fn set_with(way: &str) -> ConceptSet {
        let node = node(way);
        let files: Vec<(&str, &[u8])> = vec![
            ("sources/book.md", SOURCE.as_bytes()),
            ("objects/triangle.md", TRIANGLE.as_bytes()),
            ("objects/triangle.en.md", TRIANGLE_TEXT.as_bytes()),
            ("concepts/perimeter/node.md", node.as_bytes()),
            ("concepts/perimeter/en.md", TEXT.as_bytes()),
        ];
        load(&files).unwrap()
    }

    const SIDE_C: &str = "## Way side-c\nOutput: triangle.side-c\nInputs: triangle.perimeter, triangle.side-a, triangle.side-b\nRelation: perimeter\nSources: book\nRecognized: yes\nPermutations: side-a side-b side-c, side-b side-c side-a, side-c side-a side-b\n```calc\np |-> a |-> b |-> p - a - b\n```\n";

    #[test]
    fn rule_set_carries_version_quantities_and_rules() {
        let set = set_with(SIDE_C);
        let mut pool = ExprPool::new();
        let rules = set.rule_set(&mut pool).unwrap();
        assert_eq!(rules.version, set.version);
        assert_eq!(rules.quantities.len(), 4);
        assert_eq!(rules.rules[0].identifier, "perimeter/side-c");
    }

    #[test]
    fn formula_is_a_lambda_chain_in_the_callers_pool() {
        let set = set_with(SIDE_C);
        let mut pool = ExprPool::new();
        let rules = set.rule_set(&mut pool).unwrap();
        assert_eq!(
            crate::load::lambda_parameter_count(&pool, rules.rules[0].formula),
            3
        );
    }

    #[test]
    fn permutations_add_renamed_rules() {
        let set = set_with(SIDE_C);
        let mut pool = ExprPool::new();
        let rules = set.rule_set(&mut pool).unwrap();
        let second = rules
            .rules
            .iter()
            .find(|rule| rule.identifier == "perimeter/side-c~1")
            .unwrap();
        assert_eq!(second.output, "triangle.side-a");
        assert_eq!(
            second.inputs,
            vec![
                "triangle.perimeter".to_string(),
                "triangle.side-b".to_string(),
                "triangle.side-c".to_string()
            ]
        );
        assert_eq!(rules.rules.len(), 3);
    }

    fn exactness_of(formula: &str) -> bool {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, formula).unwrap();
        is_exact_formula(&pool, expression)
    }

    #[test]
    fn arithmetic_and_square_roots_are_exact() {
        assert!(exactness_of("a |-> b |-> sqrt(a^2 + b^2) / 2"));
    }

    #[test]
    fn pi_is_an_exact_constant() {
        assert!(exactness_of("r |-> 2 * pi * r"));
    }

    #[test]
    fn trigonometric_function_is_not_exact() {
        assert!(!exactness_of("a |-> sin(a)"));
    }

    #[test]
    fn machine_number_is_not_exact() {
        assert!(!exactness_of("a |-> f64'0.5' * a"));
    }

    #[test]
    fn bounds_are_built_into_the_pool() {
        let bound = "## Bound at-most-sum\nRole: triangle.side-c\nInputs: triangle.side-a, triangle.side-b\nRelation kind: at-most\nSources: book\n```calc\na |-> b |-> a + b\n```\n```calc\na |-> b |-> a > 0\n```\n";
        let set = set_with(&format!("{SIDE_C}\n{bound}"));
        let mut pool = ExprPool::new();
        let bounds = set.bounds(&mut pool).unwrap();
        assert_eq!(bounds.len(), 1);
        assert_eq!(bounds[0].relation, BoundRelation::AtMost);
        assert_eq!(
            (bounds[0].expressions.len(), bounds[0].conditions.len()),
            (1, 1)
        );
    }
}
