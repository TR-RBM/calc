use std::ops::Range;

use calc_expr::Operator;

use crate::ast::{Ast, AstKind};
use crate::error::{ParseError, ParseErrorKind};
use crate::unit_endings::children;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FractionBeforeUnit {
    numerator_start: usize,
    denominator_start: usize,
    denominator_end: usize,
    quantity_end: usize,
}

impl FractionBeforeUnit {
    pub fn written(&self, text: &str) -> String {
        text.get(self.numerator_start..self.quantity_end)
            .unwrap_or_default()
            .to_string()
    }

    pub fn as_a_fraction(&self, text: &str) -> String {
        [
            text.get(..self.numerator_start).unwrap_or_default(),
            "(",
            text.get(self.numerator_start..self.denominator_end)
                .unwrap_or_default(),
            ")",
            text.get(self.denominator_end..).unwrap_or_default(),
        ]
        .concat()
    }

    pub fn as_a_reciprocal(&self, text: &str) -> String {
        [
            text.get(..self.denominator_start).unwrap_or_default(),
            "(",
            text.get(self.denominator_start..self.quantity_end)
                .unwrap_or_default(),
            ")",
            text.get(self.quantity_end..).unwrap_or_default(),
        ]
        .concat()
    }
}

pub(crate) fn check(expression: &Ast) -> Result<(), ParseError> {
    let mut found: Vec<(Range<usize>, FractionBeforeUnit)> = Vec::new();
    collect(expression, &mut found);
    match found.into_iter().min_by_key(|(span, _)| span.start) {
        Some((span, fraction)) => Err(ParseError::new(
            ParseErrorKind::FractionBeforeUnit(fraction),
            span,
        )),
        None => Ok(()),
    }
}

fn collect(node: &Ast, found: &mut Vec<(Range<usize>, FractionBeforeUnit)>) {
    if let Some(fraction) = fraction_before_unit(node) {
        found.push(fraction);
    }
    for child in children(node) {
        collect(child, found);
    }
}

fn is_numeral(node: &Ast) -> bool {
    matches!(node.kind, AstKind::Number { .. })
}

fn numerator_before_unit(node: &Ast) -> Option<&Ast> {
    let numerator = match &node.kind {
        AstKind::Operation {
            operator: Operator::Mul,
            operands,
        } => operands.last()?,
        _ => node,
    };
    (!holds_a_unit(numerator)).then_some(numerator)
}

fn holds_a_unit(node: &Ast) -> bool {
    matches!(
        node.kind,
        AstKind::Quantity { .. } | AstKind::Conversion { .. }
    ) || children(node).into_iter().any(holds_a_unit)
}

fn numeral_under_a_unit(node: &Ast) -> Option<&Ast> {
    match &node.kind {
        AstKind::Quantity { value, .. } => Some(value.as_ref()).filter(|value| is_numeral(value)),
        AstKind::Operation {
            operator: Operator::Percent,
            operands,
        } => {
            let operand = operands.first()?;
            if is_numeral(operand) {
                Some(operand)
            } else {
                numeral_under_a_unit(operand)
            }
        }
        _ => None,
    }
}

fn fraction_before_unit(node: &Ast) -> Option<(Range<usize>, FractionBeforeUnit)> {
    let AstKind::Operation { operator, operands } = &node.kind else {
        return None;
    };
    if *operator != Operator::Div {
        return None;
    }
    let [numerator, denominator] = operands.as_slice() else {
        return None;
    };
    let numerator = numerator_before_unit(numerator)?;
    let value = numeral_under_a_unit(denominator)?;
    Some((
        numerator.span.start..node.span.end,
        FractionBeforeUnit {
            numerator_start: numerator.span.start,
            denominator_start: value.span.start,
            denominator_end: value.span.end,
            quantity_end: denominator.span.end,
        },
    ))
}
