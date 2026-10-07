const UNARY: usize = 1;
const BINARY: usize = 2;
const TERNARY: usize = 3;
const QUATERNARY: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Operator {
    Add,
    Sub,
    Mul,
    Div,
    Neg,
    Pow,
    Sqrt,
    Abs,
    MulAdd,
    Floor,
    Ceil,
    Trunc,
    RoundTiesEven,
    Round,
    Percent,
    CopySign,
    Min,
    Max,
    Factorial,
    Binomial,
    Gcd,
    Lcm,
    Mod,
    Count,
    Total,
    Mean,
    Median,
    Transpose,
    Determinant,
    Trace,
    Inverse,
    Rank,
    Eigenvalues,
    Kernel,
    Eigenvectors,
    At,
    Smallest,
    Largest,
    Sorted,
    Exp,
    Ln,
    Log,
    Log2,
    Log10,
    Sinh,
    Cosh,
    Tanh,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    And,
    Or,
    Not,
    Select,
    Complex,
    ToF32,
    ToF64,
    ToExact,
    EnclosureLower,
    EnclosureUpper,
    Uncertain,
    UncertainExpanded,
    ConvertUnit,
    FromCelsius,
    FromFahrenheit,
    ToCelsius,
    ToFahrenheit,
    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
    BitNot,
    Wrap,
    InType,
    Radix,
    Bytes,
    Between,
    Tolerance,
    Substance,
    Reaction,
    MolarMass,
    Nuclide,
    NuclearReaction,
    QValue,
    RationalPart,
    CoefficientOf,
    RealPart,
    ImaginaryPart,
    Conjugate,
    Philox4x32_10,
}

impl Operator {
    pub const ALL: [Operator; 101] = [
        Operator::Add,
        Operator::Sub,
        Operator::Mul,
        Operator::Div,
        Operator::Neg,
        Operator::Pow,
        Operator::Sqrt,
        Operator::Abs,
        Operator::MulAdd,
        Operator::Floor,
        Operator::Ceil,
        Operator::Trunc,
        Operator::RoundTiesEven,
        Operator::Round,
        Operator::Percent,
        Operator::CopySign,
        Operator::Min,
        Operator::Max,
        Operator::Factorial,
        Operator::Binomial,
        Operator::Gcd,
        Operator::Lcm,
        Operator::Mod,
        Operator::Count,
        Operator::Total,
        Operator::Mean,
        Operator::Median,
        Operator::Transpose,
        Operator::Determinant,
        Operator::Trace,
        Operator::Inverse,
        Operator::Rank,
        Operator::Eigenvalues,
        Operator::Kernel,
        Operator::Eigenvectors,
        Operator::At,
        Operator::Smallest,
        Operator::Largest,
        Operator::Sorted,
        Operator::Exp,
        Operator::Ln,
        Operator::Log,
        Operator::Log2,
        Operator::Log10,
        Operator::Sinh,
        Operator::Cosh,
        Operator::Tanh,
        Operator::Sin,
        Operator::Cos,
        Operator::Tan,
        Operator::Asin,
        Operator::Acos,
        Operator::Atan,
        Operator::Atan2,
        Operator::Equal,
        Operator::NotEqual,
        Operator::Less,
        Operator::LessOrEqual,
        Operator::Greater,
        Operator::GreaterOrEqual,
        Operator::And,
        Operator::Or,
        Operator::Not,
        Operator::Select,
        Operator::Complex,
        Operator::ToF32,
        Operator::ToF64,
        Operator::ToExact,
        Operator::EnclosureLower,
        Operator::EnclosureUpper,
        Operator::Uncertain,
        Operator::UncertainExpanded,
        Operator::ConvertUnit,
        Operator::FromCelsius,
        Operator::FromFahrenheit,
        Operator::ToCelsius,
        Operator::ToFahrenheit,
        Operator::BitAnd,
        Operator::BitOr,
        Operator::BitXor,
        Operator::ShiftLeft,
        Operator::ShiftRight,
        Operator::BitNot,
        Operator::Wrap,
        Operator::InType,
        Operator::Radix,
        Operator::Bytes,
        Operator::Between,
        Operator::Tolerance,
        Operator::Substance,
        Operator::Reaction,
        Operator::MolarMass,
        Operator::Nuclide,
        Operator::NuclearReaction,
        Operator::QValue,
        Operator::RationalPart,
        Operator::CoefficientOf,
        Operator::RealPart,
        Operator::ImaginaryPart,
        Operator::Conjugate,
        Operator::Philox4x32_10,
    ];

    pub fn arity(self) -> usize {
        match self {
            Operator::Smallest
            | Operator::Largest
            | Operator::Sorted
            | Operator::Count
            | Operator::Total
            | Operator::Mean
            | Operator::Median
            | Operator::Transpose
            | Operator::Determinant
            | Operator::Trace
            | Operator::Inverse
            | Operator::Rank
            | Operator::Eigenvalues
            | Operator::Kernel
            | Operator::Eigenvectors
            | Operator::Neg
            | Operator::Sqrt
            | Operator::Abs
            | Operator::Floor
            | Operator::Ceil
            | Operator::Trunc
            | Operator::RoundTiesEven
            | Operator::Factorial
            | Operator::Exp
            | Operator::Ln
            | Operator::Log2
            | Operator::Log10
            | Operator::Sinh
            | Operator::Cosh
            | Operator::Tanh
            | Operator::Round
            | Operator::Percent
            | Operator::Sin
            | Operator::Cos
            | Operator::Tan
            | Operator::Asin
            | Operator::Acos
            | Operator::Atan
            | Operator::Not
            | Operator::ToF32
            | Operator::ToF64
            | Operator::ToExact
            | Operator::FromCelsius
            | Operator::FromFahrenheit
            | Operator::ToCelsius
            | Operator::ToFahrenheit
            | Operator::MolarMass
            | Operator::QValue
            | Operator::RationalPart
            | Operator::RealPart
            | Operator::ImaginaryPart
            | Operator::Conjugate => UNARY,
            Operator::MulAdd | Operator::Select | Operator::Philox4x32_10 => TERNARY,
            Operator::EnclosureLower | Operator::EnclosureUpper | Operator::CoefficientOf => BINARY,
            Operator::Add
            | Operator::Sub
            | Operator::Mul
            | Operator::Div
            | Operator::Pow
            | Operator::CopySign
            | Operator::Min
            | Operator::Max
            | Operator::Gcd
            | Operator::Lcm
            | Operator::Mod
            | Operator::Log
            | Operator::Binomial
            | Operator::At
            | Operator::Atan2
            | Operator::Equal
            | Operator::NotEqual
            | Operator::Less
            | Operator::LessOrEqual
            | Operator::Greater
            | Operator::GreaterOrEqual
            | Operator::And
            | Operator::Or
            | Operator::Complex
            | Operator::ConvertUnit => BINARY,
            Operator::Uncertain => TERNARY,
            Operator::UncertainExpanded => QUATERNARY,
            Operator::BitAnd
            | Operator::BitOr
            | Operator::BitXor
            | Operator::ShiftLeft
            | Operator::ShiftRight => BINARY,
            Operator::BitNot | Operator::Wrap | Operator::InType => TERNARY,
            Operator::Radix | Operator::Bytes => QUATERNARY,
            Operator::Between | Operator::Tolerance | Operator::Substance | Operator::Nuclide => {
                BINARY
            }
            Operator::Reaction | Operator::NuclearReaction => QUATERNARY,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_lists_every_operator_once() {
        let distinct: HashSet<Operator> = Operator::ALL.iter().copied().collect();
        assert_eq!(distinct.len(), Operator::ALL.len());
    }

    #[test]
    fn arithmetic_operators_keep_written_arity() {
        assert_eq!(Operator::Add.arity(), 2);
        assert_eq!(Operator::Neg.arity(), 1);
        assert_eq!(Operator::MulAdd.arity(), 3);
    }

    #[test]
    fn a_007_extension_operators_have_their_argument_counts() {
        assert_eq!(Operator::Uncertain.arity(), 3);
        assert_eq!(Operator::UncertainExpanded.arity(), 4);
        assert_eq!(Operator::ConvertUnit.arity(), 2);
        assert_eq!(Operator::Factorial.arity(), 1);
        assert_eq!(Operator::ToExact.arity(), 1);
    }

    #[test]
    fn temperature_conversion_operators_are_unary() {
        let arities = [
            Operator::FromCelsius,
            Operator::FromFahrenheit,
            Operator::ToCelsius,
            Operator::ToFahrenheit,
        ]
        .map(Operator::arity);

        assert_eq!(arities, [1, 1, 1, 1]);
    }

    #[test]
    fn select_takes_condition_and_two_branches() {
        assert_eq!(Operator::Select.arity(), 3);
    }
}
