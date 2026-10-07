use calc_expr::{ExprId, ExprPool, Head, NodeView, Operator};
use calc_numbers::{Integer, Number};

fn constant(pool: &mut ExprPool, value: i64) -> Option<ExprId> {
    pool.number(Number::Integer(Integer::from(value))).ok()
}

fn integer_of(pool: &ExprPool, expression: ExprId) -> Option<Integer> {
    match pool.node(expression).ok()? {
        NodeView::Number(number) => match pool.number_value(number).ok()? {
            Number::Integer(value) => Some(value.clone()),
            Number::Rational(_) | Number::F32(_) | Number::F64(_) => None,
        },
        _ => None,
    }
}

fn is_zero(pool: &ExprPool, expression: ExprId) -> bool {
    integer_of(pool, expression).is_some_and(|value| value.is_zero())
}

fn is_one(pool: &ExprPool, expression: ExprId) -> bool {
    integer_of(pool, expression).is_some_and(|value| value == Integer::one())
}

fn apply(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> Option<ExprId> {
    pool.apply(Head::Operator(operator), arguments).ok()
}

fn sum(pool: &mut ExprPool, left: ExprId, right: ExprId) -> Option<ExprId> {
    if is_zero(pool, left) {
        return Some(right);
    }
    if is_zero(pool, right) {
        return Some(left);
    }
    if let (Some(left), Some(right)) = (integer_of(pool, left), integer_of(pool, right)) {
        return pool.number(Number::Integer(&left + &right)).ok();
    }
    apply(pool, Operator::Add, &[left, right])
}

fn difference(pool: &mut ExprPool, left: ExprId, right: ExprId) -> Option<ExprId> {
    if is_zero(pool, right) {
        return Some(left);
    }
    if let (Some(left), Some(right)) = (integer_of(pool, left), integer_of(pool, right)) {
        return pool.number(Number::Integer(&left - &right)).ok();
    }
    if is_zero(pool, left) {
        return apply(pool, Operator::Neg, &[right]);
    }
    apply(pool, Operator::Sub, &[left, right])
}

fn product(pool: &mut ExprPool, left: ExprId, right: ExprId) -> Option<ExprId> {
    if is_zero(pool, left) || is_zero(pool, right) {
        return constant(pool, 0);
    }
    if is_one(pool, left) {
        return Some(right);
    }
    if is_one(pool, right) {
        return Some(left);
    }
    if let (Some(left), Some(right)) = (integer_of(pool, left), integer_of(pool, right)) {
        return pool.number(Number::Integer(&left * &right)).ok();
    }
    apply(pool, Operator::Mul, &[left, right])
}

fn quotient(pool: &mut ExprPool, left: ExprId, right: ExprId) -> Option<ExprId> {
    if is_zero(pool, left) {
        return constant(pool, 0);
    }
    if is_one(pool, right) {
        return Some(left);
    }
    apply(pool, Operator::Div, &[left, right])
}

fn power(pool: &mut ExprPool, base: ExprId, exponent: ExprId) -> Option<ExprId> {
    if is_one(pool, exponent) {
        return Some(base);
    }
    if is_zero(pool, exponent) {
        return constant(pool, 1);
    }
    apply(pool, Operator::Pow, &[base, exponent])
}

pub(crate) fn depends_on(pool: &ExprPool, expression: ExprId, depth: u32) -> bool {
    match pool.node(expression) {
        Ok(NodeView::Bound(index)) => index == depth,
        Ok(NodeView::Apply { arguments, .. }) => arguments
            .to_vec()
            .iter()
            .any(|argument| depends_on(pool, *argument, depth)),
        Ok(NodeView::Bind {
            arguments, body, ..
        }) => {
            arguments
                .to_vec()
                .iter()
                .any(|argument| depends_on(pool, *argument, depth))
                || depends_on(pool, body, depth + 1)
        }
        Ok(NodeView::Array { elements, .. }) => elements
            .to_vec()
            .iter()
            .any(|element| depends_on(pool, *element, depth)),
        Ok(NodeView::Quantity { value, .. }) => depends_on(pool, value, depth),
        _ => false,
    }
}

fn chain(pool: &mut ExprPool, outer: ExprId, inner: ExprId, depth: u32) -> Option<ExprId> {
    let inner_derivative = derivative(pool, inner, depth)?;
    product(pool, outer, inner_derivative)
}

pub(crate) fn folded(pool: &mut ExprPool, expression: ExprId) -> Option<ExprId> {
    let NodeView::Apply { head, arguments } = pool.node(expression).ok()? else {
        return Some(expression);
    };
    let arguments = arguments.to_vec();
    let mut parts = Vec::with_capacity(arguments.len());
    for argument in arguments {
        parts.push(folded(pool, argument)?);
    }
    match (head, parts.as_slice()) {
        (Head::Operator(Operator::Add), [left, right]) => sum(pool, *left, *right),
        (Head::Operator(Operator::Sub), [left, right]) => difference(pool, *left, *right),
        (Head::Operator(Operator::Mul), [left, right]) => product(pool, *left, *right),
        (Head::Operator(Operator::Div), [left, right]) => quotient(pool, *left, *right),
        (Head::Operator(Operator::Pow), [base, exponent]) => power(pool, *base, *exponent),
        _ => pool.apply(head, &parts).ok(),
    }
}

pub fn derivative(pool: &mut ExprPool, expression: ExprId, depth: u32) -> Option<ExprId> {
    let view = pool.node(expression).ok()?;
    let (operator, arguments) = match view {
        NodeView::Bound(index) => {
            return if index == depth {
                constant(pool, 1)
            } else {
                constant(pool, 0)
            };
        }
        NodeView::Number(_) | NodeView::Symbol(_) | NodeView::Quantity { .. } => {
            return constant(pool, 0);
        }
        NodeView::Apply {
            head: Head::Operator(operator),
            arguments,
        } => (operator, arguments.to_vec()),
        NodeView::Apply { .. } | NodeView::Bind { .. } | NodeView::Array { .. } => return None,
    };
    if !depends_on(pool, expression, depth) {
        return constant(pool, 0);
    }
    match (operator, arguments.as_slice()) {
        (Operator::Add, [left, right]) => {
            let (left, right) = (*left, *right);
            let left = derivative(pool, left, depth)?;
            let right = derivative(pool, right, depth)?;
            sum(pool, left, right)
        }
        (Operator::Sub, [left, right]) => {
            let (left, right) = (*left, *right);
            let left = derivative(pool, left, depth)?;
            let right = derivative(pool, right, depth)?;
            difference(pool, left, right)
        }
        (Operator::Neg, [single]) => {
            let single = *single;
            let single = derivative(pool, single, depth)?;
            let zero = constant(pool, 0)?;
            difference(pool, zero, single)
        }
        (Operator::Mul, [left, right]) => {
            let (left, right) = (*left, *right);
            let left_derivative = derivative(pool, left, depth)?;
            let right_derivative = derivative(pool, right, depth)?;
            let first = product(pool, left_derivative, right)?;
            let second = product(pool, left, right_derivative)?;
            sum(pool, first, second)
        }
        (Operator::Div, [left, right]) => {
            let (left, right) = (*left, *right);
            let left_derivative = derivative(pool, left, depth)?;
            let right_derivative = derivative(pool, right, depth)?;
            let first = product(pool, left_derivative, right)?;
            let second = product(pool, left, right_derivative)?;
            let numerator = difference(pool, first, second)?;
            let two = constant(pool, 2)?;
            let denominator = power(pool, right, two)?;
            quotient(pool, numerator, denominator)
        }
        (Operator::Pow, [base, exponent]) => {
            let (base, exponent) = (*base, *exponent);
            let exponent_varies = depends_on(pool, exponent, depth);
            let base_varies = depends_on(pool, base, depth);
            match (base_varies, exponent_varies) {
                (true, false) => {
                    let one = constant(pool, 1)?;
                    let lowered = difference(pool, exponent, one)?;
                    let raised = power(pool, base, lowered)?;
                    let factor = product(pool, exponent, raised)?;
                    chain(pool, factor, base, depth)
                }
                (false, true) => {
                    let logarithm = apply(pool, Operator::Ln, &[base])?;
                    let factor = product(pool, expression, logarithm)?;
                    chain(pool, factor, exponent, depth)
                }
                _ => None,
            }
        }
        (Operator::Exp, [single]) => {
            let single = *single;
            chain(pool, expression, single, depth)
        }
        (Operator::Ln, [single]) => {
            let single = *single;
            let one = constant(pool, 1)?;
            let outer = quotient(pool, one, single)?;
            chain(pool, outer, single, depth)
        }
        (Operator::Sqrt, [single]) => {
            let single = *single;
            let one = constant(pool, 1)?;
            let two = constant(pool, 2)?;
            let doubled = product(pool, two, expression)?;
            let outer = quotient(pool, one, doubled)?;
            chain(pool, outer, single, depth)
        }
        (Operator::Sin, [single]) => {
            let single = *single;
            let outer = apply(pool, Operator::Cos, &[single])?;
            chain(pool, outer, single, depth)
        }
        (Operator::Cos, [single]) => {
            let single = *single;
            let sine = apply(pool, Operator::Sin, &[single])?;
            let zero = constant(pool, 0)?;
            let outer = difference(pool, zero, sine)?;
            chain(pool, outer, single, depth)
        }
        (Operator::Tan, [single]) => {
            let single = *single;
            let cosine = apply(pool, Operator::Cos, &[single])?;
            let two = constant(pool, 2)?;
            let squared = power(pool, cosine, two)?;
            let one = constant(pool, 1)?;
            let outer = quotient(pool, one, squared)?;
            chain(pool, outer, single, depth)
        }
        (Operator::Sinh, [single]) => {
            let single = *single;
            let outer = apply(pool, Operator::Cosh, &[single])?;
            chain(pool, outer, single, depth)
        }
        (Operator::Cosh, [single]) => {
            let single = *single;
            let outer = apply(pool, Operator::Sinh, &[single])?;
            chain(pool, outer, single, depth)
        }
        (Operator::Tanh, [single]) => {
            let single = *single;
            let cosine = apply(pool, Operator::Cosh, &[single])?;
            let two = constant(pool, 2)?;
            let squared = power(pool, cosine, two)?;
            let one = constant(pool, 1)?;
            let outer = quotient(pool, one, squared)?;
            chain(pool, outer, single, depth)
        }
        (Operator::Asin | Operator::Acos, [single]) => {
            let single = *single;
            let one = constant(pool, 1)?;
            let two = constant(pool, 2)?;
            let squared = power(pool, single, two)?;
            let rest = difference(pool, one, squared)?;
            let root = apply(pool, Operator::Sqrt, &[rest])?;
            let one = constant(pool, 1)?;
            let outer = quotient(pool, one, root)?;
            let outer = if operator == Operator::Acos {
                let zero = constant(pool, 0)?;
                difference(pool, zero, outer)?
            } else {
                outer
            };
            chain(pool, outer, single, depth)
        }
        (Operator::Atan, [single]) => {
            let single = *single;
            let one = constant(pool, 1)?;
            let two = constant(pool, 2)?;
            let squared = power(pool, single, two)?;
            let denominator = sum(pool, one, squared)?;
            let one = constant(pool, 1)?;
            let outer = quotient(pool, one, denominator)?;
            chain(pool, outer, single, depth)
        }
        (Operator::Log, [value, base]) => {
            let (value, base) = (*value, *base);
            if depends_on(pool, base, depth) {
                return None;
            }
            let logarithm = apply(pool, Operator::Ln, &[base])?;
            let denominator = product(pool, value, logarithm)?;
            let one = constant(pool, 1)?;
            let outer = quotient(pool, one, denominator)?;
            chain(pool, outer, value, depth)
        }
        (Operator::Log2 | Operator::Log10, [single]) => {
            let single = *single;
            let base = constant(pool, if operator == Operator::Log2 { 2 } else { 10 })?;
            let logarithm = apply(pool, Operator::Ln, &[base])?;
            let denominator = product(pool, single, logarithm)?;
            let one = constant(pool, 1)?;
            let outer = quotient(pool, one, denominator)?;
            chain(pool, outer, single, depth)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_syntax::{PrintMode, parse_expression, print_expression};

    fn differentiated(text: &str) -> Option<String> {
        let mut pool = ExprPool::new();
        let expression = parse_expression(&mut pool, text).ok()?;
        let NodeView::Bind { body, .. } = pool.node(expression).ok()? else {
            return None;
        };
        let derivative = derivative(&mut pool, body, 0)?;
        let symbol = pool
            .intern_symbol("x", calc_expr::SymbolKind::Variable)
            .ok()?;
        let symbol = pool.symbol(symbol).ok()?;
        let closed = crate::rule_search::replace_parameters(&mut pool, derivative, &[symbol], 0)?;
        print_expression(&pool, closed, PrintMode::Ascii).ok()
    }

    #[test]
    fn the_derivative_of_a_power_lowers_the_exponent() {
        assert_eq!(differentiated("diff(x^2, x)").as_deref(), Some("2 * x"));
    }

    #[test]
    fn the_derivative_of_a_constant_is_zero() {
        assert_eq!(differentiated("diff(7, x)").as_deref(), Some("0"));
    }

    #[test]
    fn the_derivative_of_the_variable_itself_is_one() {
        assert_eq!(differentiated("diff(x, x)").as_deref(), Some("1"));
    }

    #[test]
    fn the_derivative_of_a_sum_is_the_sum_of_the_derivatives() {
        assert_eq!(
            differentiated("diff(x^3 + x, x)").as_deref(),
            Some("3 * x^2 + 1")
        );
    }

    #[test]
    fn the_derivative_of_a_product_follows_the_product_rule() {
        assert_eq!(
            differentiated("diff(x * sin(x), x)").as_deref(),
            Some("sin(x) + x * cos(x)")
        );
    }

    #[test]
    fn the_derivative_of_the_natural_logarithm_is_the_reciprocal() {
        assert_eq!(differentiated("diff(ln(x), x)").as_deref(), Some("1 / x"));
    }

    #[test]
    fn the_chain_rule_reaches_into_a_nested_call() {
        assert_eq!(
            differentiated("diff(sin(x^2), x)").as_deref(),
            Some("cos(x^2) * (2 * x)")
        );
    }

    #[test]
    fn an_exponent_that_varies_uses_the_logarithm_of_the_base() {
        assert_eq!(
            differentiated("diff(2^x, x)").as_deref(),
            Some("2^x * ln(2)")
        );
    }

    #[test]
    fn a_base_and_an_exponent_that_both_vary_are_refused() {
        assert_eq!(differentiated("diff(x^x, x)"), None);
    }

    #[test]
    fn an_absolute_value_is_refused_because_it_has_a_corner() {
        assert_eq!(differentiated("diff(abs(x), x)"), None);
    }
}
