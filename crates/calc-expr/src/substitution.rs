use std::collections::{HashMap, HashSet};

use crate::ids::{ExprId, SymbolId};
use crate::node::NodeView;
use crate::pool::{AccessError, BuildError, ExprPool};
use crate::symbol::SymbolKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubstitutionError {
    Access(AccessError),
    Build(BuildError),
    ReplacementNotClosed(ExprId),
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

fn post_order(pool: &ExprPool, root: ExprId) -> Result<Vec<ExprId>, AccessError> {
    let mut order = Vec::new();
    let mut visited: HashSet<ExprId> = HashSet::new();
    let mut pending = vec![(root, false)];
    while let Some((expression, children_are_done)) = pending.pop() {
        if children_are_done {
            order.push(expression);
            continue;
        }
        if !visited.insert(expression) {
            continue;
        }
        let view = pool.node(expression)?;
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

pub fn is_closed(pool: &ExprPool, expression: ExprId) -> Result<bool, AccessError> {
    let mut loose_depth: HashMap<ExprId, u64> = HashMap::new();
    for node in post_order(pool, expression)? {
        let view = pool.node(node)?;
        let depth_of = |child: &ExprId| loose_depth.get(child).copied().unwrap_or(0);
        let depth = match &view {
            NodeView::Bound(index) => u64::from(*index) + 1,
            NodeView::Bind {
                arguments, body, ..
            } => arguments
                .iter()
                .map(depth_of)
                .chain([depth_of(body).saturating_sub(1)])
                .max()
                .unwrap_or(0),
            _ => children(&view).iter().map(depth_of).max().unwrap_or(0),
        };
        loose_depth.insert(node, depth);
    }
    Ok(loose_depth.get(&expression).copied().unwrap_or(0) == 0)
}

pub fn substitute_symbols(
    pool: &mut ExprPool,
    root: ExprId,
    replacements: &HashMap<SymbolId, ExprId>,
) -> Result<ExprId, SubstitutionError> {
    for replacement in replacements.values() {
        if !is_closed(pool, *replacement).map_err(SubstitutionError::Access)? {
            return Err(SubstitutionError::ReplacementNotClosed(*replacement));
        }
    }
    let order = post_order(pool, root).map_err(SubstitutionError::Access)?;
    let mut rebuilt: HashMap<ExprId, ExprId> = HashMap::new();
    for node in order {
        let new_id = rebuild(pool, node, replacements, &rebuilt)?;
        rebuilt.insert(node, new_id);
    }
    rebuilt
        .get(&root)
        .copied()
        .ok_or(SubstitutionError::Access(AccessError::UnknownExprId(root)))
}

fn rebuild(
    pool: &mut ExprPool,
    node: ExprId,
    replacements: &HashMap<SymbolId, ExprId>,
    rebuilt: &HashMap<ExprId, ExprId>,
) -> Result<ExprId, SubstitutionError> {
    let mapped = |child: &ExprId| rebuilt.get(child).copied().unwrap_or(*child);
    let view = pool.node(node).map_err(SubstitutionError::Access)?;
    let result = match view {
        NodeView::Symbol(symbol) => match replacements.get(&symbol) {
            Some(replacement) => return Ok(*replacement),
            None => return Ok(node),
        },
        NodeView::Number(_) | NodeView::Bound(_) => return Ok(node),
        NodeView::Apply { head, arguments } => {
            let arguments: Vec<ExprId> = arguments.iter().map(mapped).collect();
            pool.apply(head, &arguments)
        }
        NodeView::Bind {
            binder,
            arguments,
            body,
        } => {
            let arguments: Vec<ExprId> = arguments.iter().map(mapped).collect();
            let body = mapped(&body);
            let bound_name = pool.bound_name(node).map(str::to_string);
            let binder_node = pool
                .bind(binder, &arguments, body)
                .map_err(SubstitutionError::Build)?;
            if let Some(name) = bound_name {
                pool.record_bound_name(binder_node, &name)
                    .map_err(SubstitutionError::Access)?;
            }
            Ok(binder_node)
        }
        NodeView::Quantity { value, unit } => pool.quantity(mapped(&value), unit),
        NodeView::Array { shape, elements } => {
            let shape = shape.to_vec();
            let elements: Vec<ExprId> = elements.iter().map(mapped).collect();
            pool.array(&shape, &elements)
        }
    };
    result.map_err(SubstitutionError::Build)
}

pub fn bound_by_name(pool: &mut ExprPool, root: ExprId) -> Result<ExprId, SubstitutionError> {
    let mut scopes: Vec<String> = Vec::new();
    bound_within(pool, root, &mut scopes)
}

fn bound_within(
    pool: &mut ExprPool,
    node: ExprId,
    scopes: &mut Vec<String>,
) -> Result<ExprId, SubstitutionError> {
    let view = pool.node(node).map_err(SubstitutionError::Access)?;
    let result = match view {
        NodeView::Symbol(symbol) => {
            if scopes.is_empty() || pool.symbol_kind(symbol) != Ok(SymbolKind::Variable) {
                return Ok(node);
            }
            let name = pool
                .symbol_name(symbol)
                .map_err(|_| SubstitutionError::Access(AccessError::UnknownExprId(node)))?
                .to_owned();
            let Some(depth) = scopes.iter().rev().position(|scope| *scope == name) else {
                return Ok(node);
            };
            let index = u32::try_from(depth)
                .map_err(|_| SubstitutionError::Access(AccessError::UnknownExprId(node)))?;
            pool.bound(index)
        }
        NodeView::Number(_) | NodeView::Bound(_) => return Ok(node),
        NodeView::Apply { head, arguments } => {
            let arguments = arguments.to_vec();
            let mut rebuilt = Vec::with_capacity(arguments.len());
            for argument in arguments {
                rebuilt.push(bound_within(pool, argument, scopes)?);
            }
            pool.apply(head, &rebuilt)
        }
        NodeView::Bind {
            binder,
            arguments,
            body,
        } => {
            let arguments = arguments.to_vec();
            let mut rebuilt = Vec::with_capacity(arguments.len());
            for argument in arguments {
                rebuilt.push(bound_within(pool, argument, scopes)?);
            }
            let bound_name = pool.bound_name(node).map(str::to_string);
            scopes.push(bound_name.clone().unwrap_or_default());
            let body = bound_within(pool, body, scopes);
            scopes.pop();
            let binder_node = pool
                .bind(binder, &rebuilt, body?)
                .map_err(SubstitutionError::Build)?;
            if let Some(name) = bound_name {
                pool.record_bound_name(binder_node, &name)
                    .map_err(SubstitutionError::Access)?;
            }
            Ok(binder_node)
        }
        NodeView::Quantity { value, unit } => {
            let value = bound_within(pool, value, scopes)?;
            pool.quantity(value, unit)
        }
        NodeView::Array { shape, elements } => {
            let shape = shape.to_vec();
            let elements = elements.to_vec();
            let mut rebuilt = Vec::with_capacity(elements.len());
            for element in elements {
                rebuilt.push(bound_within(pool, element, scopes)?);
            }
            pool.array(&shape, &rebuilt)
        }
    };
    result.map_err(SubstitutionError::Build)
}

pub fn without_measurement_marks(
    pool: &mut ExprPool,
    root: ExprId,
) -> Result<ExprId, SubstitutionError> {
    let mut marks: Vec<SymbolId> = Vec::new();
    for node in post_order(pool, root).map_err(SubstitutionError::Access)? {
        if let NodeView::Symbol(symbol) = pool.node(node).map_err(SubstitutionError::Access)?
            && pool.symbol_kind(symbol) == Ok(SymbolKind::Measurement)
        {
            marks.push(symbol);
        }
    }
    if marks.is_empty() {
        return Ok(root);
    }
    let anonymous = pool
        .anonymous_measurement()
        .map_err(SubstitutionError::Build)?;
    let replacements: HashMap<SymbolId, ExprId> =
        marks.into_iter().map(|mark| (mark, anonymous)).collect();
    substitute_symbols(pool, root, &replacements)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{BinderKind, Head};
    use crate::operator::Operator;
    use crate::symbol::SymbolKind;
    use calc_numbers::Number;

    fn variable(pool: &mut ExprPool, name: &str) -> (SymbolId, ExprId) {
        let symbol = pool.intern_symbol(name, SymbolKind::Variable).unwrap();
        (symbol, pool.symbol(symbol).unwrap())
    }

    fn integer(pool: &mut ExprPool, value: i64) -> ExprId {
        pool.number(Number::from(value)).unwrap()
    }

    fn apply(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> ExprId {
        pool.apply(Head::Operator(operator), arguments).unwrap()
    }

    #[test]
    fn symbol_is_replaced_in_place_keeping_the_written_shape() {
        let mut pool = ExprPool::new();
        let (x, x_node) = variable(&mut pool, "x");
        let one = integer(&mut pool, 1);
        let two = integer(&mut pool, 2);
        let sum = apply(&mut pool, Operator::Add, &[x_node, one]);
        let replacements = HashMap::from([(x, two)]);

        let substituted = substitute_symbols(&mut pool, sum, &replacements).unwrap();

        assert_eq!(substituted, apply(&mut pool, Operator::Add, &[two, one]));
    }

    #[test]
    fn symbols_without_replacement_stay() {
        let mut pool = ExprPool::new();
        let (x, _) = variable(&mut pool, "x");
        let (_, y_node) = variable(&mut pool, "y");
        let one = integer(&mut pool, 1);
        let replacements = HashMap::from([(x, one)]);

        let substituted = substitute_symbols(&mut pool, y_node, &replacements).unwrap();

        assert_eq!(substituted, y_node);
    }

    #[test]
    fn substitution_is_simultaneous() {
        let mut pool = ExprPool::new();
        let (x, x_node) = variable(&mut pool, "x");
        let (y, y_node) = variable(&mut pool, "y");
        let sum = apply(&mut pool, Operator::Add, &[x_node, y_node]);
        let replacements = HashMap::from([(x, y_node), (y, x_node)]);

        let substituted = substitute_symbols(&mut pool, sum, &replacements).unwrap();

        assert_eq!(
            substituted,
            apply(&mut pool, Operator::Add, &[y_node, x_node])
        );
    }

    #[test]
    fn replacement_under_a_binder_needs_no_shift_and_keeps_the_bound_name() {
        let mut pool = ExprPool::new();
        let (x, x_node) = variable(&mut pool, "x");
        let bound = pool.bound(0).unwrap();
        let body = apply(&mut pool, Operator::Mul, &[bound, x_node]);
        let lambda = pool.bind(BinderKind::Lambda, &[], body).unwrap();
        pool.record_bound_name(lambda, "t").unwrap();
        let three = integer(&mut pool, 3);
        let replacements = HashMap::from([(x, three)]);

        let substituted = substitute_symbols(&mut pool, lambda, &replacements).unwrap();

        let expected_body = apply(&mut pool, Operator::Mul, &[bound, three]);
        let expected = pool.bind(BinderKind::Lambda, &[], expected_body).unwrap();
        assert_eq!(
            (substituted, pool.bound_name(substituted)),
            (expected, Some("t"))
        );
    }

    #[test]
    fn open_replacement_is_rejected() {
        let mut pool = ExprPool::new();
        let (x, x_node) = variable(&mut pool, "x");
        let loose = pool.bound(0).unwrap();
        let replacements = HashMap::from([(x, loose)]);

        let result = substitute_symbols(&mut pool, x_node, &replacements);

        assert_eq!(result, Err(SubstitutionError::ReplacementNotClosed(loose)));
    }

    #[test]
    fn binder_closes_its_own_variable() {
        let mut pool = ExprPool::new();
        let bound = pool.bound(0).unwrap();
        let lambda = pool.bind(BinderKind::Lambda, &[], bound).unwrap();

        assert_eq!(
            (is_closed(&pool, bound), is_closed(&pool, lambda)),
            (Ok(false), Ok(true))
        );
    }

    #[test]
    fn index_pointing_past_the_binder_is_open() {
        let mut pool = ExprPool::new();
        let outer = pool.bound(1).unwrap();
        let lambda = pool.bind(BinderKind::Lambda, &[], outer).unwrap();

        assert_eq!(is_closed(&pool, lambda), Ok(false));
    }

    #[test]
    fn unknown_root_is_an_access_error() {
        let mut other = ExprPool::new();
        let mut last = integer(&mut other, 0);
        for value in 1..5 {
            last = integer(&mut other, value);
        }
        let mut pool = ExprPool::new();

        let result = substitute_symbols(&mut pool, last, &HashMap::new());

        assert_eq!(
            result,
            Err(SubstitutionError::Access(AccessError::UnknownExprId(last)))
        );
    }
}
