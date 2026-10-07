use std::ops::Range;

use calc_expr::Operator;

use crate::ast::{Ast, AstKind, StatementSyntax};
use crate::lexer::{Token, TokenKind};
use crate::parser::unit_extent;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitEndedAtSpace {
    pub quantity: Range<usize>,
    pub unit: Range<usize>,
    pub operator: Operator,
    pub operator_span: Range<usize>,
    pub left_operand: Range<usize>,
    pub right_operand: Range<usize>,
    pub right_unit: Option<RightUnit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RightUnit {
    pub name: Range<usize>,
    pub unit: Range<usize>,
}

pub(crate) fn statement_value(statement: &StatementSyntax) -> &Ast {
    match statement {
        StatementSyntax::Naming { value, .. }
        | StatementSyntax::FunctionNaming { value, .. }
        | StatementSyntax::Expression(value) => value,
    }
}

pub(crate) fn in_statement(
    tokens: &[Token],
    text_length: usize,
    statement: &StatementSyntax,
) -> Vec<UnitEndedAtSpace> {
    in_expression(tokens, text_length, statement_value(statement))
}

pub(crate) fn in_expression(
    tokens: &[Token],
    text_length: usize,
    expression: &Ast,
) -> Vec<UnitEndedAtSpace> {
    let mut endings = Vec::new();
    collect(tokens, text_length, expression, &mut endings);
    endings.sort_by_key(|ending| ending.operator_span.start);
    endings
}

pub(crate) fn children(node: &Ast) -> Vec<&Ast> {
    match &node.kind {
        AstKind::Number { .. } | AstKind::Typed { .. } | AstKind::Name(_) => Vec::new(),
        AstKind::Group(inner) => vec![inner],
        AstKind::Operation { operands, .. } => operands.iter().collect(),
        AstKind::Quantity { value, .. }
        | AstKind::Conversion { value, .. }
        | AstKind::RadixConversion { value, .. } => vec![value],
        AstKind::Call { arguments, .. } => arguments.iter().collect(),
        AstKind::Lambda { body, .. } => vec![body],
        AstKind::Array { rows } => rows.iter().flatten().collect(),
        AstKind::Binder {
            arguments, body, ..
        } => arguments.iter().chain(std::iter::once(&**body)).collect(),
    }
}

fn collect(tokens: &[Token], text_length: usize, node: &Ast, endings: &mut Vec<UnitEndedAtSpace>) {
    if let AstKind::Operation { operator, operands } = &node.kind
        && matches!(operator, Operator::Mul | Operator::Div)
        && let [left, right] = operands.as_slice()
        && let Some(ending) = ending(tokens, text_length, *operator, left, right)
    {
        endings.push(ending);
    }
    for child in children(node) {
        collect(tokens, text_length, child, endings);
    }
}

fn trailing_quantity(node: &Ast) -> Option<&Ast> {
    match &node.kind {
        AstKind::Quantity { .. } => Some(node),
        AstKind::Operation { operands, .. } => operands
            .last()
            .filter(|last| last.span.end == node.span.end)
            .and_then(trailing_quantity),
        _ => None,
    }
}

fn ending(
    tokens: &[Token],
    text_length: usize,
    operator: Operator,
    left: &Ast,
    right: &Ast,
) -> Option<UnitEndedAtSpace> {
    let quantity = trailing_quantity(left)?;
    let AstKind::Quantity { unit, .. } = &quantity.kind else {
        return None;
    };
    let operator_token = tokens.iter().find(|token| {
        token.span.start >= left.span.end
            && token.span.end <= right.span.start
            && matches!(token.kind, TokenKind::Star | TokenKind::Slash)
    })?;
    if !operator_token.spaced_before {
        return None;
    }
    let unit_start = unit.first()?.span.start;
    Some(UnitEndedAtSpace {
        quantity: quantity.span.clone(),
        unit: unit_start..quantity.span.end,
        operator,
        operator_span: operator_token.span.clone(),
        left_operand: left.span.clone(),
        right_operand: right.span.clone(),
        right_unit: right_unit(tokens, text_length, right),
    })
}

fn right_unit(tokens: &[Token], text_length: usize, right: &Ast) -> Option<RightUnit> {
    let index = tokens
        .iter()
        .position(|token| token.span.start == right.span.start)?;
    let first = &tokens[index];
    if !matches!(first.kind, TokenKind::Identifier(_)) {
        return None;
    }
    let end = unit_extent(tokens, index, text_length)?;
    Some(RightUnit {
        name: first.span.clone(),
        unit: first.span.start..end,
    })
}

#[cfg(test)]
mod tests {
    use calc_expr::{ExprPool, Operator};

    use crate::parse_statement_with_unit_endings;

    use super::*;

    fn endings(text: &str) -> Vec<UnitEndedAtSpace> {
        parse_statement_with_unit_endings(&mut ExprPool::new(), text)
            .unwrap()
            .unit_endings
    }

    #[test]
    fn unit_ended_at_space_before_division_is_recorded() {
        let text = "9.81 m / s^2";
        assert_eq!(
            endings(text),
            vec![UnitEndedAtSpace {
                quantity: 0..6,
                unit: 5..6,
                operator: Operator::Div,
                operator_span: 7..8,
                left_operand: 0..6,
                right_operand: 9..12,
                right_unit: Some(RightUnit {
                    name: 9..10,
                    unit: 9..12,
                }),
            }]
        );
    }

    #[test]
    fn unit_written_without_spaces_records_nothing() {
        assert!(endings("9.81 m/s^2").is_empty());
    }

    #[test]
    fn quantity_inside_a_larger_left_operand_is_recorded_with_that_operand() {
        let ending = endings("2 * 3 m / s").pop().unwrap();
        assert_eq!(ending.quantity, 4..7);
        assert_eq!(ending.left_operand, 0..7);
    }

    #[test]
    fn name_left_of_the_operator_records_nothing() {
        assert!(endings("d / s").is_empty());
    }

    #[test]
    fn parenthesised_quantity_records_nothing() {
        assert!(endings("(9.81 m) / s").is_empty());
    }

    #[test]
    fn right_operand_that_is_not_a_name_has_no_right_unit() {
        assert_eq!(endings("3 m / 2")[0].right_unit, None);
    }

    #[test]
    fn multiplication_in_a_naming_is_recorded() {
        let ending = endings("v = 3 s * t").pop().unwrap();
        assert_eq!(ending.operator, Operator::Mul);
        assert_eq!(ending.right_unit.unwrap().unit, 10..11);
    }

    #[test]
    fn superscript_exponent_belongs_to_the_right_unit() {
        let text = "9.81 m / s\u{00B2}";
        let right = endings(text).pop().unwrap().right_unit.unwrap();
        assert_eq!(&text[right.unit], "s\u{00B2}");
    }
}
