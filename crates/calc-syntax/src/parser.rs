use std::ops::Range;

use calc_expr::{
    IntegerTypeSpelling, LARGEST_INTEGER_WIDTH, LimitSide, Operator, ReductionShape, SortMethod,
    SortOrder, SortPartition, SortSpec,
};

use crate::ast::{Ast, AstKind, BinderSyntax, RadixTarget, StatementSyntax, UnitFactorSyntax};
use crate::builder::DEPTH_LIMIT;
use crate::error::{Keyword, ParseError, ParseErrorKind};
use crate::names::{integer_type, radix_base};

const BYTES_TARGET: &str = "bytes";
const BIG_ENDIAN: &str = "be";
const LITTLE_ENDIAN: &str = "le";
const RECIPROCAL_NUMERATOR: &str = "1";
use crate::lexer::{Token, TokenKind};
use crate::names::{
    BASE_KEYWORD, BOTH_SIDES_VALUE, COVERAGE_KEYWORD, DECREASING_VALUE, DEGREE_SIGN,
    DERIVATIVE_FORM, DERIVATIVE_NAME, FIRST_VALUE, HALVING_VALUE, HOARE_VALUE, INCREASING_VALUE,
    INFINITY_NAME, INTEGRAL_NAME, KEYWORD_AND, KEYWORD_NOT, KEYWORD_OR, LAST_VALUE,
    LEFT_FOLD_VALUE, LEFT_SIDE_VALUE, LIMIT_FORM, LIMIT_KEYWORD, LIMIT_NAME, LOMUTO_VALUE,
    ORDER_KEYWORD, PARTITION_KEYWORD, PIVOT_KEYWORD, PRODUCT_NAME, RANDOM_VALUE, RIGHT_SIDE_VALUE,
    ROOT_NAME, SEED_KEYWORD, SHAPE_KEYWORD, SIDE_KEYWORD, SORT_VARIABLE, SUM_NAME, TAYLOR_NAME,
    is_constant_name, is_keyword, is_prefix_function, sort_method_for_name, sort_with_form,
};

pub const NESTING_LIMIT: usize = 64;

pub const CHAIN_LIMIT: usize = 512;

pub(crate) struct Parser<'tokens> {
    tokens: &'tokens [Token],
    position: usize,
    end: usize,
    text_length: usize,
    open: Vec<usize>,
}

pub(crate) fn unit_extent(tokens: &[Token], index: usize, text_length: usize) -> Option<usize> {
    let mut parser = Parser::new(tokens, text_length);
    parser.position = index;
    parser.parse_unit_expression().ok()?;
    parser
        .tokens
        .get(parser.position.checked_sub(1)?)
        .map(|token| token.span.end)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnitSuffix {
    Any,
    DegreeOnly,
    None,
}

struct KeywordArgument {
    name: String,
    span: Range<usize>,
    value: Ast,
}

struct CallArguments {
    positional: Vec<Ast>,
    keywords: Vec<KeywordArgument>,
}

impl<'tokens> Parser<'tokens> {
    pub(crate) fn new(tokens: &'tokens [Token], text_length: usize) -> Self {
        Self {
            tokens,
            position: 0,
            end: tokens.len(),
            text_length,
            open: Vec::new(),
        }
    }

    fn deeper<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        if self.open.len() >= NESTING_LIMIT {
            return Err(self.error_here(ParseErrorKind::NestedTooDeeply {
                limit: NESTING_LIMIT,
            }));
        }
        self.open.push(self.opening_offset());
        let parsed = parse(self);
        self.open.pop();
        parsed
    }

    fn opening_offset(&self) -> usize {
        self.position
            .checked_sub(1)
            .and_then(|before| self.tokens.get(before))
            .map_or_else(|| self.start_offset(), |token| token.span.start)
    }

    fn chained(&self, joined: usize) -> Result<(), ParseError> {
        if joined >= CHAIN_LIMIT {
            return Err(self.error_here(ParseErrorKind::ChainTooLong { limit: CHAIN_LIMIT }));
        }
        Ok(())
    }

    fn peek(&self) -> Option<&'tokens Token> {
        if self.position < self.end {
            self.tokens.get(self.position)
        } else {
            None
        }
    }

    fn peek_at(&self, offset: usize) -> Option<&'tokens Token> {
        let index = self.position + offset;
        if index < self.end {
            self.tokens.get(index)
        } else {
            None
        }
    }

    fn peek_kind(&self) -> Option<&'tokens TokenKind> {
        self.peek().map(|token| &token.kind)
    }

    fn advance(&mut self) -> Option<&'tokens Token> {
        let token = self.peek()?;
        self.position += 1;
        Some(token)
    }

    fn end_span(&self) -> Range<usize> {
        let at = self
            .tokens
            .get(self.end.saturating_sub(1))
            .filter(|_| self.end > 0)
            .map_or(self.text_length, |token| token.span.end);
        at..at
    }

    fn error_here(&self, kind: ParseErrorKind) -> ParseError {
        match self.peek() {
            Some(token) => ParseError::new(kind, token.span.clone()),
            None => ParseError::new(ParseErrorKind::UnexpectedEnd, self.end_span()),
        }
    }

    fn unexpected(&self) -> ParseError {
        let follows_a_number = self
            .position
            .checked_sub(1)
            .and_then(|previous| self.tokens.get(previous))
            .is_some_and(|previous| matches!(previous.kind, TokenKind::Number(_)));
        if follows_a_number
            && let Some(Token {
                kind: TokenKind::TypedLiteral(crate::lexer::TypedLiteralKind::Chemistry, _),
                span,
                ..
            }) = self.peek()
        {
            return ParseError::new(
                ParseErrorKind::Chemistry(
                    crate::chemistry::ChemistryProblem::CoefficientOutsideLiteral,
                ),
                span.clone(),
            );
        }
        if follows_a_number
            && let Some(Token {
                kind: TokenKind::TypedLiteral(crate::lexer::TypedLiteralKind::Nuclear, _),
                span,
                ..
            }) = self.peek()
        {
            return ParseError::new(
                ParseErrorKind::Nuclear(crate::nuclear::NuclearProblem::CoefficientOutsideLiteral),
                span.clone(),
            );
        }
        self.error_here(ParseErrorKind::UnexpectedToken)
    }

    fn expect(&mut self, kind: &TokenKind) -> Result<&'tokens Token, ParseError> {
        match self.peek() {
            Some(token) if &token.kind == kind => {
                self.position += 1;
                Ok(token)
            }
            _ => Err(self.unexpected()),
        }
    }

    fn expect_identifier(&mut self) -> Result<(String, Range<usize>), ParseError> {
        match self.peek() {
            Some(Token {
                kind: TokenKind::Identifier(name),
                span,
                ..
            }) => {
                self.position += 1;
                Ok((name.clone(), span.clone()))
            }
            _ => Err(self.unexpected()),
        }
    }

    fn is_identifier(&self, offset: usize, name: &str) -> bool {
        matches!(self.peek_at(offset), Some(Token { kind: TokenKind::Identifier(found), .. }) if found == name)
    }

    fn span_from(&self, start: usize) -> Range<usize> {
        let end = self
            .tokens
            .get(self.position.saturating_sub(1))
            .map_or(start, |token| token.span.end);
        start..end.max(start)
    }

    fn start_offset(&self) -> usize {
        self.peek()
            .map_or_else(|| self.end_span().start, |token| token.span.start)
    }

    pub(crate) fn parse_statement(mut self) -> Result<StatementSyntax, ParseError> {
        let statement = if let Some((name, name_span, parameters)) = self.naming_header() {
            let value = self.parse_expression()?;
            match parameters {
                None => StatementSyntax::Naming {
                    name,
                    name_span,
                    value,
                },
                Some(parameters) => StatementSyntax::FunctionNaming {
                    name,
                    name_span,
                    parameters,
                    value,
                },
            }
        } else {
            StatementSyntax::Expression(self.parse_expression()?)
        };
        self.finish()?;
        Ok(statement)
    }

    pub(crate) fn parse_complete_expression(mut self) -> Result<Ast, ParseError> {
        let expression = self.parse_expression()?;
        self.finish()?;
        Ok(expression)
    }

    fn finish(&self) -> Result<(), ParseError> {
        match self.peek() {
            None => Ok(()),
            Some(_) => Err(self.unexpected()),
        }
    }

    fn naming_header(&mut self) -> Option<(String, Range<usize>, Option<Vec<String>>)> {
        let Some(Token {
            kind: TokenKind::Identifier(name),
            span,
            ..
        }) = self.peek_at(0)
        else {
            return None;
        };
        if matches!(
            self.peek_at(1).map(|token| &token.kind),
            Some(TokenKind::Equals)
        ) {
            self.position += 2;
            return Some((name.clone(), span.clone(), None));
        }
        if !matches!(
            self.peek_at(1),
            Some(Token {
                kind: TokenKind::LeftParenthesis,
                spaced_before: false,
                ..
            })
        ) {
            return None;
        }
        let mut offset = 2;
        let mut parameters = Vec::new();
        loop {
            match self.peek_at(offset).map(|token| &token.kind) {
                Some(TokenKind::Identifier(parameter)) => parameters.push(parameter.clone()),
                _ => return None,
            }
            offset += 1;
            match self.peek_at(offset).map(|token| &token.kind) {
                Some(TokenKind::Comma) => offset += 1,
                Some(TokenKind::RightParenthesis) => break,
                _ => return None,
            }
        }
        if !matches!(
            self.peek_at(offset + 1).map(|token| &token.kind),
            Some(TokenKind::Equals)
        ) {
            return None;
        }
        self.position += offset + 2;
        Some((name.clone(), span.clone(), Some(parameters)))
    }

    fn parse_expression(&mut self) -> Result<Ast, ParseError> {
        self.deeper(Self::parse_expression_within_limit)
    }

    fn parse_expression_within_limit(&mut self) -> Result<Ast, ParseError> {
        if let (
            Some(Token {
                kind: TokenKind::Identifier(parameter),
                span,
                ..
            }),
            Some(TokenKind::MapsTo),
        ) = (self.peek_at(0), self.peek_at(1).map(|token| &token.kind))
        {
            let start = span.start;
            self.position += 2;
            let body = self.parse_expression()?;
            return Ok(Ast::new(
                AstKind::Lambda {
                    parameter: parameter.clone(),
                    body: Box::new(body),
                },
                self.span_from(start),
            ));
        }
        self.parse_or()
    }

    fn operation(
        &self,
        operator: Operator,
        operands: Vec<Ast>,
        start: usize,
    ) -> Result<Ast, ParseError> {
        let built = Ast::new(
            AstKind::Operation { operator, operands },
            self.span_from(start),
        );
        if built.depth > DEPTH_LIMIT {
            let at = self.open.last().copied().unwrap_or(built.span.start);
            return Err(ParseError::new(
                ParseErrorKind::ExpressionTooDeep { limit: DEPTH_LIMIT },
                at..at,
            ));
        }
        Ok(built)
    }

    fn parse_or(&mut self) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        let mut left = self.parse_and()?;
        let mut joined = 0;
        while self.is_identifier(0, KEYWORD_OR) {
            self.chained(joined)?;
            joined += 1;
            self.advance();
            let right = self.parse_and()?;
            left = self.operation(Operator::Or, vec![left, right], start)?;
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        let mut left = self.parse_not()?;
        let mut joined = 0;
        while self.is_identifier(0, KEYWORD_AND) {
            self.chained(joined)?;
            joined += 1;
            self.advance();
            let right = self.parse_not()?;
            left = self.operation(Operator::And, vec![left, right], start)?;
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        if self.is_identifier(0, KEYWORD_NOT) {
            self.advance();
            let operand = self.parse_not()?;
            return self.operation(Operator::Not, vec![operand], start);
        }
        self.parse_relation()
    }

    fn relation_operator(&self) -> Option<Operator> {
        match self.peek_kind()? {
            TokenKind::Equals | TokenKind::DoubleEquals => Some(Operator::Equal),
            TokenKind::NotEquals => Some(Operator::NotEqual),
            TokenKind::Less => Some(Operator::Less),
            TokenKind::LessOrEqual => Some(Operator::LessOrEqual),
            TokenKind::Greater => Some(Operator::Greater),
            TokenKind::GreaterOrEqual => Some(Operator::GreaterOrEqual),
            _ => None,
        }
    }

    fn parse_relation(&mut self) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        let left = self.parse_conversion()?;
        let Some(operator) = self.relation_operator() else {
            return Ok(left);
        };
        self.advance();
        let right = self.parse_conversion()?;
        if self.relation_operator().is_some() {
            return Err(self.error_here(ParseErrorKind::ChainedRelation));
        }
        self.operation(operator, vec![left, right], start)
    }

    fn parse_conversion(&mut self) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        let mut value = self.parse_additive()?;
        while matches!(self.peek_kind(), Some(TokenKind::Arrow)) {
            self.advance();
            if let Some(target) = self.parse_radix_target()? {
                value = Ast::new(
                    AstKind::RadixConversion {
                        value: Box::new(value),
                        target,
                    },
                    self.span_from(start),
                );
                continue;
            }
            let unit = self.parse_conversion_unit()?;
            value = Ast::new(
                AstKind::Conversion {
                    value: Box::new(value),
                    unit,
                },
                self.span_from(start),
            );
        }
        Ok(value)
    }

    fn parse_radix_target(&mut self) -> Result<Option<RadixTarget>, ParseError> {
        let Some(TokenKind::Identifier(name)) = self.peek_kind() else {
            return Ok(None);
        };
        if let Some((bits, signed)) = integer_type(name) {
            self.advance();
            return Ok(Some(RadixTarget::Type { bits, signed }));
        }
        if name == BYTES_TARGET {
            self.advance();
            let big_endian = match self.peek_kind() {
                Some(TokenKind::Identifier(order)) if order == BIG_ENDIAN => true,
                Some(TokenKind::Identifier(order)) if order == LITTLE_ENDIAN => false,
                _ => {
                    let at = self
                        .peek_at(0)
                        .map_or_else(|| self.end_span(), |token| token.span.clone());
                    return Err(ParseError::new(ParseErrorKind::ByteOrderMissing, at));
                }
            };
            self.advance();
            let (bits, spelling) = self.parse_optional_type()?;
            return Ok(Some(RadixTarget::Bytes {
                big_endian,
                bits,
                spelling,
            }));
        }
        let Some(base) = radix_base(name) else {
            return Ok(None);
        };
        self.advance();
        let (bits, spelling) = self.parse_optional_type()?;
        Ok(Some(RadixTarget::Radix {
            base,
            bits,
            spelling,
        }))
    }

    fn parse_optional_type(&mut self) -> Result<(u32, IntegerTypeSpelling), ParseError> {
        match self.peek_kind() {
            Some(TokenKind::Identifier(name)) => match integer_type(name) {
                Some((bits, signed)) => {
                    self.advance();
                    Ok((bits, IntegerTypeSpelling::named(signed)))
                }
                None => Ok((0, IntegerTypeSpelling::Width)),
            },
            Some(TokenKind::Number(text)) => {
                let bits = text
                    .parse::<u32>()
                    .ok()
                    .filter(|bits| (1..=LARGEST_INTEGER_WIDTH).contains(bits))
                    .ok_or_else(|| self.error_here(ParseErrorKind::TypeExpected))?;
                self.advance();
                Ok((bits, IntegerTypeSpelling::Width))
            }
            _ => Ok((0, IntegerTypeSpelling::Width)),
        }
    }

    fn parse_additive(&mut self) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        let mut left = self.parse_multiplicative()?;
        let mut joined = 0;
        loop {
            let operator = match self.peek_kind() {
                Some(TokenKind::Plus) => Operator::Add,
                Some(TokenKind::Minus) => Operator::Sub,
                _ => return Ok(left),
            };
            self.chained(joined)?;
            joined += 1;
            self.advance();
            let right = self.parse_multiplicative()?;
            left = self.operation(operator, vec![left, right], start)?;
        }
    }

    fn parse_multiplicative(&mut self) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        let mut left = self.parse_uncertain()?;
        let mut joined = 0;
        loop {
            let operator = match self.peek_kind() {
                Some(TokenKind::Star) => Operator::Mul,
                Some(TokenKind::Slash) => Operator::Div,
                _ => return Ok(left),
            };
            self.chained(joined)?;
            joined += 1;
            self.advance();
            let right = self.parse_uncertain()?;
            left = self.operation(operator, vec![left, right], start)?;
        }
    }

    fn parse_uncertain(&mut self) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        let value = self.parse_unary(UnitSuffix::Any)?;
        if !matches!(self.peek_kind(), Some(TokenKind::PlusMinus)) {
            return Ok(value);
        }
        self.advance();
        let value_has_its_unit = matches!(value.kind, AstKind::Quantity { .. });
        if value_has_its_unit {
            let uncertainty = self.parse_unary(UnitSuffix::Any)?;
            return self.operation(Operator::Uncertain, vec![value, uncertainty], start);
        }
        let uncertainty = self.parse_unary(UnitSuffix::None)?;
        let group = if self.at_coverage_factor() {
            self.position += 3;
            let coverage = self.parse_plain_number()?;
            self.expect(&TokenKind::RightParenthesis)?;
            self.operation(
                Operator::UncertainExpanded,
                vec![value, uncertainty, coverage],
                start,
            )
        } else {
            self.operation(Operator::Uncertain, vec![value, uncertainty], start)
        }?;
        self.parse_unit_suffix(group, start)
    }

    fn at_coverage_factor(&self) -> bool {
        matches!(self.peek_kind(), Some(TokenKind::LeftParenthesis))
            && self.is_identifier(1, COVERAGE_KEYWORD)
            && matches!(
                self.peek_at(2).map(|token| &token.kind),
                Some(TokenKind::Equals)
            )
    }

    fn parse_plain_number(&mut self) -> Result<Ast, ParseError> {
        match self.peek() {
            Some(Token {
                kind: TokenKind::Number(text),
                span,
                ..
            }) => {
                self.position += 1;
                Ok(Ast::new(
                    AstKind::Number {
                        text: text.clone(),
                        is_negative: false,
                    },
                    span.clone(),
                ))
            }
            _ => Err(self.unexpected()),
        }
    }

    fn folds_into_literal(&self) -> bool {
        matches!(
            self.peek_at(1),
            Some(Token {
                kind: TokenKind::Number(_),
                spaced_before: false,
                ..
            })
        ) && !matches!(
            self.peek_at(2).map(|token| &token.kind),
            Some(
                TokenKind::Caret | TokenKind::Superscript(_) | TokenKind::Bang | TokenKind::Percent
            )
        )
    }

    fn parse_unary(&mut self, allow_unit: UnitSuffix) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        match self.peek_kind() {
            Some(TokenKind::Minus) if self.folds_into_literal() => {
                self.advance();
                let Some(Token {
                    kind: TokenKind::Number(text),
                    ..
                }) = self.advance()
                else {
                    return Err(self.unexpected());
                };
                let literal = Ast::new(
                    AstKind::Number {
                        text: text.clone(),
                        is_negative: true,
                    },
                    self.span_from(start),
                );
                match allow_unit {
                    UnitSuffix::Any => self.parse_unit_suffix(literal, start),
                    UnitSuffix::DegreeOnly if self.at_unspaced_degree_sign() => {
                        self.parse_unit_suffix(literal, start)
                    }
                    _ => Ok(literal),
                }
            }
            Some(TokenKind::Minus) => {
                self.advance();
                let operand = self.deeper(|parser| parser.parse_unary(allow_unit))?;
                self.operation(Operator::Neg, vec![operand], start)
            }
            Some(TokenKind::Plus) => {
                self.advance();
                self.deeper(|parser| parser.parse_unary(allow_unit))
            }
            Some(TokenKind::SquareRoot) => {
                self.advance();
                let operand = self.deeper(|parser| parser.parse_unary(allow_unit))?;
                self.operation(Operator::Sqrt, vec![operand], start)
            }
            _ => self.parse_prefix_application(allow_unit),
        }
    }

    fn starts_primary(token: Option<&Token>) -> bool {
        matches!(
            token.map(|token| &token.kind),
            Some(
                TokenKind::Number(_)
                    | TokenKind::TypedLiteral(..)
                    | TokenKind::Identifier(_)
                    | TokenKind::LeftParenthesis
                    | TokenKind::LeftBracket
                    | TokenKind::Infinity
            )
        )
    }

    fn parse_prefix_application(&mut self, allow_unit: UnitSuffix) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        if let Some(Token {
            kind: TokenKind::Identifier(name),
            ..
        }) = self.peek()
            && is_prefix_function(name)
        {
            let next = self.peek_at(1);
            let is_call = matches!(
                next,
                Some(Token {
                    kind: TokenKind::LeftParenthesis,
                    spaced_before: false,
                    ..
                })
            );
            if !is_call && Self::starts_primary(next) {
                self.advance();
                let argument = self.parse_power(UnitSuffix::DegreeOnly)?;
                return Ok(Ast::new(
                    AstKind::Call {
                        name: name.clone(),
                        arguments: vec![argument],
                    },
                    self.span_from(start),
                ));
            }
        }
        self.parse_power(allow_unit)
    }

    fn parse_power(&mut self, allow_unit: UnitSuffix) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        let mut base = self.parse_postfix(allow_unit)?;
        loop {
            match self.peek_kind() {
                Some(TokenKind::Superscript(value)) => {
                    let span = self.advance().map_or(0..0, |token| token.span.clone());
                    let exponent = Ast::new(
                        AstKind::Number {
                            text: value.unsigned_abs().to_string(),
                            is_negative: *value < 0,
                        },
                        span,
                    );
                    base = self.operation(Operator::Pow, vec![base, exponent], start)?;
                }
                Some(TokenKind::Caret) => {
                    self.advance();
                    let exponent = match self.peek_kind() {
                        Some(TokenKind::Minus | TokenKind::Plus) => {
                            self.deeper(|parser| parser.parse_unary(UnitSuffix::None))?
                        }
                        _ => self.deeper(|parser| parser.parse_power(UnitSuffix::None))?,
                    };
                    return self.operation(Operator::Pow, vec![base, exponent], start);
                }
                _ => return Ok(base),
            }
        }
    }

    fn parse_postfix(&mut self, allow_unit: UnitSuffix) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        let primary = self.parse_primary()?;
        let accepts_unit = match &primary.kind {
            AstKind::Number { .. } | AstKind::Typed { .. } | AstKind::Group(_) => true,
            AstKind::Name(name) => is_constant_name(name),
            _ => false,
        };
        let takes_unit = match allow_unit {
            UnitSuffix::Any => accepts_unit,
            UnitSuffix::DegreeOnly => accepts_unit && self.at_unspaced_degree_sign(),
            UnitSuffix::None => false,
        };
        let mut value = if takes_unit {
            self.parse_unit_suffix(primary, start)?
        } else {
            primary
        };
        while let Some(kind) = self.peek_kind() {
            let operator = match kind {
                TokenKind::Bang => Operator::Factorial,
                TokenKind::Percent => Operator::Percent,
                _ => break,
            };
            self.advance();
            value = self.operation(operator, vec![value], start)?;
        }
        Ok(value)
    }

    fn at_unspaced_degree_sign(&self) -> bool {
        matches!(
            self.peek(),
            Some(Token { kind: TokenKind::Identifier(name), spaced_before: false, .. })
                if name == DEGREE_SIGN
        )
    }

    fn parse_unit_suffix(&mut self, value: Ast, start: usize) -> Result<Ast, ParseError> {
        let at_unit = matches!(
            self.peek(),
            Some(Token { kind: TokenKind::Identifier(name), spaced_before, .. })
                if !is_keyword(name) && (*spaced_before || name == DEGREE_SIGN)
        );
        if !at_unit {
            return Ok(value);
        }
        let unit = self.parse_unit_expression()?;
        Ok(Ast::new(
            AstKind::Quantity {
                value: Box::new(value),
                unit,
            },
            self.span_from(start),
        ))
    }

    fn unspaced(&self, offset: usize) -> Option<&'tokens Token> {
        self.peek_at(offset).filter(|token| !token.spaced_before)
    }

    fn parse_conversion_unit(&mut self) -> Result<Vec<UnitFactorSyntax>, ParseError> {
        let opens_with_one = matches!(
            self.peek_kind(),
            Some(TokenKind::Number(text)) if text == RECIPROCAL_NUMERATOR
        ) && matches!(
            self.unspaced(1).map(|token| &token.kind),
            Some(TokenKind::Slash)
        ) && matches!(
            self.unspaced(2).map(|token| &token.kind),
            Some(TokenKind::Identifier(_) | TokenKind::LeftParenthesis)
        );
        if !opens_with_one {
            return self.parse_unit_expression();
        }
        self.advance();
        self.advance();
        let factors = self.parse_unit_term(true)?;
        self.parse_unit_expression_after(factors)
    }

    fn parse_unit_expression(&mut self) -> Result<Vec<UnitFactorSyntax>, ParseError> {
        let factors = self.parse_unit_term(false)?;
        self.parse_unit_expression_after(factors)
    }

    fn parse_unit_expression_after(
        &mut self,
        mut factors: Vec<UnitFactorSyntax>,
    ) -> Result<Vec<UnitFactorSyntax>, ParseError> {
        loop {
            let is_divisor = match self.unspaced(0).map(|token| &token.kind) {
                Some(TokenKind::Star) => false,
                Some(TokenKind::Slash) => true,
                _ => return Ok(factors),
            };
            if !matches!(
                self.unspaced(1).map(|token| &token.kind),
                Some(TokenKind::Identifier(_) | TokenKind::LeftParenthesis)
            ) {
                return Ok(factors);
            }
            let group_opens_with_a_number = matches!(
                self.unspaced(1).map(|token| &token.kind),
                Some(TokenKind::LeftParenthesis)
            ) && matches!(
                self.peek_at(2).map(|token| &token.kind),
                Some(TokenKind::Number(_))
            );
            if group_opens_with_a_number {
                return Ok(factors);
            }
            self.advance();
            factors.extend(self.parse_unit_term(is_divisor)?);
        }
    }

    fn parse_unit_term(&mut self, is_divisor: bool) -> Result<Vec<UnitFactorSyntax>, ParseError> {
        if !matches!(
            self.unspaced(0).map(|token| &token.kind),
            Some(TokenKind::LeftParenthesis)
        ) {
            let (name, span) = self.expect_identifier()?;
            let exponent = self.parse_unit_exponent()?;
            return Ok(vec![UnitFactorSyntax {
                name,
                exponent,
                is_divisor,
                span,
            }]);
        }
        self.advance();
        let grouped = self.deeper(Self::parse_unit_expression)?;
        let closing = self.unspaced(0).map(|token| &token.kind);
        if !matches!(closing, Some(TokenKind::RightParenthesis)) {
            return Err(self.unexpected());
        }
        self.advance();
        let exponent = self.parse_unit_exponent()?;
        let sign = if is_divisor { -1 } else { 1 };
        grouped
            .into_iter()
            .map(|factor| {
                let inner = if factor.is_divisor {
                    -factor.exponent
                } else {
                    factor.exponent
                };
                let signed = inner
                    .checked_mul(exponent)
                    .and_then(|product| product.checked_mul(sign))
                    .ok_or_else(|| {
                        ParseError::new(ParseErrorKind::UnitExponentOutOfRange, factor.span.clone())
                    })?;
                Ok(UnitFactorSyntax {
                    exponent: signed.abs(),
                    is_divisor: signed < 0,
                    ..factor
                })
            })
            .collect()
    }

    fn parse_unit_exponent(&mut self) -> Result<i64, ParseError> {
        match self.unspaced(0).map(|token| &token.kind) {
            Some(TokenKind::Superscript(value)) => {
                let value = *value;
                self.advance();
                Ok(value)
            }
            Some(TokenKind::Caret) => {
                self.advance();
                let is_negative = matches!(
                    self.unspaced(0).map(|token| &token.kind),
                    Some(TokenKind::Minus)
                );
                if is_negative {
                    self.advance();
                }
                match self.unspaced(0) {
                    Some(Token {
                        kind: TokenKind::Number(text),
                        span,
                        ..
                    }) => {
                        let is_whole = text.chars().all(|character| character.is_ascii_digit());
                        let kind = if is_whole {
                            ParseErrorKind::UnitExponentOutOfRange
                        } else {
                            ParseErrorKind::UnitExponentNotWhole
                        };
                        let magnitude = text
                            .parse::<i64>()
                            .map_err(|_| ParseError::new(kind, span.clone()))?;
                        self.advance();
                        Ok(if is_negative { -magnitude } else { magnitude })
                    }
                    _ => Err(self.unexpected()),
                }
            }
            _ => Ok(1),
        }
    }

    fn parse_primary(&mut self) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        let Some(token) = self.peek() else {
            return Err(self.unexpected());
        };
        match &token.kind {
            TokenKind::Number(text) => {
                self.advance();
                Ok(Ast::new(
                    AstKind::Number {
                        text: text.clone(),
                        is_negative: false,
                    },
                    token.span.clone(),
                ))
            }
            TokenKind::TypedLiteral(kind, content) => {
                self.advance();
                Ok(Ast::new(
                    AstKind::Typed {
                        kind: kind.clone(),
                        content: content.clone(),
                    },
                    token.span.clone(),
                ))
            }
            TokenKind::Infinity => {
                self.advance();
                Ok(Ast::new(
                    AstKind::Name(INFINITY_NAME.to_string()),
                    token.span.clone(),
                ))
            }
            TokenKind::LeftParenthesis => {
                self.advance();
                let inner = self.parse_expression()?;
                self.expect(&TokenKind::RightParenthesis)?;
                Ok(Ast::new(
                    AstKind::Group(Box::new(inner)),
                    self.span_from(start),
                ))
            }
            TokenKind::LeftBracket => self.parse_array(),
            TokenKind::Integral => self.parse_integral_form(),
            TokenKind::Summation => self.parse_sum_form(false),
            TokenKind::ProductSign => self.parse_sum_form(true),
            TokenKind::Identifier(name) => self.parse_identifier(name),
            _ => Err(self.unexpected()),
        }
    }

    fn parse_identifier(&mut self, name: &str) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        if name == LIMIT_FORM && self.peek_at(1).is_some_and(|token| token.spaced_before) {
            return self.parse_limit_form();
        }
        if name == DERIVATIVE_FORM && self.at_derivative_form() {
            return self.parse_derivative_form();
        }
        let is_call = matches!(
            self.peek_at(1),
            Some(Token {
                kind: TokenKind::LeftParenthesis,
                spaced_before: false,
                ..
            })
        );
        self.advance();
        if !is_call {
            return Ok(Ast::new(
                AstKind::Name(name.to_string()),
                self.span_from(start),
            ));
        }
        self.advance();
        let arguments = self.parse_call_arguments()?;
        self.build_call(name, arguments, start)
    }

    fn parse_call_arguments(&mut self) -> Result<CallArguments, ParseError> {
        let mut arguments = CallArguments {
            positional: Vec::new(),
            keywords: Vec::new(),
        };
        if matches!(self.peek_kind(), Some(TokenKind::RightParenthesis)) {
            self.advance();
            return Ok(arguments);
        }
        loop {
            let is_keyword_argument = matches!(self.peek_kind(), Some(TokenKind::Identifier(_)))
                && matches!(
                    self.peek_at(1).map(|token| &token.kind),
                    Some(TokenKind::Equals)
                );
            if is_keyword_argument {
                let (keyword, span) = self.expect_identifier()?;
                self.advance();
                if arguments
                    .keywords
                    .iter()
                    .any(|existing| existing.name == keyword)
                {
                    return Err(ParseError::new(ParseErrorKind::DuplicateKeyword, span));
                }
                let value = self.parse_expression()?;
                arguments.keywords.push(KeywordArgument {
                    name: keyword,
                    span,
                    value,
                });
            } else if arguments.keywords.is_empty() {
                arguments.positional.push(self.parse_expression()?);
            } else {
                let start = self.start_offset();
                let positional = self.parse_expression()?;
                return Err(ParseError::new(
                    ParseErrorKind::PositionalAfterKeyword,
                    start..positional.span.end,
                ));
            }
            match self.peek_kind() {
                Some(TokenKind::Comma) => {
                    self.advance();
                }
                Some(TokenKind::RightParenthesis) => {
                    self.advance();
                    return Ok(arguments);
                }
                _ => return Err(self.unexpected()),
            }
        }
    }

    fn build_call(
        &self,
        name: &str,
        arguments: CallArguments,
        start: usize,
    ) -> Result<Ast, ParseError> {
        let span = self.span_from(start);
        if let Some(method) = sort_method_for_name(name) {
            return sort_call(method, arguments, span);
        }
        let binder = match name {
            INTEGRAL_NAME => Some(BinderSyntax::Integral),
            SUM_NAME => Some(BinderSyntax::Sum(ReductionShape::LeftFold)),
            PRODUCT_NAME => Some(BinderSyntax::Product(ReductionShape::LeftFold)),
            LIMIT_NAME => Some(BinderSyntax::Limit(LimitSide::Both)),
            DERIVATIVE_NAME => Some(BinderSyntax::Derivative),
            ROOT_NAME => Some(BinderSyntax::Root),
            TAYLOR_NAME => Some(BinderSyntax::Taylor),
            _ => None,
        };
        let Some(binder) = binder else {
            if let Some(keyword) = arguments.keywords.first() {
                return Err(ParseError::new(
                    ParseErrorKind::UnknownKeyword,
                    keyword.span.clone(),
                ));
            }
            return Ok(Ast::new(
                AstKind::Call {
                    name: name.to_string(),
                    arguments: arguments.positional,
                },
                span,
            ));
        };
        let binder = apply_binder_keywords(binder, &arguments.keywords)?;
        let found = arguments.positional.len();
        let mut positional = arguments.positional.into_iter();
        let (Some(body), Some(variable)) = (positional.next(), positional.next()) else {
            return Err(ParseError::new(
                ParseErrorKind::ArityMismatch { expected: 2, found },
                span,
            ));
        };
        let AstKind::Name(variable) = variable.kind else {
            return Err(ParseError::new(
                ParseErrorKind::UnexpectedToken,
                variable.span,
            ));
        };
        let rest: Vec<Ast> = positional.collect();
        let expected: &[usize] = match binder {
            BinderSyntax::Integral => &[0, 2],
            BinderSyntax::Sum(_) | BinderSyntax::Product(_) | BinderSyntax::Taylor => &[2],
            BinderSyntax::Limit(_) | BinderSyntax::Root => &[1],
            BinderSyntax::Derivative => &[0, 1],
            BinderSyntax::Sort(_) => &[],
        };
        if !expected.contains(&rest.len()) {
            return Err(ParseError::new(
                ParseErrorKind::ArityMismatch {
                    expected: expected.last().copied().unwrap_or(0) + 2,
                    found: rest.len() + 2,
                },
                span,
            ));
        }
        let point_is_variable = binder == BinderSyntax::Derivative && rest.is_empty();
        Ok(Ast::new(
            AstKind::Binder {
                binder,
                variable,
                arguments: rest,
                point_is_variable,
                body: Box::new(body),
            },
            span,
        ))
    }

    fn parse_array(&mut self) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        self.expect(&TokenKind::LeftBracket)?;
        let mut rows = vec![Vec::new()];
        if matches!(self.peek_kind(), Some(TokenKind::RightBracket)) {
            self.advance();
            return Ok(Ast::new(
                AstKind::Array { rows: Vec::new() },
                self.span_from(start),
            ));
        }
        loop {
            let element = self.parse_expression()?;
            if let Some(row) = rows.last_mut() {
                row.push(element);
            }
            match self.peek_kind() {
                Some(TokenKind::Comma) => {
                    self.advance();
                }
                Some(TokenKind::Semicolon) => {
                    self.advance();
                    rows.push(Vec::new());
                }
                Some(TokenKind::RightBracket) => {
                    self.advance();
                    break;
                }
                _ => return Err(self.unexpected()),
            }
        }
        let span = self.span_from(start);
        let columns = rows.first().map_or(0, Vec::len);
        if rows.iter().any(|row| row.len() != columns) {
            return Err(ParseError::new(ParseErrorKind::RaggedArray, span));
        }
        Ok(Ast::new(AstKind::Array { rows }, span))
    }

    fn with_end<T>(
        &mut self,
        end: usize,
        parse: impl FnOnce(&mut Self) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        let saved = self.end;
        self.end = end;
        let result = parse(self).and_then(|value| {
            if self.position == end {
                Ok(value)
            } else {
                Err(self.unexpected())
            }
        });
        self.end = saved;
        result
    }

    fn parse_integral_form(&mut self) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        self.advance();
        let (differential_index, variable) = self.find_differential()?;
        let body = self.with_end(differential_index, Self::parse_expression)?;
        self.advance();
        let mut arguments = Vec::new();
        if matches!(self.peek_kind(), Some(TokenKind::Comma)) {
            self.advance();
            arguments.push(self.parse_additive()?);
            self.expect(&TokenKind::DotDot)?;
            arguments.push(self.parse_additive()?);
        }
        Ok(Ast::new(
            AstKind::Binder {
                binder: BinderSyntax::Integral,
                variable,
                arguments,
                point_is_variable: false,
                body: Box::new(body),
            },
            self.span_from(start),
        ))
    }

    fn find_differential(&self) -> Result<(usize, String), ParseError> {
        let mut depth = 0_usize;
        for index in self.position..self.end {
            let Some(token) = self.tokens.get(index) else {
                break;
            };
            match &token.kind {
                TokenKind::LeftParenthesis | TokenKind::LeftBracket => depth += 1,
                TokenKind::RightParenthesis | TokenKind::RightBracket => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                TokenKind::Identifier(name) if depth == 0 => {
                    let ends_body = index + 1 >= self.end
                        || matches!(
                            self.tokens.get(index + 1).map(|token| &token.kind),
                            Some(
                                TokenKind::Comma
                                    | TokenKind::RightParenthesis
                                    | TokenKind::RightBracket
                            )
                        );
                    if let Some(variable) = name.strip_prefix(DERIVATIVE_FORM)
                        && !variable.is_empty()
                        && ends_body
                        && index > self.position
                    {
                        return Ok((index, variable.to_string()));
                    }
                }
                _ => {}
            }
        }
        Err(self.error_here(ParseErrorKind::MissingDifferential))
    }

    fn parse_sum_form(&mut self, is_product: bool) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        self.advance();
        let body = self.parse_expression()?;
        self.expect(&TokenKind::Comma)?;
        let (variable, _) = self.expect_identifier()?;
        self.expect(&TokenKind::Equals)?;
        let lower = self.parse_additive()?;
        self.expect(&TokenKind::DotDot)?;
        let upper = self.parse_additive()?;
        let binder = if is_product {
            BinderSyntax::Product(ReductionShape::LeftFold)
        } else {
            BinderSyntax::Sum(ReductionShape::LeftFold)
        };
        Ok(Ast::new(
            AstKind::Binder {
                binder,
                variable,
                arguments: vec![lower, upper],
                point_is_variable: false,
                body: Box::new(body),
            },
            self.span_from(start),
        ))
    }

    fn parse_limit_form(&mut self) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        self.advance();
        let body = self.parse_expression()?;
        self.expect(&TokenKind::Comma)?;
        let (variable, _) = self.expect_identifier()?;
        self.expect(&TokenKind::Arrow)?;
        let point = self.parse_additive()?;
        Ok(Ast::new(
            AstKind::Binder {
                binder: BinderSyntax::Limit(LimitSide::Both),
                variable,
                arguments: vec![point],
                point_is_variable: false,
                body: Box::new(body),
            },
            self.span_from(start),
        ))
    }

    fn at_derivative_form(&self) -> bool {
        matches!(
            self.unspaced(1).map(|token| &token.kind),
            Some(TokenKind::Slash)
        ) && matches!(
            self.unspaced(2).map(|token| &token.kind),
            Some(TokenKind::Identifier(name)) if name.len() > DERIVATIVE_FORM.len() && name.starts_with(DERIVATIVE_FORM)
        )
    }

    fn parse_derivative_form(&mut self) -> Result<Ast, ParseError> {
        let start = self.start_offset();
        self.position += 2;
        let (differential, _) = self.expect_identifier()?;
        let variable = differential
            .strip_prefix(DERIVATIVE_FORM)
            .unwrap_or_default()
            .to_string();
        let body = self.parse_prefix_application(UnitSuffix::None)?;
        Ok(Ast::new(
            AstKind::Binder {
                binder: BinderSyntax::Derivative,
                variable,
                arguments: Vec::new(),
                point_is_variable: true,
                body: Box::new(body),
            },
            self.span_from(start),
        ))
    }
}

fn sort_call(
    method: SortMethod,
    arguments: CallArguments,
    span: Range<usize>,
) -> Result<Ast, ParseError> {
    let mut order = SortOrder::Increasing;
    let mut form = None;
    let mut partition = None;
    let mut pivot = None;
    let mut seed = None;
    let form_keyword = match method {
        SortMethod::Bubble(_) => Some(Keyword::Form),
        SortMethod::CocktailShaker(_) => Some(Keyword::ShakerForm),
        SortMethod::OddEven(_) => Some(Keyword::OddEvenForm),
        SortMethod::Comb(_) => Some(Keyword::CombForm),
        SortMethod::Shell(_) => Some(Keyword::Gaps),
        _ => None,
    };
    let is_quick = matches!(method, SortMethod::Quick(_));
    let is_radix = method == SortMethod::Radix;
    let mut base = None;
    let is_bogo = method == SortMethod::Bogo;
    let mut limit = None;
    for keyword in &arguments.keywords {
        let invalid = |named: Keyword| {
            ParseError::new(
                ParseErrorKind::InvalidKeywordValue { keyword: named },
                keyword.value.span.clone(),
            )
        };
        match keyword.name.as_str() {
            ORDER_KEYWORD => {
                order = match keyword_value(&keyword.value) {
                    Some(INCREASING_VALUE) => SortOrder::Increasing,
                    Some(DECREASING_VALUE) => SortOrder::Decreasing,
                    _ => return Err(invalid(Keyword::Order)),
                };
            }
            PARTITION_KEYWORD if is_quick => {
                partition = Some(match keyword_value(&keyword.value) {
                    Some(LOMUTO_VALUE) => LOMUTO_VALUE,
                    Some(HOARE_VALUE) => HOARE_VALUE,
                    _ => return Err(invalid(Keyword::Partition)),
                });
            }
            PIVOT_KEYWORD if is_quick => {
                pivot = Some((
                    match keyword_value(&keyword.value) {
                        Some(FIRST_VALUE) => FIRST_VALUE,
                        Some(LAST_VALUE) => LAST_VALUE,
                        Some(RANDOM_VALUE) => RANDOM_VALUE,
                        _ => return Err(invalid(Keyword::Pivot)),
                    },
                    keyword.value.span.clone(),
                ));
            }
            BASE_KEYWORD if is_radix => {
                let value = whole_seed(&keyword.value).ok_or_else(|| {
                    ParseError::new(ParseErrorKind::InvalidBase, keyword.value.span.clone())
                })?;
                base = Some((value, keyword.value.clone()));
            }
            LIMIT_KEYWORD if is_bogo => {
                let value = whole_seed(&keyword.value).ok_or_else(|| {
                    ParseError::new(ParseErrorKind::InvalidLimit, keyword.value.span.clone())
                })?;
                limit = Some((value, keyword.value.clone()));
            }
            SEED_KEYWORD if is_quick || is_bogo => {
                let value = whole_seed(&keyword.value).ok_or_else(|| {
                    ParseError::new(ParseErrorKind::InvalidSeed, keyword.value.span.clone())
                })?;
                seed = Some((value, keyword.span.clone(), keyword.value.clone()));
            }
            name if form_keyword.is_some_and(|named| named.name() == name) => {
                let named = form_keyword.unwrap_or(Keyword::Form);
                form = Some(
                    keyword_value(&keyword.value)
                        .and_then(|value| sort_with_form(method, value))
                        .ok_or_else(|| invalid(named))?,
                );
            }
            _ => {
                return Err(ParseError::new(
                    ParseErrorKind::UnknownKeyword,
                    keyword.span.clone(),
                ));
            }
        }
    }
    let method = match (method, form) {
        (_, Some(form)) => form,
        (_, None) if form_keyword.is_some() => {
            return Err(ParseError::new(
                ParseErrorKind::MissingKeyword {
                    keyword: form_keyword.unwrap_or(Keyword::Form),
                },
                span,
            ));
        }
        (SortMethod::Quick(_), _) => {
            let missing = |named: Keyword| {
                ParseError::new(
                    ParseErrorKind::MissingKeyword { keyword: named },
                    span.clone(),
                )
            };
            let partition = partition.ok_or_else(|| missing(Keyword::Partition))?;
            let built = if partition == LOMUTO_VALUE {
                LAST_VALUE
            } else {
                FIRST_VALUE
            };
            let (pivot, pivot_span) = pivot.ok_or_else(|| {
                ParseError::new(ParseErrorKind::MissingPivot { built }, span.clone())
            })?;
            if let (Some((_, seed_span, _)), false) = (&seed, pivot == RANDOM_VALUE) {
                return Err(ParseError::new(
                    ParseErrorKind::SeedWithoutRandomPivot,
                    seed_span.clone(),
                ));
            }
            SortMethod::Quick(match (partition, pivot) {
                (LOMUTO_VALUE, LAST_VALUE) => SortPartition::LomutoLast,
                (HOARE_VALUE, FIRST_VALUE) => SortPartition::HoareFirst,
                (LOMUTO_VALUE, RANDOM_VALUE) => {
                    if seed.is_none() {
                        return Err(ParseError::new(ParseErrorKind::MissingSeed, span.clone()));
                    }
                    SortPartition::LomutoRandom
                }
                _ => {
                    return Err(ParseError::new(
                        ParseErrorKind::PivotNotBuiltForPartition,
                        pivot_span,
                    ));
                }
            })
        }
        (SortMethod::Bogo, _) if seed.is_none() => {
            return Err(ParseError::new(
                ParseErrorKind::MissingShuffleSeed,
                span.clone(),
            ));
        }
        (SortMethod::Bogo, _) if limit.is_none() => {
            return Err(ParseError::new(ParseErrorKind::MissingLimit, span.clone()));
        }
        (SortMethod::Radix, _) if base.is_none() => {
            return Err(ParseError::new(ParseErrorKind::MissingBase, span.clone()));
        }
        (method, _) => method,
    };
    let found = arguments.positional.len();
    let mut positional = arguments.positional.into_iter();
    let (Some(list), key, None) = (positional.next(), positional.next(), positional.next()) else {
        return Err(ParseError::new(
            ParseErrorKind::ArityMismatch { expected: 2, found },
            span,
        ));
    };
    let (variable, body) = match key {
        None => (
            SORT_VARIABLE.to_string(),
            Ast::new(AstKind::Name(SORT_VARIABLE.to_string()), span.clone()),
        ),
        Some(Ast {
            kind: AstKind::Lambda { parameter, body },
            ..
        }) => (parameter, *body),
        Some(Ast {
            kind: AstKind::Name(function),
            span: key_span,
            ..
        }) => (
            SORT_VARIABLE.to_string(),
            Ast::new(
                AstKind::Call {
                    name: function,
                    arguments: vec![Ast::new(
                        AstKind::Name(SORT_VARIABLE.to_string()),
                        key_span.clone(),
                    )],
                },
                key_span,
            ),
        ),
        Some(other) => {
            return Err(ParseError::new(ParseErrorKind::UnexpectedToken, other.span));
        }
    };
    Ok(Ast::new(
        AstKind::Binder {
            binder: BinderSyntax::Sort(SortSpec { method, order }),
            variable,
            arguments: std::iter::once(list)
                .chain(seed.map(|(_, _, value)| value))
                .chain(base.map(|(_, value)| value))
                .chain(limit.map(|(_, value)| value))
                .collect(),
            point_is_variable: false,
            body: Box::new(body),
        },
        span,
    ))
}

fn whole_seed(value: &Ast) -> Option<u64> {
    match &value.kind {
        AstKind::Number {
            text,
            is_negative: false,
        } if text.bytes().all(|byte| byte.is_ascii_digit()) => text.parse().ok(),
        _ => None,
    }
}

fn keyword_value(value: &Ast) -> Option<&str> {
    match &value.kind {
        AstKind::Name(name) => Some(name),
        _ => None,
    }
}

fn apply_binder_keywords(
    binder: BinderSyntax,
    keywords: &[KeywordArgument],
) -> Result<BinderSyntax, ParseError> {
    keywords.iter().try_fold(binder, |binder, keyword| {
        let invalid = |named: Keyword| {
            ParseError::new(
                ParseErrorKind::InvalidKeywordValue { keyword: named },
                keyword.value.span.clone(),
            )
        };
        match (binder, keyword.name.as_str()) {
            (BinderSyntax::Sum(_) | BinderSyntax::Product(_), SHAPE_KEYWORD) => {
                let shape = match keyword_value(&keyword.value) {
                    Some(LEFT_FOLD_VALUE) => ReductionShape::LeftFold,
                    Some(HALVING_VALUE) => ReductionShape::Halving,
                    _ => return Err(invalid(Keyword::Shape)),
                };
                Ok(match binder {
                    BinderSyntax::Product(_) => BinderSyntax::Product(shape),
                    _ => BinderSyntax::Sum(shape),
                })
            }
            (BinderSyntax::Limit(_), SIDE_KEYWORD) => match keyword_value(&keyword.value) {
                Some(LEFT_SIDE_VALUE) => Ok(BinderSyntax::Limit(LimitSide::Left)),
                Some(RIGHT_SIDE_VALUE) => Ok(BinderSyntax::Limit(LimitSide::Right)),
                Some(BOTH_SIDES_VALUE) => Ok(BinderSyntax::Limit(LimitSide::Both)),
                _ => Err(invalid(Keyword::Side)),
            },
            _ => Err(ParseError::new(
                ParseErrorKind::UnknownKeyword,
                keyword.span.clone(),
            )),
        }
    })
}
