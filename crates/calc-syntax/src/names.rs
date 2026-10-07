use calc_expr::{
    Operator, SortBubbleForm, SortCombForm, SortGaps, SortMethod, SortOddEvenForm, SortPartition,
    SortShakerForm,
};

pub(crate) const KEYWORD_AND: &str = "and";
pub(crate) const KEYWORD_OR: &str = "or";
pub(crate) const KEYWORD_NOT: &str = "not";
pub(crate) const LIMIT_FORM: &str = "lim";
pub(crate) const DERIVATIVE_FORM: &str = "d";
pub(crate) const SUMMATION_LETTER: &str = "\u{03A3}";
pub(crate) const PI_LETTER: &str = "\u{03C0}";
pub(crate) const PI_NAME: &str = "pi";
pub(crate) const INFINITY_NAME: &str = "inf";
pub(crate) const INTEGRAL_NAME: &str = "integral";
pub(crate) const SUM_NAME: &str = "sum";
pub(crate) const PRODUCT_NAME: &str = "product";
pub(crate) const LIMIT_NAME: &str = "limit";
pub(crate) const DERIVATIVE_NAME: &str = "diff";
pub(crate) const ROOT_NAME: &str = "rootof";
pub(crate) const TAYLOR_NAME: &str = "taylor";
pub(crate) const INSERTION_SORT_NAME: &str = "insertion_sort";
pub(crate) const BINARY_INSERTION_SORT_NAME: &str = "binary_insertion_sort";
pub(crate) const SELECTION_SORT_NAME: &str = "selection_sort";
pub(crate) const BUBBLE_SORT_NAME: &str = "bubble_sort";
pub(crate) const MERGE_SORT_NAME: &str = "merge_sort";
pub(crate) const HEAP_SORT_NAME: &str = "heap_sort";
pub(crate) const QUICK_SORT_NAME: &str = "quick_sort";
pub(crate) const COUNTING_SORT_NAME: &str = "counting_sort";
pub(crate) const DOUBLE_SELECTION_SORT_NAME: &str = "double_selection_sort";
pub(crate) const COCKTAIL_SHAKER_SORT_NAME: &str = "cocktail_shaker_sort";
pub(crate) const GNOME_SORT_NAME: &str = "gnome_sort";
pub(crate) const ODD_EVEN_SORT_NAME: &str = "odd_even_sort";
pub(crate) const COMB_SORT_NAME: &str = "comb_sort";
pub(crate) const CYCLE_SORT_NAME: &str = "cycle_sort";
pub(crate) const PANCAKE_SORT_NAME: &str = "pancake_sort";
pub(crate) const UNTIL_SORTED_FORM_VALUE: &str = "until_sorted";
pub(crate) const FIXED_PASSES_FORM_VALUE: &str = "fixed_passes";
pub(crate) const LACEY_BOX_FORM_VALUE: &str = "lacey_box";
pub(crate) const PARTITION_KEYWORD: &str = "partition";
pub(crate) const PIVOT_KEYWORD: &str = "pivot";
pub(crate) const GAPS_KEYWORD: &str = "gaps";
pub(crate) const BASE_KEYWORD: &str = "base";
pub(crate) const RADIX_SORT_NAME: &str = "radix_sort";
pub(crate) const BEAD_SORT_NAME: &str = "bead_sort";
pub(crate) const BITONIC_SORT_NAME: &str = "bitonic_sort";
pub(crate) const BOGO_SORT_NAME: &str = "bogo_sort";
pub(crate) const LIMIT_KEYWORD: &str = "limit";
pub(crate) const SHELL_SORT_NAME: &str = "shell_sort";
pub(crate) const BOTTOM_UP_MERGE_SORT_NAME: &str = "bottom_up_merge_sort";
pub(crate) const NATURAL_MERGE_SORT_NAME: &str = "natural_merge_sort";
pub(crate) const SHELL_GAPS_VALUE: &str = "shell";
pub(crate) const KNUTH_GAPS_VALUE: &str = "knuth";
pub(crate) const CIURA_GAPS_VALUE: &str = "ciura";
pub(crate) const LOMUTO_VALUE: &str = "lomuto";
pub(crate) const HOARE_VALUE: &str = "hoare";
pub(crate) const FIRST_VALUE: &str = "first";
pub(crate) const LAST_VALUE: &str = "last";
pub(crate) const RANDOM_VALUE: &str = "random";
pub(crate) const SEED_KEYWORD: &str = "seed";
pub(crate) const FORM_KEYWORD: &str = "form";
pub(crate) const FULL_FORM_VALUE: &str = "full";
pub(crate) const SHRINKING_FORM_VALUE: &str = "shrinking";
pub(crate) const EARLY_EXIT_FORM_VALUE: &str = "early_exit";
pub(crate) const LAST_EXCHANGE_FORM_VALUE: &str = "last_exchange";
pub(crate) const SORT_VARIABLE: &str = "x";
pub(crate) const ORDER_KEYWORD: &str = "order";
pub(crate) const INCREASING_VALUE: &str = "increasing";
pub(crate) const DECREASING_VALUE: &str = "decreasing";
pub(crate) const SHAPE_KEYWORD: &str = "shape";
pub(crate) const SIDE_KEYWORD: &str = "side";
pub(crate) const COVERAGE_KEYWORD: &str = "k";
pub(crate) const LEFT_FOLD_VALUE: &str = "left";
pub(crate) const HALVING_VALUE: &str = "halving";
pub(crate) const LEFT_SIDE_VALUE: &str = "left";
pub(crate) const RIGHT_SIDE_VALUE: &str = "right";
pub(crate) const BOTH_SIDES_VALUE: &str = "both";
pub(crate) const LABEL_PREFIX: char = 'r';
pub(crate) const SUFFIX_SEPARATOR: &str = "_";

pub(crate) const CALL_NAMES: [(&str, Operator); 77] = [
    ("sqrt", Operator::Sqrt),
    ("abs", Operator::Abs),
    ("exp", Operator::Exp),
    ("ln", Operator::Ln),
    ("sin", Operator::Sin),
    ("cos", Operator::Cos),
    ("tan", Operator::Tan),
    ("asin", Operator::Asin),
    ("acos", Operator::Acos),
    ("atan", Operator::Atan),
    ("atan2", Operator::Atan2),
    ("floor", Operator::Floor),
    ("ceil", Operator::Ceil),
    ("trunc", Operator::Trunc),
    ("round_ties_even", Operator::RoundTiesEven),
    ("copysign", Operator::CopySign),
    ("round", Operator::Round),
    ("binomial", Operator::Binomial),
    ("sinh", Operator::Sinh),
    ("cosh", Operator::Cosh),
    ("tanh", Operator::Tanh),
    ("log", Operator::Log),
    ("log2", Operator::Log2),
    ("log10", Operator::Log10),
    ("gcd", Operator::Gcd),
    ("count", Operator::Count),
    ("total", Operator::Total),
    ("mean", Operator::Mean),
    ("median", Operator::Median),
    ("transpose", Operator::Transpose),
    ("determinant", Operator::Determinant),
    ("trace", Operator::Trace),
    ("inverse", Operator::Inverse),
    ("rank", Operator::Rank),
    ("eigenvalues", Operator::Eigenvalues),
    ("kernel", Operator::Kernel),
    ("eigenvectors", Operator::Eigenvectors),
    ("at", Operator::At),
    ("smallest", Operator::Smallest),
    ("largest", Operator::Largest),
    ("sorted", Operator::Sorted),
    ("lcm", Operator::Lcm),
    ("mod", Operator::Mod),
    ("min", Operator::Min),
    ("max", Operator::Max),
    ("mul_add", Operator::MulAdd),
    ("select", Operator::Select),
    ("complex", Operator::Complex),
    ("to_f32", Operator::ToF32),
    ("to_f64", Operator::ToF64),
    ("exact", Operator::ToExact),
    ("enclosure_lower", Operator::EnclosureLower),
    ("enclosure_upper", Operator::EnclosureUpper),
    ("uncertain", Operator::Uncertain),
    ("uncertain_expanded", Operator::UncertainExpanded),
    ("convert_unit", Operator::ConvertUnit),
    ("from_celsius", Operator::FromCelsius),
    ("from_fahrenheit", Operator::FromFahrenheit),
    ("to_celsius", Operator::ToCelsius),
    ("to_fahrenheit", Operator::ToFahrenheit),
    ("bitand", Operator::BitAnd),
    ("bitor", Operator::BitOr),
    ("bitxor", Operator::BitXor),
    ("shl", Operator::ShiftLeft),
    ("shr", Operator::ShiftRight),
    ("bitnot", Operator::BitNot),
    ("wrap", Operator::Wrap),
    ("between", Operator::Between),
    ("tolerance", Operator::Tolerance),
    ("molar_mass", Operator::MolarMass),
    ("q_value", Operator::QValue),
    ("philox4x32_10", Operator::Philox4x32_10),
    ("rational_part", Operator::RationalPart),
    ("coefficient_of", Operator::CoefficientOf),
    ("re", Operator::RealPart),
    ("im", Operator::ImaginaryPart),
    ("conj", Operator::Conjugate),
];

pub(crate) fn integer_type(name: &str) -> Option<(u32, bool)> {
    let (signed, digits) = match name.split_at_checked(1)? {
        ("u", digits) => (false, digits),
        ("i", digits) => (true, digits),
        _ => return None,
    };
    if digits.is_empty() || digits.starts_with('0') || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let bits = digits.parse::<u32>().ok()?;
    (1..=calc_expr::LARGEST_INTEGER_WIDTH)
        .contains(&bits)
        .then_some((bits, signed))
}

pub(crate) fn radix_base(name: &str) -> Option<u32> {
    match name {
        "hex" => Some(16),
        "bin" => Some(2),
        "oct" => Some(8),
        "dec" => Some(10),
        _ => None,
    }
}

pub(crate) const BINDER_NAMES: [&str; 29] = [
    INTEGRAL_NAME,
    SUM_NAME,
    PRODUCT_NAME,
    LIMIT_NAME,
    DERIVATIVE_NAME,
    ROOT_NAME,
    TAYLOR_NAME,
    INSERTION_SORT_NAME,
    BINARY_INSERTION_SORT_NAME,
    SELECTION_SORT_NAME,
    BUBBLE_SORT_NAME,
    MERGE_SORT_NAME,
    HEAP_SORT_NAME,
    QUICK_SORT_NAME,
    COUNTING_SORT_NAME,
    DOUBLE_SELECTION_SORT_NAME,
    COCKTAIL_SHAKER_SORT_NAME,
    GNOME_SORT_NAME,
    ODD_EVEN_SORT_NAME,
    COMB_SORT_NAME,
    CYCLE_SORT_NAME,
    PANCAKE_SORT_NAME,
    SHELL_SORT_NAME,
    BOTTOM_UP_MERGE_SORT_NAME,
    NATURAL_MERGE_SORT_NAME,
    RADIX_SORT_NAME,
    BEAD_SORT_NAME,
    BITONIC_SORT_NAME,
    BOGO_SORT_NAME,
];

pub(crate) const RESERVED_WORDS: [&str; 8] = [
    PI_NAME,
    "e",
    "i",
    INFINITY_NAME,
    KEYWORD_AND,
    KEYWORD_OR,
    KEYWORD_NOT,
    LIMIT_FORM,
];

pub(crate) fn operator_for_call(name: &str) -> Option<Operator> {
    CALL_NAMES
        .iter()
        .find(|(call_name, _)| *call_name == name)
        .map(|(_, operator)| *operator)
}

pub(crate) fn call_name(operator: Operator) -> Option<&'static str> {
    CALL_NAMES
        .iter()
        .find(|(_, candidate)| *candidate == operator)
        .map(|(name, _)| *name)
}

pub(crate) fn is_prefix_function(name: &str) -> bool {
    operator_for_call(name).is_some_and(|operator| operator.arity() == 1)
}

pub(crate) fn sort_method_for_name(name: &str) -> Option<SortMethod> {
    match name {
        INSERTION_SORT_NAME => Some(SortMethod::Insertion),
        BINARY_INSERTION_SORT_NAME => Some(SortMethod::BinaryInsertion),
        SELECTION_SORT_NAME => Some(SortMethod::Selection),
        BUBBLE_SORT_NAME => Some(SortMethod::Bubble(SortBubbleForm::Full)),
        MERGE_SORT_NAME => Some(SortMethod::Merge),
        HEAP_SORT_NAME => Some(SortMethod::Heap),
        QUICK_SORT_NAME => Some(SortMethod::Quick(SortPartition::LomutoLast)),
        COUNTING_SORT_NAME => Some(SortMethod::Counting),
        DOUBLE_SELECTION_SORT_NAME => Some(SortMethod::DoubleSelection),
        COCKTAIL_SHAKER_SORT_NAME => Some(SortMethod::CocktailShaker(SortShakerForm::Full)),
        GNOME_SORT_NAME => Some(SortMethod::Gnome),
        ODD_EVEN_SORT_NAME => Some(SortMethod::OddEven(SortOddEvenForm::UntilSorted)),
        COMB_SORT_NAME => Some(SortMethod::Comb(SortCombForm::LaceyBox)),
        CYCLE_SORT_NAME => Some(SortMethod::Cycle),
        PANCAKE_SORT_NAME => Some(SortMethod::Pancake),
        SHELL_SORT_NAME => Some(SortMethod::Shell(SortGaps::Shell)),
        BOTTOM_UP_MERGE_SORT_NAME => Some(SortMethod::BottomUpMerge),
        NATURAL_MERGE_SORT_NAME => Some(SortMethod::NaturalMerge),
        RADIX_SORT_NAME => Some(SortMethod::Radix),
        BEAD_SORT_NAME => Some(SortMethod::Bead),
        BITONIC_SORT_NAME => Some(SortMethod::Bitonic),
        BOGO_SORT_NAME => Some(SortMethod::Bogo),
        _ => None,
    }
}

pub(crate) fn sort_method_name(method: SortMethod) -> &'static str {
    match method {
        SortMethod::Insertion => INSERTION_SORT_NAME,
        SortMethod::BinaryInsertion => BINARY_INSERTION_SORT_NAME,
        SortMethod::Selection => SELECTION_SORT_NAME,
        SortMethod::Bubble(_) => BUBBLE_SORT_NAME,
        SortMethod::Merge => MERGE_SORT_NAME,
        SortMethod::Heap => HEAP_SORT_NAME,
        SortMethod::Quick(_) => QUICK_SORT_NAME,
        SortMethod::Counting => COUNTING_SORT_NAME,
        SortMethod::DoubleSelection => DOUBLE_SELECTION_SORT_NAME,
        SortMethod::CocktailShaker(_) => COCKTAIL_SHAKER_SORT_NAME,
        SortMethod::Gnome => GNOME_SORT_NAME,
        SortMethod::OddEven(_) => ODD_EVEN_SORT_NAME,
        SortMethod::Comb(_) => COMB_SORT_NAME,
        SortMethod::Cycle => CYCLE_SORT_NAME,
        SortMethod::Pancake => PANCAKE_SORT_NAME,
        SortMethod::Shell(_) => SHELL_SORT_NAME,
        SortMethod::BottomUpMerge => BOTTOM_UP_MERGE_SORT_NAME,
        SortMethod::NaturalMerge => NATURAL_MERGE_SORT_NAME,
        SortMethod::Radix => RADIX_SORT_NAME,
        SortMethod::Bead => BEAD_SORT_NAME,
        SortMethod::Bitonic => BITONIC_SORT_NAME,
        SortMethod::Bogo => BOGO_SORT_NAME,
    }
}

pub(crate) fn partition_values(partition: SortPartition) -> (&'static str, &'static str) {
    match partition {
        SortPartition::LomutoLast => (LOMUTO_VALUE, LAST_VALUE),
        SortPartition::HoareFirst => (HOARE_VALUE, FIRST_VALUE),
        SortPartition::LomutoRandom => (LOMUTO_VALUE, RANDOM_VALUE),
    }
}

pub(crate) fn bubble_form_value(form: SortBubbleForm) -> &'static str {
    match form {
        SortBubbleForm::Full => FULL_FORM_VALUE,
        SortBubbleForm::Shrinking => SHRINKING_FORM_VALUE,
        SortBubbleForm::EarlyExit => EARLY_EXIT_FORM_VALUE,
        SortBubbleForm::LastExchange => LAST_EXCHANGE_FORM_VALUE,
    }
}

pub(crate) fn is_binder_name(name: &str) -> bool {
    BINDER_NAMES.contains(&name)
}

pub(crate) const DEGREE_SIGN_CHARACTER: char = '\u{00B0}';
pub(crate) const DEGREE_SIGN: &str = "\u{00B0}";

pub(crate) fn is_keyword(name: &str) -> bool {
    [KEYWORD_AND, KEYWORD_OR, KEYWORD_NOT].contains(&name)
}

pub(crate) fn is_label(name: &str) -> bool {
    let mut characters = name.chars();
    characters.next() == Some(LABEL_PREFIX)
        && name.len() > 1
        && characters.all(|character| character.is_ascii_digit())
}

pub(crate) fn is_constant_name(name: &str) -> bool {
    [PI_NAME, PI_LETTER, "e", "i", INFINITY_NAME].contains(&name)
}

pub(crate) fn is_reserved(name: &str) -> bool {
    RESERVED_WORDS.contains(&name)
        || name == PI_LETTER
        || name == SUMMATION_LETTER
        || is_label(name)
        || operator_for_call(name).is_some()
        || is_binder_name(name)
}

pub(crate) fn changes_parsing(name: &str) -> bool {
    is_keyword(name)
        || name == LIMIT_FORM
        || name == PI_LETTER
        || name == SUMMATION_LETTER
        || operator_for_call(name).is_some()
        || is_binder_name(name)
}

pub(crate) fn canonical_symbol_name(name: &str) -> &str {
    if name == PI_LETTER { PI_NAME } else { name }
}

pub(crate) fn sort_form_value(method: SortMethod) -> Option<&'static str> {
    Some(match method {
        SortMethod::Bubble(form) => bubble_form_value(form),
        SortMethod::CocktailShaker(form) => match form {
            SortShakerForm::Full => FULL_FORM_VALUE,
            SortShakerForm::Shrinking => SHRINKING_FORM_VALUE,
            SortShakerForm::LastExchange => LAST_EXCHANGE_FORM_VALUE,
        },
        SortMethod::OddEven(form) => match form {
            SortOddEvenForm::UntilSorted => UNTIL_SORTED_FORM_VALUE,
            SortOddEvenForm::FixedPasses => FIXED_PASSES_FORM_VALUE,
        },
        SortMethod::Comb(SortCombForm::LaceyBox) => LACEY_BOX_FORM_VALUE,
        SortMethod::Shell(gaps) => match gaps {
            SortGaps::Shell => SHELL_GAPS_VALUE,
            SortGaps::Knuth => KNUTH_GAPS_VALUE,
            SortGaps::Ciura => CIURA_GAPS_VALUE,
        },
        _ => return None,
    })
}

pub(crate) fn sort_with_form(method: SortMethod, value: &str) -> Option<SortMethod> {
    let candidates: &[SortMethod] = match method {
        SortMethod::Bubble(_) => &[
            SortMethod::Bubble(SortBubbleForm::Full),
            SortMethod::Bubble(SortBubbleForm::Shrinking),
            SortMethod::Bubble(SortBubbleForm::EarlyExit),
            SortMethod::Bubble(SortBubbleForm::LastExchange),
        ],
        SortMethod::CocktailShaker(_) => &[
            SortMethod::CocktailShaker(SortShakerForm::Full),
            SortMethod::CocktailShaker(SortShakerForm::Shrinking),
            SortMethod::CocktailShaker(SortShakerForm::LastExchange),
        ],
        SortMethod::OddEven(_) => &[
            SortMethod::OddEven(SortOddEvenForm::UntilSorted),
            SortMethod::OddEven(SortOddEvenForm::FixedPasses),
        ],
        SortMethod::Comb(_) => &[SortMethod::Comb(SortCombForm::LaceyBox)],
        SortMethod::Shell(_) => &[
            SortMethod::Shell(SortGaps::Shell),
            SortMethod::Shell(SortGaps::Knuth),
            SortMethod::Shell(SortGaps::Ciura),
        ],
        _ => &[],
    };
    candidates
        .iter()
        .copied()
        .find(|candidate| sort_form_value(*candidate) == Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_r_followed_by_digits() {
        assert!(is_label("r12"));
        assert!(!is_label("r"));
        assert!(!is_label("rate"));
    }

    #[test]
    fn every_call_name_maps_back_to_its_operator() {
        for (name, operator) in CALL_NAMES {
            assert_eq!(operator_for_call(name), Some(operator));
            assert_eq!(call_name(operator), Some(name));
        }
    }
}
