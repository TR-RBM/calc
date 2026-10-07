use std::ops::Range;

use calc_expr::{
    BinderKind, BuildError, ExprId, ExprPool, Head, IntegerTypeSpelling, Operator, SymbolKind,
};
use calc_numbers::{Integer, Number};
use calc_units::UnitId;

use crate::ast::{Ast, AstKind, BinderSyntax, RadixTarget, StatementSyntax, UnitFactorSyntax};
use crate::decimal::{integer_from_digits, power_of_ten};
use crate::error::{ParseError, ParseErrorKind};
use crate::lexer::TypedLiteralKind;
use crate::names::integer_type;
use crate::names::{canonical_symbol_name, is_binder_name, is_reserved, operator_for_call};

const ATOMIC_MASS_UNITS: [&str; 2] = ["u", "Da"];

pub const LARGEST_DECIMAL_EXPONENT: i64 = 100_000;

pub const DEPTH_LIMIT: usize = 768;
const NAN_NAME: &str = "nan";
const INFINITY_CONTENT: &str = "inf";
const NEGATIVE_INFINITY_CONTENT: &str = "-inf";
const BITS_PREFIX: &str = "bits:0x";
const HEXADECIMAL_RADIX: u32 = 16;
const QUIET_NAN_F64_BITS: u64 = 0x7ff8_0000_0000_0000;
const QUIET_NAN_F32_BITS: u32 = 0x7fc0_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Statement {
    Naming {
        name: calc_expr::SymbolId,
        value: ExprId,
    },
    FunctionNaming {
        name: calc_expr::SymbolId,
        value: ExprId,
    },
    Expression(ExprId),
}

pub(crate) struct Builder<'pool> {
    pool: &'pool mut ExprPool,
    scope: Vec<String>,
    depth: usize,
}

fn failure(kind: ParseErrorKind, span: &Range<usize>) -> ParseError {
    ParseError::new(kind, span.clone())
}

fn build_failure(error: BuildError, span: &Range<usize>) -> ParseError {
    match error {
        BuildError::ArityMismatch { expected, found } => {
            failure(ParseErrorKind::ArityMismatch { expected, found }, span)
        }
        other => failure(ParseErrorKind::Build(other), span),
    }
}

impl<'pool> Builder<'pool> {
    pub(crate) fn new(pool: &'pool mut ExprPool) -> Self {
        Self {
            pool,
            scope: Vec::new(),
            depth: 0,
        }
    }

    pub(crate) fn build_statement(
        mut self,
        statement: StatementSyntax,
    ) -> Result<Statement, ParseError> {
        match statement {
            StatementSyntax::Expression(expression) => {
                Ok(Statement::Expression(self.build(&expression)?))
            }
            StatementSyntax::Naming {
                name,
                name_span,
                value,
            } => {
                Self::check_nameable(&name, &name_span)?;
                let value = self.build(&value)?;
                let name = self
                    .pool
                    .intern_symbol(&name, SymbolKind::Variable)
                    .map_err(|error| failure(ParseErrorKind::Symbol(error), &name_span))?;
                Ok(Statement::Naming { name, value })
            }
            StatementSyntax::FunctionNaming {
                name,
                name_span,
                parameters,
                value,
            } => {
                Self::check_nameable(&name, &name_span)?;
                for parameter in &parameters {
                    Self::check_nameable(parameter, &value.span)?;
                }
                let symbol = self
                    .pool
                    .intern_symbol(
                        &name,
                        SymbolKind::Function {
                            arity: parameters.len(),
                        },
                    )
                    .map_err(|error| failure(ParseErrorKind::Symbol(error), &name_span))?;
                self.scope.extend(parameters.iter().cloned());
                let mut body = self.build(&value)?;
                for parameter in parameters.iter().rev() {
                    self.scope.pop();
                    body = self
                        .pool
                        .bind(BinderKind::Lambda, &[], body)
                        .map_err(|error| build_failure(error, &value.span))?;
                    self.record_name(body, parameter, &value.span)?;
                }
                Ok(Statement::FunctionNaming {
                    name: symbol,
                    value: body,
                })
            }
        }
    }

    fn check_nameable(name: &str, span: &Range<usize>) -> Result<(), ParseError> {
        if is_reserved(name) {
            Err(failure(ParseErrorKind::ReservedName, span))
        } else {
            Ok(())
        }
    }

    fn record_name(
        &mut self,
        binder: ExprId,
        name: &str,
        span: &Range<usize>,
    ) -> Result<(), ParseError> {
        self.pool.record_bound_name(binder, name).map_err(|_| {
            failure(
                ParseErrorKind::Build(BuildError::UnknownExprId(binder)),
                span,
            )
        })
    }

    pub(crate) fn build(&mut self, ast: &Ast) -> Result<ExprId, ParseError> {
        if self.depth >= DEPTH_LIMIT {
            return Err(failure(
                ParseErrorKind::NestedTooDeeply { limit: DEPTH_LIMIT },
                &ast.span,
            ));
        }
        self.depth += 1;
        let built = self.build_within_limit(ast);
        self.depth -= 1;
        built
    }

    fn build_within_limit(&mut self, ast: &Ast) -> Result<ExprId, ParseError> {
        let span = &ast.span;
        match &ast.kind {
            AstKind::Number { text, is_negative } => {
                let value = exact_decimal(text, *is_negative, span)?;
                self.number(value, span)
            }
            AstKind::Typed {
                kind: TypedLiteralKind::Chemistry,
                content,
            } => self.chemistry(content, span),
            AstKind::Typed {
                kind: TypedLiteralKind::Nuclear,
                content,
            } => self.nuclear(content, span),
            AstKind::Typed { kind, content } => {
                let value = typed_literal(kind, content, span)?;
                self.number(value, span)
            }
            AstKind::Name(name) => self.name(name, span),
            AstKind::Group(inner) => self.build(inner),
            AstKind::Operation { operator, operands } => {
                let mut operands = operands
                    .iter()
                    .map(|operand| self.build(operand))
                    .collect::<Result<Vec<_>, _>>()?;
                self.mark_measurement(*operator, &mut operands, span)?;
                self.pool
                    .apply(Head::Operator(*operator), &operands)
                    .map_err(|error| build_failure(error, span))
            }
            AstKind::Quantity { value, unit } => {
                let value = self.build(value)?;
                let unit = self.unit(unit)?;
                if unit == self.pool.units().dimensionless() {
                    return Ok(value);
                }
                self.pool
                    .quantity(value, unit)
                    .map_err(|error| build_failure(error, span))
            }
            AstKind::Conversion { value, unit } => {
                let value = self.build(value)?;
                let unit = self.unit(unit)?;
                let one = self.number(Number::from(1_i64), span)?;
                let target = if unit == self.pool.units().dimensionless() {
                    one
                } else {
                    self.pool
                        .quantity(one, unit)
                        .map_err(|error| build_failure(error, span))?
                };
                self.pool
                    .apply(Head::Operator(Operator::ConvertUnit), &[value, target])
                    .map_err(|error| build_failure(error, span))
            }
            AstKind::Call { name, arguments } => self.call(name, arguments, span),
            AstKind::RadixConversion { value, target } => {
                let value = self.build(value)?;
                let (operator, numbers): (Operator, Vec<i64>) = match target {
                    RadixTarget::Radix {
                        base,
                        bits,
                        spelling,
                    } => (
                        Operator::Radix,
                        vec![i64::from(*base), i64::from(*bits), spelling.code()],
                    ),
                    RadixTarget::Bytes {
                        big_endian,
                        bits,
                        spelling,
                    } => (
                        Operator::Bytes,
                        vec![i64::from(*bits), spelling.code(), i64::from(*big_endian)],
                    ),
                    RadixTarget::Type { bits, signed } => (
                        Operator::InType,
                        vec![i64::from(*bits), IntegerTypeSpelling::named(*signed).code()],
                    ),
                };
                let mut operands = vec![value];
                for number in numbers {
                    operands.push(self.number(Number::from(number), span)?);
                }
                self.pool
                    .apply(Head::Operator(operator), &operands)
                    .map_err(|error| build_failure(error, span))
            }
            AstKind::Lambda { parameter, body } => {
                self.binder(BinderKind::Lambda, parameter, &[], body, span)
            }
            AstKind::Array { rows } => self.array(rows, span),
            AstKind::Binder {
                binder,
                variable,
                arguments,
                point_is_variable,
                body,
            } => {
                let kind = match binder {
                    BinderSyntax::Integral => BinderKind::Integral,
                    BinderSyntax::Sum(shape) => BinderKind::Sum(*shape),
                    BinderSyntax::Product(shape) => BinderKind::Product(*shape),
                    BinderSyntax::Limit(side) => BinderKind::Limit(*side),
                    BinderSyntax::Derivative => BinderKind::Derivative,
                    BinderSyntax::Root => BinderKind::Root,
                    BinderSyntax::Taylor => BinderKind::Taylor,
                    BinderSyntax::Sort(spec) => BinderKind::Sort(*spec),
                };
                let point = if *point_is_variable {
                    vec![Ast::new(AstKind::Name(variable.clone()), span.clone())]
                } else {
                    arguments.clone()
                };
                self.binder(kind, variable, &point, body, span)
            }
        }
    }

    fn number(&mut self, value: Number, span: &Range<usize>) -> Result<ExprId, ParseError> {
        self.pool
            .number(value)
            .map_err(|error| build_failure(error, span))
    }

    fn row(&mut self, values: &[i64], span: &Range<usize>) -> Result<ExprId, ParseError> {
        let elements = values
            .iter()
            .map(|value| self.number(Number::from(*value), span))
            .collect::<Result<Vec<_>, _>>()?;
        self.row_of(&elements, span)
    }

    fn row_of(&mut self, elements: &[ExprId], span: &Range<usize>) -> Result<ExprId, ParseError> {
        let length = u32::try_from(elements.len())
            .map_err(|_| failure(ParseErrorKind::InvalidTypedLiteral, span))?;
        self.pool
            .array(&[1, length], elements)
            .map_err(|error| build_failure(error, span))
    }

    fn text_codes(&mut self, text: &str, span: &Range<usize>) -> Result<ExprId, ParseError> {
        let codes: Vec<i64> = text.chars().map(|c| i64::from(u32::from(c))).collect();
        self.row(&codes, span)
    }

    fn substance(
        &mut self,
        species: &crate::chemistry::ParsedSpecies,
        span: &Range<usize>,
    ) -> Result<ExprId, ParseError> {
        let too_large = || failure(ParseErrorKind::InvalidTypedLiteral, span);
        let mut composition = vec![species.charge];
        for (element, count) in &species.elements {
            composition.push(i64::from(*element));
            composition.push(i64::try_from(*count).map_err(|_| too_large())?);
        }
        let text = self.text_codes(&species.text, span)?;
        let composition = self.row(&composition, span)?;
        self.pool
            .apply(Head::Operator(Operator::Substance), &[text, composition])
            .map_err(|error| build_failure(error, span))
    }

    fn chemistry(&mut self, content: &str, span: &Range<usize>) -> Result<ExprId, ParseError> {
        match crate::chemistry::parse_chemistry(content, span)? {
            crate::chemistry::ParsedChemistry::Substance(species) => self.substance(&species, span),
            crate::chemistry::ParsedChemistry::Reaction(all) => {
                let too_large = || failure(ParseErrorKind::InvalidTypedLiteral, span);
                let mut nodes = Vec::with_capacity(all.len());
                let mut sides = Vec::with_capacity(all.len());
                let mut coefficients = Vec::with_capacity(all.len());
                for species in &all {
                    nodes.push(self.substance(species, span)?);
                    sides.push(i64::from(species.is_product));
                    coefficients.push(match species.coefficient {
                        Some(value) => i64::try_from(value).map_err(|_| too_large())?,
                        None => calc_expr::COEFFICIENT_NOT_WRITTEN,
                    });
                }
                let text = self.text_codes(content, span)?;
                let species = self.row_of(&nodes, span)?;
                let sides = self.row(&sides, span)?;
                let coefficients = self.row(&coefficients, span)?;
                self.pool
                    .apply(
                        Head::Operator(Operator::Reaction),
                        &[text, species, sides, coefficients],
                    )
                    .map_err(|error| build_failure(error, span))
            }
        }
    }

    fn nuclide(
        &mut self,
        particle: &crate::nuclear::ParsedParticle,
        span: &Range<usize>,
    ) -> Result<ExprId, ParseError> {
        let text = self.text_codes(&particle.text, span)?;
        let codes = self.row(&particle.particle.codes(), span)?;
        self.pool
            .apply(Head::Operator(Operator::Nuclide), &[text, codes])
            .map_err(|error| build_failure(error, span))
    }

    fn nuclear(&mut self, content: &str, span: &Range<usize>) -> Result<ExprId, ParseError> {
        match crate::nuclear::parse_nuclear(content, span)? {
            crate::nuclear::ParsedNuclear::Nuclide(particle) => self.nuclide(&particle, span),
            crate::nuclear::ParsedNuclear::Reaction(all) => {
                let too_large = || failure(ParseErrorKind::InvalidTypedLiteral, span);
                let mut nodes = Vec::with_capacity(all.len());
                let mut sides = Vec::with_capacity(all.len());
                let mut coefficients = Vec::with_capacity(all.len());
                for particle in &all {
                    nodes.push(self.nuclide(particle, span)?);
                    sides.push(i64::from(particle.is_product));
                    coefficients.push(match particle.coefficient {
                        Some(value) => i64::try_from(value).map_err(|_| too_large())?,
                        None => calc_expr::COEFFICIENT_NOT_WRITTEN,
                    });
                }
                let text = self.text_codes(content, span)?;
                let species = self.row_of(&nodes, span)?;
                let sides = self.row(&sides, span)?;
                let coefficients = self.row(&coefficients, span)?;
                self.pool
                    .apply(
                        Head::Operator(Operator::NuclearReaction),
                        &[text, species, sides, coefficients],
                    )
                    .map_err(|error| build_failure(error, span))
            }
        }
    }

    fn name(&mut self, name: &str, span: &Range<usize>) -> Result<ExprId, ParseError> {
        if let Some(depth) = self.scope.iter().rev().position(|bound| bound == name) {
            let index = u32::try_from(depth).map_err(|_| {
                failure(
                    ParseErrorKind::Build(BuildError::TableFull(calc_expr::PoolTable::Nodes)),
                    span,
                )
            })?;
            return self
                .pool
                .bound(index)
                .map_err(|error| build_failure(error, span));
        }
        if operator_for_call(name).is_some() || is_binder_name(name) {
            return Err(failure(ParseErrorKind::ReservedName, span));
        }
        let name = canonical_symbol_name(name);
        let symbol = match self.pool.lookup_symbol(name) {
            Some(symbol) => symbol,
            None => self
                .pool
                .intern_symbol(name, SymbolKind::Variable)
                .map_err(|error| failure(ParseErrorKind::Symbol(error), span))?,
        };
        self.pool
            .symbol(symbol)
            .map_err(|error| build_failure(error, span))
    }

    fn mark_measurement(
        &mut self,
        operator: Operator,
        operands: &mut Vec<ExprId>,
        span: &Range<usize>,
    ) -> Result<(), ParseError> {
        if !matches!(operator, Operator::Uncertain | Operator::UncertainExpanded) {
            return Ok(());
        }
        let mark = self
            .pool
            .measurement()
            .map_err(|error| build_failure(error, span))?;
        operands.push(mark);
        Ok(())
    }

    fn call(
        &mut self,
        name: &str,
        arguments: &[Ast],
        span: &Range<usize>,
    ) -> Result<ExprId, ParseError> {
        if let Some(operator @ (Operator::Wrap | Operator::BitNot)) = operator_for_call(name) {
            return self.typed_call(operator, arguments, span);
        }
        let built = arguments
            .iter()
            .map(|argument| self.build(argument))
            .collect::<Result<Vec<_>, _>>()?;
        let mut built = built;
        let head = match operator_for_call(name) {
            Some(operator) => {
                self.mark_measurement(operator, &mut built, span)?;
                Head::Operator(operator)
            }
            None => {
                if is_reserved(name) {
                    return Err(failure(ParseErrorKind::ReservedName, span));
                }
                let symbol = match self.pool.lookup_symbol(name) {
                    Some(symbol) => symbol,
                    None => self
                        .pool
                        .intern_symbol(name, SymbolKind::Function { arity: built.len() })
                        .map_err(|error| failure(ParseErrorKind::Symbol(error), span))?,
                };
                Head::Function(symbol)
            }
        };
        self.pool
            .apply(head, &built)
            .map_err(|error| build_failure(error, span))
    }

    fn typed_call(
        &mut self,
        operator: Operator,
        arguments: &[Ast],
        span: &Range<usize>,
    ) -> Result<ExprId, ParseError> {
        let [value, kind] = arguments else {
            return Err(failure(
                ParseErrorKind::ArityMismatch {
                    expected: 2,
                    found: arguments.len(),
                },
                span,
            ));
        };
        let AstKind::Name(written) = &kind.kind else {
            return Err(failure(ParseErrorKind::TypeExpected, &kind.span));
        };
        let (bits, signed) = integer_type(written)
            .ok_or_else(|| failure(ParseErrorKind::TypeExpected, &kind.span))?;
        let value = self.build(value)?;
        let bits = self.number(Number::from(i64::from(bits)), span)?;
        let signed = self.number(
            Number::from(IntegerTypeSpelling::named(signed).code()),
            span,
        )?;
        self.pool
            .apply(Head::Operator(operator), &[value, bits, signed])
            .map_err(|error| build_failure(error, span))
    }

    fn binder(
        &mut self,
        kind: BinderKind,
        variable: &str,
        arguments: &[Ast],
        body: &Ast,
        span: &Range<usize>,
    ) -> Result<ExprId, ParseError> {
        let arguments = arguments
            .iter()
            .map(|argument| self.build(argument))
            .collect::<Result<Vec<_>, _>>()?;
        self.scope.push(variable.to_string());
        let body = self.build(body);
        self.scope.pop();
        let binder = self
            .pool
            .bind(kind, &arguments, body?)
            .map_err(|error| build_failure(error, span))?;
        self.record_name(binder, variable, span)?;
        Ok(binder)
    }

    fn array(&mut self, rows: &[Vec<Ast>], span: &Range<usize>) -> Result<ExprId, ParseError> {
        let elements = rows
            .iter()
            .flatten()
            .map(|element| self.build(element))
            .collect::<Result<Vec<_>, _>>()?;
        let column_count = rows.first().map_or(0, Vec::len);
        let too_large = || failure(ParseErrorKind::Build(BuildError::ShapeTooLarge), span);
        let shape = match rows.len() {
            0 => vec![0],
            1 => vec![u32::try_from(column_count).map_err(|_| too_large())?],
            row_count => vec![
                u32::try_from(row_count).map_err(|_| too_large())?,
                u32::try_from(column_count).map_err(|_| too_large())?,
            ],
        };
        self.pool
            .array(&shape, &elements)
            .map_err(|error| build_failure(error, span))
    }

    fn unit(&mut self, factors: &[UnitFactorSyntax]) -> Result<UnitId, ParseError> {
        let units = self.pool.units_mut();
        let mut result: Option<UnitId> = None;
        for (position, factor) in factors.iter().enumerate() {
            let named = units.lookup(&factor.name).map_err(|error| match error {
                calc_units::UnitLookupError::Ambiguous => {
                    failure(ParseErrorKind::AmbiguousUnit, &factor.span)
                }
                _ if ATOMIC_MASS_UNITS.contains(&factor.name.as_str()) => {
                    failure(ParseErrorKind::AtomicMassUnit, &factor.span)
                }
                _ if position > 0 => failure(ParseErrorKind::NotAUnitJoinedToAUnit, &factor.span),
                _ => failure(ParseErrorKind::NotAUnit, &factor.span),
            })?;
            let exponent = i8::try_from(factor.exponent)
                .map_err(|_| failure(ParseErrorKind::UnitExponentOutOfRange, &factor.span))?;
            let exponent = if result.is_none() && factor.is_divisor {
                -exponent
            } else {
                exponent
            };
            let product_failure = |error| failure(ParseErrorKind::UnitProduct(error), &factor.span);
            let powered = if exponent == 1 {
                named
            } else {
                units.power(named, exponent).map_err(product_failure)?
            };
            result = Some(match result {
                None => powered,
                Some(left) if factor.is_divisor => {
                    units.divide(left, powered).map_err(product_failure)?
                }
                Some(left) => units.multiply(left, powered).map_err(product_failure)?,
            });
        }
        Ok(result.unwrap_or_else(|| units.dimensionless()))
    }
}

fn exact_decimal(text: &str, is_negative: bool, span: &Range<usize>) -> Result<Number, ParseError> {
    if let Some(radix) = crate::decimal::radix_prefix(text) {
        let magnitude = crate::decimal::integer_from_radix(&text[2..], radix)
            .ok_or_else(|| failure(ParseErrorKind::UnexpectedCharacter, span))?;
        return Ok(Number::Integer(if is_negative {
            magnitude.negated()
        } else {
            magnitude
        }));
    }
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(position) => (&text[..position], &text[position + 1..]),
        None => (text, "0"),
    };
    let exponent = exponent
        .parse::<i64>()
        .ok()
        .filter(|exponent| exponent.abs() <= LARGEST_DECIMAL_EXPONENT)
        .ok_or_else(|| failure(ParseErrorKind::ExponentTooLarge, span))?;
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = integer_from_digits(&format!("{whole}{fraction}"));
    let fraction_length = i64::try_from(fraction.len())
        .map_err(|_| failure(ParseErrorKind::ExponentTooLarge, span))?;
    let scale = exponent - fraction_length;
    let magnitude = u32::try_from(scale.unsigned_abs())
        .map_err(|_| failure(ParseErrorKind::ExponentTooLarge, span))?;
    let numerator = if is_negative {
        digits.negated()
    } else {
        digits
    };
    let value = if scale >= 0 {
        Number::Integer(&numerator * &power_of_ten(magnitude))
    } else {
        Number::fraction(&numerator, &power_of_ten(magnitude))
            .map_err(|_| failure(ParseErrorKind::InvalidTypedLiteral, span))?
    };
    Ok(value)
}

fn is_decimal_syntax(text: &str) -> bool {
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(position) => (&unsigned[..position], Some(&unsigned[position + 1..])),
        None => (unsigned, None),
    };
    let all_digits = |part: &str| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit());
    let mantissa_valid = match mantissa.split_once('.') {
        Some((whole, fraction)) => all_digits(whole) && all_digits(fraction),
        None => all_digits(mantissa),
    };
    let exponent_valid = exponent
        .is_none_or(|exponent| all_digits(exponent.strip_prefix(['+', '-']).unwrap_or(exponent)));
    mantissa_valid && exponent_valid
}

fn typed_literal(
    kind: &TypedLiteralKind,
    content: &str,
    span: &Range<usize>,
) -> Result<Number, ParseError> {
    let invalid = || failure(ParseErrorKind::InvalidTypedLiteral, span);
    match kind {
        TypedLiteralKind::Chemistry | TypedLiteralKind::Nuclear => Err(invalid()),
        TypedLiteralKind::Rational => {
            let (numerator, denominator) = content.split_once('/').ok_or_else(invalid)?;
            let (is_negative, numerator) = match numerator.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, numerator),
            };
            let all_digits =
                |part: &str| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit());
            if !all_digits(numerator) || !all_digits(denominator) {
                return Err(invalid());
            }
            let numerator = integer_from_digits(numerator);
            let numerator = if is_negative {
                numerator.negated()
            } else {
                numerator
            };
            let denominator: Integer = integer_from_digits(denominator);
            Number::fraction(&numerator, &denominator).map_err(|_| invalid())
        }
        TypedLiteralKind::F64 => {
            let value = match content {
                NAN_NAME => f64::from_bits(QUIET_NAN_F64_BITS),
                INFINITY_CONTENT => f64::INFINITY,
                NEGATIVE_INFINITY_CONTENT => f64::NEG_INFINITY,
                _ => match content.strip_prefix(BITS_PREFIX) {
                    Some(hexadecimal) => f64::from_bits(
                        u64::from_str_radix(hexadecimal, HEXADECIMAL_RADIX)
                            .map_err(|_| invalid())?,
                    ),
                    None if is_decimal_syntax(content) => {
                        content.parse::<f64>().map_err(|_| invalid())?
                    }
                    None => return Err(invalid()),
                },
            };
            Ok(Number::F64(value))
        }
        TypedLiteralKind::F32 => {
            let value = match content {
                NAN_NAME => f32::from_bits(QUIET_NAN_F32_BITS),
                INFINITY_CONTENT => f32::INFINITY,
                NEGATIVE_INFINITY_CONTENT => f32::NEG_INFINITY,
                _ => match content.strip_prefix(BITS_PREFIX) {
                    Some(hexadecimal) => f32::from_bits(
                        u32::from_str_radix(hexadecimal, HEXADECIMAL_RADIX)
                            .map_err(|_| invalid())?,
                    ),
                    None if is_decimal_syntax(content) => {
                        content.parse::<f32>().map_err(|_| invalid())?
                    }
                    None => return Err(invalid()),
                },
            };
            Ok(Number::F32(value))
        }
    }
}
