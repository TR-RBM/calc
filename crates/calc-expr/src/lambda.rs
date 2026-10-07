use std::collections::{HashMap, HashSet};

use crate::ids::{ExprId, SymbolId};
use crate::node::{BinderKind, NodeView};
use crate::pool::{AccessError, BuildError, ExprPool};
use crate::symbol::{SymbolError, SymbolKind};

const UNNAMED_PARAMETER: &str = "x";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenedLambda {
    pub body: ExprId,
    pub parameters: Vec<SymbolId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LambdaError {
    Access(AccessError),
    Build(BuildError),
    Symbol(SymbolError),
    NotALambda(ExprId),
    ArgumentCount { expected: usize, found: usize },
}

fn leading_lambdas(
    pool: &ExprPool,
    function: ExprId,
    limit: Option<usize>,
) -> Result<(Vec<ExprId>, ExprId), LambdaError> {
    let mut binders = Vec::new();
    let mut body = function;
    while limit.is_none_or(|limit| binders.len() < limit) {
        match pool.node(body).map_err(LambdaError::Access)? {
            NodeView::Bind {
                binder: BinderKind::Lambda,
                body: inner,
                ..
            } => {
                binders.push(body);
                body = inner;
            }
            _ => break,
        }
    }
    Ok((binders, body))
}

fn free_symbol_names(pool: &ExprPool, root: ExprId) -> Result<HashSet<String>, LambdaError> {
    let mut names = HashSet::new();
    let mut visited = HashSet::new();
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if !visited.insert(expression) {
            continue;
        }
        match pool.node(expression).map_err(LambdaError::Access)? {
            NodeView::Symbol(symbol) => {
                names.insert(
                    pool.symbol_name(symbol)
                        .map_err(LambdaError::Symbol)?
                        .to_string(),
                );
            }
            NodeView::Number(_) | NodeView::Bound(_) => {}
            NodeView::Apply { arguments, .. } => pending.extend_from_slice(arguments),
            NodeView::Bind {
                arguments, body, ..
            } => {
                pending.extend_from_slice(arguments);
                pending.push(body);
            }
            NodeView::Quantity { value, .. } => pending.push(value),
            NodeView::Array { elements, .. } => pending.extend_from_slice(elements),
        }
    }
    Ok(names)
}

fn replaced_bound(
    pool: &mut ExprPool,
    expression: ExprId,
    depth: u32,
    values: &[ExprId],
    memo: &mut HashMap<(ExprId, u32), ExprId>,
) -> Result<ExprId, LambdaError> {
    if let Some(known) = memo.get(&(expression, depth)) {
        return Ok(*known);
    }
    let count = u32::try_from(values.len()).unwrap_or(u32::MAX);
    let result = match pool.node(expression).map_err(LambdaError::Access)? {
        NodeView::Number(_) | NodeView::Symbol(_) => expression,
        NodeView::Bound(index) if index < depth => expression,
        NodeView::Bound(index) => {
            let outer = index - depth;
            match values
                .len()
                .checked_sub(usize::try_from(outer).unwrap_or(usize::MAX) + 1)
            {
                Some(position) => shifted(pool, values[position], depth, 0)?,
                None => pool
                    .bound(index.saturating_sub(count))
                    .map_err(LambdaError::Build)?,
            }
        }
        NodeView::Apply { head, arguments } => {
            let arguments = arguments.to_vec();
            let replaced = arguments
                .into_iter()
                .map(|argument| replaced_bound(pool, argument, depth, values, memo))
                .collect::<Result<Vec<_>, _>>()?;
            pool.apply(head, &replaced).map_err(LambdaError::Build)?
        }
        NodeView::Bind {
            binder,
            arguments,
            body,
        } => {
            let arguments = arguments.to_vec();
            let replaced = arguments
                .into_iter()
                .map(|argument| replaced_bound(pool, argument, depth, values, memo))
                .collect::<Result<Vec<_>, _>>()?;
            let body = replaced_bound(pool, body, depth + 1, values, memo)?;
            let name = pool.bound_name(expression).map(str::to_string);
            let rebuilt = pool
                .bind(binder, &replaced, body)
                .map_err(LambdaError::Build)?;
            if let Some(name) = name {
                pool.record_bound_name(rebuilt, &name)
                    .map_err(LambdaError::Access)?;
            }
            rebuilt
        }
        NodeView::Quantity { value, unit } => {
            let value = replaced_bound(pool, value, depth, values, memo)?;
            pool.quantity(value, unit).map_err(LambdaError::Build)?
        }
        NodeView::Array { shape, elements } => {
            let shape = shape.to_vec();
            let elements = elements.to_vec();
            let replaced = elements
                .into_iter()
                .map(|element| replaced_bound(pool, element, depth, values, memo))
                .collect::<Result<Vec<_>, _>>()?;
            pool.array(&shape, &replaced).map_err(LambdaError::Build)?
        }
    };
    memo.insert((expression, depth), result);
    Ok(result)
}

fn shifted(
    pool: &mut ExprPool,
    expression: ExprId,
    amount: u32,
    cutoff: u32,
) -> Result<ExprId, LambdaError> {
    if amount == 0 {
        return Ok(expression);
    }
    match pool.node(expression).map_err(LambdaError::Access)? {
        NodeView::Number(_) | NodeView::Symbol(_) => Ok(expression),
        NodeView::Bound(index) if index < cutoff => Ok(expression),
        NodeView::Bound(index) => pool
            .bound(index.saturating_add(amount))
            .map_err(LambdaError::Build),
        NodeView::Apply { head, arguments } => {
            let arguments = arguments.to_vec();
            let moved = arguments
                .into_iter()
                .map(|argument| shifted(pool, argument, amount, cutoff))
                .collect::<Result<Vec<_>, _>>()?;
            pool.apply(head, &moved).map_err(LambdaError::Build)
        }
        NodeView::Bind {
            binder,
            arguments,
            body,
        } => {
            let arguments = arguments.to_vec();
            let moved = arguments
                .into_iter()
                .map(|argument| shifted(pool, argument, amount, cutoff))
                .collect::<Result<Vec<_>, _>>()?;
            let body = shifted(pool, body, amount, cutoff + 1)?;
            let name = pool.bound_name(expression).map(str::to_string);
            let rebuilt = pool
                .bind(binder, &moved, body)
                .map_err(LambdaError::Build)?;
            if let Some(name) = name {
                pool.record_bound_name(rebuilt, &name)
                    .map_err(LambdaError::Access)?;
            }
            Ok(rebuilt)
        }
        NodeView::Quantity { value, unit } => {
            let value = shifted(pool, value, amount, cutoff)?;
            pool.quantity(value, unit).map_err(LambdaError::Build)
        }
        NodeView::Array { shape, elements } => {
            let shape = shape.to_vec();
            let elements = elements.to_vec();
            let moved = elements
                .into_iter()
                .map(|element| shifted(pool, element, amount, cutoff))
                .collect::<Result<Vec<_>, _>>()?;
            pool.array(&shape, &moved).map_err(LambdaError::Build)
        }
    }
}

fn fresh_variable(
    pool: &mut ExprPool,
    base: &str,
    taken: &mut HashSet<String>,
) -> Result<SymbolId, LambdaError> {
    let mut suffix = 0_u64;
    loop {
        let name = if suffix == 0 {
            base.to_string()
        } else {
            format!("{base}_{suffix}")
        };
        suffix += 1;
        if taken.contains(&name) {
            continue;
        }
        match pool.intern_symbol(&name, SymbolKind::Variable) {
            Ok(symbol) => {
                taken.insert(name);
                return Ok(symbol);
            }
            Err(SymbolError::KindConflict { .. }) => {}
            Err(error) => return Err(LambdaError::Symbol(error)),
        }
    }
}

pub fn open_lambda_chain(
    pool: &mut ExprPool,
    function: ExprId,
) -> Result<OpenedLambda, LambdaError> {
    let (binders, body) = leading_lambdas(pool, function, None)?;
    if binders.is_empty() {
        return Err(LambdaError::NotALambda(function));
    }
    let mut taken = free_symbol_names(pool, body)?;
    let mut parameters = Vec::with_capacity(binders.len());
    for binder in &binders {
        let base = pool
            .bound_name(*binder)
            .unwrap_or(UNNAMED_PARAMETER)
            .to_string();
        parameters.push(fresh_variable(pool, &base, &mut taken)?);
    }
    let values = parameters
        .iter()
        .map(|parameter| pool.symbol(*parameter).map_err(LambdaError::Build))
        .collect::<Result<Vec<_>, _>>()?;
    let body = replaced_bound(pool, body, 0, &values, &mut HashMap::new())?;
    Ok(OpenedLambda { body, parameters })
}

pub fn apply_lambda(
    pool: &mut ExprPool,
    function: ExprId,
    arguments: &[ExprId],
) -> Result<ExprId, LambdaError> {
    let (binders, body) = leading_lambdas(pool, function, Some(arguments.len()))?;
    if binders.len() != arguments.len() {
        return Err(LambdaError::ArgumentCount {
            expected: binders.len(),
            found: arguments.len(),
        });
    }
    replaced_bound(pool, body, 0, arguments, &mut HashMap::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Head;
    use crate::operator::Operator;
    use calc_numbers::Number;

    fn variable(pool: &mut ExprPool, name: &str) -> ExprId {
        let symbol = pool.intern_symbol(name, SymbolKind::Variable).unwrap();
        pool.symbol(symbol).unwrap()
    }

    fn lambda(pool: &mut ExprPool, name: &str, body: ExprId) -> ExprId {
        let binder = pool.bind(BinderKind::Lambda, &[], body).unwrap();
        pool.record_bound_name(binder, name).unwrap();
        binder
    }

    fn apply(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> ExprId {
        pool.apply(Head::Operator(operator), arguments).unwrap()
    }

    fn square_function(pool: &mut ExprPool) -> ExprId {
        let bound = pool.bound(0).unwrap();
        let two = pool.number(Number::from(2)).unwrap();
        let body = apply(pool, Operator::Pow, &[bound, two]);
        lambda(pool, "x", body)
    }

    #[test]
    fn opened_square_function_is_the_body_over_its_named_parameter() {
        let mut pool = ExprPool::new();
        let function = square_function(&mut pool);

        let opened = open_lambda_chain(&mut pool, function).unwrap();

        let x = variable(&mut pool, "x");
        let two = pool.number(Number::from(2)).unwrap();
        assert_eq!(opened.body, apply(&mut pool, Operator::Pow, &[x, two]));
    }

    #[test]
    fn parameters_are_listed_outermost_first() {
        let mut pool = ExprPool::new();
        let outer = pool.bound(1).unwrap();
        let inner = pool.bound(0).unwrap();
        let body = apply(&mut pool, Operator::Sub, &[outer, inner]);
        let inner_lambda = lambda(&mut pool, "y", body);
        let function = lambda(&mut pool, "x", inner_lambda);

        let opened = open_lambda_chain(&mut pool, function).unwrap();

        let names: Vec<&str> = opened
            .parameters
            .iter()
            .map(|parameter| pool.symbol_name(*parameter).unwrap())
            .collect();
        assert_eq!(names, ["x", "y"]);
    }

    #[test]
    fn parameter_name_that_clashes_with_a_free_symbol_is_renamed() {
        let mut pool = ExprPool::new();
        let bound = pool.bound(0).unwrap();
        let free = variable(&mut pool, "x");
        let body = apply(&mut pool, Operator::Add, &[bound, free]);
        let function = lambda(&mut pool, "x", body);

        let opened = open_lambda_chain(&mut pool, function).unwrap();

        assert_eq!(pool.symbol_name(opened.parameters[0]), Ok("x_1"));
    }

    #[test]
    fn opening_a_non_lambda_is_an_error() {
        let mut pool = ExprPool::new();
        let x = variable(&mut pool, "x");

        assert_eq!(
            open_lambda_chain(&mut pool, x),
            Err(LambdaError::NotALambda(x))
        );
    }

    #[test]
    fn applying_the_square_function_substitutes_the_argument() {
        let mut pool = ExprPool::new();
        let function = square_function(&mut pool);
        let t = variable(&mut pool, "t");

        let applied = apply_lambda(&mut pool, function, &[t]).unwrap();

        let two = pool.number(Number::from(2)).unwrap();
        assert_eq!(applied, apply(&mut pool, Operator::Pow, &[t, two]));
    }

    #[test]
    fn applying_with_too_many_arguments_is_an_error() {
        let mut pool = ExprPool::new();
        let function = square_function(&mut pool);
        let t = variable(&mut pool, "t");

        assert_eq!(
            apply_lambda(&mut pool, function, &[t, t]),
            Err(LambdaError::ArgumentCount {
                expected: 1,
                found: 2
            })
        );
    }

    #[test]
    fn open_argument_under_a_binder_of_the_body_is_shifted_past_it() {
        let mut pool = ExprPool::new();
        let inner_bound = pool.bound(0).unwrap();
        let outer_bound = pool.bound(1).unwrap();
        let one = pool.number(Number::from(1)).unwrap();
        let three = pool.number(Number::from(3)).unwrap();
        let summand = apply(&mut pool, Operator::Mul, &[inner_bound, outer_bound]);
        let sum = pool
            .bind(
                BinderKind::Sum(crate::node::ReductionShape::LeftFold),
                &[one, three],
                summand,
            )
            .unwrap();
        let function = lambda(&mut pool, "x", sum);
        let open = pool.bound(0).unwrap();

        let applied = apply_lambda(&mut pool, function, &[open]).unwrap();

        let shifted_open = pool.bound(1).unwrap();
        let expected_summand = apply(&mut pool, Operator::Mul, &[inner_bound, shifted_open]);
        let expected = pool
            .bind(
                BinderKind::Sum(crate::node::ReductionShape::LeftFold),
                &[one, three],
                expected_summand,
            )
            .unwrap();
        assert_eq!(applied, expected);
    }
}
