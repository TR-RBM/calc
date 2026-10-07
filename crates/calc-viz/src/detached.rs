use std::collections::HashMap;

use calc_expr::{ExprId, ExprPool, Head, NodeView, SymbolId};

pub(crate) struct DetachedExpressions {
    pub(crate) pool: ExprPool,
    pub(crate) roots: Vec<ExprId>,
    pub(crate) symbols: Vec<SymbolId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NotDetachable {
    Node(ExprId),
    Symbol(SymbolId),
}

fn copy_symbol(
    source: &ExprPool,
    target: &mut ExprPool,
    symbol: SymbolId,
) -> Result<SymbolId, NotDetachable> {
    let fail = |_| NotDetachable::Symbol(symbol);
    let name = source.symbol_name(symbol).map_err(fail)?;
    let kind = source.symbol_kind(symbol).map_err(fail)?;
    target.intern_symbol(name, kind).map_err(fail)
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

fn copy_node(
    source: &ExprPool,
    target: &mut ExprPool,
    expression: ExprId,
    copies: &HashMap<ExprId, ExprId>,
) -> Result<ExprId, NotDetachable> {
    let fail = || NotDetachable::Node(expression);
    let view = source.node(expression).map_err(|_| fail())?;
    let copied = |child: &ExprId| copies.get(child).copied().ok_or_else(fail);
    let copy = match view {
        NodeView::Number(number) => {
            let value = source.number_value(number).map_err(|_| fail())?.clone();
            target.number(value)
        }
        NodeView::Symbol(symbol) => {
            let symbol = copy_symbol(source, target, symbol)?;
            target.symbol(symbol)
        }
        NodeView::Bound(index) => target.bound(index),
        NodeView::Apply { head, arguments } => {
            let arguments = arguments
                .iter()
                .map(copied)
                .collect::<Result<Vec<ExprId>, NotDetachable>>()?;
            let head = match head {
                Head::Operator(operator) => Head::Operator(operator),
                Head::Function(symbol) => Head::Function(copy_symbol(source, target, symbol)?),
            };
            target.apply(head, &arguments)
        }
        NodeView::Bind {
            binder,
            arguments,
            body,
        } => {
            let arguments = arguments
                .iter()
                .map(copied)
                .collect::<Result<Vec<ExprId>, NotDetachable>>()?;
            let body = copied(&body)?;
            let bound = target.bind(binder, &arguments, body).map_err(|_| fail())?;
            if let Some(name) = source.bound_name(expression) {
                target.record_bound_name(bound, name).map_err(|_| fail())?;
            }
            return Ok(bound);
        }
        NodeView::Quantity { .. } => return Err(fail()),
        NodeView::Array { shape, elements } => {
            let elements = elements
                .iter()
                .map(copied)
                .collect::<Result<Vec<ExprId>, NotDetachable>>()?;
            let shape = shape.to_vec();
            target.array(&shape, &elements)
        }
    };
    copy.map_err(|_| fail())
}

pub(crate) fn detach(
    source: &ExprPool,
    roots: &[ExprId],
    symbols: &[SymbolId],
) -> Result<DetachedExpressions, NotDetachable> {
    let mut target = ExprPool::new();
    let mut copies: HashMap<ExprId, ExprId> = HashMap::new();
    let mut detached_roots = Vec::with_capacity(roots.len());
    for root in roots {
        let mut pending = vec![(*root, false)];
        while let Some((expression, children_are_copied)) = pending.pop() {
            if copies.contains_key(&expression) {
                continue;
            }
            if children_are_copied {
                let copy = copy_node(source, &mut target, expression, &copies)?;
                copies.insert(expression, copy);
                continue;
            }
            let view = source
                .node(expression)
                .map_err(|_| NotDetachable::Node(expression))?;
            pending.push((expression, true));
            for child in children(&view).into_iter().rev() {
                if !copies.contains_key(&child) {
                    pending.push((child, false));
                }
            }
        }
        detached_roots.push(
            copies
                .get(root)
                .copied()
                .ok_or(NotDetachable::Node(*root))?,
        );
    }
    let detached_symbols = symbols
        .iter()
        .map(|symbol| copy_symbol(source, &mut target, *symbol))
        .collect::<Result<Vec<SymbolId>, NotDetachable>>()?;
    Ok(DetachedExpressions {
        pool: target,
        roots: detached_roots,
        symbols: detached_symbols,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_expr::{Operator, SymbolKind};
    use calc_numbers::Number;

    #[test]
    fn detached_expression_keeps_its_structure_and_names() {
        let mut source = ExprPool::new();
        let x = source.intern_symbol("x", SymbolKind::Variable).unwrap();
        let x_node = source.symbol(x).unwrap();
        let half = source.number(Number::F64(0.5)).unwrap();
        let product = source
            .apply(Head::Operator(Operator::Mul), &[x_node, half])
            .unwrap();

        let detached = detach(&source, &[product], &[x]).unwrap();

        let pool = &detached.pool;
        let NodeView::Apply { arguments, .. } = pool.node(detached.roots[0]).unwrap() else {
            panic!("expected an application");
        };
        assert_eq!(
            (
                pool.node(arguments[0]).unwrap(),
                pool.symbol_name(detached.symbols[0]).unwrap()
            ),
            (NodeView::Symbol(detached.symbols[0]), "x")
        );
    }

    #[test]
    fn quantity_is_not_detachable() {
        let mut source = ExprPool::new();
        let one = source.number(Number::from(1_i64)).unwrap();
        let unit = source.units_mut().lookup("m").unwrap();
        let quantity = source.quantity(one, unit).unwrap();

        let result = detach(&source, &[quantity], &[]);

        assert_eq!(result.err(), Some(NotDetachable::Node(quantity)));
    }

    #[test]
    fn symbol_of_another_pool_is_not_detachable() {
        let source = ExprPool::new();
        let mut other = ExprPool::new();
        let foreign = other.intern_symbol("y", SymbolKind::Variable).unwrap();

        let result = detach(&source, &[], &[foreign]);

        assert_eq!(result.err(), Some(NotDetachable::Symbol(foreign)));
    }
}
