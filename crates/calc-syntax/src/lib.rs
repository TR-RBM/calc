mod ambiguity;
mod answer_notation;
mod ast;
mod attempts;
mod builder;
mod chemistry;
mod decimal;
mod error;
mod fraction_unit;
mod lexer;
mod names;
mod nuclear;
mod parser;
mod percent_sum;
mod printer;
mod reference;
mod temperature_sign;
mod unit_endings;

use calc_expr::{ExprId, ExprPool};

pub use ambiguity::AmbiguousApplication;
pub use answer_notation::{
    AnswerNotation, Coordinates, DecimalSeparator, DivisionSign, Juxtaposition, MixedNumbers,
    MultiplicationSign, NotationConflict, NotationMode, NotationValueError, NotationValues,
    RecurringMark,
};
pub use attempts::{RecognisedAttempt, corrected_text};
pub use builder::{DEPTH_LIMIT, LARGEST_DECIMAL_EXPONENT, Statement};
pub use chemistry::{ChemistryProblem, element_symbol};
pub use error::{Keyword, ParseError, ParseErrorKind, PrintError};
pub use lexer::written_radix;
pub use nuclear::NuclearProblem;
pub use parser::{CHAIN_LIMIT, NESTING_LIMIT};
pub use printer::PrintMode;

pub fn operator_call_name(operator: calc_expr::Operator) -> Option<&'static str> {
    names::call_name(operator)
}
pub use fraction_unit::FractionBeforeUnit;
pub use percent_sum::PercentInASum;
pub use reference::{
    Arguments, Associativity, Construct, ConstructKind, KeywordArgument, KeywordValue,
    LanguageReference, Precedence, PrecedenceRow, PrefixEntry, ReferenceEntry, ReferenceGroup,
    Spelling, SpellingMode, UnitEntry, language_reference, operator_construct,
};
pub use temperature_sign::AmbiguousTemperatureSign;
pub use unit_endings::{RightUnit, UnitEndedAtSpace};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedStatement {
    pub statement: Statement,
    pub unit_endings: Vec<UnitEndedAtSpace>,
}

pub fn parse_statement(pool: &mut ExprPool, line: &str) -> Result<Statement, ParseError> {
    parse_statement_with_unit_endings(pool, line).map(|parsed| parsed.statement)
}

pub fn parse_statement_with_unit_endings(
    pool: &mut ExprPool,
    line: &str,
) -> Result<ParsedStatement, ParseError> {
    let tokens = lexer::tokenize(line).map_err(|error| attempts::recognise(line, error))?;
    temperature_sign::check(&tokens)?;
    let syntax = parser::Parser::new(&tokens, line.len())
        .parse_statement()
        .map_err(|error| attempts::recognise(line, error))?;
    ambiguity::check(&tokens, unit_endings::statement_value(&syntax))?;
    fraction_unit::check(unit_endings::statement_value(&syntax))?;
    percent_sum::check(unit_endings::statement_value(&syntax))?;
    let unit_endings = unit_endings::in_statement(&tokens, line.len(), &syntax);
    let statement = builder::Builder::new(pool).build_statement(syntax)?;
    Ok(ParsedStatement {
        statement,
        unit_endings,
    })
}

pub fn parse_expression(pool: &mut ExprPool, text: &str) -> Result<ExprId, ParseError> {
    let tokens = lexer::tokenize(text).map_err(|error| attempts::recognise(text, error))?;
    temperature_sign::check(&tokens)?;
    let expression = parser::Parser::new(&tokens, text.len())
        .parse_complete_expression()
        .map_err(|error| attempts::recognise(text, error))?;
    ambiguity::check(&tokens, &expression)?;
    fraction_unit::check(&expression)?;
    percent_sum::check(&expression)?;
    builder::Builder::new(pool).build(&expression)
}

pub fn print_expression(
    pool: &ExprPool,
    expression: ExprId,
    mode: PrintMode,
) -> Result<String, PrintError> {
    printer::Printer::new(pool, mode).print(expression)
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_expr::{
        BinderKind, BuildError, BuiltinConstant, Head, LimitSide, NodeView, Operator,
        ReductionShape, SortMethod, SortOrder, SortSpec, SymbolError, SymbolKind,
        without_measurement_marks,
    };
    use calc_numbers::{Integer, Number};

    fn parse(pool: &mut ExprPool, text: &str) -> ExprId {
        parse_expression(pool, text).unwrap()
    }

    fn error_kind(text: &str) -> ParseErrorKind {
        let mut pool = ExprPool::new();
        parse_statement(&mut pool, text).unwrap_err().kind
    }

    fn integer(pool: &mut ExprPool, value: i64) -> ExprId {
        pool.number(Number::from(value)).unwrap()
    }

    fn fraction(pool: &mut ExprPool, numerator: i64, denominator: i64) -> ExprId {
        let value =
            Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap();
        pool.number(value).unwrap()
    }

    fn variable(pool: &mut ExprPool, name: &str) -> ExprId {
        let symbol = pool.intern_symbol(name, SymbolKind::Variable).unwrap();
        pool.symbol(symbol).unwrap()
    }

    fn apply(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> ExprId {
        pool.apply(Head::Operator(operator), arguments).unwrap()
    }

    fn measured(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> ExprId {
        let mark = pool.anonymous_measurement().unwrap();
        let mut arguments = arguments.to_vec();
        arguments.push(mark);
        apply(pool, operator, &arguments)
    }

    fn unmarked(pool: &mut ExprPool, expression: ExprId) -> ExprId {
        without_measurement_marks(pool, expression).unwrap()
    }

    fn unit_quantity(pool: &mut ExprPool, value: ExprId, unit: &str) -> ExprId {
        let unit = pool.units_mut().lookup(unit).unwrap();
        pool.quantity(value, unit).unwrap()
    }

    #[test]
    fn naming_statement_binds_a_variable_name() {
        let mut pool = ExprPool::new();
        let statement = parse_statement(&mut pool, "F = m * a").unwrap();
        let m = variable(&mut pool, "m");
        let a = variable(&mut pool, "a");
        let product = apply(&mut pool, Operator::Mul, &[m, a]);
        let name = pool.lookup_symbol("F").unwrap();
        assert_eq!(
            statement,
            Statement::Naming {
                name,
                value: product
            }
        );
    }

    #[test]
    fn function_naming_nests_one_lambda_per_parameter() {
        let mut pool = ExprPool::new();
        let statement = parse_statement(&mut pool, "f(x, y) = x - y").unwrap();
        let outer_variable = pool.bound(1).unwrap();
        let inner_variable = pool.bound(0).unwrap();
        let body = apply(&mut pool, Operator::Sub, &[outer_variable, inner_variable]);
        let inner = pool.bind(BinderKind::Lambda, &[], body).unwrap();
        let outer = pool.bind(BinderKind::Lambda, &[], inner).unwrap();
        let name = pool.lookup_symbol("f").unwrap();
        assert_eq!(statement, Statement::FunctionNaming { name, value: outer });
        assert_eq!(
            pool.symbol_kind(name).unwrap(),
            SymbolKind::Function { arity: 2 }
        );
    }

    #[test]
    fn plain_expression_is_an_expression_statement() {
        let mut pool = ExprPool::new();
        let statement = parse_statement(&mut pool, "sqrt(2) * pi").unwrap();
        let two = integer(&mut pool, 2);
        let root = apply(&mut pool, Operator::Sqrt, &[two]);
        let pi = pool.symbol(BuiltinConstant::Pi.symbol()).unwrap();
        let product = apply(&mut pool, Operator::Mul, &[root, pi]);
        assert_eq!(statement, Statement::Expression(product));
    }

    #[test]
    fn double_equals_is_an_equation_and_not_a_naming() {
        let mut pool = ExprPool::new();
        let statement = parse_statement(&mut pool, "x == 3").unwrap();
        let x = variable(&mut pool, "x");
        let three = integer(&mut pool, 3);
        let equation = apply(&mut pool, Operator::Equal, &[x, three]);
        assert_eq!(statement, Statement::Expression(equation));
    }

    #[test]
    fn label_reference_is_a_symbol() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "r3 + r5");
        let r3 = variable(&mut pool, "r3");
        let r5 = variable(&mut pool, "r5");
        assert_eq!(parsed, apply(&mut pool, Operator::Add, &[r3, r5]));
    }

    #[test]
    fn label_cannot_be_named() {
        assert_eq!(error_kind("r2 = 5"), ParseErrorKind::ReservedName);
    }

    #[test]
    fn builtin_constant_cannot_be_named() {
        assert_eq!(error_kind("pi = 3"), ParseErrorKind::ReservedName);
    }

    #[test]
    fn comment_produces_no_node() {
        let mut pool = ExprPool::new();
        let with_comment = parse(&mut pool, "2 # two");
        assert_eq!(with_comment, integer(&mut pool, 2));
    }

    #[test]
    fn decimal_literal_is_an_exact_rational() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "0.1");
        assert_eq!(parsed, fraction(&mut pool, 1, 10));
    }

    #[test]
    fn exponent_literal_with_integer_value_is_an_integer() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "1e8");
        assert_eq!(parsed, integer(&mut pool, 100_000_000));
    }

    #[test]
    fn negative_exponent_literal_is_an_exact_fraction() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "1.5e-3");
        assert_eq!(parsed, fraction(&mut pool, 3, 2000));
    }

    #[test]
    fn huge_exponent_is_rejected() {
        assert_eq!(error_kind("1e999999"), ParseErrorKind::ExponentTooLarge);
    }

    #[test]
    fn minus_before_a_literal_folds_into_the_number() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "-2");
        assert_eq!(parsed, integer(&mut pool, -2));
    }

    #[test]
    fn minus_before_a_power_is_a_negation() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "-2^2");
        let two = integer(&mut pool, 2);
        let power = apply(&mut pool, Operator::Pow, &[two, two]);
        assert_eq!(parsed, apply(&mut pool, Operator::Neg, &[power]));
    }

    #[test]
    fn minus_before_a_parenthesized_literal_is_a_negation() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "-(2)");
        let two = integer(&mut pool, 2);
        assert_eq!(parsed, apply(&mut pool, Operator::Neg, &[two]));
    }

    #[test]
    fn negative_exponent_folds_into_the_literal() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "2^-1");
        let two = integer(&mut pool, 2);
        let minus_one = integer(&mut pool, -1);
        assert_eq!(parsed, apply(&mut pool, Operator::Pow, &[two, minus_one]));
    }

    #[test]
    fn typed_rational_literal_is_exact() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "q'-1/3'");
        assert_eq!(parsed, fraction(&mut pool, -1, 3));
    }

    #[test]
    fn typed_f64_literal_rounds_the_decimal() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "f64'0.1'");
        assert_eq!(parsed, pool.number(Number::F64(0.1)).unwrap());
    }

    #[test]
    fn typed_f32_literal_keeps_negative_zero() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "f32'-0'");
        assert_eq!(parsed, pool.number(Number::F32(-0.0)).unwrap());
    }

    #[test]
    fn typed_f64_nan_is_the_quiet_nan() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "f64'nan'");
        let quiet = f64::from_bits(0x7ff8_0000_0000_0000);
        assert_eq!(parsed, pool.number(Number::F64(quiet)).unwrap());
    }

    #[test]
    fn typed_bits_literal_gives_any_nan_payload() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "f64'bits:0x7ff8000000000001'");
        let payload = f64::from_bits(0x7ff8_0000_0000_0001);
        assert_eq!(parsed, pool.number(Number::F64(payload)).unwrap());
    }

    #[test]
    fn typed_literal_rejects_non_decimal_content() {
        assert_eq!(
            error_kind("f64'infinity'"),
            ParseErrorKind::InvalidTypedLiteral
        );
    }

    #[test]
    fn conversion_to_machine_format_is_an_operator_over_the_exact_value() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "to_f64(0.1)");
        let tenth = fraction(&mut pool, 1, 10);
        assert_eq!(parsed, apply(&mut pool, Operator::ToF64, &[tenth]));
    }

    #[test]
    fn temperature_call_names_are_the_a_053_operators() {
        let mut pool = ExprPool::new();
        let names = [
            "from_celsius",
            "from_fahrenheit",
            "to_celsius",
            "to_fahrenheit",
        ];

        let operators: Vec<Option<Operator>> = names
            .iter()
            .map(|name| {
                let parsed = parse(&mut pool, &format!("{name}(x)"));
                match pool.node(parsed).unwrap() {
                    NodeView::Apply {
                        head: Head::Operator(operator),
                        ..
                    } => Some(operator),
                    _ => None,
                }
            })
            .collect();

        assert_eq!(
            operators,
            [
                Some(Operator::FromCelsius),
                Some(Operator::FromFahrenheit),
                Some(Operator::ToCelsius),
                Some(Operator::ToFahrenheit)
            ]
        );
    }

    #[test]
    fn temperature_operator_prints_in_call_form_in_both_modes() {
        let mut pool = ExprPool::new();
        let reading = parse(&mut pool, "from_celsius(20.5)");

        let printed = [PrintMode::Ascii, PrintMode::Unicode]
            .map(|mode| print_expression(&pool, reading, mode).unwrap());

        assert_eq!(printed, ["from_celsius(20.5)", "from_celsius(20.5)"]);
    }

    #[test]
    fn temperature_call_names_are_reserved() {
        let kinds = [
            "from_celsius",
            "from_fahrenheit",
            "to_celsius",
            "to_fahrenheit",
        ]
        .map(|name| error_kind(&format!("{name} = 3")));

        assert_eq!(kinds, [ParseErrorKind::ReservedName; 4]);
    }

    #[test]
    fn exact_call_is_to_exact() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "exact(x)");
        let x = variable(&mut pool, "x");
        assert_eq!(parsed, apply(&mut pool, Operator::ToExact, &[x]));
    }

    #[test]
    fn constants_are_pre_registered_symbols() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "e");
        assert_eq!(parsed, pool.symbol(BuiltinConstant::E.symbol()).unwrap());
    }

    #[test]
    fn infinity_sign_is_the_inf_constant() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "\u{221E}");
        assert_eq!(
            parsed,
            pool.symbol(BuiltinConstant::Infinity.symbol()).unwrap()
        );
    }

    #[test]
    fn plus_minus_states_a_standard_uncertainty() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "2 +- 0.1");
        let two = integer(&mut pool, 2);
        let tenth = fraction(&mut pool, 1, 10);
        let expected = measured(&mut pool, Operator::Uncertain, &[two, tenth]);

        assert_eq!(unmarked(&mut pool, parsed), expected);
    }

    #[test]
    fn two_measurements_written_alike_are_two_expressions() {
        let mut pool = ExprPool::new();

        let first = parse(&mut pool, "10 +- 1");
        let second = parse(&mut pool, "10 +- 1");

        assert_ne!(first, second);
    }

    #[test]
    fn two_measurements_written_alike_print_alike() {
        let mut pool = ExprPool::new();
        let first = parse(&mut pool, "10 +- 1");
        let second = parse(&mut pool, "10 +- 1");

        assert_eq!(
            print_expression(&pool, first, PrintMode::Unicode).unwrap(),
            print_expression(&pool, second, PrintMode::Unicode).unwrap()
        );
    }

    #[test]
    fn coverage_factor_states_an_expanded_uncertainty() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "2 \u{00B1} 0.1 (k=2)");
        let two = integer(&mut pool, 2);
        let tenth = fraction(&mut pool, 1, 10);
        let expected = measured(&mut pool, Operator::UncertainExpanded, &[two, tenth, two]);

        assert_eq!(unmarked(&mut pool, parsed), expected);
    }

    #[test]
    fn unit_after_a_literal_makes_a_quantity() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "2.50 kg");
        let value = fraction(&mut pool, 5, 2);
        assert_eq!(parsed, unit_quantity(&mut pool, value, "kg"));
    }

    #[test]
    fn unit_after_an_uncertainty_applies_to_the_whole_group() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "2.50 +- 0.01 kg");
        let value = fraction(&mut pool, 5, 2);
        let uncertainty = fraction(&mut pool, 1, 100);
        let group = measured(&mut pool, Operator::Uncertain, &[value, uncertainty]);
        let expected = unit_quantity(&mut pool, group, "kg");

        assert_eq!(unmarked(&mut pool, parsed), expected);
    }

    #[test]
    fn unit_after_a_reserved_constant_makes_a_quantity() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "pi rad");
        let pi = pool.symbol(BuiltinConstant::Pi.symbol()).unwrap();
        assert_eq!(parsed, unit_quantity(&mut pool, pi, "rad"));
    }

    #[test]
    fn unit_after_the_pi_letter_makes_the_same_quantity() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "\u{03C0} rad");
        let pi = pool.symbol(BuiltinConstant::Pi.symbol()).unwrap();
        assert_eq!(parsed, unit_quantity(&mut pool, pi, "rad"));
    }

    #[test]
    fn a_unit_after_a_constant_binds_before_a_product() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "2 * pi rad");
        let two = integer(&mut pool, 2);
        let pi = pool.symbol(BuiltinConstant::Pi.symbol()).unwrap();
        let radians = unit_quantity(&mut pool, pi, "rad");
        assert_eq!(parsed, apply(&mut pool, Operator::Mul, &[two, radians]));
    }

    #[test]
    fn a_constant_with_a_unit_is_the_node_a_grouping_builds() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "pi rad");
        let grouped = parse(&mut pool, "(pi) rad");
        assert_eq!(parsed, grouped);
    }

    #[test]
    fn a_name_that_is_not_reserved_takes_no_unit() {
        assert_eq!(error_kind("x rad"), ParseErrorKind::UnexpectedToken);
    }

    #[test]
    fn a_line_label_takes_no_unit() {
        assert_eq!(error_kind("r1 kg"), ParseErrorKind::UnexpectedToken);
    }

    #[test]
    fn every_reserved_constant_takes_a_unit() {
        for name in ["pi", "\u{03C0}", "e", "i", "inf"] {
            let mut pool = ExprPool::new();
            assert!(
                parse_expression(&mut pool, &format!("{name} s")).is_ok(),
                "{name} s"
            );
        }
    }

    #[test]
    fn the_degree_sign_binds_to_a_constant_without_a_space() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "pi\u{00B0}");
        let pi = pool.symbol(BuiltinConstant::Pi.symbol()).unwrap();
        assert_eq!(parsed, unit_quantity(&mut pool, pi, "deg"));
    }

    #[test]
    fn unit_after_a_grouping_parenthesis_makes_a_quantity() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "(a + b) m");
        let a = variable(&mut pool, "a");
        let b = variable(&mut pool, "b");
        let sum = apply(&mut pool, Operator::Add, &[a, b]);
        assert_eq!(parsed, unit_quantity(&mut pool, sum, "m"));
    }

    #[test]
    fn compound_unit_is_one_unit_product() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "9.81 m/s^2");
        let value = fraction(&mut pool, 981, 100);
        let metre = pool.units_mut().lookup("m").unwrap();
        let second = pool.units_mut().lookup("s").unwrap();
        let squared = pool.units_mut().power(second, 2).unwrap();
        let acceleration = pool.units_mut().divide(metre, squared).unwrap();
        assert_eq!(parsed, pool.quantity(value, acceleration).unwrap());
    }

    #[test]
    fn space_ends_a_unit_expression() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "2 m / s");
        let two = integer(&mut pool, 2);
        let length = unit_quantity(&mut pool, two, "m");
        let s = variable(&mut pool, "s");
        assert_eq!(parsed, apply(&mut pool, Operator::Div, &[length, s]));
    }

    #[test]
    fn prefixed_unit_is_one_unit_name() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "3 km");
        let three = integer(&mut pool, 3);
        assert_eq!(parsed, unit_quantity(&mut pool, three, "km"));
    }

    #[test]
    fn unit_name_is_looked_up_only_at_unit_positions() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "m * 2 m");
        let m = variable(&mut pool, "m");
        let two = integer(&mut pool, 2);
        let metres = unit_quantity(&mut pool, two, "m");
        assert_eq!(parsed, apply(&mut pool, Operator::Mul, &[m, metres]));
    }

    #[test]
    fn arrow_converts_to_a_unit() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "F -> N");
        let force = variable(&mut pool, "F");
        let one = integer(&mut pool, 1);
        let newton = unit_quantity(&mut pool, one, "N");
        assert_eq!(
            parsed,
            apply(&mut pool, Operator::ConvertUnit, &[force, newton])
        );
    }

    #[test]
    fn name_after_a_literal_that_is_not_a_unit_is_an_error() {
        assert_eq!(error_kind("2 apples"), ParseErrorKind::NotAUnit);
    }

    #[test]
    fn implicit_multiplication_is_an_error() {
        assert_eq!(error_kind("a b"), ParseErrorKind::UnexpectedToken);
    }

    #[test]
    fn multiplication_binds_tighter_than_addition() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "a + b * c");
        let a = variable(&mut pool, "a");
        let b = variable(&mut pool, "b");
        let c = variable(&mut pool, "c");
        let product = apply(&mut pool, Operator::Mul, &[b, c]);
        assert_eq!(parsed, apply(&mut pool, Operator::Add, &[a, product]));
    }

    #[test]
    fn additive_operators_associate_to_the_left() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "a + b + c");
        let a = variable(&mut pool, "a");
        let b = variable(&mut pool, "b");
        let c = variable(&mut pool, "c");
        let inner = apply(&mut pool, Operator::Add, &[a, b]);
        assert_eq!(parsed, apply(&mut pool, Operator::Add, &[inner, c]));
    }

    #[test]
    fn parentheses_keep_the_written_tree() {
        let mut pool = ExprPool::new();
        let left = parse(&mut pool, "a + b + c");
        let right = parse(&mut pool, "a + (b + c)");
        assert_ne!(left, right);
    }

    #[test]
    fn power_associates_to_the_right() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "a^b^c");
        let a = variable(&mut pool, "a");
        let b = variable(&mut pool, "b");
        let c = variable(&mut pool, "c");
        let inner = apply(&mut pool, Operator::Pow, &[b, c]);
        assert_eq!(parsed, apply(&mut pool, Operator::Pow, &[a, inner]));
    }

    #[test]
    fn both_summation_signs_read_as_the_sum() {
        let mut pool = ExprPool::new();
        let call = parse(&mut pool, "sum(i^2, i, 1, n)");
        assert_eq!(parse(&mut pool, "\u{2211} i^2, i=1..n"), call);
        assert_eq!(parse(&mut pool, "\u{03A3} i^2, i=1..n"), call);
    }

    #[test]
    fn both_product_signs_read_as_the_product() {
        let mut pool = ExprPool::new();
        let call = parse(&mut pool, "product(k, k, 1, n)");
        assert_eq!(parse(&mut pool, "\u{220F} k, k=1..n"), call);
        assert_eq!(parse(&mut pool, "\u{03A0} k, k=1..n"), call);
    }

    #[test]
    fn the_unicode_constant_letters_read_as_e_and_i() {
        let mut pool = ExprPool::new();
        assert_eq!(parse(&mut pool, "\u{212F}"), parse(&mut pool, "e"));
        assert_eq!(parse(&mut pool, "\u{2148}"), parse(&mut pool, "i"));
    }

    #[test]
    fn italic_letters_and_j_stay_variable_names() {
        let mut pool = ExprPool::new();
        assert_ne!(parse(&mut pool, "\u{1D452}"), parse(&mut pool, "e"));
        assert_ne!(parse(&mut pool, "j"), parse(&mut pool, "i"));
    }

    #[test]
    fn the_times_and_division_signs_stay_recognised_attempts() {
        assert_eq!(
            error_kind("2 \u{00D7} 3"),
            ParseErrorKind::RecognisedAttempt(RecognisedAttempt::TimesSign)
        );
        assert_eq!(
            error_kind("6 \u{00F7} 3"),
            ParseErrorKind::RecognisedAttempt(RecognisedAttempt::DivisionSign)
        );
    }

    #[test]
    fn degree_sign_is_the_degree_unit() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "30 \u{00B0}");
        assert_eq!(parsed, parse(&mut pool, "30 deg"));
    }

    #[test]
    fn degree_sign_binds_without_a_space() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "30\u{00B0}");
        assert_eq!(parsed, parse(&mut pool, "30 deg"));
    }

    #[test]
    fn degree_sign_binds_inside_a_prefix_application() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "sin 30\u{00B0}");
        assert_eq!(parsed, parse(&mut pool, "sin(30 deg)"));
    }

    #[test]
    fn degree_sign_stands_in_a_unit_expression() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "30 \u{00B0}/s");
        assert_eq!(parsed, parse(&mut pool, "30 deg/s"));
    }

    #[test]
    fn degree_sign_stands_after_a_group_and_as_a_conversion_target() {
        let mut pool = ExprPool::new();
        assert_eq!(
            parse(&mut pool, "(a + b)\u{00B0}"),
            parse(&mut pool, "(a + b) deg")
        );
        assert_eq!(
            parse(&mut pool, "x -> \u{00B0}"),
            parse(&mut pool, "x -> deg")
        );
    }

    #[test]
    fn degree_sign_after_an_uncertainty_applies_to_the_whole_group() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "2 \u{00B1} 0.5\u{00B0}");
        let spelled = parse(&mut pool, "2 \u{00B1} 0.5 deg");

        assert_eq!(unmarked(&mut pool, parsed), unmarked(&mut pool, spelled));
    }

    #[test]
    fn spaced_degree_sign_in_a_prefix_application_is_an_error() {
        assert!(parse_statement(&mut ExprPool::new(), "sin 30 \u{00B0}").is_err());
    }

    #[test]
    fn every_display_symbol_is_input_or_names_its_replacement() {
        let table = calc_units::UnitTable::new();
        for named_unit in table.named_units() {
            let symbol = table.symbol(named_unit).unwrap().to_string();
            let display_symbol = table.display_symbol(named_unit).unwrap().to_string();
            if display_symbol == symbol {
                continue;
            }
            let mut pool = ExprPool::new();
            let written = parse_expression(&mut pool, &format!("1 {symbol}")).unwrap();
            match parse_expression(&mut pool, &format!("1 {display_symbol}")) {
                Ok(displayed) => assert_eq!(displayed, written, "{display_symbol}"),
                Err(error) => assert!(
                    matches!(
                        error.kind,
                        ParseErrorKind::RecognisedAttempt(_)
                            | ParseErrorKind::AmbiguousTemperatureSign(_)
                    ),
                    "{display_symbol}: {:?}",
                    error.kind
                ),
            }
        }
    }

    #[test]
    fn superscript_is_a_power() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "x\u{00B2}");
        assert_eq!(parsed, parse(&mut pool, "x^2"));
    }

    #[test]
    fn prefix_application_takes_a_power_argument() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "sin x^2");
        assert_eq!(parsed, parse(&mut pool, "sin(x^2)"));
    }

    #[test]
    fn unary_plus_produces_no_node() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "+x");
        assert_eq!(parsed, variable(&mut pool, "x"));
    }

    #[test]
    fn square_root_sign_is_sqrt() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "\u{221A}x");
        assert_eq!(parsed, parse(&mut pool, "sqrt(x)"));
    }

    #[test]
    fn factorial_is_postfix() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "n!");
        let n = variable(&mut pool, "n");
        assert_eq!(parsed, apply(&mut pool, Operator::Factorial, &[n]));
    }

    #[test]
    fn single_equals_inside_an_expression_is_an_equation() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "x^2 = 2");
        assert_eq!(parsed, parse(&mut pool, "x^2 == 2"));
    }

    #[test]
    fn unicode_relation_signs_are_aliases() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "a \u{2264} b");
        assert_eq!(parsed, parse(&mut pool, "a <= b"));
    }

    #[test]
    fn chained_relation_is_an_error() {
        assert_eq!(error_kind("a < b < c"), ParseErrorKind::ChainedRelation);
    }

    #[test]
    fn nesting_past_the_limit_is_an_error_and_not_a_stack_overflow() {
        let depth = NESTING_LIMIT + 1;
        let written = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
        assert_eq!(
            error_kind(&written),
            ParseErrorKind::NestedTooDeeply {
                limit: NESTING_LIMIT
            }
        );
    }

    #[test]
    fn a_unit_denominator_may_be_a_group() {
        let mut pool = ExprPool::new();
        let grouped = parse(&mut pool, "4186 J/(kg*K)");
        let chained = parse(&mut pool, "4186 J/kg/K");
        assert_eq!(grouped, chained);
    }

    #[test]
    fn a_group_in_a_unit_keeps_a_division_inside_it() {
        let mut pool = ExprPool::new();
        let grouped = parse(&mut pool, "1 kg/(m/s)");
        let written = parse(&mut pool, "1 kg*s/m");
        assert_eq!(grouped, written);
    }

    #[test]
    fn a_groups_exponent_multiplies_the_exponent_of_each_factor() {
        let mut pool = ExprPool::new();
        let grouped = parse(&mut pool, "1 J/(kg*K)^2");
        let written = parse(&mut pool, "1 J/kg^2/K^2");
        assert_eq!(grouped, written);
    }

    #[test]
    fn a_unit_expression_does_not_begin_with_a_group() {
        assert_eq!(error_kind("1 (kg*m)/s^2"), ParseErrorKind::UnexpectedToken);
    }

    #[test]
    fn a_unit_nested_past_the_limit_is_refused_like_any_other_nesting() {
        let depth = NESTING_LIMIT + 1;
        let written = format!("1 m/{}s{}", "(".repeat(depth), ")".repeat(depth));
        assert_eq!(
            error_kind(&written),
            ParseErrorKind::NestedTooDeeply {
                limit: NESTING_LIMIT
            }
        );
    }

    #[test]
    fn a_chain_past_the_limit_is_an_error_and_says_it_is_a_chain() {
        let written = format!("{}1", "1+".repeat(CHAIN_LIMIT + 1));
        assert_eq!(
            error_kind(&written),
            ParseErrorKind::ChainTooLong { limit: CHAIN_LIMIT }
        );
    }

    #[test]
    fn nesting_at_the_limit_still_parses() {
        let mut pool = ExprPool::new();
        let parentheses = NESTING_LIMIT - 1;
        let written = format!("{}1{}", "(".repeat(parentheses), ")".repeat(parentheses));
        assert!(parse_statement(&mut pool, &written).is_ok());
    }

    #[test]
    fn logical_keywords_follow_their_precedence() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "not p and q or r");
        let p = variable(&mut pool, "p");
        let q = variable(&mut pool, "q");
        let r = variable(&mut pool, "r");
        let negated = apply(&mut pool, Operator::Not, &[p]);
        let conjunction = apply(&mut pool, Operator::And, &[negated, q]);
        assert_eq!(parsed, apply(&mut pool, Operator::Or, &[conjunction, r]));
    }

    #[test]
    fn middle_dot_is_multiplication() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "m\u{00B7}a");
        assert_eq!(parsed, parse(&mut pool, "m * a"));
    }

    #[test]
    fn builtin_call_applies_the_operator() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "atan2(y, x)");
        let y = variable(&mut pool, "y");
        let x = variable(&mut pool, "x");
        assert_eq!(parsed, apply(&mut pool, Operator::Atan2, &[y, x]));
    }

    #[test]
    fn user_function_call_registers_a_function_symbol() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "g(1, 2)");
        let g = pool.lookup_symbol("g").unwrap();
        let one = integer(&mut pool, 1);
        let two = integer(&mut pool, 2);
        assert_eq!(parsed, pool.apply(Head::Function(g), &[one, two]).unwrap());
    }

    #[test]
    fn builtin_call_with_wrong_argument_count_is_an_error() {
        assert_eq!(
            error_kind("sin(1, 2)"),
            ParseErrorKind::ArityMismatch {
                expected: 1,
                found: 2
            }
        );
    }

    #[test]
    fn calling_a_variable_is_an_error() {
        let mut pool = ExprPool::new();
        let v = pool.intern_symbol("v", SymbolKind::Variable).unwrap();
        assert_eq!(
            parse_expression(&mut pool, "v(1)").unwrap_err().kind,
            ParseErrorKind::Build(BuildError::NotAFunction(v))
        );
    }

    #[test]
    fn naming_an_existing_function_as_a_variable_is_a_symbol_conflict() {
        let mut pool = ExprPool::new();
        let h = pool
            .intern_symbol("h", SymbolKind::Function { arity: 1 })
            .unwrap();
        assert_eq!(
            parse_statement(&mut pool, "h = 1").unwrap_err().kind,
            ParseErrorKind::Symbol(SymbolError::KindConflict {
                symbol: h,
                existing: SymbolKind::Function { arity: 1 }
            })
        );
    }

    #[test]
    fn anonymous_function_binds_its_parameter() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "x |-> x^2");
        let bound = pool.bound(0).unwrap();
        let two = integer(&mut pool, 2);
        let body = apply(&mut pool, Operator::Pow, &[bound, two]);
        assert_eq!(parsed, pool.bind(BinderKind::Lambda, &[], body).unwrap());
        assert_eq!(pool.bound_name(parsed), Some("x"));
    }

    #[test]
    fn maps_to_sign_is_an_alias() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "t \u{21A6} t + 1");
        assert_eq!(parsed, parse(&mut pool, "x |-> x + 1"));
    }

    #[test]
    fn vector_has_a_one_dimensional_shape() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "[1, 2, 3]");
        let elements: Vec<ExprId> = (1..=3).map(|value| integer(&mut pool, value)).collect();
        assert_eq!(parsed, pool.array(&[3], &elements).unwrap());
    }

    #[test]
    fn matrix_rows_are_separated_by_semicolons() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "[1, 2; 3, 4]");
        let elements: Vec<ExprId> = (1..=4).map(|value| integer(&mut pool, value)).collect();
        assert_eq!(parsed, pool.array(&[2, 2], &elements).unwrap());
    }

    #[test]
    fn ragged_matrix_is_an_error() {
        assert_eq!(error_kind("[1, 2; 3]"), ParseErrorKind::RaggedArray);
    }

    #[test]
    fn definite_integral_binds_its_variable_over_the_body() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "integral(sin(x), x, 0, pi)");
        let bound = pool.bound(0).unwrap();
        let body = apply(&mut pool, Operator::Sin, &[bound]);
        let zero = integer(&mut pool, 0);
        let pi = pool.symbol(BuiltinConstant::Pi.symbol()).unwrap();
        assert_eq!(
            parsed,
            pool.bind(BinderKind::Integral, &[zero, pi], body).unwrap()
        );
    }

    #[test]
    fn integral_sign_form_gives_the_same_node() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "\u{222B} sin x dx, 0..\u{03C0}");
        assert_eq!(parsed, parse(&mut pool, "integral(sin(x), x, 0, pi)"));
    }

    #[test]
    fn indefinite_integral_sign_form_has_no_bounds() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "\u{222B} x^2 dx");
        assert_eq!(parsed, parse(&mut pool, "integral(x^2, x)"));
    }

    #[test]
    fn integral_sign_without_differential_is_an_error() {
        assert_eq!(
            error_kind("\u{222B} x^2"),
            ParseErrorKind::MissingDifferential
        );
    }

    #[test]
    fn sum_uses_left_fold_by_default() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "sum(i^2, i, 1, n)");
        let NodeView::Bind { binder, .. } = pool.node(parsed).unwrap() else {
            panic!("expected a binder");
        };
        assert_eq!(binder, BinderKind::Sum(ReductionShape::LeftFold));
    }

    #[test]
    fn shape_keyword_selects_the_halving_tree() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "sum(i, i, 1, 8, shape=halving)");
        let NodeView::Bind { binder, .. } = pool.node(parsed).unwrap() else {
            panic!("expected a binder");
        };
        assert_eq!(binder, BinderKind::Sum(ReductionShape::Halving));
    }

    #[test]
    fn a_named_sort_is_a_binder_over_its_list_with_the_key_as_body() {
        let mut pool = ExprPool::new();
        let parsed = parse(
            &mut pool,
            "insertion_sort([3, 1], x |-> abs(x), order=decreasing)",
        );
        let NodeView::Bind {
            binder, arguments, ..
        } = pool.node(parsed).unwrap()
        else {
            panic!("expected a binder");
        };
        assert_eq!(
            (binder, arguments.len()),
            (
                BinderKind::Sort(SortSpec {
                    method: SortMethod::Insertion,
                    order: SortOrder::Decreasing,
                }),
                1
            )
        );
    }

    #[test]
    fn a_sort_without_a_key_prints_without_one() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "insertion_sort([3, 1, 2])");

        assert_eq!(
            print_expression(&pool, parsed, PrintMode::Ascii).unwrap(),
            "insertion_sort([3, 1, 2])"
        );
    }

    #[test]
    fn an_unknown_order_is_refused() {
        let mut pool = ExprPool::new();

        let error =
            parse_expression(&mut pool, "insertion_sort([3, 1], order=sideways)").unwrap_err();

        assert_eq!(
            error.kind,
            ParseErrorKind::InvalidKeywordValue {
                keyword: Keyword::Order
            }
        );
    }

    #[test]
    fn sigma_form_gives_the_same_sum() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "\u{03A3} i^2, i=1..n");
        assert_eq!(parsed, parse(&mut pool, "sum(i^2, i, 1, n)"));
    }

    #[test]
    fn product_sign_form_gives_the_same_product() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "\u{220F} k, k=1..n");
        assert_eq!(parsed, parse(&mut pool, "product(k, k, 1, n)"));
    }

    #[test]
    fn binder_variable_shadows_a_constant_of_the_same_name() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "sum(i, i, 1, 3)");
        let NodeView::Bind { body, .. } = pool.node(parsed).unwrap() else {
            panic!("expected a binder");
        };
        assert_eq!(pool.node(body).unwrap(), NodeView::Bound(0));
    }

    #[test]
    fn unknown_keyword_is_an_error() {
        assert_eq!(
            error_kind("sum(i, i, 1, 3, speed=fast)"),
            ParseErrorKind::UnknownKeyword
        );
    }

    #[test]
    fn a_positional_argument_after_a_keyword_is_named_as_such() {
        assert_eq!(
            error_kind("sum(i, i, 1, 3, shape=halving, 4)"),
            ParseErrorKind::PositionalAfterKeyword
        );
    }

    #[test]
    fn invalid_keyword_value_is_an_error() {
        assert_eq!(
            error_kind("sum(i, i, 1, 3, shape=round)"),
            ParseErrorKind::InvalidKeywordValue {
                keyword: Keyword::Shape
            }
        );
    }

    #[test]
    fn one_sided_limit_keeps_its_side() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "limit(1/x, x, 0, side=right)");
        let NodeView::Bind { binder, .. } = pool.node(parsed).unwrap() else {
            panic!("expected a binder");
        };
        assert_eq!(binder, BinderKind::Limit(LimitSide::Right));
    }

    #[test]
    fn lim_form_gives_the_same_limit() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "lim sin(x)/x, x\u{2192}0");
        assert_eq!(parsed, parse(&mut pool, "limit(sin(x)/x, x, 0)"));
    }

    #[test]
    fn derivative_takes_its_point_at_the_free_variable() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "diff(x^2, x)");
        let point = variable(&mut pool, "x");
        let bound = pool.bound(0).unwrap();
        let two = integer(&mut pool, 2);
        let body = apply(&mut pool, Operator::Pow, &[bound, two]);
        assert_eq!(
            parsed,
            pool.bind(BinderKind::Derivative, &[point], body).unwrap()
        );
    }

    #[test]
    fn derivative_at_a_point_uses_that_point() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "diff(x^2, x, 3)");
        let NodeView::Bind { arguments, .. } = pool.node(parsed).unwrap() else {
            panic!("expected a binder");
        };
        let point = arguments[0];
        assert_eq!(point, integer(&mut pool, 3));
    }

    #[test]
    fn d_over_dx_form_gives_the_same_derivative() {
        let mut pool = ExprPool::new();
        let parsed = parse(&mut pool, "d/dx x^2");
        assert_eq!(parsed, parse(&mut pool, "diff(x^2, x)"));
    }

    #[test]
    fn operator_name_used_as_a_value_is_an_error() {
        assert_eq!(error_kind("sin + 1"), ParseErrorKind::ReservedName);
    }

    #[test]
    fn error_span_points_at_the_offending_token() {
        let mut pool = ExprPool::new();
        let error = parse_statement(&mut pool, "1 + $").unwrap_err();
        assert_eq!(error.span, 4..5);
    }

    #[test]
    fn missing_operand_is_an_unexpected_end() {
        assert_eq!(error_kind("1 +"), ParseErrorKind::UnexpectedEnd);
    }

    #[test]
    fn dimensionless_quantity_cannot_be_printed() {
        let mut pool = ExprPool::new();
        let two = integer(&mut pool, 2);
        let dimensionless = pool.units().dimensionless();
        let quantity = pool.quantity(two, dimensionless).unwrap();
        assert_eq!(
            print_expression(&pool, quantity, PrintMode::Ascii),
            Err(PrintError::DimensionlessQuantity)
        );
    }

    #[test]
    fn printer_renames_a_bound_variable_that_would_capture_a_free_one() {
        let mut pool = ExprPool::new();
        let free = variable(&mut pool, "x");
        let bound = pool.bound(0).unwrap();
        let body = apply(&mut pool, Operator::Add, &[bound, free]);
        let lambda = pool.bind(BinderKind::Lambda, &[], body).unwrap();
        pool.record_bound_name(lambda, "x").unwrap();
        assert_eq!(
            print_expression(&pool, lambda, PrintMode::Ascii).unwrap(),
            "x_1 |-> x_1 + x"
        );
    }

    #[test]
    fn printing_then_parsing_gives_the_same_expression() {
        let sources = [
            "F = m * a",
            "a + (b + c) - d",
            "(a + b) + c",
            "a * b / c * (d / e)",
            "-2",
            "-(2)",
            "-2^2",
            "(-2)^2",
            "2^-1",
            "2^-x",
            "x^y^z",
            "(x^y)^z",
            "- -x",
            "-x!",
            "(-3)!",
            "0.1 + q'1/3' - 1e30",
            "-0.0015",
            "f64'0.1' + f64'-0' + f64'inf' + f64'-inf' + f64'nan'",
            "f64'bits:0x7ff8000000000001' * f32'1e-40' * f32'3.5'",
            "f64'1e300' / f64'2.5e-320'",
            "to_f32(x) + exact(y) + to_f64(0.1)",
            "pi + e * i - inf",
            "2.50 +- 0.01 kg",
            "(2.50 +- 0.01 kg) * 9.81 m/s^2",
            "a +- (2 m)",
            "2 +- 0.1 (k=2)",
            "uncertain_expanded(a, b, c)",
            "-2 kg",
            "-(2 kg)",
            "(a + b) m",
            "(2 m)^2",
            "3 km -> m",
            "F -> kg*m/s^2 -> N",
            "convert_unit(x, y)",
            "from_celsius(20.5) + from_fahrenheit(-40)",
            "to_celsius(300 K) - to_fahrenheit(from_celsius(t))",
            "sin(x)^2 + cos(x)^2",
            "sqrt(2) * abs(-3)",
            "atan2(y, x) + mul_add(a, b, c) + select(p, a, b) + complex(1, 2)",
            "min(a, b) + max(a, b) + floor(x) + ceil(x) + trunc(x) + round_ties_even(x) + copysign(a, b)",
            "x == 3",
            "a != b and a <= b or not a > b",
            "a >= b",
            "(a < b) == (c < d)",
            "n! + (n!)!",
            "[1, 2, 3]",
            "[1, 2; 3, 4] + [a; b]",
            "[]",
            "x |-> x^2 + 1",
            "x |-> y |-> x * y",
            "(x |-> x) + 1",
            "integral(sin(x), x, 0, pi)",
            "integral(x^2, x)",
            "sum(i^2, i, 1, n) + sum(i, i, 1, 8, shape=halving)",
            "product(k, k, 1, n, shape=halving)",
            "insertion_sort([3, 1, 2]) + insertion_sort(a, x |-> abs(x), order=decreasing)",
            "limit(sin(x) / x, x, 0) + limit(1 / x, x, 0, side=left)",
            "diff(x^2, x) + diff(x^3, x, 2)",
            "x |-> diff(x^2, x)",
            "sum(sum(i * j, j, 1, i), i, 1, n)",
            "g(1, 2) * h(x)",
            "r1 + r2",
            "sum(i * e, i, 1, 3) + (x |-> x + e) + sum(i * i, i, 1, 2)",
            "-1000000 + 2500000000",
            "x\u{00B2} + x\u{207B}\u{00B9}",
        ];
        for source in sources {
            let mut pool = ExprPool::new();
            let original = match parse_statement(&mut pool, source).unwrap() {
                Statement::Naming { value, .. }
                | Statement::FunctionNaming { value, .. }
                | Statement::Expression(value) => value,
            };
            for mode in [PrintMode::Ascii, PrintMode::Unicode] {
                let printed = print_expression(&pool, original, mode).unwrap();
                let reparsed = parse_expression(&mut pool, &printed)
                    .unwrap_or_else(|error| panic!("{source} printed as {printed}: {error:?}"));
                let reparsed = unmarked(&mut pool, reparsed);
                let original = unmarked(&mut pool, original);

                assert_eq!(reparsed, original, "{source} printed as {printed}");
            }
        }
    }
}
