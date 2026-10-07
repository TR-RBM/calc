use std::collections::HashSet;

use calc_expr::{
    BinderKind, ExprId, ExprPool, Head, IntegerTypeSpelling, LimitSide, NodeView, Operator,
    ReductionShape, SortMethod, SortOrder, SortPartition,
};
use calc_numbers::{Integer, Number};
use calc_units::UnitId;

use crate::decimal::number_to_exact_text;
use crate::error::PrintError;
use crate::lexer::{is_identifier_continue, superscript_digit, superscript_minus};
use crate::names::{
    BASE_KEYWORD, DECREASING_VALUE, DERIVATIVE_NAME, FORM_KEYWORD, GAPS_KEYWORD, HALVING_VALUE,
    INFINITY_NAME, INTEGRAL_NAME, LEFT_SIDE_VALUE, LIMIT_KEYWORD, LIMIT_NAME, ORDER_KEYWORD,
    PARTITION_KEYWORD, PI_LETTER, PI_NAME, PIVOT_KEYWORD, PRODUCT_NAME, RIGHT_SIDE_VALUE,
    ROOT_NAME, SEED_KEYWORD, SHAPE_KEYWORD, SIDE_KEYWORD, SUFFIX_SEPARATOR, SUM_NAME, TAYLOR_NAME,
    call_name, changes_parsing, partition_values, sort_form_value, sort_method_name,
};

const PRIMARY: u8 = 1;
const POSTFIX: u8 = 2;
const POWER: u8 = 3;
const UNARY: u8 = 5;
const UNCERTAIN: u8 = 6;
const MULTIPLICATIVE: u8 = 7;
const ADDITIVE: u8 = 8;
const CONVERSION: u8 = 9;
const RELATION: u8 = 10;
const NOT: u8 = 11;
const AND: u8 = 12;
const OR: u8 = 13;
const LAMBDA: u8 = 14;
const DEFAULT_VARIABLE: &str = "x";
const INFINITY_SIGN: &str = "\u{221E}";
const QUIET_NAN_F64_BITS: u64 = 0x7ff8_0000_0000_0000;
const QUIET_NAN_F32_BITS: u32 = 0x7fc0_0000;
const SHORT_DECIMAL_LOWER: f64 = 1e-5;
const SHORT_DECIMAL_UPPER: f64 = 1e16;
const DECIMAL_RADIX: u32 = 10;
const SHORTENED_ZERO_COUNT: usize = 6;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PrintMode {
    #[default]
    Ascii,
    Unicode,
}

struct Printed {
    text: String,
    level: u8,
    starts_with_plain_literal: bool,
    ends_with_unit: bool,
    is_literal: bool,
    closed_root: Option<String>,
}

impl Printed {
    fn new(text: String, level: u8) -> Self {
        Self {
            text,
            level,
            starts_with_plain_literal: false,
            ends_with_unit: false,
            is_literal: false,
            closed_root: None,
        }
    }

    fn before_operator(self, level: u8) -> String {
        if self.level > level {
            format!("({})", self.text)
        } else {
            self.closed_root.unwrap_or(self.text)
        }
    }

    fn within(self, level: u8) -> String {
        if self.level > level {
            format!("({})", self.text)
        } else {
            self.text
        }
    }

    fn parenthesized(self) -> String {
        format!("({})", self.text)
    }
}

pub(crate) struct Printer<'pool> {
    pool: &'pool ExprPool,
    mode: PrintMode,
    scope: Vec<String>,
}

impl<'pool> Printer<'pool> {
    pub(crate) fn new(pool: &'pool ExprPool, mode: PrintMode) -> Self {
        Self {
            pool,
            mode,
            scope: Vec::new(),
        }
    }

    fn is_unicode(&self) -> bool {
        self.mode == PrintMode::Unicode
    }

    pub(crate) fn print(&mut self, expression: ExprId) -> Result<String, PrintError> {
        self.expression(expression).map(|printed| printed.text)
    }

    fn expression(&mut self, expression: ExprId) -> Result<Printed, PrintError> {
        match self.pool.node(expression).map_err(PrintError::Access)? {
            NodeView::Number(number) => {
                let value = self.pool.number_value(number).map_err(PrintError::Access)?;
                Ok(self.number(value))
            }
            NodeView::Symbol(symbol) => {
                let name = self.pool.symbol_name(symbol).map_err(PrintError::Symbol)?;
                let text = match (self.mode, name) {
                    (PrintMode::Unicode, PI_NAME) => PI_LETTER,
                    (PrintMode::Unicode, INFINITY_NAME) => INFINITY_SIGN,
                    _ => name,
                };
                Ok(Printed::new(text.to_string(), PRIMARY))
            }
            NodeView::Bound(index) => {
                let depth = usize::try_from(index).map_err(|_| PrintError::UnboundIndex(index))?;
                self.scope
                    .iter()
                    .rev()
                    .nth(depth)
                    .map(|name| Printed::new(name.clone(), PRIMARY))
                    .ok_or(PrintError::UnboundIndex(index))
            }
            NodeView::Apply {
                head: Head::Operator(operator),
                arguments,
            } => self.operation(operator, arguments),
            NodeView::Apply {
                head: Head::Function(symbol),
                arguments,
            } => {
                let name = self
                    .pool
                    .symbol_name(symbol)
                    .map_err(PrintError::Symbol)?
                    .to_string();
                self.call(&name, arguments, &[])
            }
            NodeView::Bind {
                binder,
                arguments,
                body,
            } => self.binder(expression, binder, arguments, body),
            NodeView::Quantity { value, unit } => self.quantity(value, unit),
            NodeView::Array { shape, elements } => self.array(shape, elements),
        }
    }

    fn number(&self, value: &Number) -> Printed {
        match value {
            Number::F64(value) => {
                let content = if value.is_nan() && value.to_bits() != QUIET_NAN_F64_BITS {
                    format!("bits:0x{:016x}", value.to_bits())
                } else {
                    float_content(*value)
                };
                typed("f64", &content)
            }
            Number::F32(value) => {
                let content = if value.is_nan() && value.to_bits() != QUIET_NAN_F32_BITS {
                    format!("bits:0x{:08x}", value.to_bits())
                } else {
                    float32_content(*value)
                };
                typed("f32", &content)
            }
            exact => {
                let text = number_to_exact_text(exact)
                    .map(|text| shorten_trailing_zeros(&text))
                    .unwrap_or_default();
                let is_typed = text.starts_with('q');
                let is_negative = text.starts_with('-');
                Printed {
                    level: if is_negative { UNARY } else { PRIMARY },
                    starts_with_plain_literal: !is_typed,
                    ends_with_unit: false,
                    is_literal: true,
                    closed_root: None,
                    text,
                }
            }
        }
    }

    fn operation(
        &mut self,
        operator: Operator,
        arguments: &[ExprId],
    ) -> Result<Printed, PrintError> {
        match (operator, arguments) {
            (Operator::Add, [left, right]) => self.binary(*left, " + ", *right, ADDITIVE),
            (Operator::Sub, [left, right]) => self.binary(*left, " - ", *right, ADDITIVE),
            (Operator::Mul, [left, right]) => {
                let sign = if self.is_unicode() {
                    " \u{00B7} "
                } else {
                    " * "
                };
                self.binary(*left, sign, *right, MULTIPLICATIVE)
            }
            (Operator::Div, [left, right]) => self.binary(*left, " / ", *right, MULTIPLICATIVE),
            (Operator::And, [left, right]) => self.binary(*left, " and ", *right, AND),
            (Operator::Or, [left, right]) => self.binary(*left, " or ", *right, OR),
            (Operator::Equal, [left, right]) => self.relation(*left, " == ", *right),
            (Operator::NotEqual, [left, right]) => {
                let sign = if self.is_unicode() {
                    " \u{2260} "
                } else {
                    " != "
                };
                self.relation(*left, sign, *right)
            }
            (Operator::Less, [left, right]) => self.relation(*left, " < ", *right),
            (Operator::LessOrEqual, [left, right]) => {
                let sign = if self.is_unicode() {
                    " \u{2264} "
                } else {
                    " <= "
                };
                self.relation(*left, sign, *right)
            }
            (Operator::Greater, [left, right]) => self.relation(*left, " > ", *right),
            (Operator::GreaterOrEqual, [left, right]) => {
                let sign = if self.is_unicode() {
                    " \u{2265} "
                } else {
                    " >= "
                };
                self.relation(*left, sign, *right)
            }
            (Operator::Not, [operand]) => {
                let operand = self.expression(*operand)?.within(NOT);
                Ok(Printed::new(format!("not {operand}"), NOT))
            }
            (Operator::Neg, [operand]) => self.negation(*operand),
            (Operator::Pow, [base, exponent]) => self.power(*base, *exponent),
            (Operator::Factorial, [operand]) => {
                let operand = self.expression(*operand)?;
                let operand = if operand.level > PRIMARY {
                    operand.parenthesized()
                } else {
                    operand.text
                };
                Ok(Printed::new(format!("{operand}!"), POSTFIX))
            }
            (Operator::Percent, [operand]) => {
                let operand = self.expression(*operand)?;
                let operand = if operand.level > PRIMARY {
                    operand.parenthesized()
                } else {
                    operand.text
                };
                Ok(Printed::new(format!("{operand}%"), POSTFIX))
            }
            (Operator::Sqrt, [operand]) if self.is_unicode() => {
                let operand = self.expression(*operand)?;
                if operand.level > PRIMARY || operand.ends_with_unit {
                    return Ok(Printed::new(
                        format!("\u{221A}{}", operand.parenthesized()),
                        UNARY,
                    ));
                }
                let mut printed = Printed::new(format!("\u{221A}{}", operand.text), UNARY);
                printed.closed_root = Some(format!("\u{221A}({})", operand.text));
                Ok(printed)
            }
            (Operator::Uncertain, [value, uncertainty, _]) => {
                self.uncertain(*value, *uncertainty, None)
            }
            (Operator::UncertainExpanded, [value, uncertainty, coverage, _]) => {
                match self.plain_unsigned_literal(*coverage)? {
                    Some(coverage) => self.uncertain(*value, *uncertainty, Some(coverage)),
                    None => self.call_operator(operator, arguments),
                }
            }
            (
                Operator::Radix
                | Operator::Bytes
                | Operator::InType
                | Operator::Wrap
                | Operator::BitNot,
                _,
            ) => self.integer_form(operator, arguments),
            (Operator::Substance | Operator::Reaction, [text, ..]) => {
                match self.written_text(*text)? {
                    Some(written) => Ok(Printed::new(format!("chem'{written}'"), PRIMARY)),
                    None => self.call_operator(operator, arguments),
                }
            }
            (Operator::Nuclide | Operator::NuclearReaction, [text, ..]) => {
                match self.written_text(*text)? {
                    Some(written) => Ok(Printed::new(format!("nuc'{written}'"), PRIMARY)),
                    None => self.call_operator(operator, arguments),
                }
            }
            (Operator::ConvertUnit, [value, target]) => match self.conversion_unit(*target)? {
                Some(unit) => {
                    let value = self.expression(*value)?.within(CONVERSION);
                    let arrow = if self.is_unicode() { "\u{2192}" } else { "->" };
                    let unit = self.unit_text(unit)?;
                    Ok(Printed::new(format!("{value} {arrow} {unit}"), CONVERSION))
                }
                None => self.call_operator(operator, arguments),
            },
            _ => self.call_operator(operator, arguments),
        }
    }

    fn call_operator(
        &mut self,
        operator: Operator,
        arguments: &[ExprId],
    ) -> Result<Printed, PrintError> {
        let name = call_name(operator).unwrap_or_default();
        let written = match operator {
            Operator::Uncertain | Operator::UncertainExpanded => {
                arguments.split_last().map_or(arguments, |(_, rest)| rest)
            }
            _ => arguments,
        };
        self.call(name, written, &[])
    }

    fn binary(
        &mut self,
        left: ExprId,
        sign: &str,
        right: ExprId,
        level: u8,
    ) -> Result<Printed, PrintError> {
        let left = self.expression(left)?.before_operator(level);
        let right = self.expression(right)?;
        let closed_root = right
            .closed_root
            .clone()
            .filter(|_| right.level < level)
            .map(|closed| format!("{left}{sign}{closed}"));
        let right = right.within(level - 1);
        let mut printed = Printed::new(format!("{left}{sign}{right}"), level);
        printed.closed_root = closed_root;
        Ok(printed)
    }

    fn relation(&mut self, left: ExprId, sign: &str, right: ExprId) -> Result<Printed, PrintError> {
        let left = self.expression(left)?.within(RELATION - 1);
        let right = self.expression(right)?.within(RELATION - 1);
        Ok(Printed::new(format!("{left}{sign}{right}"), RELATION))
    }

    fn negation(&mut self, operand: ExprId) -> Result<Printed, PrintError> {
        let operand = self.expression(operand)?;
        if operand.level > UNARY || operand.starts_with_plain_literal {
            return Ok(Printed::new(format!("-{}", operand.parenthesized()), UNARY));
        }
        let mut printed = Printed::new(format!("-{}", operand.text), UNARY);
        printed.closed_root = operand.closed_root.map(|closed| format!("-{closed}"));
        Ok(printed)
    }

    fn power(&mut self, base: ExprId, exponent: ExprId) -> Result<Printed, PrintError> {
        let base = self.expression(base)?;
        let base = if base.level > POSTFIX || base.ends_with_unit {
            base.parenthesized()
        } else {
            base.text
        };
        if self.is_unicode()
            && let Some(superscript) = self.superscript_exponent(exponent)?
        {
            return Ok(Printed::new(format!("{base}{superscript}"), POWER));
        }
        let exponent = self.expression(exponent)?;
        let starts_with_sign = exponent.text.starts_with('-');
        let exponent = if exponent.ends_with_unit
            || exponent.level > UNARY
            || (exponent.level > POWER && !starts_with_sign)
        {
            exponent.parenthesized()
        } else {
            exponent.text
        };
        Ok(Printed::new(format!("{base}^{exponent}"), POWER))
    }

    fn superscript_exponent(&self, exponent: ExprId) -> Result<Option<String>, PrintError> {
        let NodeView::Number(number) = self.pool.node(exponent).map_err(PrintError::Access)? else {
            return Ok(None);
        };
        let Number::Integer(integer) =
            self.pool.number_value(number).map_err(PrintError::Access)?
        else {
            return Ok(None);
        };
        let Some(value) = integer.to_i64() else {
            return Ok(None);
        };
        let mut text = String::new();
        if value < 0 {
            text.push(superscript_minus());
        }
        for digit in value.unsigned_abs().to_string().chars() {
            match digit.to_digit(DECIMAL_RADIX).and_then(superscript_digit) {
                Some(superscript) => text.push(superscript),
                None => return Ok(None),
            }
        }
        Ok(Some(text))
    }

    fn uncertain(
        &mut self,
        value: ExprId,
        uncertainty: ExprId,
        coverage: Option<String>,
    ) -> Result<Printed, PrintError> {
        let value = self.expression(value)?.before_operator(UNARY);
        let uncertainty = self.expression(uncertainty)?;
        let uncertainty = if uncertainty.level > UNARY || uncertainty.ends_with_unit {
            uncertainty.parenthesized()
        } else {
            uncertainty.text
        };
        let sign = if self.is_unicode() { "\u{00B1}" } else { "+-" };
        let coverage = coverage.map_or_else(String::new, |k| format!(" (k={k})"));
        Ok(Printed::new(
            format!("{value} {sign} {uncertainty}{coverage}"),
            UNCERTAIN,
        ))
    }

    #[inline(never)]
    fn integer_form(
        &mut self,
        operator: Operator,
        arguments: &[ExprId],
    ) -> Result<Printed, PrintError> {
        match (operator, arguments) {
            (Operator::Radix, [value, base, bits, signed]) => {
                let base = match self.small_literal(*base)? {
                    Some(16) => "hex",
                    Some(2) => "bin",
                    Some(8) => "oct",
                    Some(10) => "dec",
                    _ => return self.call_operator(operator, arguments),
                };
                let kind = self.type_text(*bits, *signed, true)?;
                self.conversion_to(*value, &format!("{base}{kind}"))
            }
            (Operator::Bytes, [value, bits, signed, big_endian]) => {
                let order = match self.small_literal(*big_endian)? {
                    Some(1) => "be",
                    Some(0) => "le",
                    _ => return self.call_operator(operator, arguments),
                };
                let kind = self.type_text(*bits, *signed, false)?;
                self.conversion_to(*value, &format!("bytes {order}{kind}"))
            }
            (Operator::InType, [value, bits, signed]) => {
                let kind = self.type_text(*bits, *signed, false)?;
                self.conversion_to(*value, kind.trim_start())
            }
            (Operator::Wrap | Operator::BitNot, [value, bits, signed]) => {
                let kind = self.type_text(*bits, *signed, false)?;
                let value = self.expression(*value)?.text;
                let name = call_name(operator).unwrap_or_default();
                Ok(Printed::new(
                    format!("{name}({value}, {})", kind.trim_start()),
                    PRIMARY,
                ))
            }
            _ => self.call_operator(operator, arguments),
        }
    }

    fn small_literal(&self, expression: ExprId) -> Result<Option<i64>, PrintError> {
        let NodeView::Number(number) = self.pool.node(expression).map_err(PrintError::Access)?
        else {
            return Ok(None);
        };
        let value = self.pool.number_value(number).map_err(PrintError::Access)?;
        Ok(match value {
            calc_numbers::Number::Integer(integer) => integer.to_i64(),
            _ => None,
        })
    }

    fn type_text(
        &self,
        bits: ExprId,
        signed: ExprId,
        width_alone: bool,
    ) -> Result<String, PrintError> {
        let bits = self.small_literal(bits)?.unwrap_or(0);
        let spelling = self
            .small_literal(signed)?
            .and_then(IntegerTypeSpelling::from_code)
            .unwrap_or(IntegerTypeSpelling::Width);
        Ok(match (bits, spelling, width_alone) {
            (0, _, _) => String::new(),
            (bits, IntegerTypeSpelling::Width, true) => format!(" {bits}"),
            (bits, IntegerTypeSpelling::Width | IntegerTypeSpelling::Unsigned, _) => {
                format!(" u{bits}")
            }
            (bits, IntegerTypeSpelling::Signed, _) => format!(" i{bits}"),
        })
    }

    fn conversion_to(&mut self, value: ExprId, target: &str) -> Result<Printed, PrintError> {
        let value = self.expression(value)?.within(CONVERSION);
        let arrow = if self.is_unicode() { "\u{2192}" } else { "->" };
        Ok(Printed::new(
            format!("{value} {arrow} {target}"),
            CONVERSION,
        ))
    }

    fn written_text(&self, codes: ExprId) -> Result<Option<String>, PrintError> {
        let NodeView::Array { elements, .. } = self.pool.node(codes).map_err(PrintError::Access)?
        else {
            return Ok(None);
        };
        let mut text = String::new();
        for element in elements.iter().copied() {
            let Some(code) = self.small_literal(element)? else {
                return Ok(None);
            };
            let Some(character) = u32::try_from(code).ok().and_then(char::from_u32) else {
                return Ok(None);
            };
            text.push(character);
        }
        Ok(Some(text))
    }

    fn plain_unsigned_literal(&self, expression: ExprId) -> Result<Option<String>, PrintError> {
        let NodeView::Number(number) = self.pool.node(expression).map_err(PrintError::Access)?
        else {
            return Ok(None);
        };
        let value = self.pool.number_value(number).map_err(PrintError::Access)?;
        Ok(number_to_exact_text(value)
            .filter(|text| !text.starts_with('-') && !text.starts_with('q')))
    }

    fn conversion_unit(&self, target: ExprId) -> Result<Option<UnitId>, PrintError> {
        let NodeView::Quantity { value, unit } =
            self.pool.node(target).map_err(PrintError::Access)?
        else {
            return Ok(None);
        };
        let NodeView::Number(number) = self.pool.node(value).map_err(PrintError::Access)? else {
            return Ok(None);
        };
        let is_one = matches!(
            self.pool.number_value(number).map_err(PrintError::Access)?,
            Number::Integer(integer) if integer.is_one()
        );
        Ok(is_one.then_some(unit))
    }

    fn quantity(&mut self, value: ExprId, unit: UnitId) -> Result<Printed, PrintError> {
        let unit_text = self.unit_text(unit)?;
        let printed = self.expression(value)?;
        let (value_text, level, starts_with_plain_literal) = if printed.is_literal {
            (
                printed.text,
                printed.level,
                printed.starts_with_plain_literal,
            )
        } else if printed.level == UNCERTAIN && !printed.ends_with_unit {
            (printed.text, UNCERTAIN, false)
        } else {
            (printed.parenthesized(), POSTFIX, false)
        };
        Ok(Printed {
            text: format!("{value_text} {unit_text}"),
            level: level.max(POSTFIX),
            starts_with_plain_literal,
            ends_with_unit: true,
            is_literal: false,
            closed_root: None,
        })
    }

    fn unit_text(&self, unit: UnitId) -> Result<String, PrintError> {
        let units = self.pool.units();
        let factors = units.factors(unit).map_err(PrintError::UnitAccess)?;
        if factors.is_empty() {
            return Err(PrintError::DimensionlessQuantity);
        }
        let product_sign = if self.is_unicode() { "\u{00B7}" } else { "*" };
        let has_positive = factors.iter().any(|factor| factor.exponent() > 0);
        let mut text = String::new();
        for factor in factors.iter().filter(|factor| factor.exponent() > 0) {
            if !text.is_empty() {
                text.push_str(product_sign);
            }
            let symbol = units
                .symbol(factor.named_unit())
                .map_err(PrintError::UnitAccess)?;
            text.push_str(symbol);
            text.push_str(&self.unit_exponent(i64::from(factor.exponent())));
        }
        for factor in factors.iter().filter(|factor| factor.exponent() < 0) {
            let symbol = units
                .symbol(factor.named_unit())
                .map_err(PrintError::UnitAccess)?;
            if has_positive {
                text.push('/');
                text.push_str(symbol);
                text.push_str(&self.unit_exponent(-i64::from(factor.exponent())));
            } else {
                if !text.is_empty() {
                    text.push_str(product_sign);
                }
                text.push_str(symbol);
                text.push_str(&self.unit_exponent(i64::from(factor.exponent())));
            }
        }
        Ok(text)
    }

    fn unit_exponent(&self, exponent: i64) -> String {
        if exponent == 1 {
            return String::new();
        }
        if self.is_unicode() {
            let mut text = String::new();
            if exponent < 0 {
                text.push(superscript_minus());
            }
            text.extend(
                exponent
                    .unsigned_abs()
                    .to_string()
                    .chars()
                    .filter_map(|digit| digit.to_digit(DECIMAL_RADIX).and_then(superscript_digit)),
            );
            text
        } else {
            format!("^{exponent}")
        }
    }

    fn call(
        &mut self,
        name: &str,
        arguments: &[ExprId],
        keywords: &[(&str, &str)],
    ) -> Result<Printed, PrintError> {
        let mut parts = Vec::new();
        for argument in arguments {
            parts.push(self.expression(*argument)?.within(LAMBDA));
        }
        parts.extend(
            keywords
                .iter()
                .map(|(keyword, value)| format!("{keyword}={value}")),
        );
        Ok(Printed::new(
            format!("{name}({})", parts.join(", ")),
            PRIMARY,
        ))
    }

    fn array(&mut self, shape: &[u32], elements: &[ExprId]) -> Result<Printed, PrintError> {
        let mut printed = Vec::new();
        for element in elements {
            printed.push(self.expression(*element)?.within(LAMBDA));
        }
        let text = match shape {
            [0] => "[]".to_string(),
            [_] => format!("[{}]", printed.join(", ")),
            [rows, columns] if *rows > 1 && *columns > 0 => {
                let columns = usize::try_from(*columns).unwrap_or(usize::MAX);
                let rows: Vec<String> = printed.chunks(columns).map(|row| row.join(", ")).collect();
                format!("[{}]", rows.join("; "))
            }
            _ => return Err(PrintError::UnprintableArrayShape),
        };
        Ok(Printed::new(text, PRIMARY))
    }

    fn binder(
        &mut self,
        expression: ExprId,
        binder: BinderKind,
        arguments: &[ExprId],
        body: ExprId,
    ) -> Result<Printed, PrintError> {
        let mut outer = Vec::new();
        for argument in arguments {
            outer.push(self.expression(*argument)?.within(LAMBDA));
        }
        let variable = self.variable_name(expression, body)?;
        self.scope.push(variable.clone());
        let body = self.expression(body);
        self.scope.pop();
        let body = body?.within(LAMBDA);
        let mut parts = vec![body, variable.clone()];
        let text = match binder {
            BinderKind::Lambda => {
                let arrow = if self.is_unicode() { "\u{21A6}" } else { "|->" };
                let body = parts.swap_remove(0);
                return Ok(Printed::new(format!("{variable} {arrow} {body}"), LAMBDA));
            }
            BinderKind::Integral => {
                parts.extend(outer);
                format!("{INTEGRAL_NAME}({})", parts.join(", "))
            }
            BinderKind::Sum(shape) | BinderKind::Product(shape) => {
                parts.extend(outer);
                if shape == ReductionShape::Halving {
                    parts.push(format!("{SHAPE_KEYWORD}={HALVING_VALUE}"));
                }
                let name = if matches!(binder, BinderKind::Sum(_)) {
                    SUM_NAME
                } else {
                    PRODUCT_NAME
                };
                format!("{name}({})", parts.join(", "))
            }
            BinderKind::Limit(side) => {
                parts.extend(outer);
                match side {
                    LimitSide::Left => parts.push(format!("{SIDE_KEYWORD}={LEFT_SIDE_VALUE}")),
                    LimitSide::Right => parts.push(format!("{SIDE_KEYWORD}={RIGHT_SIDE_VALUE}")),
                    LimitSide::Both => {}
                }
                format!("{LIMIT_NAME}({})", parts.join(", "))
            }
            BinderKind::Derivative => {
                if outer.first() != Some(&variable) {
                    parts.extend(outer);
                }
                format!("{DERIVATIVE_NAME}({})", parts.join(", "))
            }
            BinderKind::Root => {
                parts.extend(outer);
                format!("{ROOT_NAME}({})", parts.join(", "))
            }
            BinderKind::Taylor => {
                parts.extend(outer);
                format!("{TAYLOR_NAME}({})", parts.join(", "))
            }
            BinderKind::Sort(spec) => {
                let key = parts.swap_remove(0);
                let mut shown = outer;
                let seed = if shown.len() > 1 {
                    shown.split_off(1)
                } else {
                    Vec::new()
                };
                if key != variable {
                    let arrow = if self.is_unicode() { "\u{21A6}" } else { "|->" };
                    shown.push(format!("{variable} {arrow} {key}"));
                }
                if let Some(form) = sort_form_value(spec.method) {
                    let keyword = if matches!(spec.method, SortMethod::Shell(_)) {
                        GAPS_KEYWORD
                    } else {
                        FORM_KEYWORD
                    };
                    shown.push(format!("{keyword}={form}"));
                }
                if let SortMethod::Quick(partition) = spec.method {
                    shown.extend(quick_sort_keywords(partition, seed.first()));
                }
                if let (SortMethod::Bogo, Some(drawn), Some(limit)) =
                    (spec.method, seed.first(), seed.get(1))
                {
                    shown.push(format!("{SEED_KEYWORD}={drawn}"));
                    shown.push(format!("{LIMIT_KEYWORD}={limit}"));
                }
                if let (SortMethod::Radix, Some(base)) = (spec.method, seed.first()) {
                    shown.push(format!("{BASE_KEYWORD}={base}"));
                }
                if spec.order == SortOrder::Decreasing {
                    shown.push(format!("{ORDER_KEYWORD}={DECREASING_VALUE}"));
                }
                format!("{}({})", sort_method_name(spec.method), shown.join(", "))
            }
        };
        Ok(Printed::new(text, PRIMARY))
    }

    fn variable_name(&self, binder: ExprId, body: ExprId) -> Result<String, PrintError> {
        let base = self
            .pool
            .bound_name(binder)
            .filter(|name| is_valid_name(name))
            .unwrap_or(DEFAULT_VARIABLE)
            .to_string();
        let free = self.free_symbol_names(body)?;
        let taken = |candidate: &str| {
            self.scope.iter().any(|name| name == candidate) || free.contains(candidate)
        };
        if !taken(&base) {
            return Ok(base);
        }
        let mut suffix = Integer::one();
        loop {
            let candidate = format!(
                "{base}{SUFFIX_SEPARATOR}{}",
                crate::decimal::integer_to_digits(&suffix)
            );
            if !taken(&candidate) {
                return Ok(candidate);
            }
            suffix = &suffix + &Integer::one();
        }
    }

    fn free_symbol_names(&self, root: ExprId) -> Result<HashSet<String>, PrintError> {
        let mut names = HashSet::new();
        let mut visited = HashSet::new();
        let mut pending = vec![root];
        while let Some(expression) = pending.pop() {
            if !visited.insert(expression) {
                continue;
            }
            match self.pool.node(expression).map_err(PrintError::Access)? {
                NodeView::Symbol(symbol) => {
                    let name = self.pool.symbol_name(symbol).map_err(PrintError::Symbol)?;
                    names.insert(name.to_string());
                }
                NodeView::Apply { arguments, .. } => pending.extend_from_slice(arguments),
                NodeView::Bind {
                    arguments, body, ..
                } => {
                    pending.extend_from_slice(arguments);
                    pending.push(body);
                }
                NodeView::Quantity { value, .. } => pending.push(value),
                NodeView::Array { elements, .. } => pending.extend_from_slice(elements),
                NodeView::Number(_) | NodeView::Bound(_) => {}
            }
        }
        Ok(names)
    }
}

fn is_valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters.next().is_some_and(char::is_alphabetic)
        && characters.all(is_identifier_continue)
        && !changes_parsing(name)
}

fn shorten_trailing_zeros(text: &str) -> String {
    if text.contains(['.', 'q']) {
        return text.to_string();
    }
    let trimmed = text.trim_end_matches('0');
    let zeros = text.len() - trimmed.len();
    if zeros >= SHORTENED_ZERO_COUNT && trimmed.chars().any(|c| c.is_ascii_digit()) {
        format!("{trimmed}e{zeros}")
    } else {
        text.to_string()
    }
}

fn typed(prefix: &str, content: &str) -> Printed {
    Printed {
        text: format!("{prefix}'{content}'"),
        level: PRIMARY,
        starts_with_plain_literal: false,
        ends_with_unit: false,
        is_literal: true,
        closed_root: None,
    }
}

fn float_content(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    let magnitude = value.abs();
    if magnitude == 0.0 || (SHORT_DECIMAL_LOWER..SHORT_DECIMAL_UPPER).contains(&magnitude) {
        format!("{value}")
    } else {
        format!("{value:e}")
    }
}

fn float32_content(value: f32) -> String {
    if value.is_nan() || value.is_infinite() {
        return float_content(f64::from(value));
    }
    let magnitude = f64::from(value.abs());
    if magnitude == 0.0 || (SHORT_DECIMAL_LOWER..SHORT_DECIMAL_UPPER).contains(&magnitude) {
        format!("{value}")
    } else {
        format!("{value:e}")
    }
}

fn quick_sort_keywords(partition: SortPartition, seed: Option<&String>) -> Vec<String> {
    let (partition_value, pivot) = partition_values(partition);
    let mut keywords = vec![
        format!("{PARTITION_KEYWORD}={partition_value}"),
        format!("{PIVOT_KEYWORD}={pivot}"),
    ];
    if let (SortPartition::LomutoRandom, Some(seed)) = (partition, seed) {
        keywords.push(format!("{SEED_KEYWORD}={seed}"));
    }
    keywords
}
