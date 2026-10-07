use std::ops::Range;

use calc_expr::{AccessError, BuildError, SymbolError};
use calc_units::{UnitAccessError, UnitProductError};

use crate::ambiguity::AmbiguousApplication;
use crate::attempts::RecognisedAttempt;
use crate::chemistry::ChemistryProblem;
use crate::fraction_unit::FractionBeforeUnit;
use crate::nuclear::NuclearProblem;
use crate::percent_sum::PercentInASum;
use crate::temperature_sign::AmbiguousTemperatureSign;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keyword {
    Shape,
    Side,
    Order,
    Form,
    Partition,
    Pivot,
    ShakerForm,
    OddEvenForm,
    CombForm,
    Gaps,
}

impl Keyword {
    pub fn name(self) -> &'static str {
        match self {
            Self::Shape => crate::names::SHAPE_KEYWORD,
            Self::Side => crate::names::SIDE_KEYWORD,
            Self::Order => crate::names::ORDER_KEYWORD,
            Self::Form => crate::names::FORM_KEYWORD,
            Self::Partition => crate::names::PARTITION_KEYWORD,
            Self::Pivot => crate::names::PIVOT_KEYWORD,
            Self::ShakerForm | Self::OddEvenForm | Self::CombForm => crate::names::FORM_KEYWORD,
            Self::Gaps => crate::names::GAPS_KEYWORD,
        }
    }

    pub fn values(self) -> &'static [&'static str] {
        match self {
            Self::Shape => &[crate::names::LEFT_FOLD_VALUE, crate::names::HALVING_VALUE],
            Self::Side => &[
                crate::names::LEFT_SIDE_VALUE,
                crate::names::RIGHT_SIDE_VALUE,
                crate::names::BOTH_SIDES_VALUE,
            ],
            Self::Order => &[
                crate::names::INCREASING_VALUE,
                crate::names::DECREASING_VALUE,
            ],
            Self::Form => &[
                crate::names::FULL_FORM_VALUE,
                crate::names::SHRINKING_FORM_VALUE,
                crate::names::EARLY_EXIT_FORM_VALUE,
                crate::names::LAST_EXCHANGE_FORM_VALUE,
            ],
            Self::Partition => &[crate::names::LOMUTO_VALUE, crate::names::HOARE_VALUE],
            Self::Pivot => &[
                crate::names::FIRST_VALUE,
                crate::names::LAST_VALUE,
                crate::names::RANDOM_VALUE,
            ],
            Self::ShakerForm => &[
                crate::names::FULL_FORM_VALUE,
                crate::names::SHRINKING_FORM_VALUE,
                crate::names::LAST_EXCHANGE_FORM_VALUE,
            ],
            Self::OddEvenForm => &[
                crate::names::UNTIL_SORTED_FORM_VALUE,
                crate::names::FIXED_PASSES_FORM_VALUE,
            ],
            Self::CombForm => &[crate::names::LACEY_BOX_FORM_VALUE],
            Self::Gaps => &[
                crate::names::SHELL_GAPS_VALUE,
                crate::names::KNUTH_GAPS_VALUE,
                crate::names::CIURA_GAPS_VALUE,
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseErrorKind {
    UnexpectedCharacter,
    UnexpectedToken,
    UnexpectedEnd,
    InvalidTypedLiteral,
    ExponentTooLarge,
    NotAUnit,
    AtomicMassUnit,
    NotAUnitJoinedToAUnit,
    AmbiguousUnit,
    TypeExpected,
    ByteOrderMissing,
    UnitExponentOutOfRange,
    UnitExponentNotWhole,
    ReservedName,
    ChainedRelation,
    RaggedArray,
    UnknownKeyword,
    DuplicateKeyword,
    PositionalAfterKeyword,
    InvalidKeywordValue { keyword: Keyword },
    MissingKeyword { keyword: Keyword },
    PivotNotBuiltForPartition,
    MissingPivot { built: &'static str },
    MissingSeed,
    InvalidSeed,
    MissingBase,
    InvalidBase,
    MissingShuffleSeed,
    MissingLimit,
    InvalidLimit,
    SeedWithoutRandomPivot,
    MissingDifferential,
    NestedTooDeeply { limit: usize },
    ChainTooLong { limit: usize },
    ExpressionTooDeep { limit: usize },
    ArityMismatch { expected: usize, found: usize },
    Build(BuildError),
    Symbol(SymbolError),
    UnitProduct(UnitProductError),
    RecognisedAttempt(RecognisedAttempt),
    AmbiguousApplication(AmbiguousApplication),
    AmbiguousTemperatureSign(AmbiguousTemperatureSign),
    FractionBeforeUnit(FractionBeforeUnit),
    PercentInASum(PercentInASum),
    Chemistry(ChemistryProblem),
    Nuclear(NuclearProblem),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub span: Range<usize>,
}

impl ParseError {
    pub(crate) fn new(kind: ParseErrorKind, span: Range<usize>) -> Self {
        Self { kind, span }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrintError {
    Access(AccessError),
    Symbol(SymbolError),
    UnitAccess(UnitAccessError),
    DimensionlessQuantity,
    UnprintableArrayShape,
    UnboundIndex(u32),
}
