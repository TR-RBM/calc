use std::ops::Range;

use calc_expr::{
    BinderKind, BuiltinConstant, ExprPool, LimitSide, Operator, ReductionShape, SortBubbleForm,
    SortCombForm, SortGaps, SortMethod, SortOddEvenForm, SortOrder, SortPartition, SortShakerForm,
    SortSpec,
};
use calc_units::UnitTable;

use crate::decimal::integer_to_digits;
use crate::lexer::{TokenKind, TypedLiteralKind, tokenize};
use crate::names::{is_reserved, operator_for_call};
use crate::printer::PrintMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceGroup {
    Numbers,
    Uncertainty,
    Units,
    Operators,
    Relations,
    Calls,
    KeywordArguments,
    Arrays,
    Binders,
    Statements,
}

impl ReferenceGroup {
    pub const ALL: [ReferenceGroup; 10] = [
        ReferenceGroup::Numbers,
        ReferenceGroup::Uncertainty,
        ReferenceGroup::Units,
        ReferenceGroup::Operators,
        ReferenceGroup::Relations,
        ReferenceGroup::Calls,
        ReferenceGroup::KeywordArguments,
        ReferenceGroup::Arrays,
        ReferenceGroup::Binders,
        ReferenceGroup::Statements,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ReferenceGroup::Numbers => "numbers",
            ReferenceGroup::Uncertainty => "uncertainty",
            ReferenceGroup::Units => "units",
            ReferenceGroup::Operators => "operators",
            ReferenceGroup::Relations => "relations",
            ReferenceGroup::Calls => "calls",
            ReferenceGroup::KeywordArguments => "keyword_arguments",
            ReferenceGroup::Arrays => "arrays",
            ReferenceGroup::Binders => "binders",
            ReferenceGroup::Statements => "statements",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstructKind {
    Operator,
    Constant,
    Binder,
    TypedLiteral,
    Literal,
    KeywordArgument,
    Statement,
    Form,
}

impl ConstructKind {
    pub fn name(self) -> &'static str {
        match self {
            ConstructKind::Operator => "operator",
            ConstructKind::Constant => "constant",
            ConstructKind::Binder => "binder",
            ConstructKind::TypedLiteral => "typed_literal",
            ConstructKind::Literal => "literal",
            ConstructKind::KeywordArgument => "keyword_argument",
            ConstructKind::Statement => "statement",
            ConstructKind::Form => "form",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Construct {
    pub kind: ConstructKind,
    pub name: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellingMode {
    Ascii,
    Unicode,
}

impl SpellingMode {
    pub fn name(self) -> &'static str {
        match self {
            SpellingMode::Ascii => "ascii",
            SpellingMode::Unicode => "unicode",
        }
    }

    fn print_mode(self) -> PrintMode {
        match self {
            SpellingMode::Ascii => PrintMode::Ascii,
            SpellingMode::Unicode => PrintMode::Unicode,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spelling {
    pub symbol: &'static str,
    pub pattern: &'static str,
    pub mode: SpellingMode,
    pub example: &'static str,
    pub placeholders: Vec<Range<usize>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Associativity {
    Left,
    Right,
    None,
}

impl Associativity {
    pub fn name(self) -> &'static str {
        match self {
            Associativity::Left => "left",
            Associativity::Right => "right",
            Associativity::None => "none",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Precedence {
    pub level: u8,
    pub associativity: Associativity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeywordValue {
    Identifier,
    ExactNumber,
    Integer,
}

impl KeywordValue {
    pub fn name(self) -> &'static str {
        match self {
            KeywordValue::Identifier => "identifier",
            KeywordValue::ExactNumber => "exact_number",
            KeywordValue::Integer => "integer",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeywordArgument {
    pub name: &'static str,
    pub value: KeywordValue,
    pub values: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arguments {
    pub least: usize,
    pub largest: usize,
    pub keywords: &'static [KeywordArgument],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceEntry {
    pub group: ReferenceGroup,
    pub construct: Construct,
    pub spellings: Vec<Spelling>,
    pub canonical: Vec<(SpellingMode, String)>,
    pub precedence: Option<Precedence>,
    pub arguments: Option<Arguments>,
    pub is_computed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrecedenceRow {
    pub level: u8,
    pub construct: Construct,
    pub associativity: Associativity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitEntry {
    pub symbol: String,
    pub display_symbol: String,
    pub dimension: [i8; calc_units::BASE_DIMENSION_COUNT],
    pub scale_numerator: String,
    pub scale_denominator: String,
    pub scale_pi_exponent: i32,
    pub accepts_prefix: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrefixEntry {
    pub symbol: &'static str,
    pub exponent: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageReference {
    pub entries: Vec<ReferenceEntry>,
    pub precedence: Vec<PrecedenceRow>,
    pub units: Vec<UnitEntry>,
    pub prefixes: Vec<PrefixEntry>,
}

type SpellingForm = (SpellingMode, &'static str, &'static str, &'static str);

struct Forms {
    group: ReferenceGroup,
    spellings: Vec<SpellingForm>,
    precedence: Option<Precedence>,
    arguments: Option<Arguments>,
}

const SHAPE_KEYWORD_ARGUMENT: KeywordArgument = KeywordArgument {
    name: "shape",
    value: KeywordValue::Identifier,
    values: &["left", "halving"],
};

const SIDE_KEYWORD_ARGUMENT: KeywordArgument = KeywordArgument {
    name: "side",
    value: KeywordValue::Identifier,
    values: &["left", "right", "both"],
};

const PARTITION_KEYWORD_ARGUMENT: KeywordArgument = KeywordArgument {
    name: "partition",
    value: KeywordValue::Identifier,
    values: &["lomuto", "hoare"],
};

const PIVOT_KEYWORD_ARGUMENT: KeywordArgument = KeywordArgument {
    name: "pivot",
    value: KeywordValue::Identifier,
    values: &["last", "first", "random"],
};

const SEED_KEYWORD_ARGUMENT: KeywordArgument = KeywordArgument {
    name: "seed",
    value: KeywordValue::ExactNumber,
    values: &[],
};

const LIMIT_KEYWORD_ARGUMENT: KeywordArgument = KeywordArgument {
    name: "limit",
    value: KeywordValue::ExactNumber,
    values: &[],
};

const BASE_KEYWORD_ARGUMENT: KeywordArgument = KeywordArgument {
    name: "base",
    value: KeywordValue::ExactNumber,
    values: &[],
};

const GAPS_KEYWORD_ARGUMENT: KeywordArgument = KeywordArgument {
    name: "gaps",
    value: KeywordValue::Identifier,
    values: &["shell", "knuth", "ciura"],
};

const SHAKER_FORM_KEYWORD_ARGUMENT: KeywordArgument = KeywordArgument {
    name: "form",
    value: KeywordValue::Identifier,
    values: &["full", "shrinking", "last_exchange"],
};

const ODD_EVEN_FORM_KEYWORD_ARGUMENT: KeywordArgument = KeywordArgument {
    name: "form",
    value: KeywordValue::Identifier,
    values: &["until_sorted", "fixed_passes"],
};

const COMB_FORM_KEYWORD_ARGUMENT: KeywordArgument = KeywordArgument {
    name: "form",
    value: KeywordValue::Identifier,
    values: &["lacey_box"],
};

const FORM_KEYWORD_ARGUMENT: KeywordArgument = KeywordArgument {
    name: "form",
    value: KeywordValue::Identifier,
    values: &["full", "shrinking", "early_exit", "last_exchange"],
};

const ORDER_KEYWORD_ARGUMENT: KeywordArgument = KeywordArgument {
    name: "order",
    value: KeywordValue::Identifier,
    values: &["increasing", "decreasing"],
};

const COVERAGE_KEYWORD_ARGUMENT: KeywordArgument = KeywordArgument {
    name: "k",
    value: KeywordValue::ExactNumber,
    values: &[],
};

const MULTIPLICATIVE: u8 = 7;
const ADDITIVE: u8 = 8;
const POWER: u8 = 3;
const UNARY: u8 = 5;
const POSTFIX: u8 = 2;
const UNCERTAIN: u8 = 6;
const CONVERSION: u8 = 9;
const RELATION: u8 = 10;
const NOT_LEVEL: u8 = 11;
const AND_LEVEL: u8 = 12;
const OR_LEVEL: u8 = 13;
const LAMBDA_LEVEL: u8 = 14;
const APPLICATION: u8 = 4;
const PRIMARY: u8 = 1;

fn binary(spellings: &'static [SpellingForm], level: u8) -> Forms {
    Forms {
        group: if level == RELATION {
            ReferenceGroup::Relations
        } else {
            ReferenceGroup::Operators
        },
        spellings: spellings.to_vec(),
        precedence: Some(Precedence {
            level,
            associativity: if level == RELATION {
                Associativity::None
            } else {
                Associativity::Left
            },
        }),
        arguments: Some(Arguments {
            least: 2,
            largest: 2,
            keywords: &[],
        }),
    }
}

fn conversion_form(form: &'static str, example: &'static str) -> Forms {
    Forms {
        group: ReferenceGroup::Units,
        spellings: vec![(SpellingMode::Ascii, "->", form, example)],
        precedence: Some(Precedence {
            level: CONVERSION,
            associativity: Associativity::Left,
        }),
        arguments: None,
    }
}

fn chemistry_form(form: &'static str, example: &'static str) -> Forms {
    Forms {
        group: ReferenceGroup::Numbers,
        spellings: vec![(SpellingMode::Ascii, "chem'", form, example)],
        precedence: Some(Precedence {
            level: PRIMARY,
            associativity: Associativity::None,
        }),
        arguments: None,
    }
}

fn nuclear_form(form: &'static str, example: &'static str) -> Forms {
    Forms {
        group: ReferenceGroup::Numbers,
        spellings: vec![(SpellingMode::Ascii, "nuc'", form, example)],
        precedence: Some(Precedence {
            level: PRIMARY,
            associativity: Associativity::None,
        }),
        arguments: None,
    }
}

fn function(spellings: &'static [SpellingForm], arity: usize) -> Forms {
    Forms {
        group: ReferenceGroup::Calls,
        spellings: spellings.to_vec(),
        precedence: None,
        arguments: Some(Arguments {
            least: arity,
            largest: arity,
            keywords: &[],
        }),
    }
}

fn operator_forms(operator: Operator) -> Forms {
    match operator {
        Operator::Add => binary(&[(SpellingMode::Ascii, "+", "x + y", "x + y")], ADDITIVE),
        Operator::Sub => binary(
            &[
                (SpellingMode::Ascii, "-", "x - y", "x - y"),
                (
                    SpellingMode::Unicode,
                    "\u{2212}",
                    "x \u{2212} y",
                    "x \u{2212} y",
                ),
            ],
            ADDITIVE,
        ),
        Operator::Mul => binary(
            &[
                (SpellingMode::Ascii, "*", "x * y", "x * y"),
                (
                    SpellingMode::Unicode,
                    "\u{00B7}",
                    "x \u{00B7} y",
                    "x \u{00B7} y",
                ),
            ],
            MULTIPLICATIVE,
        ),
        Operator::Div => binary(
            &[(SpellingMode::Ascii, "/", "x / y", "x / y")],
            MULTIPLICATIVE,
        ),
        Operator::Neg => Forms {
            group: ReferenceGroup::Operators,
            spellings: vec![(SpellingMode::Ascii, "-", "-x", "-x")],
            precedence: Some(Precedence {
                level: UNARY,
                associativity: Associativity::None,
            }),
            arguments: Some(Arguments {
                least: 1,
                largest: 1,
                keywords: &[],
            }),
        },
        Operator::Pow => Forms {
            group: ReferenceGroup::Operators,
            spellings: vec![
                (SpellingMode::Ascii, "^", "x^y", "x^2"),
                (SpellingMode::Unicode, "\u{00B2}", "x\u{00B2}", "x\u{00B2}"),
            ],
            precedence: Some(Precedence {
                level: POWER,
                associativity: Associativity::Right,
            }),
            arguments: Some(Arguments {
                least: 2,
                largest: 2,
                keywords: &[],
            }),
        },
        Operator::Sqrt => Forms {
            group: ReferenceGroup::Calls,
            spellings: vec![
                (SpellingMode::Ascii, "sqrt", "sqrt(x)", "sqrt(x)"),
                (SpellingMode::Unicode, "\u{221A}", "\u{221A}x", "\u{221A}x"),
            ],
            precedence: Some(Precedence {
                level: UNARY,
                associativity: Associativity::None,
            }),
            arguments: Some(Arguments {
                least: 1,
                largest: 1,
                keywords: &[],
            }),
        },
        Operator::Abs => function(&[(SpellingMode::Ascii, "abs", "abs(x)", "abs(x)")], 1),
        Operator::MulAdd => function(
            &[(
                SpellingMode::Ascii,
                "mul_add",
                "mul_add(x, y, z)",
                "mul_add(x, y, z)",
            )],
            3,
        ),
        Operator::Floor => function(&[(SpellingMode::Ascii, "floor", "floor(x)", "floor(x)")], 1),
        Operator::Ceil => function(&[(SpellingMode::Ascii, "ceil", "ceil(x)", "ceil(x)")], 1),
        Operator::Trunc => function(&[(SpellingMode::Ascii, "trunc", "trunc(x)", "trunc(x)")], 1),
        Operator::RoundTiesEven => function(
            &[(
                SpellingMode::Ascii,
                "round_ties_even",
                "round_ties_even(x)",
                "round_ties_even(x)",
            )],
            1,
        ),
        Operator::CopySign => function(
            &[(
                SpellingMode::Ascii,
                "copysign",
                "copysign(x, y)",
                "copysign(x, y)",
            )],
            2,
        ),
        Operator::Round => function(
            &[(SpellingMode::Ascii, "round", "round(x)", "round(2.5)")],
            1,
        ),
        Operator::Binomial => function(
            &[(
                SpellingMode::Ascii,
                "binomial",
                "binomial(n, k)",
                "binomial(10, 3)",
            )],
            2,
        ),
        Operator::Sinh => function(&[(SpellingMode::Ascii, "sinh", "sinh(x)", "sinh(1)")], 1),
        Operator::Cosh => function(&[(SpellingMode::Ascii, "cosh", "cosh(x)", "cosh(1)")], 1),
        Operator::Tanh => function(&[(SpellingMode::Ascii, "tanh", "tanh(x)", "tanh(1)")], 1),
        Operator::Log => function(&[(SpellingMode::Ascii, "log", "log(x, b)", "log(8, 2)")], 2),
        Operator::Log2 => function(&[(SpellingMode::Ascii, "log2", "log2(x)", "log2(1024)")], 1),
        Operator::Log10 => function(
            &[(SpellingMode::Ascii, "log10", "log10(x)", "log10(1000)")],
            1,
        ),
        Operator::Gcd => function(
            &[(SpellingMode::Ascii, "gcd", "gcd(a, b)", "gcd(12, 18)")],
            2,
        ),
        Operator::Lcm => function(&[(SpellingMode::Ascii, "lcm", "lcm(a, b)", "lcm(4, 6)")], 2),
        Operator::Count => function(
            &[(SpellingMode::Ascii, "count", "count(a)", "count([1, 2, 3])")],
            1,
        ),
        Operator::Total => function(
            &[(SpellingMode::Ascii, "total", "total(a)", "total([1, 2, 3])")],
            1,
        ),
        Operator::Mean => function(
            &[(SpellingMode::Ascii, "mean", "mean(a)", "mean([1, 2, 3])")],
            1,
        ),
        Operator::Median => function(
            &[(
                SpellingMode::Ascii,
                "median",
                "median(a)",
                "median([3, 1, 2])",
            )],
            1,
        ),
        Operator::Transpose => function(
            &[(
                SpellingMode::Ascii,
                "transpose",
                "transpose(a)",
                "transpose([1, 2; 3, 4])",
            )],
            1,
        ),
        Operator::Determinant => function(
            &[(
                SpellingMode::Ascii,
                "determinant",
                "determinant(a)",
                "determinant([1, 2; 3, 4])",
            )],
            1,
        ),
        Operator::Trace => function(
            &[(
                SpellingMode::Ascii,
                "trace",
                "trace(a)",
                "trace([1, 2; 3, 4])",
            )],
            1,
        ),
        Operator::Inverse => function(
            &[(
                SpellingMode::Ascii,
                "inverse",
                "inverse(a)",
                "inverse([1, 2; 3, 4])",
            )],
            1,
        ),
        Operator::Rank => function(
            &[(SpellingMode::Ascii, "rank", "rank(a)", "rank([1, 2; 2, 4])")],
            1,
        ),
        Operator::Eigenvalues => function(
            &[(
                SpellingMode::Ascii,
                "eigenvalues",
                "eigenvalues(a)",
                "eigenvalues([2, 1; 1, 2])",
            )],
            1,
        ),
        Operator::Kernel => function(
            &[(
                SpellingMode::Ascii,
                "kernel",
                "kernel(a)",
                "kernel([1, 2; 2, 4])",
            )],
            1,
        ),
        Operator::Eigenvectors => function(
            &[(
                SpellingMode::Ascii,
                "eigenvectors",
                "eigenvectors(a)",
                "eigenvectors([2, 1; 1, 2])",
            )],
            1,
        ),
        Operator::At => function(
            &[(SpellingMode::Ascii, "at", "at(a, i)", "at([5, 6, 7], 2)")],
            2,
        ),
        Operator::Smallest => function(
            &[(
                SpellingMode::Ascii,
                "smallest",
                "smallest(a)",
                "smallest([3, 1, 2])",
            )],
            1,
        ),
        Operator::Largest => function(
            &[(
                SpellingMode::Ascii,
                "largest",
                "largest(a)",
                "largest([3, 1, 2])",
            )],
            1,
        ),
        Operator::Sorted => function(
            &[(
                SpellingMode::Ascii,
                "sorted",
                "sorted(a)",
                "sorted([3, 1, 2])",
            )],
            1,
        ),
        Operator::Mod => function(
            &[(SpellingMode::Ascii, "mod", "mod(a, b)", "mod(17, 5)")],
            2,
        ),
        Operator::Min => function(&[(SpellingMode::Ascii, "min", "min(x, y)", "min(x, y)")], 2),
        Operator::Max => function(&[(SpellingMode::Ascii, "max", "max(x, y)", "max(x, y)")], 2),
        Operator::Factorial => Forms {
            group: ReferenceGroup::Operators,
            spellings: vec![(SpellingMode::Ascii, "!", "x!", "n!")],
            precedence: Some(Precedence {
                level: POSTFIX,
                associativity: Associativity::None,
            }),
            arguments: Some(Arguments {
                least: 1,
                largest: 1,
                keywords: &[],
            }),
        },
        Operator::Percent => Forms {
            group: ReferenceGroup::Operators,
            spellings: vec![(SpellingMode::Ascii, "%", "x%", "n%")],
            precedence: Some(Precedence {
                level: POSTFIX,
                associativity: Associativity::None,
            }),
            arguments: Some(Arguments {
                least: 1,
                largest: 1,
                keywords: &[],
            }),
        },
        Operator::Exp => prefix_function("exp", "exp(x)", "exp x"),
        Operator::Ln => prefix_function("ln", "ln(x)", "ln x"),
        Operator::Sin => prefix_function("sin", "sin(x)", "sin x"),
        Operator::Cos => prefix_function("cos", "cos(x)", "cos x"),
        Operator::Tan => prefix_function("tan", "tan(x)", "tan x"),
        Operator::Asin => prefix_function("asin", "asin(x)", "asin x"),
        Operator::Acos => prefix_function("acos", "acos(x)", "acos x"),
        Operator::Atan => prefix_function("atan", "atan(x)", "atan x"),
        Operator::Atan2 => function(
            &[(SpellingMode::Ascii, "atan2", "atan2(y, x)", "atan2(y, x)")],
            2,
        ),
        Operator::Equal => Forms {
            group: ReferenceGroup::Relations,
            spellings: vec![
                (SpellingMode::Ascii, "=", "x = y", "x^2 = 2"),
                (SpellingMode::Ascii, "==", "x == y", "x^2 == 2"),
            ],
            precedence: Some(Precedence {
                level: RELATION,
                associativity: Associativity::None,
            }),
            arguments: Some(Arguments {
                least: 2,
                largest: 2,
                keywords: &[],
            }),
        },
        Operator::NotEqual => binary(
            &[
                (SpellingMode::Ascii, "!=", "x != y", "x != y"),
                (
                    SpellingMode::Unicode,
                    "\u{2260}",
                    "x \u{2260} y",
                    "x \u{2260} y",
                ),
            ],
            RELATION,
        ),
        Operator::Less => binary(&[(SpellingMode::Ascii, "<", "x < y", "x < y")], RELATION),
        Operator::LessOrEqual => binary(
            &[
                (SpellingMode::Ascii, "<=", "x <= y", "x <= y"),
                (
                    SpellingMode::Unicode,
                    "\u{2264}",
                    "x \u{2264} y",
                    "x \u{2264} y",
                ),
            ],
            RELATION,
        ),
        Operator::Greater => binary(&[(SpellingMode::Ascii, ">", "x > y", "x > y")], RELATION),
        Operator::GreaterOrEqual => binary(
            &[
                (SpellingMode::Ascii, ">=", "x >= y", "x >= y"),
                (
                    SpellingMode::Unicode,
                    "\u{2265}",
                    "x \u{2265} y",
                    "x \u{2265} y",
                ),
            ],
            RELATION,
        ),
        Operator::And => Forms {
            group: ReferenceGroup::Relations,
            spellings: vec![(SpellingMode::Ascii, "and", "p and q", "p and q")],
            precedence: Some(Precedence {
                level: AND_LEVEL,
                associativity: Associativity::Left,
            }),
            arguments: Some(Arguments {
                least: 2,
                largest: 2,
                keywords: &[],
            }),
        },
        Operator::Or => Forms {
            group: ReferenceGroup::Relations,
            spellings: vec![(SpellingMode::Ascii, "or", "p or q", "p or q")],
            precedence: Some(Precedence {
                level: OR_LEVEL,
                associativity: Associativity::Left,
            }),
            arguments: Some(Arguments {
                least: 2,
                largest: 2,
                keywords: &[],
            }),
        },
        Operator::Not => Forms {
            group: ReferenceGroup::Relations,
            spellings: vec![(SpellingMode::Ascii, "not", "not p", "not p")],
            precedence: Some(Precedence {
                level: NOT_LEVEL,
                associativity: Associativity::None,
            }),
            arguments: Some(Arguments {
                least: 1,
                largest: 1,
                keywords: &[],
            }),
        },
        Operator::Select => function(
            &[(
                SpellingMode::Ascii,
                "select",
                "select(p, x, y)",
                "select(p, x, y)",
            )],
            3,
        ),
        Operator::Complex => function(
            &[(
                SpellingMode::Ascii,
                "complex",
                "complex(x, y)",
                "complex(x, y)",
            )],
            2,
        ),
        Operator::ToF32 => function(
            &[(SpellingMode::Ascii, "to_f32", "to_f32(x)", "to_f32(x)")],
            1,
        ),
        Operator::ToF64 => function(
            &[(SpellingMode::Ascii, "to_f64", "to_f64(x)", "to_f64(x)")],
            1,
        ),
        Operator::ToExact => function(&[(SpellingMode::Ascii, "exact", "exact(x)", "exact(x)")], 1),
        Operator::EnclosureLower => function(
            &[(
                SpellingMode::Ascii,
                "enclosure_lower",
                "enclosure_lower(x, n)",
                "enclosure_lower(pi, 12)",
            )],
            2,
        ),
        Operator::EnclosureUpper => function(
            &[(
                SpellingMode::Ascii,
                "enclosure_upper",
                "enclosure_upper(x, n)",
                "enclosure_upper(pi, 12)",
            )],
            2,
        ),
        Operator::Uncertain => Forms {
            group: ReferenceGroup::Uncertainty,
            spellings: vec![
                (SpellingMode::Ascii, "+-", "x +- u", "2 +- 0.1"),
                (
                    SpellingMode::Unicode,
                    "\u{00B1}",
                    "x \u{00B1} u",
                    "2 \u{00B1} 0.1",
                ),
                (
                    SpellingMode::Ascii,
                    "uncertain",
                    "uncertain(x, u)",
                    "uncertain(2, 0.1)",
                ),
            ],
            precedence: Some(Precedence {
                level: UNCERTAIN,
                associativity: Associativity::None,
            }),
            arguments: Some(Arguments {
                least: 2,
                largest: 2,
                keywords: &[],
            }),
        },
        Operator::UncertainExpanded => Forms {
            group: ReferenceGroup::Uncertainty,
            spellings: vec![
                (SpellingMode::Ascii, "+-", "x +- u (k=c)", "2 +- 0.1 (k=2)"),
                (
                    SpellingMode::Unicode,
                    "\u{00B1}",
                    "x \u{00B1} u (k=c)",
                    "2 \u{00B1} 0.1 (k=2)",
                ),
                (
                    SpellingMode::Ascii,
                    "uncertain_expanded",
                    "uncertain_expanded(x, u, c)",
                    "uncertain_expanded(2, 0.1, 2)",
                ),
            ],
            precedence: Some(Precedence {
                level: UNCERTAIN,
                associativity: Associativity::None,
            }),
            arguments: Some(Arguments {
                least: 3,
                largest: 3,
                keywords: &[COVERAGE_KEYWORD_ARGUMENT],
            }),
        },
        Operator::ConvertUnit => Forms {
            group: ReferenceGroup::Units,
            spellings: vec![
                (SpellingMode::Ascii, "->", "x -> unit", "F -> N"),
                (
                    SpellingMode::Unicode,
                    "\u{2192}",
                    "x \u{2192} unit",
                    "F \u{2192} N",
                ),
                (
                    SpellingMode::Ascii,
                    "convert_unit",
                    "convert_unit(x, 1 unit)",
                    "convert_unit(F, 1 N)",
                ),
            ],
            precedence: Some(Precedence {
                level: CONVERSION,
                associativity: Associativity::Left,
            }),
            arguments: Some(Arguments {
                least: 2,
                largest: 2,
                keywords: &[],
            }),
        },
        Operator::BitAnd => function(
            &[(
                SpellingMode::Ascii,
                "bitand",
                "bitand(a, b)",
                "bitand(0b1100, 0b1010)",
            )],
            2,
        ),
        Operator::BitOr => function(
            &[(
                SpellingMode::Ascii,
                "bitor",
                "bitor(a, b)",
                "bitor(0b1100, 0b1010)",
            )],
            2,
        ),
        Operator::BitXor => function(
            &[(
                SpellingMode::Ascii,
                "bitxor",
                "bitxor(a, b)",
                "bitxor(0b1100, 0b1010)",
            )],
            2,
        ),
        Operator::ShiftLeft => {
            function(&[(SpellingMode::Ascii, "shl", "shl(x, n)", "shl(1, 4)")], 2)
        }
        Operator::ShiftRight => function(
            &[(SpellingMode::Ascii, "shr", "shr(x, n)", "shr(0xFF, 4)")],
            2,
        ),
        Operator::BitNot => function(
            &[(
                SpellingMode::Ascii,
                "bitnot",
                "bitnot(x, type)",
                "bitnot(0, u8)",
            )],
            2,
        ),
        Operator::Wrap => function(
            &[(
                SpellingMode::Ascii,
                "wrap",
                "wrap(x, type)",
                "wrap(200, i8)",
            )],
            2,
        ),
        Operator::InType => conversion_form("x -> type", "200 -> u8"),
        Operator::Between => function(
            &[(
                SpellingMode::Ascii,
                "between",
                "between(low, high)",
                "between(22.5 Ω, 27.5 Ω)",
            )],
            2,
        ),
        Operator::Tolerance => function(
            &[(
                SpellingMode::Ascii,
                "tolerance",
                "tolerance(x, p)",
                "tolerance(25 Ω, 10%)",
            )],
            2,
        ),
        Operator::Substance => chemistry_form("chem'formula'", "chem'CuSO4·5H2O'"),
        Operator::Reaction => chemistry_form(
            "chem'reactants -> products'",
            "chem'C3H8 + O2 -> CO2 + H2O'",
        ),
        Operator::MolarMass => function(
            &[(
                SpellingMode::Ascii,
                "molar_mass",
                "molar_mass(chem'formula')",
                "molar_mass(chem'H2O')",
            )],
            1,
        ),
        Operator::Nuclide => nuclear_form("nuc'nuclide'", "nuc'^14C'"),
        Operator::NuclearReaction => {
            nuclear_form("nuc'reactants -> products'", "nuc'^2H + ^3H -> ^4He + n'")
        }
        Operator::QValue => function(
            &[(
                SpellingMode::Ascii,
                "q_value",
                "q_value(nuc'reaction')",
                "q_value(nuc'^2H + ^3H -> ^4He + n')",
            )],
            1,
        ),
        Operator::Philox4x32_10 => function(
            &[(
                SpellingMode::Ascii,
                "philox4x32_10",
                "philox4x32_10(seed, stream, index)",
                "philox4x32_10(1, 2, 3)",
            )],
            3,
        ),
        Operator::RationalPart => function(
            &[(
                SpellingMode::Ascii,
                "rational_part",
                "rational_part(x)",
                "rational_part(17 + 12*sqrt(2))",
            )],
            1,
        ),
        Operator::CoefficientOf => function(
            &[(
                SpellingMode::Ascii,
                "coefficient_of",
                "coefficient_of(x, sqrt(n))",
                "coefficient_of(17 + 12*sqrt(2), sqrt(2))",
            )],
            2,
        ),
        Operator::RealPart => function(&[(SpellingMode::Ascii, "re", "re(z)", "re(3 + 4*i)")], 1),
        Operator::ImaginaryPart => {
            function(&[(SpellingMode::Ascii, "im", "im(z)", "im(3 + 4*i)")], 1)
        }
        Operator::Conjugate => function(
            &[(SpellingMode::Ascii, "conj", "conj(z)", "conj(3 + 4*i)")],
            1,
        ),
        Operator::Radix => conversion_form("x -> hex | bin | oct | dec [type]", "-1 -> hex i16"),
        Operator::Bytes => conversion_form("x -> bytes be | le [type]", "0xFFAF0100 -> bytes le"),
        Operator::FromCelsius => function(
            &[(
                SpellingMode::Ascii,
                "from_celsius",
                "from_celsius(t)",
                "from_celsius(20)",
            )],
            1,
        ),
        Operator::FromFahrenheit => function(
            &[(
                SpellingMode::Ascii,
                "from_fahrenheit",
                "from_fahrenheit(t)",
                "from_fahrenheit(68)",
            )],
            1,
        ),
        Operator::ToCelsius => function(
            &[(
                SpellingMode::Ascii,
                "to_celsius",
                "to_celsius(T)",
                "to_celsius(x)",
            )],
            1,
        ),
        Operator::ToFahrenheit => function(
            &[(
                SpellingMode::Ascii,
                "to_fahrenheit",
                "to_fahrenheit(T)",
                "to_fahrenheit(x)",
            )],
            1,
        ),
    }
}

fn prefix_function(
    symbol: &'static str,
    call_form: &'static str,
    prefix_form: &'static str,
) -> Forms {
    Forms {
        group: ReferenceGroup::Calls,
        spellings: vec![
            (SpellingMode::Ascii, symbol, call_form, call_form),
            (SpellingMode::Ascii, symbol, prefix_form, prefix_form),
        ],
        precedence: Some(Precedence {
            level: APPLICATION,
            associativity: Associativity::None,
        }),
        arguments: Some(Arguments {
            least: 1,
            largest: 1,
            keywords: &[],
        }),
    }
}

fn binder_forms(binder: BinderKind) -> (&'static str, Forms) {
    match binder {
        BinderKind::Lambda => (
            "lambda",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![
                    (SpellingMode::Ascii, "|->", "x |-> body", "x |-> x^2"),
                    (
                        SpellingMode::Unicode,
                        "\u{21A6}",
                        "x \u{21A6} body",
                        "x \u{21A6} x^2",
                    ),
                ],
                precedence: Some(Precedence {
                    level: LAMBDA_LEVEL,
                    associativity: Associativity::Right,
                }),
                arguments: Some(Arguments {
                    least: 1,
                    largest: 1,
                    keywords: &[],
                }),
            },
        ),
        BinderKind::Sum(ReductionShape::LeftFold) => ("sum", {
            let mut forms = reduction(
                "sum",
                "sum(i^2, i, 1, n)",
                "\u{03A3} i^2, i=1..n",
                "\u{03A3}",
            );
            forms.spellings.push((
                SpellingMode::Unicode,
                "\u{2211}",
                "\u{2211} body, i=a..b",
                "\u{2211} i^2, i=1..n",
            ));
            forms
        }),
        BinderKind::Sum(ReductionShape::Halving) => (
            "sum_halving",
            shaped("sum", "sum(i, i, 1, 8, shape=halving)"),
        ),
        BinderKind::Product(ReductionShape::LeftFold) => ("product", {
            let mut forms = reduction(
                "product",
                "product(k, k, 1, n)",
                "\u{220F} k, k=1..n",
                "\u{220F}",
            );
            forms.spellings.push((
                SpellingMode::Unicode,
                "\u{03A0}",
                "\u{03A0} body, k=a..b",
                "\u{03A0} k, k=1..n",
            ));
            forms
        }),
        BinderKind::Product(ReductionShape::Halving) => (
            "product_halving",
            shaped("product", "product(k, k, 1, 8, shape=halving)"),
        ),
        BinderKind::Integral => (
            "integral",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![
                    (
                        SpellingMode::Ascii,
                        "integral",
                        "integral(body, x, a, b)",
                        "integral(sin(x), x, 0, pi)",
                    ),
                    (
                        SpellingMode::Unicode,
                        "\u{222B}",
                        "\u{222B} body dx, a..b",
                        "\u{222B} sin x dx, 0..\u{03C0}",
                    ),
                ],
                precedence: None,
                arguments: Some(Arguments {
                    least: 2,
                    largest: 4,
                    keywords: &[],
                }),
            },
        ),
        BinderKind::Limit(LimitSide::Both) => (
            "limit",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![
                    (
                        SpellingMode::Ascii,
                        "limit",
                        "limit(body, x, a)",
                        "limit(sin(x)/x, x, 0)",
                    ),
                    (
                        SpellingMode::Unicode,
                        "lim",
                        "lim body, x\u{2192}a",
                        "lim sin(x)/x, x\u{2192}0",
                    ),
                ],
                precedence: None,
                arguments: Some(Arguments {
                    least: 3,
                    largest: 3,
                    keywords: &[SIDE_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Limit(LimitSide::Left) => ("limit_left", sided("limit(1/x, x, 0, side=left)")),
        BinderKind::Limit(LimitSide::Right) => {
            ("limit_right", sided("limit(1/x, x, 0, side=right)"))
        }
        BinderKind::Derivative => (
            "derivative",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![
                    (SpellingMode::Ascii, "diff", "diff(body, x)", "diff(x^2, x)"),
                    (SpellingMode::Ascii, "d", "d/dx body", "d/dx x^2"),
                ],
                precedence: None,
                arguments: Some(Arguments {
                    least: 2,
                    largest: 3,
                    keywords: &[],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Insertion,
            ..
        }) => (
            "insertion_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "insertion_sort",
                    "insertion_sort(a, x |-> key)",
                    "insertion_sort([3, 1, 2])",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::BinaryInsertion,
            ..
        }) => (
            "binary_insertion_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "binary_insertion_sort",
                    "binary_insertion_sort(a, x |-> key)",
                    "binary_insertion_sort([3, 1, 2])",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Selection,
            ..
        }) => (
            "selection_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "selection_sort",
                    "selection_sort(a, x |-> key)",
                    "selection_sort([3, 1, 2])",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Bubble(_),
            ..
        }) => (
            "bubble_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "bubble_sort",
                    "bubble_sort(a, x |-> key, form=f)",
                    "bubble_sort([3, 1, 2], form=early_exit)",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[FORM_KEYWORD_ARGUMENT, ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Merge,
            ..
        }) => (
            "merge_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "merge_sort",
                    "merge_sort(a, x |-> key)",
                    "merge_sort([3, 1, 2])",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Heap,
            ..
        }) => (
            "heap_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "heap_sort",
                    "heap_sort(a, x |-> key)",
                    "heap_sort([3, 1, 2])",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Quick(_),
            ..
        }) => (
            "quick_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "quick_sort",
                    "quick_sort(a, x |-> key, partition=p, pivot=q)",
                    "quick_sort([3, 1, 2], partition=lomuto, pivot=last)",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[
                        PARTITION_KEYWORD_ARGUMENT,
                        PIVOT_KEYWORD_ARGUMENT,
                        SEED_KEYWORD_ARGUMENT,
                        ORDER_KEYWORD_ARGUMENT,
                    ],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Counting,
            ..
        }) => (
            "counting_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "counting_sort",
                    "counting_sort(a, x |-> key)",
                    "counting_sort([3, 1, 2, 1])",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::DoubleSelection,
            ..
        }) => (
            "double_selection_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "double_selection_sort",
                    "double_selection_sort(a, x |-> key)",
                    "double_selection_sort([3, 1, 2])",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::CocktailShaker(_),
            ..
        }) => (
            "cocktail_shaker_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "cocktail_shaker_sort",
                    "cocktail_shaker_sort(a, x |-> key, form=f)",
                    "cocktail_shaker_sort([3, 1, 2], form=full)",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[SHAKER_FORM_KEYWORD_ARGUMENT, ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Gnome,
            ..
        }) => (
            "gnome_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "gnome_sort",
                    "gnome_sort(a, x |-> key)",
                    "gnome_sort([3, 1, 2])",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::OddEven(_),
            ..
        }) => (
            "odd_even_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "odd_even_sort",
                    "odd_even_sort(a, x |-> key, form=f)",
                    "odd_even_sort([3, 1, 2], form=until_sorted)",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[ODD_EVEN_FORM_KEYWORD_ARGUMENT, ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Comb(_),
            ..
        }) => (
            "comb_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "comb_sort",
                    "comb_sort(a, x |-> key, form=f)",
                    "comb_sort([3, 1, 2], form=lacey_box)",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[COMB_FORM_KEYWORD_ARGUMENT, ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Cycle,
            ..
        }) => (
            "cycle_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "cycle_sort",
                    "cycle_sort(a, x |-> key)",
                    "cycle_sort([3, 1, 2])",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Pancake,
            ..
        }) => (
            "pancake_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "pancake_sort",
                    "pancake_sort(a, x |-> key)",
                    "pancake_sort([3, 1, 2])",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Shell(_),
            ..
        }) => (
            "shell_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "shell_sort",
                    "shell_sort(a, x |-> key, gaps=g)",
                    "shell_sort([3, 1, 2], gaps=knuth)",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[GAPS_KEYWORD_ARGUMENT, ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::BottomUpMerge,
            ..
        }) => (
            "bottom_up_merge_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "bottom_up_merge_sort",
                    "bottom_up_merge_sort(a, x |-> key)",
                    "bottom_up_merge_sort([3, 1, 2])",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::NaturalMerge,
            ..
        }) => (
            "natural_merge_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "natural_merge_sort",
                    "natural_merge_sort(a, x |-> key)",
                    "natural_merge_sort([3, 1, 2])",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Bogo,
            ..
        }) => (
            "bogo_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "bogo_sort",
                    "bogo_sort(a, x |-> key, seed=s, limit=m)",
                    "bogo_sort([3, 1, 2], seed=7, limit=100)",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[
                        SEED_KEYWORD_ARGUMENT,
                        LIMIT_KEYWORD_ARGUMENT,
                        ORDER_KEYWORD_ARGUMENT,
                    ],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Bitonic,
            ..
        }) => (
            "bitonic_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "bitonic_sort",
                    "bitonic_sort(a, x |-> key)",
                    "bitonic_sort([5, 3, 8, 1])",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Bead,
            ..
        }) => (
            "bead_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "bead_sort",
                    "bead_sort(a)",
                    "bead_sort([3, 1, 4, 1, 5])",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 1,
                    keywords: &[ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Radix,
            ..
        }) => (
            "radix_sort",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "radix_sort",
                    "radix_sort(a, x |-> key, base=b)",
                    "radix_sort([170, 45, 75, 90, 802, 24, 2, 66], base=10)",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 1,
                    largest: 2,
                    keywords: &[BASE_KEYWORD_ARGUMENT, ORDER_KEYWORD_ARGUMENT],
                }),
            },
        ),
        BinderKind::Taylor => (
            "taylor",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "taylor",
                    "taylor(body, x, a, n)",
                    "taylor(sin(x), x, 0, 5)",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 4,
                    largest: 4,
                    keywords: &[],
                }),
            },
        ),
        BinderKind::Root => (
            "root",
            Forms {
                group: ReferenceGroup::Binders,
                spellings: vec![(
                    SpellingMode::Ascii,
                    "rootof",
                    "rootof(body, x, k)",
                    "rootof(x^3 - 2, x, 1)",
                )],
                precedence: None,
                arguments: Some(Arguments {
                    least: 3,
                    largest: 3,
                    keywords: &[],
                }),
            },
        ),
    }
}

fn reduction(
    name: &'static str,
    call_form: &'static str,
    letter_form: &'static str,
    letter: &'static str,
) -> Forms {
    Forms {
        group: ReferenceGroup::Binders,
        spellings: vec![
            (SpellingMode::Ascii, name, call_form, call_form),
            (SpellingMode::Unicode, letter, letter_form, letter_form),
        ],
        precedence: None,
        arguments: Some(Arguments {
            least: 4,
            largest: 4,
            keywords: &[SHAPE_KEYWORD_ARGUMENT],
        }),
    }
}

fn shaped(name: &'static str, example: &'static str) -> Forms {
    Forms {
        group: ReferenceGroup::Binders,
        spellings: vec![(SpellingMode::Ascii, name, example, example)],
        precedence: None,
        arguments: Some(Arguments {
            least: 4,
            largest: 4,
            keywords: &[SHAPE_KEYWORD_ARGUMENT],
        }),
    }
}

fn sided(example: &'static str) -> Forms {
    Forms {
        group: ReferenceGroup::Binders,
        spellings: vec![(SpellingMode::Ascii, "limit", example, example)],
        precedence: None,
        arguments: Some(Arguments {
            least: 3,
            largest: 3,
            keywords: &[SIDE_KEYWORD_ARGUMENT],
        }),
    }
}

fn constant_forms(constant: BuiltinConstant) -> (&'static str, Forms) {
    let spellings: Vec<SpellingForm> = match constant {
        BuiltinConstant::Pi => vec![
            (SpellingMode::Ascii, "pi", "pi", "pi"),
            (SpellingMode::Unicode, "\u{03C0}", "\u{03C0}", "\u{03C0}"),
        ],
        BuiltinConstant::E => vec![
            (SpellingMode::Ascii, "e", "e", "e"),
            (SpellingMode::Unicode, "\u{212F}", "\u{212F}", "\u{212F}"),
        ],
        BuiltinConstant::ImaginaryUnit => vec![
            (SpellingMode::Ascii, "i", "i", "i"),
            (SpellingMode::Unicode, "\u{2148}", "\u{2148}", "\u{2148}"),
        ],
        BuiltinConstant::Infinity => vec![
            (SpellingMode::Ascii, "inf", "inf", "inf"),
            (SpellingMode::Unicode, "\u{221E}", "\u{221E}", "\u{221E}"),
        ],
    };
    let name = match constant {
        BuiltinConstant::Pi => "pi",
        BuiltinConstant::E => "e",
        BuiltinConstant::ImaginaryUnit => "imaginary_unit",
        BuiltinConstant::Infinity => "infinity",
    };
    (
        name,
        Forms {
            group: ReferenceGroup::Numbers,
            spellings,
            precedence: Some(Precedence {
                level: PRIMARY,
                associativity: Associativity::None,
            }),
            arguments: None,
        },
    )
}

fn typed_literal_forms(kind: TypedLiteralKind) -> (&'static str, Forms) {
    let (name, example) = match kind {
        TypedLiteralKind::Rational => ("rational", "q'1/3'"),
        TypedLiteralKind::F32 => ("f32", "f32'1.5'"),
        TypedLiteralKind::F64 => ("f64", "f64'0.1'"),
        TypedLiteralKind::Chemistry => ("chemistry", "chem'H2O'"),
        TypedLiteralKind::Nuclear => ("nuclear", "nuc'^14C'"),
    };
    (
        name,
        Forms {
            group: ReferenceGroup::Numbers,
            spellings: vec![(SpellingMode::Ascii, example, example, example)],
            precedence: Some(Precedence {
                level: PRIMARY,
                associativity: Associativity::None,
            }),
            arguments: None,
        },
    )
}

fn form_entries() -> Vec<(ConstructKind, &'static str, Forms)> {
    vec![
        (
            ConstructKind::Literal,
            "decimal",
            simple(
                ReferenceGroup::Numbers,
                &[(SpellingMode::Ascii, ".", "a.b", "0.1")],
            ),
        ),
        (
            ConstructKind::Literal,
            "exponent",
            simple(
                ReferenceGroup::Numbers,
                &[(SpellingMode::Ascii, "e", "aeb", "1.5e-3")],
            ),
        ),
        (
            ConstructKind::Literal,
            "line_label",
            simple(
                ReferenceGroup::Numbers,
                &[(SpellingMode::Ascii, "r", "rn", "r3")],
            ),
        ),
        (
            ConstructKind::Form,
            "quantity",
            simple(
                ReferenceGroup::Units,
                &[(SpellingMode::Ascii, "kg", "x unit", "2.50 kg")],
            ),
        ),
        (
            ConstructKind::Form,
            "unit_expression",
            simple(
                ReferenceGroup::Units,
                &[(SpellingMode::Ascii, "m/s^2", "x unit/unit^n", "9.81 m/s^2")],
            ),
        ),
        (
            ConstructKind::Form,
            "degree_sign",
            simple(
                ReferenceGroup::Units,
                &[
                    (SpellingMode::Ascii, "deg", "x deg", "30 deg"),
                    (SpellingMode::Unicode, "\u{00B0}", "x\u{00B0}", "30\u{00B0}"),
                ],
            ),
        ),
        (
            ConstructKind::Form,
            "array",
            simple(
                ReferenceGroup::Arrays,
                &[(SpellingMode::Ascii, "[", "[a, b]", "[1, 2, 3]")],
            ),
        ),
        (
            ConstructKind::Form,
            "matrix",
            simple(
                ReferenceGroup::Arrays,
                &[(SpellingMode::Ascii, ";", "[a, b; c, d]", "[1, 2; 3, 4]")],
            ),
        ),
        (
            ConstructKind::KeywordArgument,
            "shape",
            simple(
                ReferenceGroup::KeywordArguments,
                &[(
                    SpellingMode::Ascii,
                    "shape",
                    "shape=value",
                    "sum(i, i, 1, 8, shape=halving)",
                )],
            ),
        ),
        (
            ConstructKind::KeywordArgument,
            "side",
            simple(
                ReferenceGroup::KeywordArguments,
                &[(
                    SpellingMode::Ascii,
                    "side",
                    "side=value",
                    "limit(1/x, x, 0, side=right)",
                )],
            ),
        ),
        (
            ConstructKind::KeywordArgument,
            "coverage",
            simple(
                ReferenceGroup::KeywordArguments,
                &[(SpellingMode::Ascii, "k", "k=value", "2 \u{00B1} 0.1 (k=2)")],
            ),
        ),
        (
            ConstructKind::Form,
            "comment",
            simple(
                ReferenceGroup::Statements,
                &[(SpellingMode::Ascii, "#", "value # text", "2 # two")],
            ),
        ),
        (
            ConstructKind::Statement,
            "naming",
            simple(
                ReferenceGroup::Statements,
                &[(SpellingMode::Ascii, "=", "name = value", "x = 2")],
            ),
        ),
        (
            ConstructKind::Statement,
            "function_naming",
            simple(
                ReferenceGroup::Statements,
                &[(SpellingMode::Ascii, "=", "name(x) = value", "f(x) = x^2")],
            ),
        ),
    ]
}

fn simple(group: ReferenceGroup, spellings: &'static [SpellingForm]) -> Forms {
    Forms {
        group,
        spellings: spellings.to_vec(),
        precedence: None,
        arguments: None,
    }
}

fn placeholders(example: &str) -> Vec<Range<usize>> {
    let Ok(tokens) = tokenize(example) else {
        return Vec::new();
    };
    tokens
        .iter()
        .filter_map(|token| match &token.kind {
            TokenKind::Identifier(name)
                if !is_reserved(name) && operator_for_call(name).is_none() =>
            {
                Some(token.span.clone())
            }
            _ => None,
        })
        .collect()
}

fn spellings_of(forms: &Forms) -> Vec<Spelling> {
    forms
        .spellings
        .iter()
        .map(|(mode, symbol, pattern, example)| Spelling {
            symbol,
            pattern,
            mode: *mode,
            example,
            placeholders: placeholders(example),
        })
        .collect()
}

fn canonical_of(group: ReferenceGroup, spellings: &[Spelling]) -> Vec<(SpellingMode, String)> {
    if group == ReferenceGroup::Statements {
        return Vec::new();
    }
    let Some(spelling) = spellings.first() else {
        return Vec::new();
    };
    let mut pool = ExprPool::new();
    let Ok(expression) = crate::parse_expression(&mut pool, spelling.example) else {
        return Vec::new();
    };
    [SpellingMode::Ascii, SpellingMode::Unicode]
        .into_iter()
        .filter_map(|mode| {
            crate::print_expression(&pool, expression, mode.print_mode())
                .ok()
                .map(|text| (mode, text))
        })
        .collect()
}

fn entry(group: ReferenceGroup, construct: Construct, forms: &Forms) -> ReferenceEntry {
    let spellings = spellings_of(forms);
    ReferenceEntry {
        group,
        construct,
        canonical: canonical_of(group, &spellings),
        spellings,
        precedence: forms.precedence,
        arguments: forms.arguments,
        is_computed: is_computed(construct),
    }
}

const NOT_COMPUTED: [&str; 0] = [];

fn is_computed(construct: Construct) -> bool {
    !NOT_COMPUTED.contains(&construct.name)
}

pub fn operator_construct(operator: Operator) -> Construct {
    Construct {
        kind: ConstructKind::Operator,
        name: operator_name(operator),
    }
}

fn operator_name(operator: Operator) -> &'static str {
    match operator {
        Operator::Add => "add",
        Operator::Sub => "sub",
        Operator::Mul => "mul",
        Operator::Div => "div",
        Operator::Neg => "neg",
        Operator::Pow => "pow",
        Operator::Sqrt => "sqrt",
        Operator::Abs => "abs",
        Operator::MulAdd => "mul_add",
        Operator::Floor => "floor",
        Operator::Ceil => "ceil",
        Operator::Trunc => "trunc",
        Operator::RoundTiesEven => "round_ties_even",
        Operator::CopySign => "copysign",
        Operator::Round => "round",
        Operator::Binomial => "binomial",
        Operator::Sinh => "sinh",
        Operator::Cosh => "cosh",
        Operator::Tanh => "tanh",
        Operator::Log => "log",
        Operator::Log2 => "log2",
        Operator::Log10 => "log10",
        Operator::Gcd => "gcd",
        Operator::Count => "count",
        Operator::Total => "total",
        Operator::Mean => "mean",
        Operator::Median => "median",
        Operator::Transpose => "transpose",
        Operator::Determinant => "determinant",
        Operator::Trace => "trace",
        Operator::Inverse => "inverse",
        Operator::Rank => "rank",
        Operator::Eigenvalues => "eigenvalues",
        Operator::Kernel => "kernel",
        Operator::Eigenvectors => "eigenvectors",
        Operator::At => "at",
        Operator::Smallest => "smallest",
        Operator::Largest => "largest",
        Operator::Sorted => "sorted",
        Operator::Lcm => "lcm",
        Operator::Mod => "mod",
        Operator::Min => "min",
        Operator::Max => "max",
        Operator::Factorial => "factorial",
        Operator::Percent => "percent",
        Operator::Exp => "exp",
        Operator::Ln => "ln",
        Operator::Sin => "sin",
        Operator::Cos => "cos",
        Operator::Tan => "tan",
        Operator::Asin => "asin",
        Operator::Acos => "acos",
        Operator::Atan => "atan",
        Operator::Atan2 => "atan2",
        Operator::Equal => "equal",
        Operator::NotEqual => "not_equal",
        Operator::Less => "less",
        Operator::LessOrEqual => "less_or_equal",
        Operator::Greater => "greater",
        Operator::GreaterOrEqual => "greater_or_equal",
        Operator::And => "and",
        Operator::Or => "or",
        Operator::Not => "not",
        Operator::Select => "select",
        Operator::Complex => "complex",
        Operator::ToF32 => "to_f32",
        Operator::ToF64 => "to_f64",
        Operator::ToExact => "exact",
        Operator::EnclosureLower => "enclosure_lower",
        Operator::EnclosureUpper => "enclosure_upper",
        Operator::Uncertain => "uncertain",
        Operator::UncertainExpanded => "uncertain_expanded",
        Operator::ConvertUnit => "convert_unit",
        Operator::FromCelsius => "from_celsius",
        Operator::FromFahrenheit => "from_fahrenheit",
        Operator::ToCelsius => "to_celsius",
        Operator::ToFahrenheit => "to_fahrenheit",
        Operator::BitAnd => "bitand",
        Operator::BitOr => "bitor",
        Operator::BitXor => "bitxor",
        Operator::ShiftLeft => "shl",
        Operator::ShiftRight => "shr",
        Operator::BitNot => "bitnot",
        Operator::Wrap => "wrap",
        Operator::InType => "in_type",
        Operator::Radix => "radix",
        Operator::Bytes => "bytes",
        Operator::Between => "between",
        Operator::Tolerance => "tolerance",
        Operator::Substance => "substance",
        Operator::Reaction => "reaction",
        Operator::MolarMass => "molar_mass",
        Operator::Nuclide => "nuclide",
        Operator::NuclearReaction => "nuclear_reaction",
        Operator::QValue => "q_value",
        Operator::Philox4x32_10 => "philox4x32_10",
        Operator::RationalPart => "rational_part",
        Operator::CoefficientOf => "coefficient_of",
        Operator::RealPart => "re",
        Operator::ImaginaryPart => "im",
        Operator::Conjugate => "conj",
    }
}

fn precedence_rows(entries: &[ReferenceEntry]) -> Vec<PrecedenceRow> {
    let mut rows: Vec<PrecedenceRow> = entries
        .iter()
        .filter_map(|entry| {
            entry.precedence.map(|precedence| PrecedenceRow {
                level: precedence.level,
                construct: entry.construct,
                associativity: precedence.associativity,
            })
        })
        .collect();
    rows.sort_by_key(|row| (row.level, row.construct.name));
    rows
}

fn unit_entries() -> Vec<UnitEntry> {
    let mut table = UnitTable::new();
    let mut entries = Vec::new();
    for named_unit in table.named_units() {
        let Ok(symbol) = table.symbol(named_unit).map(str::to_string) else {
            continue;
        };
        let Ok(display_symbol) = table.display_symbol(named_unit).map(str::to_string) else {
            continue;
        };
        let Ok(accepts_prefix) = table.accepts_prefix(named_unit) else {
            continue;
        };
        let Ok(unit) = table.lookup(&symbol) else {
            continue;
        };
        let Ok(dimension) = table.dimension(unit) else {
            continue;
        };
        let Ok(scale_factor) = table.scale_factor(unit) else {
            continue;
        };
        entries.push(UnitEntry {
            symbol,
            display_symbol,
            dimension: dimension.exponents(),
            scale_numerator: integer_to_digits(scale_factor.numerator()),
            scale_denominator: integer_to_digits(scale_factor.denominator()),
            scale_pi_exponent: scale_factor.pi_exponent(),
            accepts_prefix,
        });
    }
    entries.sort_by(|left, right| {
        left.dimension
            .cmp(&right.dimension)
            .then_with(|| left.symbol.cmp(&right.symbol))
    });
    entries
}

pub fn language_reference() -> LanguageReference {
    let mut entries = Vec::new();
    for constant in BuiltinConstant::ALL {
        let (name, forms) = constant_forms(constant);
        entries.push(entry(
            forms.group,
            Construct {
                kind: ConstructKind::Constant,
                name,
            },
            &forms,
        ));
    }
    for kind in [
        TypedLiteralKind::Rational,
        TypedLiteralKind::F32,
        TypedLiteralKind::F64,
    ] {
        let (name, forms) = typed_literal_forms(kind);
        entries.push(entry(
            forms.group,
            Construct {
                kind: ConstructKind::TypedLiteral,
                name,
            },
            &forms,
        ));
    }
    for operator in Operator::ALL {
        let forms = operator_forms(operator);
        entries.push(entry(forms.group, operator_construct(operator), &forms));
    }
    for binder in [
        BinderKind::Lambda,
        BinderKind::Sum(ReductionShape::LeftFold),
        BinderKind::Sum(ReductionShape::Halving),
        BinderKind::Product(ReductionShape::LeftFold),
        BinderKind::Product(ReductionShape::Halving),
        BinderKind::Integral,
        BinderKind::Limit(LimitSide::Both),
        BinderKind::Limit(LimitSide::Left),
        BinderKind::Limit(LimitSide::Right),
        BinderKind::Derivative,
        BinderKind::Root,
        BinderKind::Taylor,
        BinderKind::Sort(SortSpec {
            method: SortMethod::Insertion,
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::BinaryInsertion,
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Selection,
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Bubble(SortBubbleForm::EarlyExit),
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Merge,
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Heap,
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Quick(SortPartition::LomutoLast),
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Counting,
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::DoubleSelection,
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::CocktailShaker(SortShakerForm::Full),
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Gnome,
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::OddEven(SortOddEvenForm::UntilSorted),
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Comb(SortCombForm::LaceyBox),
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Cycle,
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Pancake,
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Shell(SortGaps::Knuth),
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::BottomUpMerge,
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::NaturalMerge,
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Radix,
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Bead,
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Bitonic,
            order: SortOrder::Increasing,
        }),
        BinderKind::Sort(SortSpec {
            method: SortMethod::Bogo,
            order: SortOrder::Increasing,
        }),
    ] {
        let (name, forms) = binder_forms(binder);
        entries.push(entry(
            forms.group,
            Construct {
                kind: ConstructKind::Binder,
                name,
            },
            &forms,
        ));
    }
    for (kind, name, forms) in form_entries() {
        entries.push(entry(forms.group, Construct { kind, name }, &forms));
    }
    entries.sort_by_key(|entry| {
        ReferenceGroup::ALL
            .iter()
            .position(|group| *group == entry.group)
            .unwrap_or(usize::MAX)
    });
    let precedence = precedence_rows(&entries);
    LanguageReference {
        precedence,
        units: unit_entries(),
        prefixes: UnitTable::decimal_prefixes()
            .into_iter()
            .map(|(symbol, exponent)| PrefixEntry { symbol, exponent })
            .collect(),
        entries,
    }
}

#[cfg(test)]
mod tests {
    use calc_expr::ExprId;

    use crate::lexer::{SINGLE_CHARACTER_TOKENS, TWO_CHARACTER_TOKENS};
    use crate::names::{BINDER_NAMES, CALL_NAMES, RESERVED_WORDS};
    use calc_expr::without_measurement_marks;

    use crate::{parse_expression, parse_statement};

    use super::*;

    fn reference() -> LanguageReference {
        language_reference()
    }

    fn expression_of(pool: &mut ExprPool, spelling: &Spelling) -> ExprId {
        let parsed = parse_expression(pool, spelling.example)
            .unwrap_or_else(|error| panic!("{} does not parse: {error:?}", spelling.example));
        without_measurement_marks(pool, parsed).unwrap()
    }

    fn all_texts(reference: &LanguageReference) -> Vec<String> {
        reference
            .entries
            .iter()
            .flat_map(|entry| {
                entry.spellings.iter().flat_map(|spelling| {
                    [spelling.symbol.to_string(), spelling.example.to_string()]
                })
            })
            .collect()
    }

    #[test]
    fn every_example_parses() {
        for entry in reference().entries {
            for spelling in &entry.spellings {
                let parsed = parse_statement(&mut ExprPool::new(), spelling.example);
                assert!(parsed.is_ok(), "{}: {parsed:?}", spelling.example);
            }
        }
    }

    #[test]
    fn every_spelling_of_an_entry_gives_one_expression() {
        for entry in reference().entries {
            if entry.group == ReferenceGroup::Statements {
                continue;
            }
            let mut pool = ExprPool::new();
            let mut spellings = entry.spellings.iter();
            let Some(first) = spellings.next() else {
                panic!("{} has no spelling", entry.construct.name);
            };
            let expected = expression_of(&mut pool, first);
            for spelling in spellings {
                assert_eq!(
                    expression_of(&mut pool, spelling),
                    expected,
                    "{} and {}",
                    first.example,
                    spelling.example
                );
            }
        }
    }

    #[test]
    fn every_canonical_form_parses_back_to_its_entry() {
        for entry in reference().entries {
            if entry.group == ReferenceGroup::Statements {
                continue;
            }
            let mut pool = ExprPool::new();
            let expected = expression_of(&mut pool, &entry.spellings[0]);
            for (mode, text) in &entry.canonical {
                let parsed = parse_expression(&mut pool, text)
                    .unwrap_or_else(|error| panic!("{text} does not parse: {error:?}"));
                let parsed = without_measurement_marks(&mut pool, parsed).unwrap();

                assert_eq!(parsed, expected, "{} in {}", text, mode.name());
            }
        }
    }

    #[test]
    fn every_entry_has_a_name_and_a_spelling() {
        for entry in reference().entries {
            assert!(!entry.construct.name.is_empty(), "{:?}", entry.construct);
            assert!(!entry.spellings.is_empty(), "{}", entry.construct.name);
            for spelling in &entry.spellings {
                assert!(
                    spelling.example.contains(spelling.symbol),
                    "{} misses {}",
                    spelling.example,
                    spelling.symbol
                );
            }
        }
    }

    #[test]
    fn every_operator_has_its_own_entry() {
        let reference = reference();
        for operator in Operator::ALL {
            let name = operator_name(operator);
            assert!(
                reference.entries.iter().any(|entry| entry.construct
                    == Construct {
                        kind: ConstructKind::Operator,
                        name,
                    }),
                "{name}"
            );
        }
    }

    #[test]
    fn every_token_character_occurs_in_a_spelling() {
        let texts = all_texts(&reference());
        for (character, _) in SINGLE_CHARACTER_TOKENS {
            assert!(
                texts.iter().any(|text| text.contains(character)),
                "{character}"
            );
        }
        for (text, _) in TWO_CHARACTER_TOKENS {
            assert!(texts.iter().any(|written| written.contains(text)), "{text}");
        }
    }

    #[test]
    fn every_call_name_and_reserved_word_occurs_in_a_spelling() {
        let texts = all_texts(&reference());
        for (name, _) in CALL_NAMES {
            assert!(texts.iter().any(|text| text.contains(name)), "{name}");
        }
        for name in RESERVED_WORDS.iter().chain(BINDER_NAMES.iter()) {
            assert!(texts.iter().any(|text| text.contains(name)), "{name}");
        }
    }

    #[test]
    fn placeholders_name_the_operands_of_an_example() {
        let reference = reference();
        let entry = reference
            .entries
            .iter()
            .find(|entry| entry.construct.name == "mul")
            .unwrap();
        let spelling = &entry.spellings[0];
        let operands: Vec<&str> = spelling
            .placeholders
            .iter()
            .map(|range| &spelling.example[range.clone()])
            .collect();
        assert_eq!(operands, ["x", "y"]);
    }

    #[test]
    fn the_precedence_table_orders_the_constructs_by_level() {
        let reference = reference();
        let levels: Vec<u8> = reference.precedence.iter().map(|row| row.level).collect();
        assert!(levels.windows(2).all(|pair| pair[0] <= pair[1]));
        let multiplication = reference
            .precedence
            .iter()
            .find(|row| row.construct.name == "mul")
            .unwrap();
        let addition = reference
            .precedence
            .iter()
            .find(|row| row.construct.name == "add")
            .unwrap();
        assert!(multiplication.level < addition.level);
        assert_eq!(multiplication.associativity, Associativity::Left);
    }

    #[test]
    fn the_unit_table_holds_every_unit_with_its_scale_factor() {
        let reference = reference();
        let metre = reference
            .units
            .iter()
            .find(|unit| unit.symbol == "m")
            .unwrap();
        assert_eq!(
            (
                metre.dimension,
                metre.scale_numerator.as_str(),
                metre.scale_denominator.as_str(),
                metre.scale_pi_exponent,
                metre.accepts_prefix,
            ),
            ([1, 0, 0, 0, 0, 0, 0, 0], "1", "1", 0, true)
        );
        let degree = reference
            .units
            .iter()
            .find(|unit| unit.symbol == "deg")
            .unwrap();
        assert_eq!(
            (
                degree.display_symbol.as_str(),
                degree.scale_denominator.as_str(),
                degree.scale_pi_exponent,
            ),
            ("\u{00B0}", "180", 1)
        );
    }

    #[test]
    fn the_prefix_table_holds_the_decimal_prefixes() {
        let reference = reference();
        assert!(
            reference
                .prefixes
                .iter()
                .any(|prefix| prefix.symbol == "k" && prefix.exponent == 3)
        );
        assert_eq!(reference.prefixes.len(), 24);
    }

    #[test]
    fn entries_are_ordered_by_group() {
        let reference = reference();
        let positions: Vec<usize> = reference
            .entries
            .iter()
            .map(|entry| {
                ReferenceGroup::ALL
                    .iter()
                    .position(|group| *group == entry.group)
                    .unwrap()
            })
            .collect();
        assert!(positions.windows(2).all(|pair| pair[0] <= pair[1]));
    }
}
