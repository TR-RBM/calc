use std::cmp::Ordering;

use calc_core::{
    BEAD_SORT_METHOD, BINARY_INSERTION_SORT_METHOD, BITONIC_SORT_METHOD, BOGO_SORT_METHOD,
    BOTTOM_UP_MERGE_SORT_METHOD, BUBBLE_SORT_METHOD, COCKTAIL_SHAKER_SORT_METHOD, COMB_SORT_METHOD,
    COUNTING_SORT_METHOD, CYCLE_SORT_METHOD, ComputedResult, DOUBLE_SELECTION_SORT_METHOD,
    GNOME_SORT_METHOD, HEAP_SORT_METHOD, INSERTION_SORT_METHOD, MERGE_SORT_METHOD,
    NATURAL_MERGE_SORT_METHOD, ODD_EVEN_SORT_METHOD, PANCAKE_SORT_METHOD, ParameterValue,
    QUICK_SORT_METHOD, RADIX_SORT_METHOD, SELECTION_SORT_METHOD, SHELL_SORT_METHOD, SortReport,
};
use calc_expr::{SortBubbleForm, SortPartition};
use calc_numbers::{Integer, Number};
use calc_sort::{
    BubbleForm, Case, CombForm, Formula, Gaps, Measure, Method, OddEvenForm, Partition, Place,
    Provenance, ShakerForm, Source, Step,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CostSummary {
    pub measure: Measure,
    pub case: Case,
    pub formula: Option<String>,
    pub value: Option<String>,
    pub holds_from: u64,
    pub provenance: Provenance,
    pub over: calc_sort::Over,
    pub checked_up_to: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortSummary {
    pub method: Method,
    pub is_decreasing: bool,
    pub is_stable: bool,
    pub source: Source,
    pub length: String,
    pub from_positions: Option<String>,
    pub comparisons: String,
    pub writes: String,
    pub key_evaluations: Option<u64>,
    pub tallies: Option<String>,
    pub key_range: Option<u64>,
    pub draws: Option<u64>,
    pub seed: Option<u64>,
    pub flips: Option<u64>,
    pub gaps: Option<String>,
    pub passes: Option<u64>,
    pub shuffles: Option<u64>,
    pub limit: Option<u64>,
    pub expected_shuffles: Option<String>,
    pub base: Option<u64>,
    pub costs: Vec<CostSummary>,
    pub lower_bound: Option<String>,
}

pub(crate) const INCREASING_ORDER: &str = "increasing";
pub(crate) const DECREASING_ORDER: &str = "decreasing";
pub(crate) const ORDER_PARAMETER: &str = "order";
pub(crate) const FORM_PARAMETER: &str = "form";
pub(crate) const GAPS_PARAMETER: &str = "gaps";
pub(crate) const BASE_PARAMETER: &str = "base";
pub(crate) const LIMIT_PARAMETER: &str = "limit";
pub(crate) const PARTITION_PARAMETER: &str = "partition";
const LOMUTO_LAST_NAME: &str = "lomuto_last";
const HOARE_FIRST_NAME: &str = "hoare_first";
const LOMUTO_RANDOM_NAME: &str = "lomuto_random";

pub(crate) fn partition_name(partition: SortPartition) -> &'static str {
    match partition {
        SortPartition::LomutoLast => LOMUTO_LAST_NAME,
        SortPartition::HoareFirst => HOARE_FIRST_NAME,
        SortPartition::LomutoRandom => LOMUTO_RANDOM_NAME,
    }
}
const INDEX_NAME: &str = "k";
const MERGE_AVERAGE_TEXT: &str = "A(n) (A(n) = A(ceil(n/2)) + A(floor(n/2)) + n - ceil(n/2)/(floor(n/2) + 1) - floor(n/2)/(ceil(n/2) + 1), A(0) = A(1) = 0)";
const BUBBLE_FORMS: [(BubbleForm, SortBubbleForm, &str); 4] = [
    (BubbleForm::Full, SortBubbleForm::Full, "full"),
    (
        BubbleForm::Shrinking,
        SortBubbleForm::Shrinking,
        "shrinking",
    ),
    (
        BubbleForm::EarlyExit,
        SortBubbleForm::EarlyExit,
        "early_exit",
    ),
    (
        BubbleForm::LastExchange,
        SortBubbleForm::LastExchange,
        "last_exchange",
    ),
];

pub(crate) fn form_name(method: calc_expr::SortMethod) -> Option<&'static str> {
    Some(match method {
        calc_expr::SortMethod::Bubble(form) => bubble_form_name(form),
        calc_expr::SortMethod::CocktailShaker(form) => match form {
            calc_expr::SortShakerForm::Full => "full",
            calc_expr::SortShakerForm::Shrinking => "shrinking",
            calc_expr::SortShakerForm::LastExchange => "last_exchange",
        },
        calc_expr::SortMethod::OddEven(form) => match form {
            calc_expr::SortOddEvenForm::UntilSorted => "until_sorted",
            calc_expr::SortOddEvenForm::FixedPasses => "fixed_passes",
        },
        calc_expr::SortMethod::Comb(calc_expr::SortCombForm::LaceyBox) => "lacey_box",
        calc_expr::SortMethod::Shell(gaps) => match gaps {
            calc_expr::SortGaps::Shell => "shell",
            calc_expr::SortGaps::Knuth => "knuth",
            calc_expr::SortGaps::Ciura => "ciura",
        },
        _ => return None,
    })
}

fn bubble_form_name(form: SortBubbleForm) -> &'static str {
    BUBBLE_FORMS
        .iter()
        .find(|(_, candidate, _)| *candidate == form)
        .map_or("", |(_, _, name)| name)
}
const LENGTH_NAME: &str = "n";
const GAP_SUM_TEXT: &str = "sum(n - h over the gaps h)";
const HARMONIC_TEXT: &str = "sum(1/k, k, 1, n)";
const ENTRY_SEPARATOR: &str = ", ";

pub(crate) fn sort_method_of(
    name: &str,
    form: Option<&ParameterValue>,
    partition: Option<&ParameterValue>,
    seed: Option<u64>,
    base: Option<&ParameterValue>,
    limit: Option<&ParameterValue>,
) -> Option<Method> {
    match name {
        INSERTION_SORT_METHOD => Some(Method::Insertion),
        BINARY_INSERTION_SORT_METHOD => Some(Method::BinaryInsertion),
        SELECTION_SORT_METHOD => Some(Method::Selection),
        MERGE_SORT_METHOD => Some(Method::Merge),
        COUNTING_SORT_METHOD => Some(Method::Counting),
        QUICK_SORT_METHOD => {
            let Some(ParameterValue::Identifier(partition)) = partition else {
                return None;
            };
            match partition.as_str() {
                LOMUTO_LAST_NAME => Some(Method::Quick(Partition::LomutoLast)),
                HOARE_FIRST_NAME => Some(Method::Quick(Partition::HoareFirst)),
                LOMUTO_RANDOM_NAME => Some(Method::Quick(Partition::LomutoRandom { seed: seed? })),
                _ => None,
            }
        }
        HEAP_SORT_METHOD => Some(Method::Heap),
        BOTTOM_UP_MERGE_SORT_METHOD => Some(Method::BottomUpMerge),
        NATURAL_MERGE_SORT_METHOD => Some(Method::NaturalMerge),
        BEAD_SORT_METHOD => Some(Method::Bead),
        BITONIC_SORT_METHOD => Some(Method::Bitonic),
        BOGO_SORT_METHOD => Some(Method::Bogo {
            seed: seed?,
            limit: whole_parameter(limit?)?,
        }),
        RADIX_SORT_METHOD => {
            let Some(ParameterValue::Value(calc_core::ResultValue::Number(Number::Integer(base)))) =
                base
            else {
                return None;
            };
            Some(Method::Radix {
                base: u64::try_from(base.to_i128()?).ok()?,
            })
        }
        SHELL_SORT_METHOD => {
            let Some(ParameterValue::Identifier(gaps)) = form else {
                return None;
            };
            match gaps.as_str() {
                "shell" => Some(Method::Shell(Gaps::Shell)),
                "knuth" => Some(Method::Shell(Gaps::Knuth)),
                "ciura" => Some(Method::Shell(Gaps::Ciura)),
                _ => None,
            }
        }
        DOUBLE_SELECTION_SORT_METHOD => Some(Method::DoubleSelection),
        COCKTAIL_SHAKER_SORT_METHOD => {
            let Some(ParameterValue::Identifier(form)) = form else {
                return None;
            };
            match form.as_str() {
                "full" => Some(Method::CocktailShaker(ShakerForm::Full)),
                "shrinking" => Some(Method::CocktailShaker(ShakerForm::Shrinking)),
                "last_exchange" => Some(Method::CocktailShaker(ShakerForm::LastExchange)),
                _ => None,
            }
        }
        GNOME_SORT_METHOD => Some(Method::Gnome),
        ODD_EVEN_SORT_METHOD => {
            let Some(ParameterValue::Identifier(form)) = form else {
                return None;
            };
            match form.as_str() {
                "until_sorted" => Some(Method::OddEven(OddEvenForm::UntilSorted)),
                "fixed_passes" => Some(Method::OddEven(OddEvenForm::FixedPasses)),
                _ => None,
            }
        }
        COMB_SORT_METHOD => {
            let Some(ParameterValue::Identifier(form)) = form else {
                return None;
            };
            match form.as_str() {
                "lacey_box" => Some(Method::Comb(CombForm::LaceyBox)),
                _ => None,
            }
        }
        CYCLE_SORT_METHOD => Some(Method::Cycle),
        PANCAKE_SORT_METHOD => Some(Method::Pancake),
        BUBBLE_SORT_METHOD => {
            let Some(ParameterValue::Identifier(form)) = form else {
                return None;
            };
            BUBBLE_FORMS
                .iter()
                .find(|(_, _, name)| name == form)
                .map(|(form, _, _)| Method::Bubble(*form))
        }
        _ => None,
    }
}

fn whole_parameter(value: &ParameterValue) -> Option<u64> {
    let ParameterValue::Value(calc_core::ResultValue::Number(Number::Integer(whole))) = value
    else {
        return None;
    };
    u64::try_from(whole.to_i128()?).ok()
}

fn listed_length(value: &calc_core::ResultValue) -> usize {
    match value {
        calc_core::ResultValue::Array { shape, .. } => shape.first().copied().unwrap_or(0),
        _ => 0,
    }
}

fn philox_seed(computed: &ComputedResult) -> Option<u64> {
    computed
        .seed()
        .filter(|seed| seed.generator == calc_core::PHILOX_GENERATOR)
        .map(|seed| seed.value)
}

pub(crate) fn sort_summary(computed: &ComputedResult) -> Option<SortSummary> {
    let sort = computed.sort()?;
    let method = sort_method_of(
        &computed.method().name,
        computed
            .method()
            .parameters
            .get(FORM_PARAMETER)
            .or_else(|| computed.method().parameters.get(GAPS_PARAMETER)),
        computed.method().parameters.get(PARTITION_PARAMETER),
        philox_seed(computed),
        computed.method().parameters.get(BASE_PARAMETER),
        computed.method().parameters.get(LIMIT_PARAMETER),
    )?;
    let is_decreasing = matches!(
        computed.method().parameters.get(ORDER_PARAMETER),
        Some(ParameterValue::Identifier(order)) if order == DECREASING_ORDER
    );
    let length = u64::try_from(match &sort.from_positions {
        Some(positions) => positions.len(),
        None => listed_length(computed.value()),
    })
    .unwrap_or(u64::MAX);
    let costs = calc_sort::costs(method)
        .into_iter()
        .filter_map(|cost| {
            let value = match cost.value_at(length) {
                Some(value) => Some(number_text(&value.ok()?)),
                None => None,
            };
            Some(CostSummary {
                measure: cost.measure,
                case: cost.case,
                formula: cost.formula.as_ref().map(formula_text),
                value,
                holds_from: cost.holds_from,
                provenance: cost.provenance,
                over: cost.over,
                checked_up_to: cost.checked_up_to,
            })
        })
        .collect();
    let positions: Option<Vec<String>> = sort
        .from_positions
        .as_ref()
        .map(|positions| positions.iter().map(u64::to_string).collect());
    Some(SortSummary {
        method,
        is_decreasing,
        is_stable: calc_sort::is_stable(method),
        source: calc_sort::method_source(method),
        length: length.to_string(),
        from_positions: positions.map(|positions| format!("[{}]", positions.join(ENTRY_SEPARATOR))),
        comparisons: sort.comparisons.to_string(),
        writes: sort.writes.to_string(),
        key_evaluations: sort.key_evaluations,
        tallies: sort.tallies.map(|count| count.to_string()),
        key_range: sort.key_range,
        draws: sort.draws,
        seed: philox_seed(computed),
        flips: sort.flips,
        gaps: match method {
            Method::Shell(gaps) => {
                let used: Vec<String> =
                    calc_sort::gaps_for(gaps, usize::try_from(length).unwrap_or(usize::MAX))
                        .iter()
                        .map(usize::to_string)
                        .collect();
                Some(used.join(ENTRY_SEPARATOR))
            }
            _ => None,
        },
        costs,
        passes: match (method, sort.tallies) {
            (Method::Radix { base }, Some(tallies)) => {
                let per_pass = 2 * length + base - 1;
                Some(tallies / per_pass)
            }
            _ => None,
        },
        base: match method {
            Method::Radix { base } => Some(base),
            _ => None,
        },
        shuffles: match method {
            Method::Bogo { .. } => Some(
                sort.writes
                    .checked_div(2 * length.saturating_sub(1))
                    .unwrap_or(0),
            ),
            _ => None,
        },
        limit: match method {
            Method::Bogo { limit, .. } => Some(limit),
            _ => None,
        },
        expected_shuffles: (matches!(method, Method::Bogo { .. }) && length > 1).then(|| {
            let factorial = (1..=length).fold(Integer::from(1_u64), |product, factor| {
                &product * &Integer::from(factor)
            });
            number_text(&Number::Integer(factorial))
        }),
        lower_bound: (!matches!(
            method,
            Method::Counting | Method::Radix { .. } | Method::Bead
        ))
        .then(|| number_text(&Number::Integer(calc_sort::comparison_lower_bound(length)))),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TracePlace {
    Position(u64),
    Held { taken_from: u64 },
    Scratch(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TracedEntry {
    pub value: String,
    pub key: Option<String>,
    pub place: TracePlace,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceStep {
    Compare {
        left: TracedEntry,
        right: TracedEntry,
        outcome: Option<Ordering>,
    },
    Write {
        value: String,
        from: TracePlace,
        to: TracePlace,
    },
    Exchange {
        left: TracedEntry,
        right: TracedEntry,
    },
    Tally {
        value: String,
        counter: u64,
    },
    Draw {
        from: u64,
        to: u64,
        chosen: u64,
        draws: u64,
    },
    PrefixSum {
        from: u64,
        to: u64,
    },
    Decrement {
        counter: u64,
    },
    Flip {
        length: u64,
    },
    Pass {
        gap: u64,
    },
    DigitPass {
        place: u64,
        least: String,
    },
    DigitTally {
        value: String,
        digit: u64,
        counter: u64,
    },
    BeadFalls {
        value: String,
        pole: u64,
    },
    BeadRead {
        pole: u64,
        row: u64,
    },
    Rebuild {
        row: u64,
        position: u64,
        beads: u64,
    },
    BitonicMerge {
        block: u64,
        distance: u64,
    },
    Shuffle {
        number: u64,
    },
    ShuffleDraw {
        position: u64,
        chosen: u64,
        draws: u64,
    },
}

fn counted_from_one(position: usize) -> u64 {
    u64::try_from(position).map_or(u64::MAX, |position| position + 1)
}

fn trace_place(place: Place) -> TracePlace {
    match place {
        Place::List(position) => TracePlace::Position(counted_from_one(position)),
        Place::Held { taken_from } => TracePlace::Held {
            taken_from: counted_from_one(taken_from),
        },
        Place::Scratch(slot) => TracePlace::Scratch(counted_from_one(slot)),
    }
}

pub(crate) fn trace_steps(
    report: &SortReport,
    values: &[String],
    keys: Option<&[String]>,
) -> Vec<TraceStep> {
    let traced = |entry: usize, place: Place| TracedEntry {
        value: values[entry].clone(),
        key: keys.map(|keys| keys[entry].clone()),
        place: trace_place(place),
    };
    report
        .steps
        .iter()
        .map(|step| match *step {
            Step::Compare {
                left,
                right,
                outcome,
            } => TraceStep::Compare {
                left: traced(left.entry, left.place),
                right: traced(right.entry, right.place),
                outcome,
            },
            Step::Write { entry, from, to } => TraceStep::Write {
                value: values[entry].clone(),
                from: trace_place(from),
                to: trace_place(to),
            },
            Step::Tally { entry, counter } => TraceStep::Tally {
                value: values[entry].clone(),
                counter: counted_from_one(counter),
            },
            Step::Draw {
                from,
                to,
                chosen,
                draws,
            } => TraceStep::Draw {
                from: counted_from_one(from),
                to: counted_from_one(to),
                chosen: counted_from_one(chosen),
                draws,
            },
            Step::DigitPass { place, least_entry } => TraceStep::DigitPass {
                place: counted_from_one(place),
                least: keys
                    .map_or(&values[least_entry], |keys| &keys[least_entry])
                    .clone(),
            },
            Step::DigitTally {
                entry,
                digit,
                counter,
            } => TraceStep::DigitTally {
                value: values[entry].clone(),
                digit: u64::try_from(digit).unwrap_or(u64::MAX),
                counter: counted_from_one(counter),
            },
            Step::BeadFalls { entry, pole } => TraceStep::BeadFalls {
                value: values[entry].clone(),
                pole: counted_from_one(pole),
            },
            Step::BeadRead { pole, row } => TraceStep::BeadRead {
                pole: counted_from_one(pole),
                row: counted_from_one(row),
            },
            Step::Rebuild {
                row,
                position,
                beads,
            } => TraceStep::Rebuild {
                row: counted_from_one(row),
                position: counted_from_one(position),
                beads: u64::try_from(beads).unwrap_or(u64::MAX),
            },
            Step::BitonicMerge { block, distance } => TraceStep::BitonicMerge {
                block: u64::try_from(block).unwrap_or(u64::MAX),
                distance: u64::try_from(distance).unwrap_or(u64::MAX),
            },
            Step::Shuffle { number } => TraceStep::Shuffle { number },
            Step::ShuffleDraw {
                position,
                chosen,
                draws,
            } => TraceStep::ShuffleDraw {
                position: counted_from_one(position),
                chosen: counted_from_one(chosen),
                draws,
            },
            Step::Pass { gap } => TraceStep::Pass {
                gap: u64::try_from(gap).unwrap_or(u64::MAX),
            },
            Step::Flip { length } => TraceStep::Flip {
                length: u64::try_from(length).unwrap_or(u64::MAX),
            },
            Step::Decrement { counter } => TraceStep::Decrement {
                counter: counted_from_one(counter),
            },
            Step::PrefixSum { counter } => TraceStep::PrefixSum {
                from: counted_from_one(counter - 1),
                to: counted_from_one(counter),
            },
            Step::Exchange { left, right } => TraceStep::Exchange {
                left: traced(left.entry, left.place),
                right: traced(right.entry, right.place),
            },
        })
        .collect()
}

fn number_text(number: &Number) -> String {
    crate::summary::value_text_in(
        &calc_core::ResultValue::Number(number.clone()),
        calc_core::RationalForm::Fraction,
    )
}

fn is_compound(formula: &Formula) -> bool {
    matches!(formula, Formula::Sum(_) | Formula::Difference(..))
}

fn grouped_power(formula: &Formula) -> String {
    match formula {
        Formula::Length
        | Formula::Whole(_)
        | Formula::Index
        | Formula::FloorLog2(_)
        | Formula::CeilLog2(_) => formula_text(formula),
        other => format!("({})", formula_text(other)),
    }
}

fn grouped(formula: &Formula) -> String {
    if is_compound(formula) {
        format!("({})", formula_text(formula))
    } else {
        formula_text(formula)
    }
}

pub(crate) fn formula_text(formula: &Formula) -> String {
    match formula {
        Formula::Length => LENGTH_NAME.to_string(),
        Formula::Whole(whole) => whole.to_string(),
        Formula::Harmonic => HARMONIC_TEXT.to_string(),
        Formula::Index => INDEX_NAME.to_string(),
        Formula::SumOver { from, body } => {
            format!(
                "sum({}, {INDEX_NAME}, {from}, {LENGTH_NAME})",
                formula_text(body)
            )
        }
        Formula::FloorLog2(argument) => format!("floor(log2({}))", formula_text(argument)),
        Formula::CeilLog2(argument) => format!("ceil(log2({}))", formula_text(argument)),
        Formula::PowerOfTwo(argument) => format!("2^{}", grouped_power(argument)),
        Formula::BitCount(argument) => format!("popcount({})", formula_text(argument)),
        Formula::MergeAverage => MERGE_AVERAGE_TEXT.to_string(),
        Formula::Floor(argument) => format!("floor({})", formula_text(argument)),
        Formula::GapSum(_) => GAP_SUM_TEXT.to_string(),
        Formula::Sum(terms) => terms
            .iter()
            .map(formula_text)
            .collect::<Vec<_>>()
            .join(" + "),
        Formula::Difference(left, right) => {
            format!("{} - {}", formula_text(left), grouped(right))
        }
        Formula::Product(factors) => factors.iter().map(grouped).collect::<Vec<_>>().join("*"),
        Formula::Quotient(numerator, denominator) => {
            let numerator = match numerator.as_ref() {
                Formula::Product(_) => formula_text(numerator),
                other => grouped(other),
            };
            let denominator = match denominator.as_ref() {
                Formula::Length | Formula::Whole(_) | Formula::Index => formula_text(denominator),
                other => format!("({})", formula_text(other)),
            };
            format!("{numerator}/{denominator}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insertion_costs_print_as_calc_input() {
        let texts: Vec<String> = calc_sort::costs(Method::Insertion)
            .iter()
            .map(|cost| formula_text(cost.formula.as_ref().unwrap()))
            .collect();

        assert_eq!(
            texts,
            vec![
                "n - 1",
                "n*(n - 1)/2",
                "n*(n - 1)/4 + n - sum(1/k, k, 1, n)",
                "n - 1",
                "n*(n - 1)/2 + n - 1",
                "n*(n - 1)/4 + n - 1",
            ]
        );
    }
}
