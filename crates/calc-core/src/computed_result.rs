use std::collections::{BTreeMap, BTreeSet};

use calc_numbers::{Integer, Number};
use calc_units::UnitId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultKind {
    ExactRational,
    Algebraic,
    Symbolic,
    MachineFloat,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResultValue {
    Number(Number),
    Complex {
        real: Number,
        imaginary: Number,
    },
    Array {
        shape: Vec<usize>,
        elements: Vec<ResultValue>,
    },
    Expression(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoundingError {
    None,
    Bound(ResultValue),
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uncertainty {
    standard: ResultValue,
    coverage_factor: Number,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParameterValue {
    Value(ResultValue),
    Identifier(String),
    Boolean(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConvergenceStatus {
    Converged,
    NotConverged,
    IterationLimit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Convergence {
    pub status: ConvergenceStatus,
    pub iterations: u64,
    pub error_estimate: Option<ResultValue>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConditionKind {
    Absolute,
    Relative,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Condition {
    pub number: ResultValue,
    pub kind: ConditionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: String,
    pub data: BTreeMap<String, ParameterValue>,
}

pub const EXACT_EVALUATION_METHOD: &str = "exact_evaluation";

pub const PLAN_EVALUATION_METHOD: &str = "plan_evaluation";

pub const WAY_EVALUATION_METHOD: &str = "way_evaluation";

pub const EXACT_AFTER_CONVERSION_METHOD: &str = "exact_after_conversion";

pub const INSERTION_SORT_METHOD: &str = "insertion_sort";
pub const BINARY_INSERTION_SORT_METHOD: &str = "binary_insertion_sort";
pub const SELECTION_SORT_METHOD: &str = "selection_sort";
pub const BUBBLE_SORT_METHOD: &str = "bubble_sort";
pub const MERGE_SORT_METHOD: &str = "merge_sort";
pub const HEAP_SORT_METHOD: &str = "heap_sort";
pub const QUICK_SORT_METHOD: &str = "quick_sort";
pub const COUNTING_SORT_METHOD: &str = "counting_sort";
pub const PHILOX_METHOD: &str = "philox4x32_10";
pub const DOUBLE_SELECTION_SORT_METHOD: &str = "double_selection_sort";
pub const COCKTAIL_SHAKER_SORT_METHOD: &str = "cocktail_shaker_sort";
pub const GNOME_SORT_METHOD: &str = "gnome_sort";
pub const ODD_EVEN_SORT_METHOD: &str = "odd_even_sort";
pub const COMB_SORT_METHOD: &str = "comb_sort";
pub const CYCLE_SORT_METHOD: &str = "cycle_sort";
pub const PANCAKE_SORT_METHOD: &str = "pancake_sort";
pub const SHELL_SORT_METHOD: &str = "shell_sort";
pub const BOTTOM_UP_MERGE_SORT_METHOD: &str = "bottom_up_merge_sort";
pub const NATURAL_MERGE_SORT_METHOD: &str = "natural_merge_sort";
pub const RADIX_SORT_METHOD: &str = "radix_sort";
pub const BEAD_SORT_METHOD: &str = "bead_sort";
pub const BITONIC_SORT_METHOD: &str = "bitonic_sort";
pub const BOGO_SORT_METHOD: &str = "bogo_sort";

pub const METHOD_NAMES: [&str; 27] = [
    EXACT_EVALUATION_METHOD,
    PLAN_EVALUATION_METHOD,
    WAY_EVALUATION_METHOD,
    EXACT_AFTER_CONVERSION_METHOD,
    INSERTION_SORT_METHOD,
    BINARY_INSERTION_SORT_METHOD,
    SELECTION_SORT_METHOD,
    BUBBLE_SORT_METHOD,
    MERGE_SORT_METHOD,
    HEAP_SORT_METHOD,
    QUICK_SORT_METHOD,
    COUNTING_SORT_METHOD,
    PHILOX_METHOD,
    DOUBLE_SELECTION_SORT_METHOD,
    COCKTAIL_SHAKER_SORT_METHOD,
    GNOME_SORT_METHOD,
    ODD_EVEN_SORT_METHOD,
    COMB_SORT_METHOD,
    CYCLE_SORT_METHOD,
    PANCAKE_SORT_METHOD,
    SHELL_SORT_METHOD,
    BOTTOM_UP_MERGE_SORT_METHOD,
    NATURAL_MERGE_SORT_METHOD,
    RADIX_SORT_METHOD,
    BEAD_SORT_METHOD,
    BITONIC_SORT_METHOD,
    BOGO_SORT_METHOD,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Method {
    pub name: String,
    pub parameters: BTreeMap<String, ParameterValue>,
    pub convergence: Option<Convergence>,
    pub condition: Option<Condition>,
    pub notes: Vec<Diagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seed {
    pub value: u64,
    pub generator: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConceptId(pub String);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recognition {
    concepts: Vec<ConceptId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortRecord {
    pub from_positions: Option<Vec<u64>>,
    pub comparisons: u64,
    pub writes: u64,
    pub key_evaluations: Option<u64>,
    pub tallies: Option<u64>,
    pub key_range: Option<u64>,
    pub draws: Option<u64>,
    pub flips: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComputedResult {
    kind: ResultKind,
    value: ResultValue,
    unit: Option<UnitId>,
    rounding_error: RoundingError,
    uncertainty: Option<Uncertainty>,
    method: Method,
    corpus_references: BTreeSet<String>,
    sources: BTreeSet<String>,
    seed: Option<Seed>,
    recognition: Recognition,
    sort: Option<SortRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComputedResultError {
    KindDoesNotMatchValue {
        kind: ResultKind,
    },
    ArrayShapeDoesNotMatchElements {
        shape: Vec<usize>,
        element_count: usize,
    },
    BoundIsNotExactOrF64,
    BoundIsNegative,
    CoverageFactorBelowOne,
}

impl ResultValue {
    fn numbers(&self) -> Vec<&Number> {
        match self {
            Self::Number(number) => vec![number],
            Self::Complex { real, imaginary } => vec![real, imaginary],
            Self::Array { elements, .. } => elements.iter().flat_map(Self::numbers).collect(),
            Self::Expression(_) => Vec::new(),
        }
    }

    fn check_shape(&self) -> Result<(), ComputedResultError> {
        let Self::Array { shape, elements } = self else {
            return Ok(());
        };
        let expected = shape
            .iter()
            .try_fold(1usize, |count, length| count.checked_mul(*length));
        if expected != Some(elements.len()) {
            return Err(ComputedResultError::ArrayShapeDoesNotMatchElements {
                shape: shape.clone(),
                element_count: elements.len(),
            });
        }
        elements.iter().try_for_each(Self::check_shape)
    }
}

fn is_nonnegative_bound(number: &Number) -> Result<bool, ComputedResultError> {
    match number {
        Number::Integer(integer) => Ok(!integer.is_negative()),
        Number::Rational(rational) => Ok(!rational.numerator().is_negative()),
        Number::F64(value) => Ok(value.is_sign_positive() && !value.is_nan()),
        Number::F32(_) => Err(ComputedResultError::BoundIsNotExactOrF64),
    }
}

fn check_bound(value: &ResultValue) -> Result<(), ComputedResultError> {
    value.check_shape()?;
    for number in value.numbers() {
        if !is_nonnegative_bound(number)? {
            return Err(ComputedResultError::BoundIsNegative);
        }
    }
    Ok(())
}

fn number_matches_kind(number: &Number, kind: ResultKind) -> bool {
    match kind {
        ResultKind::ExactRational => number.is_exact(),
        ResultKind::MachineFloat => !number.is_exact(),
        ResultKind::Algebraic | ResultKind::Symbolic => false,
    }
}

impl Uncertainty {
    pub fn new(
        standard: ResultValue,
        coverage_factor: Number,
    ) -> Result<Self, ComputedResultError> {
        check_bound(&standard)?;
        let excess = coverage_factor
            .to_exact()
            .ok()
            .and_then(|exact| exact.sub_exact(&Number::Integer(Integer::one())).ok())
            .ok_or(ComputedResultError::CoverageFactorBelowOne)?;
        if !is_nonnegative_bound(&excess)? {
            return Err(ComputedResultError::CoverageFactorBelowOne);
        }
        Ok(Self {
            standard,
            coverage_factor,
        })
    }

    pub fn standard(&self) -> &ResultValue {
        &self.standard
    }

    pub fn coverage_factor(&self) -> &Number {
        &self.coverage_factor
    }
}

impl Method {
    pub fn named(name: &str) -> Self {
        Self {
            name: name.to_string(),
            parameters: BTreeMap::new(),
            convergence: None,
            condition: None,
            notes: Vec::new(),
        }
    }
}

impl Recognition {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn concepts(&self) -> &[ConceptId] {
        &self.concepts
    }

    pub fn is_empty(&self) -> bool {
        self.concepts.is_empty()
    }
}

impl ComputedResult {
    pub fn new(
        kind: ResultKind,
        value: ResultValue,
        unit: Option<UnitId>,
        method: Method,
    ) -> Result<Self, ComputedResultError> {
        value.check_shape()?;
        let is_expression = matches!(value, ResultValue::Expression(_));
        let numbers = value.numbers();
        let is_array = matches!(value, ResultValue::Array { .. });
        let matches_kind = if is_expression {
            matches!(kind, ResultKind::Algebraic | ResultKind::Symbolic)
        } else {
            (is_array || !numbers.is_empty())
                && numbers
                    .iter()
                    .all(|number| number_matches_kind(number, kind))
        };
        if !matches_kind {
            return Err(ComputedResultError::KindDoesNotMatchValue { kind });
        }
        let rounding_error = if kind == ResultKind::ExactRational || is_expression {
            RoundingError::None
        } else {
            RoundingError::Unknown
        };
        Ok(Self {
            kind,
            value,
            unit,
            rounding_error,
            uncertainty: None,
            method,
            corpus_references: BTreeSet::new(),
            sources: BTreeSet::new(),
            seed: None,
            recognition: Recognition::empty(),
            sort: None,
        })
    }

    pub fn with_rounding_bound(mut self, bound: ResultValue) -> Result<Self, ComputedResultError> {
        check_bound(&bound)?;
        self.rounding_error = RoundingError::Bound(bound);
        Ok(self)
    }

    pub fn with_method(mut self, method: Method) -> Self {
        self.method = method;
        self
    }

    pub fn with_uncertainty(mut self, uncertainty: Uncertainty) -> Self {
        self.uncertainty = Some(uncertainty);
        self
    }

    pub fn with_corpus_reference(mut self, block_id: &str) -> Self {
        self.corpus_references.insert(block_id.to_string());
        self
    }

    pub fn with_source(mut self, source_id: &str) -> Self {
        self.sources.insert(source_id.to_string());
        self
    }

    pub fn with_sort(mut self, sort: SortRecord) -> Self {
        self.sort = Some(sort);
        self
    }

    pub fn sort(&self) -> Option<&SortRecord> {
        self.sort.as_ref()
    }

    pub fn with_seed(mut self, seed: Seed) -> Self {
        self.seed = Some(seed);
        self
    }

    pub fn kind(&self) -> ResultKind {
        self.kind
    }

    pub fn value(&self) -> &ResultValue {
        &self.value
    }

    pub fn unit(&self) -> Option<UnitId> {
        self.unit
    }

    pub fn rounding_error(&self) -> &RoundingError {
        &self.rounding_error
    }

    pub fn uncertainty(&self) -> Option<&Uncertainty> {
        self.uncertainty.as_ref()
    }

    pub fn method(&self) -> &Method {
        &self.method
    }

    pub fn corpus_references(&self) -> impl Iterator<Item = &str> {
        self.corpus_references.iter().map(String::as_str)
    }

    pub fn sources(&self) -> impl Iterator<Item = &str> {
        self.sources.iter().map(String::as_str)
    }

    pub fn seed(&self) -> Option<&Seed> {
        self.seed.as_ref()
    }

    pub fn recognition(&self) -> &Recognition {
        &self.recognition
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integer(value: i64) -> Number {
        Number::from(value)
    }

    fn exact_result() -> ComputedResult {
        ComputedResult::new(
            ResultKind::ExactRational,
            ResultValue::Number(integer(2)),
            None,
            Method::named("exact_evaluation"),
        )
        .unwrap()
    }

    fn machine_result() -> ComputedResult {
        ComputedResult::new(
            ResultKind::MachineFloat,
            ResultValue::Number(Number::F64(24.525)),
            None,
            Method::named("plan_evaluation"),
        )
        .unwrap()
    }

    #[test]
    fn exact_result_has_no_rounding_error() {
        assert_eq!(exact_result().rounding_error(), &RoundingError::None);
    }

    #[test]
    fn machine_result_starts_with_unknown_rounding_error() {
        assert_eq!(machine_result().rounding_error(), &RoundingError::Unknown);
    }

    #[test]
    fn machine_value_with_exact_kind_is_rejected() {
        let result = ComputedResult::new(
            ResultKind::ExactRational,
            ResultValue::Number(Number::F64(1.0)),
            None,
            Method::named("exact_evaluation"),
        );

        assert_eq!(
            result,
            Err(ComputedResultError::KindDoesNotMatchValue {
                kind: ResultKind::ExactRational
            })
        );
    }

    #[test]
    fn array_with_wrong_element_count_is_rejected() {
        let value = ResultValue::Array {
            shape: vec![2, 2],
            elements: vec![ResultValue::Number(integer(1))],
        };

        let result = ComputedResult::new(
            ResultKind::ExactRational,
            value,
            None,
            Method::named("exact_evaluation"),
        );

        assert_eq!(
            result,
            Err(ComputedResultError::ArrayShapeDoesNotMatchElements {
                shape: vec![2, 2],
                element_count: 1
            })
        );
    }

    #[test]
    fn rounding_bound_is_stored() {
        let bound = ResultValue::Number(Number::F64(3.552713678800501e-15));

        let result = machine_result().with_rounding_bound(bound.clone()).unwrap();

        assert_eq!(result.rounding_error(), &RoundingError::Bound(bound));
    }

    #[test]
    fn negative_rounding_bound_is_rejected() {
        let result = machine_result().with_rounding_bound(ResultValue::Number(Number::F64(-0.0)));

        assert_eq!(result, Err(ComputedResultError::BoundIsNegative));
    }

    #[test]
    fn f32_rounding_bound_is_rejected() {
        let result = machine_result().with_rounding_bound(ResultValue::Number(Number::F32(1.0)));

        assert_eq!(result, Err(ComputedResultError::BoundIsNotExactOrF64));
    }

    #[test]
    fn uncertainty_keeps_standard_uncertainty_and_coverage_factor() {
        let uncertainty =
            Uncertainty::new(ResultValue::Number(Number::F64(0.11)), integer(2)).unwrap();

        let result = machine_result().with_uncertainty(uncertainty);

        let stored = result.uncertainty().unwrap();
        assert_eq!(stored.standard(), &ResultValue::Number(Number::F64(0.11)));
        assert_eq!(stored.coverage_factor(), &integer(2));
    }

    #[test]
    fn coverage_factor_below_one_is_rejected() {
        let coverage_factor = Number::fraction(&Integer::one(), &Integer::from(2i64)).unwrap();

        let uncertainty = Uncertainty::new(ResultValue::Number(integer(1)), coverage_factor);

        assert_eq!(
            uncertainty,
            Err(ComputedResultError::CoverageFactorBelowOne)
        );
    }

    #[test]
    fn corpus_references_are_sorted_without_duplicates() {
        let result = exact_result()
            .with_corpus_reference("P-GRD-D-038")
            .with_corpus_reference("P-GRD-D-013")
            .with_corpus_reference("P-GRD-D-038");

        assert_eq!(
            result.corpus_references().collect::<Vec<_>>(),
            vec!["P-GRD-D-013", "P-GRD-D-038"]
        );
    }

    #[test]
    fn sources_are_sorted_without_duplicates() {
        let result = exact_result()
            .with_source("openstax-prealgebra-2e")
            .with_source("openstax-algebra-and-trigonometry-2e")
            .with_source("openstax-prealgebra-2e");

        assert_eq!(
            result.sources().collect::<Vec<_>>(),
            vec![
                "openstax-algebra-and-trigonometry-2e",
                "openstax-prealgebra-2e"
            ]
        );
    }

    #[test]
    fn result_without_rules_has_no_sources() {
        assert_eq!(exact_result().sources().count(), 0);
    }

    #[test]
    fn seed_is_absent_until_set() {
        let seed = Seed {
            value: 42,
            generator: "xoshiro256starstar_1".to_string(),
        };

        assert_eq!(exact_result().seed(), None);
        assert_eq!(exact_result().with_seed(seed.clone()).seed(), Some(&seed));
    }

    #[test]
    fn method_keeps_parameters_and_diagnostics() {
        let mut method = Method::named("gauss_kronrod_adaptive");
        method.parameters.insert(
            "tolerance".to_string(),
            ParameterValue::Value(ResultValue::Number(Number::F64(1e-10))),
        );
        method.convergence = Some(Convergence {
            status: ConvergenceStatus::Converged,
            iterations: 7,
            error_estimate: Some(ResultValue::Number(Number::F64(2e-10))),
        });

        let result = ComputedResult::new(
            ResultKind::MachineFloat,
            ResultValue::Number(Number::F64(0.746824133)),
            None,
            method.clone(),
        )
        .unwrap();

        assert_eq!(result.method(), &method);
    }

    #[test]
    fn recognition_is_empty_for_now() {
        assert!(exact_result().recognition().is_empty());
    }

    #[test]
    fn symbolic_expression_value_is_exact() {
        let result = ComputedResult::new(
            ResultKind::Symbolic,
            ResultValue::Expression("pi / 6".to_string()),
            None,
            Method::named("exact_evaluation"),
        )
        .unwrap();

        assert_eq!(result.rounding_error(), &RoundingError::None);
    }

    #[test]
    fn expression_value_is_not_an_exact_rational() {
        let result = ComputedResult::new(
            ResultKind::ExactRational,
            ResultValue::Expression("pi / 6".to_string()),
            None,
            Method::named("exact_evaluation"),
        );

        assert!(matches!(
            result,
            Err(ComputedResultError::KindDoesNotMatchValue { .. })
        ));
    }
}
