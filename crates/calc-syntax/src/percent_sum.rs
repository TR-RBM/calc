use std::ops::Range;

use calc_expr::Operator;

use crate::ast::{Ast, AstKind};
use crate::error::{ParseError, ParseErrorKind};
use crate::unit_endings::children;

const PRODUCT: &str = " * ";
const SPACE: &str = " ";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PercentInASum {
    start: usize,
    sign_start: usize,
    sign_end: usize,
    end: usize,
    is_addition: bool,
}

impl PercentInASum {
    pub fn written(&self, text: &str) -> String {
        text.get(self.start..self.end)
            .unwrap_or_default()
            .to_string()
    }

    pub fn literal(&self, text: &str) -> String {
        [
            text.get(..self.sign_end).unwrap_or_default(),
            "(",
            text.get(self.sign_end..self.end).unwrap_or_default(),
            ")",
            text.get(self.end..).unwrap_or_default(),
        ]
        .concat()
    }

    pub fn relative(&self, text: &str) -> String {
        let sign = if self.is_addition { "+" } else { "-" };
        let left = text.get(self.start..self.sign_start).unwrap_or_default();
        [
            text.get(..self.start).unwrap_or_default(),
            left,
            SPACE,
            sign,
            SPACE,
            left,
            PRODUCT,
            text.get(self.sign_end..self.end).unwrap_or_default(),
            text.get(self.end..).unwrap_or_default(),
        ]
        .concat()
    }
}

pub(crate) fn check(expression: &Ast) -> Result<(), ParseError> {
    let mut found: Vec<(Range<usize>, PercentInASum)> = Vec::new();
    collect(expression, &mut found);
    match found.into_iter().min_by_key(|(span, _)| span.start) {
        Some((span, percent)) => Err(ParseError::new(
            ParseErrorKind::PercentInASum(percent),
            span,
        )),
        None => Ok(()),
    }
}

fn collect(node: &Ast, found: &mut Vec<(Range<usize>, PercentInASum)>) {
    if let Some(percent) = percent_in_a_sum(node) {
        found.push(percent);
    }
    for child in children(node) {
        collect(child, found);
    }
}

fn is_the_number_one(node: &Ast) -> bool {
    let AstKind::Number { text, is_negative } = &node.kind else {
        return false;
    };
    let (whole, fraction) = text.split_once('.').unwrap_or((text.as_str(), ""));
    !is_negative
        && whole.trim_start_matches('0') == "1"
        && fraction.chars().all(|digit| digit == '0')
}

fn percent_in_a_sum(node: &Ast) -> Option<(Range<usize>, PercentInASum)> {
    let AstKind::Operation { operator, operands } = &node.kind else {
        return None;
    };
    let is_addition = match operator {
        Operator::Add => true,
        Operator::Sub => false,
        _ => return None,
    };
    let [left, right] = operands.as_slice() else {
        return None;
    };
    if is_the_number_one(left) {
        return None;
    }
    let AstKind::Operation {
        operator: Operator::Percent,
        operands: percent_operands,
    } = &right.kind
    else {
        return None;
    };
    if !matches!(
        percent_operands.first().map(|operand| &operand.kind),
        Some(AstKind::Number { .. })
    ) {
        return None;
    }
    Some((
        node.span.clone(),
        PercentInASum {
            start: node.span.start,
            sign_start: left.span.end,
            sign_end: right.span.start,
            end: right.span.end,
            is_addition,
        },
    ))
}
