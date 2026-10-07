use std::cmp::Ordering;

use calc_expr::{
    BinderKind, ExprId, ExprPool, Head, NodeView, Operator, SortBubbleForm, SortCombForm, SortGaps,
    SortMethod, SortOddEvenForm, SortOrder, SortPartition, SortShakerForm, SortSpec,
};
use calc_numbers::{Integer, Interval, Number};
use calc_sort::{
    BubbleForm, CombForm, Counts, Gaps, Method, OddEvenForm, Order, Partition, Refusal, ShakerForm,
    Step, StepRecording,
};

use crate::array::{ArrayError, ArrayExpression, array_expression, reduce_arrays};
use crate::claim::{Verdict, check_relation};
use crate::exact_evaluation::{ExactEvaluationError, evaluate_exact};
use crate::exact_rational::ExactRational;

pub(crate) struct OrderedEntry {
    pub(crate) value: Option<ExactRational>,
    pub(crate) enclosure: Option<Interval>,
    pub(crate) expression: ExprId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortRefusal {
    LimitReached {
        limit: u64,
        counts: Counts,
    },
    BeadBelowZero(ExprId),
    BeadTakesNoKey,
    LengthNotAPowerOfTwo {
        length: usize,
    },
    KeysDifferInDimension {
        first: usize,
        second: usize,
        first_key: ExprId,
        second_key: ExprId,
        by_key: bool,
    },
    BaseRefused(calc_sort::BaseRefused),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortReport {
    pub spec: SortSpec,
    pub records: Vec<ExprId>,
    pub keys: Option<Vec<ExprId>>,
    pub arrangement: Option<Vec<usize>>,
    pub counts: Counts,
    pub steps: Vec<Step>,
    pub key_range: Option<u64>,
    pub seed: Option<u64>,
    pub base: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SortFailure {
    Array(ArrayError),
    Undecided {
        left: ExprId,
        right: ExprId,
        counts: Counts,
        steps: Vec<Step>,
    },
    RangeTooWide {
        range: Integer,
        limit: u64,
    },
    BaseRefused(calc_sort::BaseRefused),
    BeadTakesNoKey,
    LengthNotAPowerOfTwo {
        length: usize,
    },
    LimitReached {
        limit: u64,
        counts: Counts,
        steps: Vec<Step>,
    },
    KeysDifferInDimension {
        first: usize,
        second: usize,
        first_key: ExprId,
        second_key: ExprId,
        by_key: bool,
    },
    BeadBelowZero(ExprId),
    TooManyBeads {
        beads: Integer,
        limit: u64,
    },
}

impl SortFailure {
    pub fn to_array_error(&self) -> ArrayError {
        match self {
            Self::Array(error) => *error,
            Self::Undecided { left, right, .. } => ArrayError::NotComparable(*left, *right),
            Self::RangeTooWide { .. } => ArrayError::RangeTooWide,
            Self::TooManyBeads { .. } => ArrayError::Unsupported,
            Self::BaseRefused(refused) => {
                ArrayError::SortRefused(SortRefusal::BaseRefused(*refused))
            }
            Self::BeadTakesNoKey => ArrayError::SortRefused(SortRefusal::BeadTakesNoKey),
            Self::BeadBelowZero(entry) => {
                ArrayError::SortRefused(SortRefusal::BeadBelowZero(*entry))
            }
            Self::LengthNotAPowerOfTwo { length } => {
                ArrayError::SortRefused(SortRefusal::LengthNotAPowerOfTwo { length: *length })
            }
            Self::LimitReached { limit, counts, .. } => {
                ArrayError::SortRefused(SortRefusal::LimitReached {
                    limit: *limit,
                    counts: *counts,
                })
            }
            Self::KeysDifferInDimension {
                first,
                second,
                first_key,
                second_key,
                by_key,
            } => ArrayError::SortRefused(SortRefusal::KeysDifferInDimension {
                first: *first,
                second: *second,
                first_key: *first_key,
                second_key: *second_key,
                by_key: *by_key,
            }),
        }
    }
}

fn enclosed_order(left: Option<&Interval>, right: Option<&Interval>) -> Option<Ordering> {
    let (left, right) = (left?, right?);
    if left.upper() < right.lower() {
        Some(Ordering::Less)
    } else if right.upper() < left.lower() {
        Some(Ordering::Greater)
    } else {
        None
    }
}

pub(crate) fn proven_order(
    pool: &mut ExprPool,
    left: &OrderedEntry,
    right: &OrderedEntry,
) -> Result<Ordering, ArrayError> {
    if let (Some(left_value), Some(right_value)) = (&left.value, &right.value) {
        return Ok(left_value.compare(right_value));
    }
    if left.expression == right.expression {
        return Ok(Ordering::Equal);
    }
    if let Some(order) = enclosed_order(left.enclosure.as_ref(), right.enclosure.as_ref()) {
        return Ok(order);
    }
    let not_comparable = ArrayError::NotComparable(left.expression, right.expression);
    let relation = |pool: &mut ExprPool, operator| {
        pool.apply(
            Head::Operator(operator),
            &[left.expression, right.expression],
        )
        .map_err(|_| ArrayError::Unsupported)
    };
    let below = relation(pool, Operator::Less)?;
    let above = relation(pool, Operator::Greater)?;
    match (check_relation(pool, below), check_relation(pool, above)) {
        (Some(Verdict::Holds), _) => Ok(Ordering::Less),
        (_, Some(Verdict::Holds)) => Ok(Ordering::Greater),
        (Some(Verdict::Fails), Some(Verdict::Fails)) => Ok(Ordering::Equal),
        _ => Err(not_comparable),
    }
}

pub(crate) fn real_entry(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Result<OrderedEntry, ArrayError> {
    let is_not_real = crate::complex_construction::constructed_complex(pool, expression)
        .is_some_and(|(_, imaginary)| !is_zero(&imaginary))
        || crate::machine_evaluation::is_provably_not_real(pool, expression);
    if is_not_real {
        return Err(ArrayError::NotReal(expression));
    }
    let rational = match evaluate_exact(pool, expression) {
        Err(
            ExactEvaluationError::DivisionByZero(_) | ExactEvaluationError::OutsideDomain { .. },
        ) => {
            return Err(ArrayError::EntryWithoutValue(expression));
        }
        Err(_) => None,
        Ok(evaluation) => evaluation.rational_value().cloned(),
    };
    let enclosure = match &rational {
        Some(number) => Interval::from_exact(number),
        None => crate::machine_evaluation::real_enclosure(pool, expression),
    };
    let value = rational.as_ref().and_then(ExactRational::from_number);
    Ok(OrderedEntry {
        value,
        enclosure,
        expression,
    })
}

fn is_zero(number: &Number) -> bool {
    match number {
        Number::Integer(integer) => integer.is_zero(),
        Number::Rational(rational) => rational.numerator().is_zero(),
        Number::F32(value) => *value == 0.0,
        Number::F64(value) => *value == 0.0,
    }
}

fn library_method(method: SortMethod, seed: Option<u64>, limit: Option<u64>) -> Option<Method> {
    Some(match method {
        SortMethod::Insertion => Method::Insertion,
        SortMethod::BinaryInsertion => Method::BinaryInsertion,
        SortMethod::Selection => Method::Selection,
        SortMethod::Bubble(form) => Method::Bubble(match form {
            SortBubbleForm::Full => BubbleForm::Full,
            SortBubbleForm::Shrinking => BubbleForm::Shrinking,
            SortBubbleForm::EarlyExit => BubbleForm::EarlyExit,
            SortBubbleForm::LastExchange => BubbleForm::LastExchange,
        }),
        SortMethod::Merge => Method::Merge,
        SortMethod::Heap => Method::Heap,
        SortMethod::Quick(SortPartition::LomutoLast) => Method::Quick(Partition::LomutoLast),
        SortMethod::Quick(SortPartition::HoareFirst) => Method::Quick(Partition::HoareFirst),
        SortMethod::Quick(SortPartition::LomutoRandom) => {
            Method::Quick(Partition::LomutoRandom { seed: seed? })
        }
        SortMethod::Counting => Method::Counting,
        SortMethod::DoubleSelection => Method::DoubleSelection,
        SortMethod::CocktailShaker(form) => Method::CocktailShaker(match form {
            SortShakerForm::Full => ShakerForm::Full,
            SortShakerForm::Shrinking => ShakerForm::Shrinking,
            SortShakerForm::LastExchange => ShakerForm::LastExchange,
        }),
        SortMethod::Gnome => Method::Gnome,
        SortMethod::OddEven(form) => Method::OddEven(match form {
            SortOddEvenForm::UntilSorted => OddEvenForm::UntilSorted,
            SortOddEvenForm::FixedPasses => OddEvenForm::FixedPasses,
        }),
        SortMethod::Comb(form) => Method::Comb(match form {
            SortCombForm::LaceyBox => CombForm::LaceyBox,
        }),
        SortMethod::Cycle => Method::Cycle,
        SortMethod::Pancake => Method::Pancake,
        SortMethod::BottomUpMerge => Method::BottomUpMerge,
        SortMethod::NaturalMerge => Method::NaturalMerge,
        SortMethod::Radix => Method::Radix { base: seed? },
        SortMethod::Bead => Method::Bead,
        SortMethod::Bitonic => Method::Bitonic,
        SortMethod::Bogo => Method::Bogo {
            seed: seed?,
            limit: limit?,
        },
        SortMethod::Shell(gaps) => Method::Shell(match gaps {
            SortGaps::Shell => Gaps::Shell,
            SortGaps::Knuth => Gaps::Knuth,
            SortGaps::Ciura => Gaps::Ciura,
        }),
    })
}

fn library_order(order: SortOrder) -> Order {
    match order {
        SortOrder::Increasing => Order::Increasing,
        SortOrder::Decreasing => Order::Decreasing,
    }
}

fn records_of(pool: &mut ExprPool, array: &ArrayExpression) -> Result<Vec<ExprId>, ArrayError> {
    match array.shape.as_slice() {
        [_] => Ok(array.elements.clone()),
        [_, columns] => {
            let width = usize::try_from(*columns).map_err(|_| ArrayError::Unsupported)?;
            array
                .elements
                .chunks(width.max(1))
                .map(|row| {
                    pool.array(&[*columns], row)
                        .map_err(|_| ArrayError::Unsupported)
                })
                .collect()
        }
        _ => Err(ArrayError::NotAList),
    }
}

fn elements_in(
    pool: &ExprPool,
    array: &ArrayExpression,
    records: &[ExprId],
    arrangement: &[usize],
) -> Result<Vec<ExprId>, ArrayError> {
    if array.shape.len() == 1 {
        return Ok(arrangement.iter().map(|&index| records[index]).collect());
    }
    let mut elements = Vec::with_capacity(array.elements.len());
    for &index in arrangement {
        match pool.node(records[index]) {
            Ok(NodeView::Array { elements: row, .. }) => elements.extend_from_slice(row),
            _ => return Err(ArrayError::Unsupported),
        }
    }
    Ok(elements)
}

fn is_identity(pool: &ExprPool, body: ExprId) -> bool {
    matches!(pool.node(body), Ok(NodeView::Bound(0)))
}

pub fn named_sort(
    pool: &mut ExprPool,
    expression: ExprId,
    recording: StepRecording,
) -> Option<Result<(ArrayExpression, SortReport), SortFailure>> {
    let NodeView::Bind {
        binder: BinderKind::Sort(spec),
        arguments,
        body,
    } = pool.node(expression).ok()?
    else {
        return None;
    };
    let (list, seed, limit) = match arguments {
        [list] => (*list, None, None),
        [list, seed] => (*list, Some(*seed), None),
        [list, seed, limit] => (*list, Some(*seed), Some(*limit)),
        _ => return None,
    };
    let limit = match limit {
        None => None,
        Some(limit) => match whole_seed(pool, limit) {
            Some(limit) => Some(limit),
            None => return Some(Err(SortFailure::Array(ArrayError::Unsupported))),
        },
    };
    let seed = match seed {
        None => None,
        Some(seed) => match whole_seed(pool, seed) {
            Some(seed) => Some(seed),
            None => return Some(Err(SortFailure::Array(ArrayError::Unsupported))),
        },
    };
    Some(sorted_by(pool, spec, (seed, limit), list, body, recording))
}

fn whole_seed(pool: &mut ExprPool, seed: ExprId) -> Option<u64> {
    let evaluation = evaluate_exact(pool, seed).ok()?;
    match evaluation.rational_value()? {
        Number::Integer(integer) => u64::try_from(integer.to_i128()?).ok(),
        _ => None,
    }
}

fn sorted_by(
    pool: &mut ExprPool,
    spec: SortSpec,
    (seed, limit): (Option<u64>, Option<u64>),
    list: ExprId,
    body: ExprId,
    recording: StepRecording,
) -> Result<(ArrayExpression, SortReport), SortFailure> {
    let array = match array_expression(pool, list) {
        Some(array) => array.map_err(SortFailure::Array)?,
        None => return Err(SortFailure::Array(ArrayError::NotAList)),
    };
    let records = records_of(pool, &array).map_err(SortFailure::Array)?;
    let keys = if is_identity(pool, body) {
        if array.shape.len() > 1 {
            return Err(SortFailure::Array(ArrayError::RecordsNeedKey));
        }
        None
    } else {
        let mut keys = Vec::with_capacity(records.len());
        for record in &records {
            let key = crate::rule_search::replace_parameters(pool, body, &[*record], 0)
                .ok_or(SortFailure::Array(ArrayError::Unsupported))?;
            keys.push(reduce_arrays(pool, key).map_err(SortFailure::Array)?);
        }
        Some(keys)
    };
    let compared = keys.as_deref().unwrap_or(&records).to_vec();
    let entries: Vec<OrderedEntry> = compared
        .iter()
        .map(|key| real_entry(pool, *key))
        .collect::<Result<_, _>>()
        .map_err(SortFailure::Array)?;
    let is_radix = spec.method == SortMethod::Radix;
    if spec.method == SortMethod::Bead {
        return bead_sorted(pool, spec, &array, records, keys, &entries, recording);
    }
    let (sorted, key_range) = if is_radix {
        let whole =
            whole_keys(pool, &entries, keys.is_some(), spec.method).map_err(SortFailure::Array)?;
        let base = seed.ok_or(SortFailure::Array(ArrayError::Unsupported))?;
        let radix =
            calc_sort::radix_sort_whole_numbers(&whole, base, library_order(spec.order), recording)
                .map_err(SortFailure::BaseRefused)?;
        (radix.sorted, None)
    } else if spec.method == SortMethod::Counting {
        let whole =
            whole_keys(pool, &entries, keys.is_some(), spec.method).map_err(SortFailure::Array)?;
        let key_range = whole_range(&whole);
        let sorted = calc_sort::sort_whole_numbers(&whole, library_order(spec.order), recording)
            .map_err(|wide| SortFailure::RangeTooWide {
                range: wide.range,
                limit: wide.limit,
            })?;
        (sorted, key_range)
    } else {
        if let Some((first, second)) = first_dimension_clash(pool, &entries) {
            return Err(SortFailure::KeysDifferInDimension {
                first,
                second,
                first_key: compared[first],
                second_key: compared[second],
                by_key: keys.is_some(),
            });
        }
        let outcome = calc_sort::sort(
            &entries,
            library_method(spec.method, seed, limit)
                .ok_or(SortFailure::Array(ArrayError::Unsupported))?,
            library_order(spec.order),
            |left, right| proven_order(pool, left, right),
            recording,
        );
        let sorted = outcome.map_err(|refusal| match refusal {
            Refusal::Stopped(stopped) => match stopped.error {
                ArrayError::NotComparable(..) => SortFailure::Undecided {
                    left: compared[stopped.left.min(stopped.right)],
                    right: compared[stopped.left.max(stopped.right)],
                    counts: stopped.counts,
                    steps: stopped.steps,
                },
                other => SortFailure::Array(other),
            },
            Refusal::WholeNumbersNeeded => SortFailure::Array(ArrayError::Unsupported),
            Refusal::LimitReached { counts, steps } => SortFailure::LimitReached {
                limit: limit.unwrap_or(0),
                counts,
                steps,
            },
            Refusal::LengthNotAPowerOfTwo { length } => {
                SortFailure::LengthNotAPowerOfTwo { length }
            }
        })?;
        (sorted, None)
    };
    let elements =
        elements_in(pool, &array, &records, &sorted.arrangement).map_err(SortFailure::Array)?;
    Ok((
        ArrayExpression {
            shape: array.shape.clone(),
            elements,
        },
        SortReport {
            spec,
            records,
            keys,
            arrangement: Some(sorted.arrangement),
            counts: sorted.counts,
            steps: sorted.steps,
            key_range,
            seed: seed.filter(|_| !is_radix),
            base: seed.filter(|_| is_radix),
            limit,
        },
    ))
}

fn bead_sorted(
    pool: &mut ExprPool,
    spec: SortSpec,
    array: &ArrayExpression,
    records: Vec<ExprId>,
    keys: Option<Vec<ExprId>>,
    entries: &[OrderedEntry],
    recording: StepRecording,
) -> Result<(ArrayExpression, SortReport), SortFailure> {
    if keys.is_some() {
        return Err(SortFailure::BeadTakesNoKey);
    }
    let whole = whole_keys(pool, entries, false, spec.method).map_err(SortFailure::Array)?;
    let beads = calc_sort::bead_sort_whole_numbers(&whole, library_order(spec.order), recording)
        .map_err(|refused| match refused {
            calc_sort::BeadRefused::Negative { entry } => {
                SortFailure::BeadBelowZero(entries[entry].expression)
            }
            calc_sort::BeadRefused::TooManyBeads { beads, limit } => {
                SortFailure::TooManyBeads { beads, limit }
            }
        })?;
    let elements = beads
        .values
        .iter()
        .map(|value| pool.number(Number::Integer(Integer::from(*value))))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| SortFailure::Array(ArrayError::Unsupported))?;
    Ok((
        ArrayExpression {
            shape: array.shape.clone(),
            elements,
        },
        SortReport {
            spec,
            records,
            keys,
            arrangement: None,
            counts: beads.counts,
            steps: beads.steps,
            key_range: None,
            seed: None,
            base: None,
            limit: None,
        },
    ))
}

pub(crate) fn first_dimension_clash(
    pool: &mut ExprPool,
    entries: &[OrderedEntry],
) -> Option<(usize, usize)> {
    let mut dimensions = entries.iter().map(|entry| {
        evaluate_exact(pool, entry.expression)
            .ok()
            .map(|evaluation| {
                evaluation
                    .unit()
                    .and_then(|unit| pool.units().dimension(unit).ok())
            })
    });
    let first = dimensions.next()??;
    dimensions
        .position(|dimension| dimension.is_some_and(|dimension| dimension != first))
        .map(|offset| (0, offset + 1))
}

fn whole_keys(
    pool: &mut ExprPool,
    entries: &[OrderedEntry],
    from_function: bool,
    method: SortMethod,
) -> Result<Vec<Integer>, ArrayError> {
    entries
        .iter()
        .map(|entry| {
            let carries_unit = crate::quantities::to_coherent_units(pool, entry.expression)
                .is_ok_and(|coherent| coherent.unit.is_some())
                || is_written_in_a_unit(pool, entry.expression);
            match &entry.value {
                _ if carries_unit => Err(ArrayError::KeyWithUnit(entry.expression, method)),
                Some(value) if value.is_integer() => Ok(value.numerator().clone()),
                _ if from_function => {
                    Err(ArrayError::FunctionKeyNotWhole(entry.expression, method))
                }
                _ => Err(ArrayError::KeyNotWhole(entry.expression, method)),
            }
        })
        .collect()
}

pub(crate) fn is_written_in_a_unit(pool: &ExprPool, expression: ExprId) -> bool {
    dimensionless_unit_power(pool, expression) != Some(0)
}

fn dimensionless_unit_power(pool: &ExprPool, expression: ExprId) -> Option<i64> {
    let power_of = |argument: &ExprId| dimensionless_unit_power(pool, *argument);
    match pool.node(expression).ok()? {
        NodeView::Quantity { value, unit } => {
            let own = i64::from(pool.units().dimension(unit).ok()?.is_dimensionless());
            Some(power_of(&value)? + own)
        }
        NodeView::Apply {
            head: Head::Operator(operator),
            arguments,
        } => {
            let powers = arguments.iter().map(power_of).collect::<Option<Vec<_>>>()?;
            match (operator, powers.as_slice()) {
                (Operator::Percent, [single]) => Some(single + 1),
                (Operator::Neg, [single]) => Some(*single),
                (Operator::Mul, _) => Some(powers.iter().sum()),
                (Operator::Div, [numerator, denominator]) => Some(numerator - denominator),
                (Operator::Add | Operator::Sub, [first, rest @ ..]) => {
                    rest.iter().all(|power| power == first).then_some(*first)
                }
                (Operator::Pow, [0, _]) => Some(0),
                (Operator::Pow, [base, 0]) => base.checked_mul(whole_exponent(pool, arguments[1])?),
                _ => powers.iter().all(|power| *power == 0).then_some(0),
            }
        }
        _ => Some(0),
    }
}

fn whole_exponent(pool: &ExprPool, expression: ExprId) -> Option<i64> {
    match pool.node(expression).ok()? {
        NodeView::Number(number) => match pool.number_value(number).ok()? {
            Number::Integer(integer) => integer.to_i64(),
            _ => None,
        },
        NodeView::Apply {
            head: Head::Operator(Operator::Neg),
            arguments: [single],
        } => whole_exponent(pool, *single)?.checked_neg(),
        _ => None,
    }
}

fn whole_range(keys: &[Integer]) -> Option<u64> {
    let (least, greatest) = (keys.iter().min()?, keys.iter().max()?);
    let range = &(greatest - least) + &Integer::one();
    range.to_i128().and_then(|range| u64::try_from(range).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn around(value: f64) -> Interval {
        Interval::point(value)
            .unwrap()
            .hull(&Interval::point(value.next_up()).unwrap())
    }

    #[test]
    fn disjoint_enclosures_order_their_values() {
        assert_eq!(
            enclosed_order(Some(&around(1.0)), Some(&around(2.0))),
            Some(Ordering::Less)
        );
        assert_eq!(
            enclosed_order(Some(&around(2.0)), Some(&around(1.0))),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn overlapping_enclosures_decide_nothing() {
        assert_eq!(enclosed_order(Some(&around(1.0)), Some(&around(1.0))), None);
    }

    #[test]
    fn a_missing_enclosure_decides_nothing() {
        assert_eq!(enclosed_order(None, Some(&around(1.0))), None);
    }
}
