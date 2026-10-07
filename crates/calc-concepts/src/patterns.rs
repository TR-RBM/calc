use std::collections::BTreeSet;

use calc_expr::{BinderKind, ExprId, ExprPool, Head, NodeView, Operator, SymbolKind};

use crate::error::{LoadError, LoadErrorKind};
use crate::load::{lambda_parameter_count, parse_function};
use crate::model::{ConceptNode, ConceptSet, WayDefinition};

const LABEL_PREFIX: char = 'r';

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecognitionPattern {
    pub identifier: String,
    pub concept: String,
    pub relation: Option<String>,
    pub derived: bool,
    pub variables: Vec<String>,
    pub function: ExprId,
    pub conditions: Vec<ExprId>,
}

struct CheckedFunction {
    function: ExprId,
    variables: Vec<String>,
}

fn error(file: &str, kind: LoadErrorKind) -> LoadError {
    LoadError {
        file: file.to_string(),
        line: 0,
        kind,
    }
}

fn is_label(name: &str) -> bool {
    let mut characters = name.chars();
    characters.next() == Some(LABEL_PREFIX)
        && name.len() > 1
        && characters.all(|character| character.is_ascii_digit())
}

fn is_free_variable_name(name: &str) -> bool {
    if is_label(name) {
        return false;
    }
    let mut scratch = ExprPool::new();
    let Ok(expression) = calc_syntax::parse_expression(&mut scratch, name) else {
        return false;
    };
    match scratch.node(expression) {
        Ok(NodeView::Symbol(symbol)) => {
            scratch.symbol_kind(symbol) == Ok(SymbolKind::Variable)
                && scratch.symbol_name(symbol) == Ok(name)
        }
        _ => false,
    }
}

fn parameter_names(pool: &ExprPool, function: ExprId, count: usize) -> (Vec<String>, ExprId) {
    let mut names = Vec::with_capacity(count);
    let mut current = function;
    while names.len() < count {
        let Ok(NodeView::Bind {
            binder: BinderKind::Lambda,
            body,
            ..
        }) = pool.node(current)
        else {
            break;
        };
        names.push(pool.bound_name(current).unwrap_or_default().to_string());
        current = body;
    }
    (names, current)
}

fn scan_body(
    file: &str,
    pool: &ExprPool,
    body: ExprId,
    count: usize,
) -> Result<BTreeSet<usize>, LoadError> {
    let mut used = BTreeSet::new();
    let mut pending = vec![(body, 0u32)];
    while let Some((expression, depth)) = pending.pop() {
        let Ok(node) = pool.node(expression) else {
            continue;
        };
        match node {
            NodeView::Bound(index) if index >= depth => {
                let from_innermost = (index - depth) as usize;
                if from_innermost < count {
                    used.insert(count - 1 - from_innermost);
                }
            }
            NodeView::Bound(_) | NodeView::Number(_) => {}
            NodeView::Symbol(symbol) => {
                if pool.symbol_kind(symbol) != Ok(SymbolKind::Constant) {
                    let name = pool.symbol_name(symbol).unwrap_or_default().to_string();
                    return Err(error(file, LoadErrorKind::FreeNameInPattern(name)));
                }
            }
            NodeView::Apply { head, arguments } => {
                if let Head::Function(symbol) = head {
                    let name = pool.symbol_name(symbol).unwrap_or_default().to_string();
                    return Err(error(file, LoadErrorKind::FreeNameInPattern(name)));
                }
                pending.extend(arguments.iter().map(|argument| (*argument, depth)));
            }
            NodeView::Bind {
                arguments, body, ..
            } => {
                pending.extend(arguments.iter().map(|argument| (*argument, depth)));
                pending.push((body, depth + 1));
            }
            NodeView::Quantity { value, .. } => pending.push((value, depth)),
            NodeView::Array { elements, .. } => {
                pending.extend(elements.iter().map(|element| (*element, depth)));
            }
        }
    }
    Ok(used)
}

fn check_pattern_function(
    file: &str,
    pool: &mut ExprPool,
    text: &str,
    conditions: &[String],
) -> Result<CheckedFunction, LoadError> {
    let parsed = calc_syntax::parse_expression(pool, text)
        .map_err(|parse_error| error(file, LoadErrorKind::Formula(parse_error.kind)))?;
    let count = lambda_parameter_count(pool, parsed);
    if count == 0 {
        return Err(error(file, LoadErrorKind::FormulaNotAFunction));
    }
    let function = parse_function(file, pool, text, count)?;
    let (variables, body) = parameter_names(pool, function, count);
    let mut seen = BTreeSet::new();
    for name in &variables {
        if !is_free_variable_name(name) {
            return Err(error(file, LoadErrorKind::ReservedParameter(name.clone())));
        }
        if !seen.insert(name.as_str()) {
            return Err(error(file, LoadErrorKind::DuplicateParameter(name.clone())));
        }
    }
    if matches!(pool.node(body), Ok(NodeView::Bound(_))) {
        return Err(error(file, LoadErrorKind::PatternIsVariable));
    }
    let used = scan_body(file, pool, body, count)?;
    if let Some(unused) = (0..count).find(|position| !used.contains(position)) {
        return Err(error(
            file,
            LoadErrorKind::UnusedPatternVariable(variables[unused].clone()),
        ));
    }
    for condition in conditions {
        parse_function(file, pool, condition, count)?;
    }
    Ok(CheckedFunction {
        function,
        variables,
    })
}

struct Correspondence {
    forward: Vec<Option<usize>>,
    backward: Vec<Option<usize>>,
}

impl Correspondence {
    fn new(count: usize) -> Self {
        Self {
            forward: vec![None; count],
            backward: vec![None; count],
        }
    }

    fn pair(&mut self, left: usize, right: usize) -> bool {
        match (
            self.forward.get(left).copied(),
            self.backward.get(right).copied(),
        ) {
            (Some(Some(mapped)), _) => mapped == right,
            (_, Some(Some(mapped))) => mapped == left,
            (Some(None), Some(None)) => {
                self.forward[left] = Some(right);
                self.backward[right] = Some(left);
                true
            }
            _ => false,
        }
    }

    fn snapshot(&self) -> (Vec<Option<usize>>, Vec<Option<usize>>) {
        (self.forward.clone(), self.backward.clone())
    }

    fn restore(&mut self, snapshot: (Vec<Option<usize>>, Vec<Option<usize>>)) {
        self.forward = snapshot.0;
        self.backward = snapshot.1;
    }
}

fn chain_operands(pool: &ExprPool, expression: ExprId, operator: Operator) -> Vec<ExprId> {
    let mut operands = Vec::new();
    let mut pending = vec![expression];
    while let Some(current) = pending.pop() {
        match pool.node(current) {
            Ok(NodeView::Apply {
                head: Head::Operator(found),
                arguments,
            }) if found == operator => pending.extend(arguments.iter().rev()),
            _ => operands.push(current),
        }
    }
    operands
}

fn equivalent_all(
    pool: &ExprPool,
    left: &[ExprId],
    right: &[ExprId],
    depth: u32,
    count: usize,
    correspondence: &mut Correspondence,
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(a, b)| equivalent(pool, *a, *b, depth, count, correspondence))
}

fn equivalent_chains(
    pool: &ExprPool,
    left: &[ExprId],
    right: &[ExprId],
    used: &mut Vec<bool>,
    depth: u32,
    count: usize,
    correspondence: &mut Correspondence,
) -> bool {
    let Some((first, rest)) = left.split_first() else {
        return true;
    };
    for position in 0..right.len() {
        if used[position] {
            continue;
        }
        let snapshot = correspondence.snapshot();
        if equivalent(pool, *first, right[position], depth, count, correspondence) {
            used[position] = true;
            if equivalent_chains(pool, rest, right, used, depth, count, correspondence) {
                return true;
            }
            used[position] = false;
        }
        correspondence.restore(snapshot);
    }
    false
}

fn equivalent(
    pool: &ExprPool,
    left: ExprId,
    right: ExprId,
    depth: u32,
    count: usize,
    correspondence: &mut Correspondence,
) -> bool {
    let (Ok(left_node), Ok(right_node)) = (pool.node(left), pool.node(right)) else {
        return false;
    };
    match (left_node, right_node) {
        (NodeView::Bound(a), NodeView::Bound(b)) => {
            if a < depth || b < depth {
                return a == b;
            }
            let a = (a - depth) as usize;
            let b = (b - depth) as usize;
            a < count && b < count && correspondence.pair(count - 1 - a, count - 1 - b)
        }
        (NodeView::Number(a), NodeView::Number(b)) => a == b,
        (NodeView::Symbol(a), NodeView::Symbol(b)) => a == b,
        (
            NodeView::Quantity {
                value: a,
                unit: unit_a,
            },
            NodeView::Quantity {
                value: b,
                unit: unit_b,
            },
        ) => unit_a == unit_b && equivalent(pool, a, b, depth, count, correspondence),
        (
            NodeView::Array {
                shape: shape_a,
                elements: a,
            },
            NodeView::Array {
                shape: shape_b,
                elements: b,
            },
        ) => shape_a == shape_b && equivalent_all(pool, a, b, depth, count, correspondence),
        (
            NodeView::Bind {
                binder: binder_a,
                arguments: arguments_a,
                body: body_a,
            },
            NodeView::Bind {
                binder: binder_b,
                arguments: arguments_b,
                body: body_b,
            },
        ) => {
            binder_a == binder_b
                && equivalent_all(pool, arguments_a, arguments_b, depth, count, correspondence)
                && equivalent(pool, body_a, body_b, depth + 1, count, correspondence)
        }
        (
            NodeView::Apply {
                head: Head::Operator(operator_a),
                ..
            },
            NodeView::Apply {
                head: Head::Operator(operator_b),
                ..
            },
        ) if operator_a == operator_b && matches!(operator_a, Operator::Add | Operator::Mul) => {
            let a = chain_operands(pool, left, operator_a);
            let b = chain_operands(pool, right, operator_b);
            a.len() == b.len()
                && equivalent_chains(
                    pool,
                    &a,
                    &b,
                    &mut vec![false; b.len()],
                    depth,
                    count,
                    correspondence,
                )
        }
        (
            NodeView::Apply {
                head: head_a,
                arguments: a,
            },
            NodeView::Apply {
                head: head_b,
                arguments: b,
            },
        ) => head_a == head_b && equivalent_all(pool, a, b, depth, count, correspondence),
        _ => false,
    }
}

fn lambda_body(pool: &ExprPool, function: ExprId, count: usize) -> ExprId {
    parameter_names(pool, function, count).1
}

pub(crate) fn shapes_are_equivalent(pool: &ExprPool, left: ExprId, right: ExprId) -> bool {
    let count = lambda_parameter_count(pool, left);
    count == lambda_parameter_count(pool, right)
        && equivalent(
            pool,
            lambda_body(pool, left, count),
            lambda_body(pool, right, count),
            0,
            count,
            &mut Correspondence::new(count),
        )
}

fn derived_patterns(node: &ConceptNode) -> impl Iterator<Item = &WayDefinition> {
    node.ways.iter().filter(|way| way.recognized)
}

fn node_file(concept: &str) -> String {
    format!("concepts/{concept}/node.md")
}

pub(crate) fn validate_patterns(set: &ConceptSet) -> Result<(), LoadError> {
    let relations: BTreeSet<&str> = set
        .concepts
        .iter()
        .flat_map(|concept| concept.ways.iter().map(|way| way.relation.as_str()))
        .collect();
    let mut pool = ExprPool::new();
    for concept in &set.concepts {
        let file = node_file(&concept.identifier);
        let mut derived = Vec::new();
        for way in derived_patterns(concept) {
            let checked = check_pattern_function(&file, &mut pool, &way.formula, &way.conditions)?;
            derived.push((way.identifier.as_str(), checked.function));
        }
        for pattern in &concept.patterns {
            if let Some(relation) = &pattern.relation
                && !relations.contains(relation.as_str())
            {
                return Err(error(
                    &file,
                    LoadErrorKind::UnknownRelation(relation.clone()),
                ));
            }
            let guards: Vec<String> = pattern.guard.iter().cloned().collect();
            let checked = check_pattern_function(&file, &mut pool, &pattern.body, &guards)?;
            if let Some((way, _)) = derived
                .iter()
                .find(|(_, function)| shapes_are_equivalent(&pool, checked.function, *function))
            {
                return Err(error(
                    &file,
                    LoadErrorKind::DuplicatePattern {
                        pattern: pattern.identifier.clone(),
                        way: (*way).to_string(),
                    },
                ));
            }
        }
    }
    Ok(())
}

fn build(
    pool: &mut ExprPool,
    concept: &ConceptNode,
    identifier: &str,
    relation: Option<String>,
    derived: bool,
    text: &str,
    conditions: &[String],
) -> Result<RecognitionPattern, LoadError> {
    let file = node_file(&concept.identifier);
    let checked = check_pattern_function(&file, pool, text, conditions)?;
    let count = checked.variables.len();
    let conditions = conditions
        .iter()
        .map(|condition| parse_function(&file, pool, condition, count))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RecognitionPattern {
        identifier: identifier.to_string(),
        concept: concept.identifier.clone(),
        relation,
        derived,
        variables: checked.variables,
        function: checked.function,
        conditions,
    })
}

impl ConceptSet {
    pub fn patterns(&self, pool: &mut ExprPool) -> Result<Vec<RecognitionPattern>, LoadError> {
        let mut patterns = Vec::new();
        for concept in &self.concepts {
            for way in derived_patterns(concept) {
                patterns.push(build(
                    pool,
                    concept,
                    &way.identifier,
                    Some(way.relation.clone()),
                    true,
                    &way.formula,
                    &way.conditions,
                )?);
            }
            for pattern in &concept.patterns {
                let guards: Vec<String> = pattern.guard.iter().cloned().collect();
                patterns.push(build(
                    pool,
                    concept,
                    &pattern.identifier,
                    pattern.relation.clone(),
                    false,
                    &pattern.body,
                    &guards,
                )?);
            }
        }
        patterns.sort_by(|left, right| {
            (left.concept.as_str(), left.identifier.as_str())
                .cmp(&(right.concept.as_str(), right.identifier.as_str()))
        });
        Ok(patterns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load::load;

    const SOURCE: &str = "# book\n\nTitle: A Book\nAuthors: Ann Author\nYear: 2020\nIdentifier: https://example.org/book\nLicence: CC-BY-4.0\n";
    const OBJECT: &str = "# circle\n\n## Role radius\nDimension: m\nQuantity: length\n\n## Role diameter\nDimension: m\nQuantity: length\n";
    const OBJECT_TEXT: &str = "# Circle\nRunning: circle\n\n## Role radius\nName: Radius\nRunning: radius\n\n## Role diameter\nName: Diameter\nRunning: diameter\n";
    const TEXT: &str = "# Diameter\n\nStatement: Twice the radius.\n";

    fn node(recognized: &str, formula: &str, extra: &str) -> String {
        format!(
            "# circle-diameter\n\nLevel: isced-2\nPrerequisites:\nStart: yes\nSources: book\n\n## Way from-radius\nOutput: circle.diameter\nInputs: circle.radius\nRelation: circle-diameter-radius\nSources: book\nRecognized: {recognized}\n```calc\n{formula}\n```\n```calc\nr |-> r > 0\n```\n{extra}"
        )
    }

    fn pattern(name: &str, body: &str) -> String {
        format!("\n## Pattern {name}\nRelation: circle-diameter-radius\n```calc\n{body}\n```\n")
    }

    fn load_node(text: &str) -> Result<ConceptSet, LoadError> {
        let files: Vec<(&str, &[u8])> = vec![
            ("sources/book.md", SOURCE.as_bytes()),
            ("objects/circle.md", OBJECT.as_bytes()),
            ("objects/circle.en.md", OBJECT_TEXT.as_bytes()),
            ("concepts/circle-diameter/node.md", text.as_bytes()),
            ("concepts/circle-diameter/en.md", TEXT.as_bytes()),
        ];
        load(&files)
    }

    fn kind_of(text: &str) -> LoadErrorKind {
        load_node(text).unwrap_err().kind
    }

    fn equivalent_texts(left: &str, right: &str) -> bool {
        let mut pool = ExprPool::new();
        let left = calc_syntax::parse_expression(&mut pool, left).unwrap();
        let right = calc_syntax::parse_expression(&mut pool, right).unwrap();
        shapes_are_equivalent(&pool, left, right)
    }

    #[test]
    fn recognized_way_and_explicit_pattern_are_both_listed() {
        let set = load_node(&node("yes", "r |-> 2 * r", &pattern("sum", "r |-> r + r"))).unwrap();
        let mut pool = ExprPool::new();
        let identifiers: Vec<(String, bool)> = set
            .patterns(&mut pool)
            .unwrap()
            .into_iter()
            .map(|found| (found.identifier, found.derived))
            .collect();
        assert_eq!(
            identifiers,
            vec![
                ("circle-diameter/from-radius".to_string(), true),
                ("circle-diameter/sum".to_string(), false)
            ]
        );
    }

    #[test]
    fn way_not_recognized_gives_no_pattern() {
        let set = load_node(&node("no", "r |-> 2 * r", "")).unwrap();
        assert!(set.patterns(&mut ExprPool::new()).unwrap().is_empty());
    }

    #[test]
    fn derived_pattern_keeps_variables_and_conditions() {
        let set = load_node(&node("yes", "r |-> 2 * r", "")).unwrap();
        let derived = &set.patterns(&mut ExprPool::new()).unwrap()[0];
        assert_eq!(
            (derived.variables.clone(), derived.conditions.len()),
            (vec!["r".to_string()], 1)
        );
    }

    #[test]
    fn unknown_relation_is_rejected() {
        let text = node("no", "r |-> 2 * r", &pattern("sum", "r |-> r + r")).replace(
            "## Pattern sum\nRelation: circle-diameter-radius",
            "## Pattern sum\nRelation: sphere-volume",
        );
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::UnknownRelation("sphere-volume".to_string())
        );
    }

    #[test]
    fn reserved_parameter_name_is_rejected() {
        assert_eq!(
            kind_of(&node("no", "r |-> 2 * r", &pattern("sum", "e |-> e + e"))),
            LoadErrorKind::ReservedParameter("e".to_string())
        );
    }

    #[test]
    fn label_as_parameter_name_is_rejected() {
        assert_eq!(
            kind_of(&node(
                "no",
                "r |-> 2 * r",
                &pattern("sum", "r1 |-> r1 + r1")
            )),
            LoadErrorKind::ReservedParameter("r1".to_string())
        );
    }

    #[test]
    fn repeated_parameter_name_is_rejected() {
        assert_eq!(
            kind_of(&node(
                "no",
                "r |-> 2 * r",
                &pattern("sum", "x |-> x |-> x + x")
            )),
            LoadErrorKind::DuplicateParameter("x".to_string())
        );
    }

    #[test]
    fn free_name_in_a_pattern_is_rejected() {
        assert_eq!(
            kind_of(&node("no", "r |-> 2 * r", &pattern("sum", "x |-> x + y"))),
            LoadErrorKind::FreeNameInPattern("y".to_string())
        );
    }

    #[test]
    fn unused_variable_is_rejected() {
        assert_eq!(
            kind_of(&node(
                "no",
                "r |-> 2 * r",
                &pattern("sum", "x |-> y |-> x + x")
            )),
            LoadErrorKind::UnusedPatternVariable("y".to_string())
        );
    }

    #[test]
    fn pattern_that_is_only_a_variable_is_rejected() {
        assert_eq!(
            kind_of(&node("no", "r |-> 2 * r", &pattern("any", "x |-> x"))),
            LoadErrorKind::PatternIsVariable
        );
    }

    #[test]
    fn recognized_way_whose_formula_is_only_a_variable_is_rejected() {
        assert_eq!(
            kind_of(&node("yes", "r |-> r", "")),
            LoadErrorKind::PatternIsVariable
        );
    }

    #[test]
    fn guard_with_another_parameter_count_is_rejected() {
        let text = node(
            "no",
            "r |-> 2 * r",
            "\n## Pattern sum\n```calc\nx |-> x + x\n```\n```calc\nx |-> y |-> x > y\n```\n",
        );
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::ParameterCountMismatch {
                expected: 1,
                found: 2
            }
        );
    }

    #[test]
    fn explicit_pattern_repeating_a_recognized_way_is_rejected() {
        assert_eq!(
            kind_of(&node(
                "yes",
                "r |-> 2 * r",
                &pattern("twice", "x |-> x * 2")
            )),
            LoadErrorKind::DuplicatePattern {
                pattern: "circle-diameter/twice".to_string(),
                way: "circle-diameter/from-radius".to_string()
            }
        );
    }

    #[test]
    fn explicit_pattern_repeating_a_way_not_recognized_loads() {
        assert!(load_node(&node("no", "r |-> 2 * r", &pattern("twice", "x |-> x * 2"))).is_ok());
    }

    #[test]
    fn products_match_in_any_operand_order_with_renamed_variables() {
        assert!(equivalent_texts(
            "a |-> b |-> 2 * a * b",
            "x |-> y |-> y * 2 * x"
        ));
    }

    #[test]
    fn differences_keep_their_order() {
        assert!(!equivalent_texts(
            "a |-> b |-> a - 2 * b",
            "x |-> y |-> 2 * x - y"
        ));
    }

    #[test]
    fn chains_of_different_length_do_not_match() {
        assert!(!equivalent_texts(
            "a |-> b |-> a * b * 2",
            "x |-> y |-> (x * y) ^ 2"
        ));
    }

    #[test]
    fn constants_are_never_evaluated() {
        assert!(!equivalent_texts("a |-> a * 4", "x |-> x * 2^2"));
    }

    #[test]
    fn a_shorter_chain_does_not_match_a_longer_one() {
        assert!(!equivalent_texts(
            "a |-> b |-> a * b",
            "x |-> y |-> x * y * 2"
        ));
    }

    #[test]
    fn two_occurrences_of_one_variable_do_not_bind_two_variables() {
        assert!(!equivalent_texts(
            "a |-> b |-> (a - a) - b",
            "x |-> y |-> (x - y) - y"
        ));
    }

    #[test]
    fn one_variable_does_not_stand_for_two() {
        assert!(!equivalent_texts("a |-> b |-> a + b", "x |-> x + x"));
    }
}
