use std::ops::Range;

use calc_expr::Operator;

use crate::ast::{Ast, AstKind};
use crate::error::{ParseError, ParseErrorKind};
use crate::lexer::{Token, TokenKind};
use crate::names::{KEYWORD_AND, KEYWORD_OR, is_prefix_function};
use crate::unit_endings::children;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmbiguousApplication {
    function_start: usize,
    function_end: usize,
    narrow_start: usize,
    narrow_end: usize,
    wide_start: usize,
    wide_end: usize,
    root_over_primaries: bool,
}

impl AmbiguousApplication {
    pub fn function(&self, text: &str) -> String {
        text.get(self.function_start..self.function_end)
            .unwrap_or_default()
            .to_string()
    }

    pub fn written(&self, text: &str) -> String {
        text.get(self.function_start..self.wide_end)
            .unwrap_or_default()
            .to_string()
    }

    pub fn narrow(&self, text: &str) -> String {
        if self.root_over_primaries {
            return [
                text.get(..self.function_start).unwrap_or_default(),
                "(",
                text.get(self.function_start..self.function_end)
                    .unwrap_or_default(),
                text.get(self.narrow_start..self.narrow_end)
                    .unwrap_or_default(),
                ")",
                text.get(self.narrow_end..).unwrap_or_default(),
            ]
            .concat();
        }
        self.enclosed(text, self.narrow_start, self.narrow_end)
    }

    pub fn wide(&self, text: &str) -> String {
        self.enclosed(text, self.wide_start, self.wide_end)
    }

    fn enclosed(&self, text: &str, start: usize, end: usize) -> String {
        [
            text.get(..self.function_end).unwrap_or_default(),
            "(",
            text.get(start..end).unwrap_or_default(),
            ")",
            text.get(end..).unwrap_or_default(),
        ]
        .concat()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cause {
    Multiplicative,
    Additive,
}

pub(crate) fn check(tokens: &[Token], expression: &Ast) -> Result<(), ParseError> {
    let mut found: Vec<(Range<usize>, AmbiguousApplication)> = Vec::new();
    collect(tokens, expression, &mut found);
    match found.into_iter().min_by_key(|(cause, _)| cause.start) {
        Some((cause, application)) => Err(ParseError::new(
            ParseErrorKind::AmbiguousApplication(application),
            cause,
        )),
        None => Ok(()),
    }
}

fn collect(tokens: &[Token], node: &Ast, found: &mut Vec<(Range<usize>, AmbiguousApplication)>) {
    if let Some(ambiguity) = ambiguity(tokens, node) {
        found.push(ambiguity);
    }
    for child in children(node) {
        collect(tokens, child, found);
    }
}

fn token_at(tokens: &[Token], start: usize) -> Option<usize> {
    tokens.iter().position(|token| token.span.start == start)
}

fn token_after(tokens: &[Token], end: usize) -> Option<usize> {
    tokens.iter().position(|token| token.span.start >= end)
}

fn is_prefix_call(tokens: &[Token], node: &Ast) -> bool {
    let AstKind::Call { name, arguments } = &node.kind else {
        return false;
    };
    let Some(index) = token_at(tokens, node.span.start) else {
        return false;
    };
    let is_call_form = matches!(
        tokens.get(index + 1),
        Some(Token {
            kind: TokenKind::LeftParenthesis,
            spaced_before: false,
            ..
        })
    );
    arguments.len() == 1 && is_prefix_function(name) && !is_call_form
}

fn is_root_sign(tokens: &[Token], node: &Ast) -> bool {
    matches!(
        &node.kind,
        AstKind::Operation {
            operator: Operator::Sqrt,
            ..
        }
    ) && token_at(tokens, node.span.start)
        .is_some_and(|index| tokens[index].kind == TokenKind::SquareRoot)
}

fn starts_application(token: Option<&Token>) -> bool {
    match token.map(|token| &token.kind) {
        Some(TokenKind::SquareRoot) => true,
        Some(TokenKind::Identifier(name)) => is_prefix_function(name),
        _ => false,
    }
}

fn first_primary(node: &Ast) -> &Ast {
    match &node.kind {
        AstKind::Operation {
            operator: Operator::Neg | Operator::Pow | Operator::Factorial,
            operands,
        } => operands.first().map_or(node, first_primary),
        AstKind::Quantity { value, .. } => first_primary(value),
        _ => node,
    }
}

fn has_more_than_one_primary(node: &Ast) -> bool {
    match &node.kind {
        AstKind::Operation {
            operator: Operator::Neg,
            operands,
        } => operands.first().is_some_and(has_more_than_one_primary),
        AstKind::Operation {
            operator: Operator::Pow | Operator::Factorial,
            ..
        }
        | AstKind::Quantity { .. } => true,
        _ => false,
    }
}

fn ambiguity(tokens: &[Token], node: &Ast) -> Option<(Range<usize>, AmbiguousApplication)> {
    let is_root = is_root_sign(tokens, node);
    if !is_root && !is_prefix_call(tokens, node) {
        return None;
    }
    let argument = match &node.kind {
        AstKind::Call { arguments, .. } => arguments.first()?,
        AstKind::Operation { operands, .. } => operands.first()?,
        _ => return None,
    };
    if matches!(argument.kind, AstKind::Group(_)) {
        return None;
    }
    let function = &tokens[token_at(tokens, node.span.start)?];
    if is_root && has_more_than_one_primary(argument) {
        let primary = first_primary(argument);
        let cause = &tokens[token_after(tokens, primary.span.end)?];
        return Some((
            cause.span.clone(),
            AmbiguousApplication {
                function_start: function.span.start,
                function_end: function.span.end,
                narrow_start: primary.span.start,
                narrow_end: primary.span.end,
                wide_start: argument.span.start,
                wide_end: argument.span.end,
                root_over_primaries: true,
            },
        ));
    }
    let cause_index = token_after(tokens, argument.span.end)?;
    let cause_token = &tokens[cause_index];
    let cause = match cause_token.kind {
        TokenKind::Star | TokenKind::Slash | TokenKind::PlusMinus => Cause::Multiplicative,
        TokenKind::Plus | TokenKind::Minus if is_root => Cause::Additive,
        _ => return None,
    };
    let is_product_of_applications = !is_root
        && cause_token.kind == TokenKind::Star
        && starts_application(tokens.get(cause_index + 1));
    if is_product_of_applications {
        return None;
    }
    Some((
        cause_token.span.clone(),
        AmbiguousApplication {
            function_start: function.span.start,
            function_end: function.span.end,
            narrow_start: argument.span.start,
            narrow_end: argument.span.end,
            wide_start: argument.span.start,
            wide_end: wide_end(tokens, cause_index, cause),
            root_over_primaries: false,
        },
    ))
}

fn ends_operand(token: &Token) -> bool {
    matches!(
        token.kind,
        TokenKind::Number(_)
            | TokenKind::TypedLiteral(..)
            | TokenKind::Identifier(_)
            | TokenKind::Superscript(_)
            | TokenKind::Bang
            | TokenKind::RightParenthesis
            | TokenKind::RightBracket
            | TokenKind::Infinity
    )
}

fn ends_wide_argument(tokens: &[Token], index: usize, cause: Cause) -> bool {
    let token = &tokens[index];
    let is_binary_additive = match token.kind {
        TokenKind::Plus => true,
        TokenKind::Minus => index
            .checked_sub(1)
            .is_some_and(|previous| ends_operand(&tokens[previous])),
        _ => false,
    };
    match &token.kind {
        TokenKind::Comma
        | TokenKind::Semicolon
        | TokenKind::DotDot
        | TokenKind::Arrow
        | TokenKind::MapsTo
        | TokenKind::Equals
        | TokenKind::DoubleEquals
        | TokenKind::NotEquals
        | TokenKind::Less
        | TokenKind::LessOrEqual
        | TokenKind::Greater
        | TokenKind::GreaterOrEqual => true,
        TokenKind::Identifier(name) => name == KEYWORD_AND || name == KEYWORD_OR,
        _ => cause == Cause::Multiplicative && is_binary_additive,
    }
}

fn wide_end(tokens: &[Token], cause_index: usize, cause: Cause) -> usize {
    let mut depth = 0usize;
    let mut end = tokens[cause_index].span.end;
    for (index, token) in tokens.iter().enumerate().skip(cause_index) {
        match token.kind {
            TokenKind::LeftParenthesis | TokenKind::LeftBracket => depth += 1,
            TokenKind::RightParenthesis | TokenKind::RightBracket if depth == 0 => return end,
            TokenKind::RightParenthesis | TokenKind::RightBracket => depth -= 1,
            _ if depth == 0 && ends_wide_argument(tokens, index, cause) => return end,
            _ => {}
        }
        end = token.span.end;
    }
    end
}

#[cfg(test)]
mod tests {
    use calc_expr::ExprPool;

    use crate::{ParseErrorKind, parse_expression, parse_statement};

    struct Refusal {
        function: String,
        written: String,
        column_text: String,
        narrow: String,
        wide: String,
    }

    fn refusal(text: &str) -> Refusal {
        let error = parse_statement(&mut ExprPool::new(), text).unwrap_err();
        let ParseErrorKind::AmbiguousApplication(application) = error.kind else {
            panic!("expected an ambiguous application, found {:?}", error.kind);
        };
        Refusal {
            function: application.function(text),
            written: application.written(text),
            column_text: text[error.span].to_string(),
            narrow: application.narrow(text),
            wide: application.wide(text),
        }
    }

    fn readings(text: &str) -> (String, String) {
        let found = refusal(text);
        (found.narrow, found.wide)
    }

    fn accepted(text: &str) -> bool {
        parse_statement(&mut ExprPool::new(), text).is_ok()
    }

    #[test]
    fn named_function_before_a_quotient_is_refused_with_both_readings() {
        let found = refusal("sin \u{03C0}/2");
        assert_eq!(
            (
                found.function.as_str(),
                found.written.as_str(),
                found.column_text.as_str(),
                found.narrow.as_str(),
                found.wide.as_str()
            ),
            (
                "sin",
                "sin \u{03C0}/2",
                "/",
                "sin(\u{03C0})/2",
                "sin(\u{03C0}/2)"
            )
        );
    }

    #[test]
    fn named_function_before_a_product_is_refused() {
        assert_eq!(
            readings("sin x * y"),
            ("sin(x) * y".to_string(), "sin(x * y)".to_string())
        );
    }

    #[test]
    fn named_function_before_a_middle_dot_is_refused() {
        assert_eq!(refusal("sin x \u{00B7} y").column_text, "\u{00B7}");
    }

    #[test]
    fn named_function_before_an_uncertainty_is_refused() {
        assert_eq!(
            readings("sin x \u{00B1} 0.1"),
            (
                "sin(x) \u{00B1} 0.1".to_string(),
                "sin(x \u{00B1} 0.1)".to_string()
            )
        );
    }

    #[test]
    fn wide_reading_stops_before_a_sum() {
        let found = refusal("sin x * y + 1");
        assert_eq!(
            (found.written.as_str(), found.wide.as_str()),
            ("sin x * y", "sin(x * y) + 1")
        );
    }

    #[test]
    fn wide_reading_stops_at_a_closing_bracket() {
        assert_eq!(
            readings("2 * (ln n/2) + 1").1,
            "2 * (ln(n/2)) + 1".to_string()
        );
    }

    #[test]
    fn wide_reading_stops_at_a_relation() {
        assert_eq!(readings("x = sin y/2").1, "x = sin(y/2)".to_string());
    }

    #[test]
    fn wide_reading_keeps_a_bracketed_operand() {
        assert_eq!(
            readings("sin x * (y + 1)").1,
            "sin(x * (y + 1))".to_string()
        );
    }

    #[test]
    fn first_ambiguous_application_by_position_is_named() {
        assert_eq!(refusal("sin x/2 + cos y/2").function, "sin");
    }

    #[test]
    fn refusal_applies_to_a_whole_expression_parse() {
        assert!(matches!(
            parse_expression(&mut ExprPool::new(), "sin \u{03C0}/2")
                .unwrap_err()
                .kind,
            ParseErrorKind::AmbiguousApplication(_)
        ));
    }

    #[test]
    fn named_function_before_a_sum_is_accepted() {
        assert!(accepted("sin x + 1") && accepted("sin x - 1"));
    }

    #[test]
    fn named_function_with_a_power_argument_is_accepted() {
        assert!(accepted("sin x^2") && accepted("sin x\u{00B2}") && accepted("sin n!"));
    }

    #[test]
    fn named_function_before_a_relation_or_conversion_is_accepted() {
        assert!(accepted("sin x = 1/2") && accepted("[sin x, cos x]"));
    }

    #[test]
    fn product_of_applications_is_accepted() {
        assert!(
            accepted("sin x \u{00B7} cos x")
                && accepted("2 * sin x * cos x")
                && accepted("sin x * cos(x)")
                && accepted("sin x * \u{221A}2")
        );
    }

    #[test]
    fn right_application_of_a_product_is_checked_too() {
        assert_eq!(refusal("sin x \u{00B7} cos x/2").function, "cos");
    }

    #[test]
    fn parenthesised_argument_is_accepted() {
        assert!(
            accepted("sin (\u{03C0})/2")
                && accepted("sin(\u{03C0})/2")
                && accepted("sin (\u{03C0}/2)")
        );
    }

    #[test]
    fn root_before_a_quotient_is_refused() {
        assert_eq!(
            readings("\u{221A}2/2"),
            ("\u{221A}(2)/2".to_string(), "\u{221A}(2/2)".to_string())
        );
    }

    #[test]
    fn root_before_a_sum_is_refused_and_its_wide_reading_includes_the_sum() {
        assert_eq!(
            readings("\u{221A}x + 1"),
            ("\u{221A}(x) + 1".to_string(), "\u{221A}(x + 1)".to_string())
        );
    }

    #[test]
    fn root_over_a_power_is_refused_with_the_power_of_a_root_as_narrow_reading() {
        let found = refusal("\u{221A}x^2");
        assert_eq!(
            (
                found.column_text.as_str(),
                found.narrow.as_str(),
                found.wide.as_str()
            ),
            ("^", "(\u{221A}x)^2", "\u{221A}(x^2)")
        );
    }

    #[test]
    fn root_over_a_superscript_is_refused() {
        assert_eq!(
            readings("\u{221A}x\u{00B2}"),
            (
                "(\u{221A}x)\u{00B2}".to_string(),
                "\u{221A}(x\u{00B2})".to_string()
            )
        );
    }

    #[test]
    fn root_over_a_factorial_is_refused() {
        assert_eq!(
            readings("\u{221A}n!"),
            ("(\u{221A}n)!".to_string(), "\u{221A}(n!)".to_string())
        );
    }

    #[test]
    fn root_over_a_quantity_is_refused() {
        assert_eq!(
            readings("\u{221A}2 m"),
            ("(\u{221A}2) m".to_string(), "\u{221A}(2 m)".to_string())
        );
    }

    #[test]
    fn root_over_a_power_before_a_quotient_gives_one_narrow_reading() {
        assert_eq!(
            readings("\u{221A}x^2/2"),
            ("(\u{221A}x)^2/2".to_string(), "\u{221A}(x^2)/2".to_string())
        );
    }

    #[test]
    fn root_alone_or_before_a_relation_is_accepted() {
        assert!(
            accepted("\u{221A}2")
                && accepted("\u{221A}x = 3")
                && accepted("\u{221A}\u{221A}x")
                && accepted("-\u{221A}x")
                && accepted("1 + \u{221A}2")
        );
    }

    #[test]
    fn root_with_a_parenthesised_argument_is_accepted() {
        assert!(
            accepted("\u{221A}(x^2)") && accepted("\u{221A}(2)/2") && accepted("(\u{221A}2)/2")
        );
    }

    #[test]
    fn both_readings_of_every_refused_form_parse_to_different_expressions() {
        for text in [
            "sin \u{03C0}/2",
            "sin x * y",
            "sin x \u{00B1} 0.1",
            "sin x * y + 1",
            "\u{221A}2/2",
            "\u{221A}x + 1",
            "\u{221A}x^2",
            "\u{221A}x\u{00B2}",
            "\u{221A}n!",
            "\u{221A}2 m",
        ] {
            let (narrow, wide) = readings(text);
            let mut pool = ExprPool::new();
            let narrow_parsed = parse_expression(&mut pool, &narrow)
                .unwrap_or_else(|error| panic!("{narrow} does not parse: {error:?}"));
            let wide_parsed = parse_expression(&mut pool, &wide)
                .unwrap_or_else(|error| panic!("{wide} does not parse: {error:?}"));
            assert_ne!(narrow_parsed, wide_parsed, "{text}");
        }
    }
}
