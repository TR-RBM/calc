use calc_i18n::Message;
use calc_syntax::{
    Construct, LanguageReference, PrefixEntry, ReferenceEntry, ReferenceGroup, Spelling, UnitEntry,
    language_reference,
};

use crate::json::{self, Json, MAXIMUM_SAFE_COUNT};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceWords {
    pub name: Message,
    pub meaning: Message,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceSection {
    pub group: ReferenceGroup,
    pub heading: Message,
    pub entries: Vec<(ReferenceEntry, Option<ReferenceWords>)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalizedReference {
    pub reference: LanguageReference,
    pub sections: Vec<ReferenceSection>,
}

pub fn reference_group_message(group: ReferenceGroup) -> Message {
    match group {
        ReferenceGroup::Numbers => Message::CommonReferenceGroupNumbers,
        ReferenceGroup::Uncertainty => Message::CommonReferenceGroupUncertainty,
        ReferenceGroup::Units => Message::CommonReferenceGroupUnits,
        ReferenceGroup::Operators => Message::CommonReferenceGroupOperators,
        ReferenceGroup::Relations => Message::CommonReferenceGroupRelations,
        ReferenceGroup::Calls => Message::CommonReferenceGroupCalls,
        ReferenceGroup::KeywordArguments => Message::CommonReferenceGroupKeywordArguments,
        ReferenceGroup::Arrays => Message::CommonReferenceGroupArrays,
        ReferenceGroup::Binders => Message::CommonReferenceGroupBinders,
        ReferenceGroup::Statements => Message::CommonReferenceGroupStatements,
    }
}

pub fn construct_words(construct: &Construct) -> Option<ReferenceWords> {
    let words = match (construct.kind.name(), construct.name) {
        ("constant", "pi") => ReferenceWords {
            name: Message::CommonReferenceConstantPi,
            meaning: Message::CommonReferenceConstantPiMeaning,
        },
        ("constant", "e") => ReferenceWords {
            name: Message::CommonReferenceConstantE,
            meaning: Message::CommonReferenceConstantEMeaning,
        },
        ("constant", "imaginary_unit") => ReferenceWords {
            name: Message::CommonReferenceConstantImaginaryUnit,
            meaning: Message::CommonReferenceConstantImaginaryUnitMeaning,
        },
        ("constant", "infinity") => ReferenceWords {
            name: Message::CommonReferenceConstantInfinity,
            meaning: Message::CommonReferenceConstantInfinityMeaning,
        },
        ("typed_literal", "rational") => ReferenceWords {
            name: Message::CommonReferenceTypedLiteralRational,
            meaning: Message::CommonReferenceTypedLiteralRationalMeaning,
        },
        ("typed_literal", "f32") => ReferenceWords {
            name: Message::CommonReferenceTypedLiteralF32,
            meaning: Message::CommonReferenceTypedLiteralF32Meaning,
        },
        ("typed_literal", "f64") => ReferenceWords {
            name: Message::CommonReferenceTypedLiteralF64,
            meaning: Message::CommonReferenceTypedLiteralF64Meaning,
        },
        ("typed_literal", "chemistry") => ReferenceWords {
            name: Message::CommonReferenceTypedLiteralChemistry,
            meaning: Message::CommonReferenceTypedLiteralChemistryMeaning,
        },
        ("typed_literal", "nuclear") => ReferenceWords {
            name: Message::CommonReferenceTypedLiteralNuclear,
            meaning: Message::CommonReferenceTypedLiteralNuclearMeaning,
        },
        ("literal", "decimal") => ReferenceWords {
            name: Message::CommonReferenceLiteralDecimal,
            meaning: Message::CommonReferenceLiteralDecimalMeaning,
        },
        ("literal", "exponent") => ReferenceWords {
            name: Message::CommonReferenceLiteralExponent,
            meaning: Message::CommonReferenceLiteralExponentMeaning,
        },
        ("literal", "line_label") => ReferenceWords {
            name: Message::CommonReferenceLiteralLineLabel,
            meaning: Message::CommonReferenceLiteralLineLabelMeaning,
        },
        ("operator", "uncertain") => ReferenceWords {
            name: Message::CommonReferenceOperatorUncertain,
            meaning: Message::CommonReferenceOperatorUncertainMeaning,
        },
        ("operator", "uncertain_expanded") => ReferenceWords {
            name: Message::CommonReferenceOperatorUncertainExpanded,
            meaning: Message::CommonReferenceOperatorUncertainExpandedMeaning,
        },
        ("operator", "convert_unit") => ReferenceWords {
            name: Message::CommonReferenceOperatorConvertUnit,
            meaning: Message::CommonReferenceOperatorConvertUnitMeaning,
        },
        ("form", "quantity") => ReferenceWords {
            name: Message::CommonReferenceFormQuantity,
            meaning: Message::CommonReferenceFormQuantityMeaning,
        },
        ("form", "unit_expression") => ReferenceWords {
            name: Message::CommonReferenceFormUnitExpression,
            meaning: Message::CommonReferenceFormUnitExpressionMeaning,
        },
        ("form", "degree_sign") => ReferenceWords {
            name: Message::CommonReferenceFormDegreeSign,
            meaning: Message::CommonReferenceFormDegreeSignMeaning,
        },
        ("operator", "add") => ReferenceWords {
            name: Message::CommonReferenceOperatorAdd,
            meaning: Message::CommonReferenceOperatorAddMeaning,
        },
        ("operator", "sub") => ReferenceWords {
            name: Message::CommonReferenceOperatorSub,
            meaning: Message::CommonReferenceOperatorSubMeaning,
        },
        ("operator", "mul") => ReferenceWords {
            name: Message::CommonReferenceOperatorMul,
            meaning: Message::CommonReferenceOperatorMulMeaning,
        },
        ("operator", "div") => ReferenceWords {
            name: Message::CommonReferenceOperatorDiv,
            meaning: Message::CommonReferenceOperatorDivMeaning,
        },
        ("operator", "neg") => ReferenceWords {
            name: Message::CommonReferenceOperatorNeg,
            meaning: Message::CommonReferenceOperatorNegMeaning,
        },
        ("operator", "pow") => ReferenceWords {
            name: Message::CommonReferenceOperatorPow,
            meaning: Message::CommonReferenceOperatorPowMeaning,
        },
        ("operator", "factorial") => ReferenceWords {
            name: Message::CommonReferenceOperatorFactorial,
            meaning: Message::CommonReferenceOperatorFactorialMeaning,
        },
        ("operator", "percent") => ReferenceWords {
            name: Message::CommonReferenceOperatorPercent,
            meaning: Message::CommonReferenceOperatorPercentMeaning,
        },
        ("operator", "round") => ReferenceWords {
            name: Message::CommonReferenceOperatorRound,
            meaning: Message::CommonReferenceOperatorRoundMeaning,
        },
        ("operator", "binomial") => ReferenceWords {
            name: Message::CommonReferenceOperatorBinomial,
            meaning: Message::CommonReferenceOperatorBinomialMeaning,
        },
        ("operator", "sinh") => ReferenceWords {
            name: Message::CommonReferenceOperatorSinh,
            meaning: Message::CommonReferenceOperatorSinhMeaning,
        },
        ("operator", "cosh") => ReferenceWords {
            name: Message::CommonReferenceOperatorCosh,
            meaning: Message::CommonReferenceOperatorCoshMeaning,
        },
        ("operator", "tanh") => ReferenceWords {
            name: Message::CommonReferenceOperatorTanh,
            meaning: Message::CommonReferenceOperatorTanhMeaning,
        },
        ("operator", "log") => ReferenceWords {
            name: Message::CommonReferenceOperatorLog,
            meaning: Message::CommonReferenceOperatorLogMeaning,
        },
        ("operator", "log2") => ReferenceWords {
            name: Message::CommonReferenceOperatorLog2,
            meaning: Message::CommonReferenceOperatorLog2Meaning,
        },
        ("operator", "log10") => ReferenceWords {
            name: Message::CommonReferenceOperatorLog10,
            meaning: Message::CommonReferenceOperatorLog10Meaning,
        },
        ("operator", "gcd") => ReferenceWords {
            name: Message::CommonReferenceOperatorGcd,
            meaning: Message::CommonReferenceOperatorGcdMeaning,
        },
        ("operator", "count") => ReferenceWords {
            name: Message::CommonReferenceOperatorCount,
            meaning: Message::CommonReferenceOperatorCountMeaning,
        },
        ("operator", "total") => ReferenceWords {
            name: Message::CommonReferenceOperatorTotal,
            meaning: Message::CommonReferenceOperatorTotalMeaning,
        },
        ("operator", "mean") => ReferenceWords {
            name: Message::CommonReferenceOperatorMean,
            meaning: Message::CommonReferenceOperatorMeanMeaning,
        },
        ("operator", "median") => ReferenceWords {
            name: Message::CommonReferenceOperatorMedian,
            meaning: Message::CommonReferenceOperatorMedianMeaning,
        },
        ("operator", "transpose") => ReferenceWords {
            name: Message::CommonReferenceOperatorTranspose,
            meaning: Message::CommonReferenceOperatorTransposeMeaning,
        },
        ("operator", "determinant") => ReferenceWords {
            name: Message::CommonReferenceOperatorDeterminant,
            meaning: Message::CommonReferenceOperatorDeterminantMeaning,
        },
        ("operator", "trace") => ReferenceWords {
            name: Message::CommonReferenceOperatorTrace,
            meaning: Message::CommonReferenceOperatorTraceMeaning,
        },
        ("operator", "inverse") => ReferenceWords {
            name: Message::CommonReferenceOperatorInverse,
            meaning: Message::CommonReferenceOperatorInverseMeaning,
        },
        ("operator", "rank") => ReferenceWords {
            name: Message::CommonReferenceOperatorRank,
            meaning: Message::CommonReferenceOperatorRankMeaning,
        },
        ("operator", "eigenvectors") => ReferenceWords {
            name: Message::CommonReferenceOperatorEigenvectors,
            meaning: Message::CommonReferenceOperatorEigenvectorsMeaning,
        },
        ("operator", "kernel") => ReferenceWords {
            name: Message::CommonReferenceOperatorKernel,
            meaning: Message::CommonReferenceOperatorKernelMeaning,
        },
        ("operator", "eigenvalues") => ReferenceWords {
            name: Message::CommonReferenceOperatorEigenvalues,
            meaning: Message::CommonReferenceOperatorEigenvaluesMeaning,
        },
        ("operator", "at") => ReferenceWords {
            name: Message::CommonReferenceOperatorAt,
            meaning: Message::CommonReferenceOperatorAtMeaning,
        },
        ("operator", "smallest") => ReferenceWords {
            name: Message::CommonReferenceOperatorSmallest,
            meaning: Message::CommonReferenceOperatorSmallestMeaning,
        },
        ("operator", "largest") => ReferenceWords {
            name: Message::CommonReferenceOperatorLargest,
            meaning: Message::CommonReferenceOperatorLargestMeaning,
        },
        ("operator", "sorted") => ReferenceWords {
            name: Message::CommonReferenceOperatorSorted,
            meaning: Message::CommonReferenceOperatorSortedMeaning,
        },
        ("operator", "bitand") => ReferenceWords {
            name: Message::CommonReferenceOperatorBitAnd,
            meaning: Message::CommonReferenceOperatorBitAndMeaning,
        },
        ("operator", "bitor") => ReferenceWords {
            name: Message::CommonReferenceOperatorBitOr,
            meaning: Message::CommonReferenceOperatorBitOrMeaning,
        },
        ("operator", "bitxor") => ReferenceWords {
            name: Message::CommonReferenceOperatorBitXor,
            meaning: Message::CommonReferenceOperatorBitXorMeaning,
        },
        ("operator", "shl") => ReferenceWords {
            name: Message::CommonReferenceOperatorShiftLeft,
            meaning: Message::CommonReferenceOperatorShiftLeftMeaning,
        },
        ("operator", "shr") => ReferenceWords {
            name: Message::CommonReferenceOperatorShiftRight,
            meaning: Message::CommonReferenceOperatorShiftRightMeaning,
        },
        ("operator", "bitnot") => ReferenceWords {
            name: Message::CommonReferenceOperatorBitNot,
            meaning: Message::CommonReferenceOperatorBitNotMeaning,
        },
        ("operator", "wrap") => ReferenceWords {
            name: Message::CommonReferenceOperatorWrap,
            meaning: Message::CommonReferenceOperatorWrapMeaning,
        },
        ("operator", "in_type") => ReferenceWords {
            name: Message::CommonReferenceOperatorInType,
            meaning: Message::CommonReferenceOperatorInTypeMeaning,
        },
        ("operator", "radix") => ReferenceWords {
            name: Message::CommonReferenceOperatorRadix,
            meaning: Message::CommonReferenceOperatorRadixMeaning,
        },
        ("operator", "bytes") => ReferenceWords {
            name: Message::CommonReferenceOperatorBytes,
            meaning: Message::CommonReferenceOperatorBytesMeaning,
        },
        ("operator", "between") => ReferenceWords {
            name: Message::CommonReferenceOperatorBetween,
            meaning: Message::CommonReferenceOperatorBetweenMeaning,
        },
        ("operator", "tolerance") => ReferenceWords {
            name: Message::CommonReferenceOperatorTolerance,
            meaning: Message::CommonReferenceOperatorToleranceMeaning,
        },
        ("operator", "substance") => ReferenceWords {
            name: Message::CommonReferenceOperatorSubstance,
            meaning: Message::CommonReferenceOperatorSubstanceMeaning,
        },
        ("operator", "reaction") => ReferenceWords {
            name: Message::CommonReferenceOperatorReaction,
            meaning: Message::CommonReferenceOperatorReactionMeaning,
        },
        ("operator", "molar_mass") => ReferenceWords {
            name: Message::CommonReferenceOperatorMolarMass,
            meaning: Message::CommonReferenceOperatorMolarMassMeaning,
        },
        ("operator", "nuclide") => ReferenceWords {
            name: Message::CommonReferenceOperatorNuclide,
            meaning: Message::CommonReferenceOperatorNuclideMeaning,
        },
        ("operator", "nuclear_reaction") => ReferenceWords {
            name: Message::CommonReferenceOperatorNuclearReaction,
            meaning: Message::CommonReferenceOperatorNuclearReactionMeaning,
        },
        ("operator", "philox4x32_10") => ReferenceWords {
            name: Message::CommonReferenceOperatorPhilox4x3210,
            meaning: Message::CommonReferenceOperatorPhilox4x3210Meaning,
        },
        ("operator", "q_value") => ReferenceWords {
            name: Message::CommonReferenceOperatorQValue,
            meaning: Message::CommonReferenceOperatorQValueMeaning,
        },
        ("operator", "rational_part") => ReferenceWords {
            name: Message::CommonReferenceOperatorRationalPart,
            meaning: Message::CommonReferenceOperatorRationalPartMeaning,
        },
        ("operator", "coefficient_of") => ReferenceWords {
            name: Message::CommonReferenceOperatorCoefficientOf,
            meaning: Message::CommonReferenceOperatorCoefficientOfMeaning,
        },
        ("operator", "re") => ReferenceWords {
            name: Message::CommonReferenceOperatorRe,
            meaning: Message::CommonReferenceOperatorReMeaning,
        },
        ("operator", "im") => ReferenceWords {
            name: Message::CommonReferenceOperatorIm,
            meaning: Message::CommonReferenceOperatorImMeaning,
        },
        ("operator", "conj") => ReferenceWords {
            name: Message::CommonReferenceOperatorConj,
            meaning: Message::CommonReferenceOperatorConjMeaning,
        },
        ("operator", "lcm") => ReferenceWords {
            name: Message::CommonReferenceOperatorLcm,
            meaning: Message::CommonReferenceOperatorLcmMeaning,
        },
        ("operator", "mod") => ReferenceWords {
            name: Message::CommonReferenceOperatorMod,
            meaning: Message::CommonReferenceOperatorModMeaning,
        },
        ("operator", "equal") => ReferenceWords {
            name: Message::CommonReferenceOperatorEqual,
            meaning: Message::CommonReferenceOperatorEqualMeaning,
        },
        ("operator", "not_equal") => ReferenceWords {
            name: Message::CommonReferenceOperatorNotEqual,
            meaning: Message::CommonReferenceOperatorNotEqualMeaning,
        },
        ("operator", "less") => ReferenceWords {
            name: Message::CommonReferenceOperatorLess,
            meaning: Message::CommonReferenceOperatorLessMeaning,
        },
        ("operator", "less_or_equal") => ReferenceWords {
            name: Message::CommonReferenceOperatorLessOrEqual,
            meaning: Message::CommonReferenceOperatorLessOrEqualMeaning,
        },
        ("operator", "greater") => ReferenceWords {
            name: Message::CommonReferenceOperatorGreater,
            meaning: Message::CommonReferenceOperatorGreaterMeaning,
        },
        ("operator", "greater_or_equal") => ReferenceWords {
            name: Message::CommonReferenceOperatorGreaterOrEqual,
            meaning: Message::CommonReferenceOperatorGreaterOrEqualMeaning,
        },
        ("operator", "and") => ReferenceWords {
            name: Message::CommonReferenceOperatorAnd,
            meaning: Message::CommonReferenceOperatorAndMeaning,
        },
        ("operator", "or") => ReferenceWords {
            name: Message::CommonReferenceOperatorOr,
            meaning: Message::CommonReferenceOperatorOrMeaning,
        },
        ("operator", "not") => ReferenceWords {
            name: Message::CommonReferenceOperatorNot,
            meaning: Message::CommonReferenceOperatorNotMeaning,
        },
        ("operator", "sqrt") => ReferenceWords {
            name: Message::CommonReferenceOperatorSqrt,
            meaning: Message::CommonReferenceOperatorSqrtMeaning,
        },
        ("operator", "abs") => ReferenceWords {
            name: Message::CommonReferenceOperatorAbs,
            meaning: Message::CommonReferenceOperatorAbsMeaning,
        },
        ("operator", "mul_add") => ReferenceWords {
            name: Message::CommonReferenceOperatorMulAdd,
            meaning: Message::CommonReferenceOperatorMulAddMeaning,
        },
        ("operator", "floor") => ReferenceWords {
            name: Message::CommonReferenceOperatorFloor,
            meaning: Message::CommonReferenceOperatorFloorMeaning,
        },
        ("operator", "ceil") => ReferenceWords {
            name: Message::CommonReferenceOperatorCeil,
            meaning: Message::CommonReferenceOperatorCeilMeaning,
        },
        ("operator", "trunc") => ReferenceWords {
            name: Message::CommonReferenceOperatorTrunc,
            meaning: Message::CommonReferenceOperatorTruncMeaning,
        },
        ("operator", "round_ties_even") => ReferenceWords {
            name: Message::CommonReferenceOperatorRoundTiesEven,
            meaning: Message::CommonReferenceOperatorRoundTiesEvenMeaning,
        },
        ("operator", "copysign") => ReferenceWords {
            name: Message::CommonReferenceOperatorCopysign,
            meaning: Message::CommonReferenceOperatorCopysignMeaning,
        },
        ("operator", "min") => ReferenceWords {
            name: Message::CommonReferenceOperatorMin,
            meaning: Message::CommonReferenceOperatorMinMeaning,
        },
        ("operator", "max") => ReferenceWords {
            name: Message::CommonReferenceOperatorMax,
            meaning: Message::CommonReferenceOperatorMaxMeaning,
        },
        ("operator", "exp") => ReferenceWords {
            name: Message::CommonReferenceOperatorExp,
            meaning: Message::CommonReferenceOperatorExpMeaning,
        },
        ("operator", "ln") => ReferenceWords {
            name: Message::CommonReferenceOperatorLn,
            meaning: Message::CommonReferenceOperatorLnMeaning,
        },
        ("operator", "sin") => ReferenceWords {
            name: Message::CommonReferenceOperatorSin,
            meaning: Message::CommonReferenceOperatorSinMeaning,
        },
        ("operator", "cos") => ReferenceWords {
            name: Message::CommonReferenceOperatorCos,
            meaning: Message::CommonReferenceOperatorCosMeaning,
        },
        ("operator", "tan") => ReferenceWords {
            name: Message::CommonReferenceOperatorTan,
            meaning: Message::CommonReferenceOperatorTanMeaning,
        },
        ("operator", "asin") => ReferenceWords {
            name: Message::CommonReferenceOperatorAsin,
            meaning: Message::CommonReferenceOperatorAsinMeaning,
        },
        ("operator", "acos") => ReferenceWords {
            name: Message::CommonReferenceOperatorAcos,
            meaning: Message::CommonReferenceOperatorAcosMeaning,
        },
        ("operator", "atan") => ReferenceWords {
            name: Message::CommonReferenceOperatorAtan,
            meaning: Message::CommonReferenceOperatorAtanMeaning,
        },
        ("operator", "atan2") => ReferenceWords {
            name: Message::CommonReferenceOperatorAtan2,
            meaning: Message::CommonReferenceOperatorAtan2Meaning,
        },
        ("operator", "select") => ReferenceWords {
            name: Message::CommonReferenceOperatorSelect,
            meaning: Message::CommonReferenceOperatorSelectMeaning,
        },
        ("operator", "complex") => ReferenceWords {
            name: Message::CommonReferenceOperatorComplex,
            meaning: Message::CommonReferenceOperatorComplexMeaning,
        },
        ("operator", "to_f32") => ReferenceWords {
            name: Message::CommonReferenceOperatorToF32,
            meaning: Message::CommonReferenceOperatorToF32Meaning,
        },
        ("operator", "to_f64") => ReferenceWords {
            name: Message::CommonReferenceOperatorToF64,
            meaning: Message::CommonReferenceOperatorToF64Meaning,
        },
        ("operator", "exact") => ReferenceWords {
            name: Message::CommonReferenceOperatorExact,
            meaning: Message::CommonReferenceOperatorExactMeaning,
        },
        ("operator", "enclosure_lower") => ReferenceWords {
            name: Message::CommonReferenceOperatorEnclosureLower,
            meaning: Message::CommonReferenceOperatorEnclosureLowerMeaning,
        },
        ("operator", "enclosure_upper") => ReferenceWords {
            name: Message::CommonReferenceOperatorEnclosureUpper,
            meaning: Message::CommonReferenceOperatorEnclosureUpperMeaning,
        },
        ("operator", "from_celsius") => ReferenceWords {
            name: Message::CommonReferenceOperatorFromCelsius,
            meaning: Message::CommonReferenceOperatorFromCelsiusMeaning,
        },
        ("operator", "from_fahrenheit") => ReferenceWords {
            name: Message::CommonReferenceOperatorFromFahrenheit,
            meaning: Message::CommonReferenceOperatorFromFahrenheitMeaning,
        },
        ("operator", "to_celsius") => ReferenceWords {
            name: Message::CommonReferenceOperatorToCelsius,
            meaning: Message::CommonReferenceOperatorToCelsiusMeaning,
        },
        ("operator", "to_fahrenheit") => ReferenceWords {
            name: Message::CommonReferenceOperatorToFahrenheit,
            meaning: Message::CommonReferenceOperatorToFahrenheitMeaning,
        },
        ("keyword_argument", "shape") => ReferenceWords {
            name: Message::CommonReferenceKeywordArgumentShape,
            meaning: Message::CommonReferenceKeywordArgumentShapeMeaning,
        },
        ("keyword_argument", "side") => ReferenceWords {
            name: Message::CommonReferenceKeywordArgumentSide,
            meaning: Message::CommonReferenceKeywordArgumentSideMeaning,
        },
        ("keyword_argument", "coverage") => ReferenceWords {
            name: Message::CommonReferenceKeywordArgumentCoverage,
            meaning: Message::CommonReferenceKeywordArgumentCoverageMeaning,
        },
        ("form", "array") => ReferenceWords {
            name: Message::CommonReferenceFormArray,
            meaning: Message::CommonReferenceFormArrayMeaning,
        },
        ("form", "matrix") => ReferenceWords {
            name: Message::CommonReferenceFormMatrix,
            meaning: Message::CommonReferenceFormMatrixMeaning,
        },
        ("binder", "lambda") => ReferenceWords {
            name: Message::CommonReferenceBinderLambda,
            meaning: Message::CommonReferenceBinderLambdaMeaning,
        },
        ("binder", "sum") => ReferenceWords {
            name: Message::CommonReferenceBinderSum,
            meaning: Message::CommonReferenceBinderSumMeaning,
        },
        ("binder", "sum_halving") => ReferenceWords {
            name: Message::CommonReferenceBinderSumHalving,
            meaning: Message::CommonReferenceBinderSumHalvingMeaning,
        },
        ("binder", "product") => ReferenceWords {
            name: Message::CommonReferenceBinderProduct,
            meaning: Message::CommonReferenceBinderProductMeaning,
        },
        ("binder", "product_halving") => ReferenceWords {
            name: Message::CommonReferenceBinderProductHalving,
            meaning: Message::CommonReferenceBinderProductHalvingMeaning,
        },
        ("binder", "integral") => ReferenceWords {
            name: Message::CommonReferenceBinderIntegral,
            meaning: Message::CommonReferenceBinderIntegralMeaning,
        },
        ("binder", "limit") => ReferenceWords {
            name: Message::CommonReferenceBinderLimit,
            meaning: Message::CommonReferenceBinderLimitMeaning,
        },
        ("binder", "limit_left") => ReferenceWords {
            name: Message::CommonReferenceBinderLimitLeft,
            meaning: Message::CommonReferenceBinderLimitLeftMeaning,
        },
        ("binder", "limit_right") => ReferenceWords {
            name: Message::CommonReferenceBinderLimitRight,
            meaning: Message::CommonReferenceBinderLimitRightMeaning,
        },
        ("binder", "derivative") => ReferenceWords {
            name: Message::CommonReferenceBinderDerivative,
            meaning: Message::CommonReferenceBinderDerivativeMeaning,
        },
        ("binder", "binary_insertion_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderBinaryInsertionSort,
            meaning: Message::CommonReferenceBinderBinaryInsertionSortMeaning,
        },
        ("binder", "selection_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderSelectionSort,
            meaning: Message::CommonReferenceBinderSelectionSortMeaning,
        },
        ("binder", "bubble_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderBubbleSort,
            meaning: Message::CommonReferenceBinderBubbleSortMeaning,
        },
        ("binder", "merge_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderMergeSort,
            meaning: Message::CommonReferenceBinderMergeSortMeaning,
        },
        ("binder", "heap_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderHeapSort,
            meaning: Message::CommonReferenceBinderHeapSortMeaning,
        },
        ("binder", "quick_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderQuickSort,
            meaning: Message::CommonReferenceBinderQuickSortMeaning,
        },
        ("binder", "double_selection_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderDoubleSelectionSort,
            meaning: Message::CommonReferenceBinderDoubleSelectionSortMeaning,
        },
        ("binder", "cocktail_shaker_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderCocktailShakerSort,
            meaning: Message::CommonReferenceBinderCocktailShakerSortMeaning,
        },
        ("binder", "gnome_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderGnomeSort,
            meaning: Message::CommonReferenceBinderGnomeSortMeaning,
        },
        ("binder", "odd_even_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderOddEvenSort,
            meaning: Message::CommonReferenceBinderOddEvenSortMeaning,
        },
        ("binder", "comb_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderCombSort,
            meaning: Message::CommonReferenceBinderCombSortMeaning,
        },
        ("binder", "cycle_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderCycleSort,
            meaning: Message::CommonReferenceBinderCycleSortMeaning,
        },
        ("binder", "pancake_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderPancakeSort,
            meaning: Message::CommonReferenceBinderPancakeSortMeaning,
        },
        ("binder", "bottom_up_merge_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderBottomUpMergeSort,
            meaning: Message::CommonReferenceBinderBottomUpMergeSortMeaning,
        },
        ("binder", "natural_merge_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderNaturalMergeSort,
            meaning: Message::CommonReferenceBinderNaturalMergeSortMeaning,
        },
        ("binder", "bogo_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderBogoSort,
            meaning: Message::CommonReferenceBinderBogoSortMeaning,
        },
        ("binder", "bitonic_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderBitonicSort,
            meaning: Message::CommonReferenceBinderBitonicSortMeaning,
        },
        ("binder", "bead_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderBeadSort,
            meaning: Message::CommonReferenceBinderBeadSortMeaning,
        },
        ("binder", "radix_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderRadixSort,
            meaning: Message::CommonReferenceBinderRadixSortMeaning,
        },
        ("binder", "shell_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderShellSort,
            meaning: Message::CommonReferenceBinderShellSortMeaning,
        },
        ("binder", "counting_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderCountingSort,
            meaning: Message::CommonReferenceBinderCountingSortMeaning,
        },
        ("binder", "insertion_sort") => ReferenceWords {
            name: Message::CommonReferenceBinderInsertionSort,
            meaning: Message::CommonReferenceBinderInsertionSortMeaning,
        },
        ("binder", "taylor") => ReferenceWords {
            name: Message::CommonReferenceBinderTaylor,
            meaning: Message::CommonReferenceBinderTaylorMeaning,
        },
        ("binder", "root") => ReferenceWords {
            name: Message::CommonReferenceBinderRoot,
            meaning: Message::CommonReferenceBinderRootMeaning,
        },
        ("form", "comment") => ReferenceWords {
            name: Message::CommonReferenceFormComment,
            meaning: Message::CommonReferenceFormCommentMeaning,
        },
        ("statement", "naming") => ReferenceWords {
            name: Message::CommonReferenceStatementNaming,
            meaning: Message::CommonReferenceStatementNamingMeaning,
        },
        ("statement", "function_naming") => ReferenceWords {
            name: Message::CommonReferenceStatementFunctionNaming,
            meaning: Message::CommonReferenceStatementFunctionNamingMeaning,
        },
        _ => return None,
    };
    Some(words)
}

pub fn localized_language_reference() -> LocalizedReference {
    let reference = language_reference();
    let sections = ReferenceGroup::ALL
        .into_iter()
        .map(|group| ReferenceSection {
            group,
            heading: reference_group_message(group),
            entries: reference
                .entries
                .iter()
                .filter(|entry| entry.group == group)
                .map(|entry| (entry.clone(), construct_words(&entry.construct)))
                .collect(),
        })
        .collect();
    LocalizedReference {
        reference,
        sections,
    }
}

fn count_json(count: usize) -> Json {
    u64::try_from(count)
        .ok()
        .filter(|count| *count <= MAXIMUM_SAFE_COUNT)
        .map_or(Json::Null, Json::Count)
}

fn spelling_json(spelling: &Spelling) -> Json {
    Json::object(vec![
        ("symbol", Json::string(spelling.symbol)),
        ("pattern", Json::string(spelling.pattern)),
        ("mode", Json::string(spelling.mode.name())),
        ("example", Json::string(spelling.example)),
        (
            "placeholders",
            Json::Array(
                spelling
                    .placeholders
                    .iter()
                    .map(|range| {
                        Json::object(vec![
                            ("start", count_json(range.start)),
                            ("end", count_json(range.end)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

fn entry_json(entry: &ReferenceEntry, words: Option<&ReferenceWords>) -> Json {
    Json::object(vec![
        ("group", Json::string(entry.group.name())),
        (
            "construct",
            Json::object(vec![
                ("kind", Json::string(entry.construct.kind.name())),
                ("name", Json::string(entry.construct.name)),
            ]),
        ),
        (
            "name_key",
            Json::optional(words.map(|words| Json::string(words.name.key()))),
        ),
        (
            "meaning_key",
            Json::optional(words.map(|words| Json::string(words.meaning.key()))),
        ),
        (
            "spellings",
            Json::Array(entry.spellings.iter().map(spelling_json).collect()),
        ),
        (
            "canonical",
            Json::Array(
                entry
                    .canonical
                    .iter()
                    .map(|(mode, text)| {
                        Json::object(vec![
                            ("mode", Json::string(mode.name())),
                            ("text", Json::string(text)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "precedence",
            Json::optional(entry.precedence.map(|precedence| {
                Json::object(vec![
                    ("level", Json::Count(u64::from(precedence.level))),
                    (
                        "associativity",
                        Json::string(precedence.associativity.name()),
                    ),
                ])
            })),
        ),
        (
            "arguments",
            Json::optional(entry.arguments.map(|arguments| {
                Json::object(vec![
                    ("least", count_json(arguments.least)),
                    ("largest", count_json(arguments.largest)),
                    (
                        "keywords",
                        Json::Array(
                            arguments
                                .keywords
                                .iter()
                                .map(|keyword| {
                                    Json::object(vec![
                                        ("name", Json::string(keyword.name)),
                                        ("value", Json::string(keyword.value.name())),
                                        (
                                            "values",
                                            Json::Array(
                                                keyword
                                                    .values
                                                    .iter()
                                                    .map(|value| Json::string(value))
                                                    .collect(),
                                            ),
                                        ),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ])
            })),
        ),
    ])
}

fn unit_json(unit: &UnitEntry) -> Json {
    Json::object(vec![
        ("symbol", Json::string(&unit.symbol)),
        ("display_symbol", Json::string(&unit.display_symbol)),
        (
            "dimension",
            Json::Array(
                unit.dimension
                    .iter()
                    .map(|exponent| Json::string(&exponent.to_string()))
                    .collect(),
            ),
        ),
        (
            "scale_factor",
            Json::object(vec![
                ("numerator", Json::string(&unit.scale_numerator)),
                ("denominator", Json::string(&unit.scale_denominator)),
                (
                    "pi_exponent",
                    Json::string(&unit.scale_pi_exponent.to_string()),
                ),
            ]),
        ),
        ("accepts_prefix", Json::Boolean(unit.accepts_prefix)),
    ])
}

fn prefix_json(prefix: &PrefixEntry) -> Json {
    Json::object(vec![
        ("symbol", Json::string(prefix.symbol)),
        ("exponent", Json::string(&prefix.exponent.to_string())),
    ])
}

pub fn language_reference_json(localized: &LocalizedReference) -> Vec<u8> {
    let sections = localized
        .sections
        .iter()
        .map(|section| {
            Json::object(vec![
                ("group", Json::string(section.group.name())),
                ("heading_key", Json::string(section.heading.key())),
                (
                    "entries",
                    Json::Array(
                        section
                            .entries
                            .iter()
                            .map(|(entry, words)| entry_json(entry, words.as_ref()))
                            .collect(),
                    ),
                ),
            ])
        })
        .collect();
    let precedence = localized
        .reference
        .precedence
        .iter()
        .map(|row| {
            Json::object(vec![
                ("level", Json::Count(u64::from(row.level))),
                (
                    "construct",
                    Json::object(vec![
                        ("kind", Json::string(row.construct.kind.name())),
                        ("name", Json::string(row.construct.name)),
                    ]),
                ),
                ("associativity", Json::string(row.associativity.name())),
            ])
        })
        .collect();
    json::write_canonical(&Json::object(vec![
        ("groups", Json::Array(sections)),
        ("precedence", Json::Array(precedence)),
        (
            "units",
            Json::Array(localized.reference.units.iter().map(unit_json).collect()),
        ),
        (
            "prefixes",
            Json::Array(
                localized
                    .reference
                    .prefixes
                    .iter()
                    .map(prefix_json)
                    .collect(),
            ),
        ),
    ]))
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_i18n::{Locale, render};

    #[test]
    fn every_construct_has_its_two_words_in_every_locale() {
        for entry in &language_reference().entries {
            let words = construct_words(&entry.construct).unwrap_or_else(|| {
                panic!(
                    "no words for {} {}",
                    entry.construct.kind.name(),
                    entry.construct.name
                )
            });
            for locale in Locale::shipped() {
                for message in [&words.name, &words.meaning] {
                    assert!(!render(message, &locale).as_str().is_empty());
                }
            }
        }
    }

    #[test]
    fn every_group_has_its_heading_in_every_locale() {
        for group in ReferenceGroup::ALL {
            for locale in Locale::shipped() {
                let heading = render(&reference_group_message(group), &locale);
                assert!(!heading.as_str().is_empty());
            }
        }
    }

    #[test]
    fn the_name_and_the_meaning_of_a_construct_differ() {
        for entry in &language_reference().entries {
            let words = construct_words(&entry.construct).expect("words");
            for locale in Locale::shipped() {
                assert_ne!(
                    render(&words.name, &locale).as_str(),
                    render(&words.meaning, &locale).as_str()
                );
            }
        }
    }

    #[test]
    fn the_sections_hold_every_entry_in_the_order_of_the_reference() {
        let localized = localized_language_reference();
        let listed: Vec<&Construct> = localized
            .sections
            .iter()
            .flat_map(|section| section.entries.iter().map(|(entry, _)| &entry.construct))
            .collect();
        let expected: Vec<&Construct> = ReferenceGroup::ALL
            .into_iter()
            .flat_map(|group| {
                localized
                    .reference
                    .entries
                    .iter()
                    .filter(move |entry| entry.group == group)
                    .map(|entry| &entry.construct)
            })
            .collect();

        assert_eq!(listed, expected);
    }

    #[test]
    fn a_section_keeps_an_entry_whose_construct_has_no_words() {
        let localized = localized_language_reference();
        let counted: usize = localized
            .sections
            .iter()
            .map(|section| section.entries.len())
            .sum();

        assert_eq!(counted, localized.reference.entries.len());
    }

    #[test]
    fn words_name_the_keys_of_their_messages() {
        let construct = Construct {
            kind: calc_syntax::ConstructKind::Operator,
            name: "mul",
        };

        let words = construct_words(&construct).expect("words");

        assert_eq!(
            (words.name.key(), words.meaning.key()),
            (
                "common-reference-operator-mul",
                "common-reference-operator-mul-meaning"
            )
        );
    }

    #[test]
    fn a_construct_the_reference_does_not_hold_has_no_words() {
        let construct = Construct {
            kind: calc_syntax::ConstructKind::Operator,
            name: "cube_root",
        };

        assert_eq!(construct_words(&construct), None);
    }
}
