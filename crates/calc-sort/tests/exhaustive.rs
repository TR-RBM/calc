use std::cmp::Ordering;
use std::convert::Infallible;

use calc_numbers::{Integer, Number};
use calc_sort::{
    BubbleForm, COUNTING_RANGE_LIMIT, Case, CombForm, Cost, Counts, EXHAUSTIVE_CHECK_LIMIT,
    Formula, Gaps, Measure, Method, OddEvenForm, Order, Partition, ShakerForm, StepRecording,
    costs, is_stable, sort, sort_whole_numbers,
};

fn every_order(length: usize) -> Vec<Vec<usize>> {
    if length == 0 {
        return vec![Vec::new()];
    }
    let mut orders = Vec::new();
    for shorter in every_order(length - 1) {
        for position in 0..=shorter.len() {
            let mut order = shorter.clone();
            order.insert(position, length - 1);
            orders.push(order);
        }
    }
    orders
}

fn counted(entries: &[usize], method: Method) -> Counts {
    let compare = |left: &usize, right: &usize| Ok::<Ordering, Infallible>(left.cmp(right));
    let sorted = sort(
        entries,
        method,
        Order::Increasing,
        compare,
        StepRecording::Skip,
    )
    .unwrap();
    let values: Vec<usize> = sorted
        .arrangement
        .iter()
        .map(|&index| entries[index])
        .collect();
    let expected: Vec<usize> = (0..entries.len()).collect();
    assert_eq!(values, expected);
    sorted.counts
}

fn measured(counts: &Counts, measure: Measure) -> u64 {
    match measure {
        Measure::Comparisons => counts.comparisons,
        Measure::Writes => counts.writes,
        Measure::Flips => counts.flips,
    }
}

fn observed(all: &[Counts], measure: Measure, case: Case) -> Number {
    let values: Vec<u64> = all.iter().map(|counts| measured(counts, measure)).collect();
    match case {
        Case::Best => Number::Integer(Integer::from(*values.iter().min().unwrap())),
        Case::Worst => Number::Integer(Integer::from(*values.iter().max().unwrap())),
        Case::Average => Number::fraction(
            &Integer::from(values.iter().sum::<u64>()),
            &Integer::from(u64::try_from(values.len()).unwrap()),
        )
        .unwrap(),
    }
}

fn first_mismatch(method: Method, claimed: &[Cost]) -> Option<(Measure, Case, u64)> {
    for length in 0..=EXHAUSTIVE_CHECK_LIMIT {
        let all: Vec<Counts> = every_order(usize::try_from(length).unwrap())
            .iter()
            .map(|entries| counted(entries, method))
            .collect();
        for cost in claimed {
            let Some(value) = cost.value_at(length) else {
                continue;
            };
            if value.unwrap() != observed(&all, cost.measure, cost.case) {
                return Some((cost.measure, cost.case, length));
            }
        }
    }
    None
}

const METHODS: [Method; 27] = [
    Method::Insertion,
    Method::BinaryInsertion,
    Method::Selection,
    Method::Bubble(BubbleForm::Full),
    Method::Bubble(BubbleForm::Shrinking),
    Method::Bubble(BubbleForm::EarlyExit),
    Method::Bubble(BubbleForm::LastExchange),
    Method::Merge,
    Method::Heap,
    Method::Quick(Partition::LomutoLast),
    Method::Quick(Partition::HoareFirst),
    Method::DoubleSelection,
    Method::CocktailShaker(ShakerForm::Full),
    Method::CocktailShaker(ShakerForm::Shrinking),
    Method::CocktailShaker(ShakerForm::LastExchange),
    Method::Gnome,
    Method::OddEven(OddEvenForm::UntilSorted),
    Method::OddEven(OddEvenForm::FixedPasses),
    Method::Comb(CombForm::LaceyBox),
    Method::Cycle,
    Method::Pancake,
    Method::Shell(Gaps::Shell),
    Method::Shell(Gaps::Knuth),
    Method::Shell(Gaps::Ciura),
    Method::BottomUpMerge,
    Method::NaturalMerge,
    Method::Counting,
];

#[test]
fn every_method_states_the_three_cases_of_each_measure_it_counts() {
    for method in METHODS {
        let stated = costs(method);
        let expected = if method == Method::Pancake { 9 } else { 6 };
        assert_eq!(stated.len(), expected, "{method:?}");
    }
}

#[test]
fn every_formula_of_every_method_matches_every_order_up_to_the_check_limit() {
    for method in METHODS
        .into_iter()
        .filter(|method| *method != Method::Counting)
    {
        assert_eq!(first_mismatch(method, &costs(method)), None, "{method:?}");
    }
}

#[test]
fn every_method_that_calls_itself_stable_keeps_equal_keys_in_input_order() {
    for method in METHODS
        .into_iter()
        .filter(|method| *method != Method::Counting)
    {
        for keys in every_key_sequence(5, 3) {
            let sorted = sort(
                &keys,
                method,
                Order::Increasing,
                |left: &usize, right: &usize| Ok::<Ordering, Infallible>(left.cmp(right)),
                StepRecording::Skip,
            )
            .unwrap();
            let values: Vec<usize> = sorted
                .arrangement
                .iter()
                .map(|&index| keys[index])
                .collect();
            let mut expected = keys.clone();
            expected.sort_unstable();
            assert_eq!(values, expected, "{method:?} {keys:?}");
            let keeps_order = sorted
                .arrangement
                .windows(2)
                .all(|pair| keys[pair[0]] != keys[pair[1]] || pair[0] < pair[1]);
            if is_stable(method) {
                assert!(keeps_order, "{method:?} {keys:?}");
            }
        }
    }
}

#[test]
fn selection_sort_is_shown_not_to_keep_equal_keys_in_order() {
    let keys = vec![1_usize, 1, 0];

    let sorted = sort(
        &keys,
        Method::Selection,
        Order::Increasing,
        |left: &usize, right: &usize| Ok::<Ordering, Infallible>(left.cmp(right)),
        StepRecording::Skip,
    )
    .unwrap();

    assert_eq!(sorted.arrangement, vec![2, 1, 0]);
    assert!(!is_stable(Method::Selection));
}

fn every_key_sequence(length: usize, values: usize) -> Vec<Vec<usize>> {
    let mut sequences = vec![Vec::new()];
    for _ in 0..length {
        sequences = sequences
            .into_iter()
            .flat_map(|sequence| {
                (0..values).map(move |value| {
                    let mut longer = sequence.clone();
                    longer.push(value);
                    longer
                })
            })
            .collect();
    }
    sequences
}

#[test]
fn insertion_formulas_match_every_order_up_to_the_check_limit() {
    assert_eq!(
        first_mismatch(Method::Insertion, &costs(Method::Insertion)),
        None
    );
}

#[test]
fn a_formula_that_is_wrong_by_one_is_noticed() {
    let mut claimed = costs(Method::Insertion);
    let worst = claimed
        .iter_mut()
        .find(|cost| cost.measure == Measure::Comparisons && cost.case == Case::Worst)
        .unwrap();
    worst.formula = Some(Formula::Sum(vec![
        worst.formula.clone().unwrap(),
        Formula::Whole(1),
    ]));

    assert_eq!(
        first_mismatch(Method::Insertion, &claimed),
        Some((Measure::Comparisons, Case::Worst, 0))
    );
}

fn counted_whole(entries: &[usize], order: Order) -> Counts {
    let keys: Vec<calc_numbers::Integer> = entries
        .iter()
        .map(|&entry| calc_numbers::Integer::from(u64::try_from(entry).unwrap()))
        .collect();
    let sorted = sort_whole_numbers(&keys, order, StepRecording::Skip).unwrap();
    let values: Vec<usize> = sorted
        .arrangement
        .iter()
        .map(|&index| entries[index])
        .collect();
    let mut expected: Vec<usize> = (0..entries.len()).collect();
    if order == Order::Decreasing {
        expected.reverse();
    }
    assert_eq!(values, expected);
    sorted.counts
}

#[test]
fn counting_sort_formulas_match_every_order_up_to_the_check_limit() {
    for length in 0..=EXHAUSTIVE_CHECK_LIMIT {
        let all: Vec<Counts> = every_order(usize::try_from(length).unwrap())
            .iter()
            .map(|entries| counted_whole(entries, Order::Increasing))
            .collect();
        for cost in costs(Method::Counting) {
            let value = cost.value_at(length).unwrap().unwrap();
            assert_eq!(
                value,
                observed(&all, cost.measure, cost.case),
                "{cost:?} at {length}"
            );
        }
    }
}

#[test]
fn counting_sort_keeps_equal_keys_in_input_order_both_ways() {
    for order in [Order::Increasing, Order::Decreasing] {
        for keys in every_key_sequence(5, 3) {
            let whole: Vec<calc_numbers::Integer> = keys
                .iter()
                .map(|&key| calc_numbers::Integer::from(u64::try_from(key).unwrap()))
                .collect();
            let sorted = sort_whole_numbers(&whole, order, StepRecording::Skip).unwrap();
            let keeps_order = sorted
                .arrangement
                .windows(2)
                .all(|pair| keys[pair[0]] != keys[pair[1]] || pair[0] < pair[1]);
            assert!(keeps_order, "{order:?} {keys:?}");
        }
    }
}

#[test]
fn counting_sort_counts_its_tallies_and_prefix_sums() {
    let keys: Vec<calc_numbers::Integer> = [3_i64, 1, 2, 1]
        .into_iter()
        .map(calc_numbers::Integer::from)
        .collect();

    let sorted = sort_whole_numbers(&keys, Order::Increasing, StepRecording::Skip).unwrap();

    assert_eq!(sorted.arrangement, vec![1, 3, 2, 0]);
    assert_eq!(sorted.counts.tallies, 4 + 2 + 4);
    assert_eq!(sorted.counts.writes, 8);
}

#[test]
fn a_range_wider_than_the_limit_is_refused_with_its_width() {
    let limit = i64::try_from(COUNTING_RANGE_LIMIT).unwrap();
    let keys: Vec<calc_numbers::Integer> = [0_i64, limit]
        .into_iter()
        .map(calc_numbers::Integer::from)
        .collect();

    let refused = sort_whole_numbers(&keys, Order::Increasing, StepRecording::Skip).unwrap_err();

    assert_eq!(refused.range, calc_numbers::Integer::from(limit + 1));
}

#[test]
fn each_gap_sequence_names_the_gaps_below_the_length_largest_first() {
    assert_eq!(calc_sort::gaps_for(Gaps::Shell, 11), vec![5, 2, 1]);
    assert_eq!(calc_sort::gaps_for(Gaps::Knuth, 14), vec![4, 1]);
    assert_eq!(calc_sort::gaps_for(Gaps::Knuth, 40), vec![13, 4, 1]);
    assert_eq!(calc_sort::gaps_for(Gaps::Ciura, 24), vec![23, 10, 4, 1]);
    assert_eq!(calc_sort::gaps_for(Gaps::Ciura, 1), Vec::<usize>::new());
}

fn radix_counts(keys: &[u64], base: u64, order: Order) -> calc_sort::RadixSorted {
    let whole: Vec<Integer> = keys.iter().map(|key| Integer::from(*key)).collect();
    calc_sort::radix_sort_whole_numbers(&whole, base, order, StepRecording::Skip).unwrap()
}

#[test]
fn radix_sort_writes_two_per_entry_per_pass_on_every_order() {
    for base in [2_u64, 3, 10] {
        for length in 0..=6_usize {
            for entries in every_order(length) {
                let keys: Vec<u64> = entries.iter().map(|entry| 7 * *entry as u64 + 3).collect();
                let sorted = radix_counts(&keys, base, Order::Increasing);
                let values: Vec<u64> = sorted
                    .sorted
                    .arrangement
                    .iter()
                    .map(|&index| keys[index])
                    .collect();
                let mut expected = keys.clone();
                expected.sort_unstable();
                assert_eq!(values, expected, "{base} {keys:?}");
                let length = length as u64;
                assert_eq!(sorted.sorted.counts.writes, 2 * length * sorted.passes);
                assert_eq!(sorted.sorted.counts.comparisons, 0);
                assert_eq!(
                    sorted.sorted.counts.tallies,
                    sorted.passes * (2 * length + base - 1)
                );
            }
        }
    }
}

#[test]
fn radix_sort_makes_one_pass_per_digit_of_the_key_range() {
    assert_eq!(
        radix_counts(&[170, 45, 75, 90, 802, 24, 2, 66], 10, Order::Increasing).passes,
        3
    );
    assert_eq!(radix_counts(&[5, 5, 5], 10, Order::Increasing).passes, 0);
    assert_eq!(radix_counts(&[1000, 1001], 10, Order::Increasing).passes, 1);
    assert_eq!(radix_counts(&[0, 8], 2, Order::Increasing).passes, 4);
}

#[test]
fn radix_sort_keeps_equal_keys_in_input_order_both_ways() {
    for order in [Order::Increasing, Order::Decreasing] {
        for keys in every_key_sequence(5, 3) {
            let keys: Vec<u64> = keys.iter().map(|key| *key as u64 * 11).collect();
            let sorted = radix_counts(&keys, 3, order);
            let arrangement = &sorted.sorted.arrangement;
            let values: Vec<u64> = arrangement.iter().map(|&index| keys[index]).collect();
            let mut expected = keys.clone();
            expected.sort_unstable();
            if order == Order::Decreasing {
                expected.reverse();
            }
            assert_eq!(values, expected);
            assert!(
                arrangement
                    .windows(2)
                    .all(|pair| keys[pair[0]] != keys[pair[1]] || pair[0] < pair[1])
            );
        }
    }
}

#[test]
fn a_base_below_two_or_above_the_limit_is_refused() {
    let keys = [Integer::from(1_u64)];
    for (base, refused) in [
        (0, calc_sort::BaseRefused::TooSmall),
        (1, calc_sort::BaseRefused::TooSmall),
        (
            calc_sort::RADIX_BASE_LIMIT + 1,
            calc_sort::BaseRefused::TooLarge {
                limit: calc_sort::RADIX_BASE_LIMIT,
            },
        ),
    ] {
        assert_eq!(
            calc_sort::radix_sort_whole_numbers(
                &keys,
                base,
                Order::Increasing,
                StepRecording::Skip
            ),
            Err(refused)
        );
    }
}

fn bead_counts(keys: &[u64], order: Order) -> calc_sort::BeadSorted {
    let whole: Vec<Integer> = keys.iter().map(|key| Integer::from(*key)).collect();
    calc_sort::bead_sort_whole_numbers(&whole, order, StepRecording::Skip).unwrap()
}

#[test]
fn bead_sort_rebuilds_the_sorted_values_with_twice_the_beads_in_counter_updates() {
    for order in [Order::Increasing, Order::Decreasing] {
        for keys in every_key_sequence(5, 4) {
            let keys: Vec<u64> = keys.iter().map(|key| *key as u64).collect();
            let sorted = bead_counts(&keys, order);
            let mut expected = keys.clone();
            expected.sort_unstable();
            if order == Order::Decreasing {
                expected.reverse();
            }
            assert_eq!(sorted.values, expected, "{keys:?}");
            let beads: u64 = keys.iter().sum();
            assert_eq!(sorted.counts.tallies, 2 * beads);
            assert_eq!(sorted.counts.writes, keys.len() as u64);
            assert_eq!(sorted.counts.comparisons, 0);
        }
    }
}

#[test]
fn bead_sort_refuses_a_negative_key_and_too_many_beads() {
    let negative = [Integer::from(2_u64), Integer::from(-1_i64)];
    let heavy = [Integer::from(calc_sort::BEAD_LIMIT), Integer::from(1_u64)];

    assert_eq!(
        calc_sort::bead_sort_whole_numbers(&negative, Order::Increasing, StepRecording::Skip),
        Err(calc_sort::BeadRefused::Negative { entry: 1 })
    );
    assert_eq!(
        calc_sort::bead_sort_whole_numbers(&heavy, Order::Increasing, StepRecording::Skip),
        Err(calc_sort::BeadRefused::TooManyBeads {
            beads: Integer::from(calc_sort::BEAD_LIMIT + 1),
            limit: calc_sort::BEAD_LIMIT,
        })
    );
}

#[test]
fn bead_sort_of_nothing_and_of_zeros_makes_no_counter_update() {
    assert_eq!(
        bead_counts(&[], Order::Increasing).values,
        Vec::<u64>::new()
    );
    let zeros = bead_counts(&[0, 0, 0], Order::Increasing);
    assert_eq!(zeros.values, vec![0, 0, 0]);
    assert_eq!(zeros.counts.tallies, 0);
    assert_eq!(zeros.counts.writes, 3);
}

fn stated_rows_match(method: Method, counted: impl Fn(&[u64]) -> Counts) {
    for length in 0..=EXHAUSTIVE_CHECK_LIMIT {
        let all: Vec<Counts> = every_order(usize::try_from(length).unwrap())
            .iter()
            .map(|entries| {
                let keys: Vec<u64> = entries.iter().map(|entry| *entry as u64).collect();
                counted(&keys)
            })
            .collect();
        for cost in costs(method) {
            let Some(value) = cost.value_at(length) else {
                continue;
            };
            assert_eq!(
                value.unwrap(),
                observed(&all, cost.measure, cost.case),
                "{method:?} {cost:?} at {length}"
            );
        }
    }
}

#[test]
fn radix_sort_stated_rows_match_every_order_up_to_the_check_limit() {
    for base in [2, 10] {
        stated_rows_match(Method::Radix { base }, |keys| {
            radix_counts(keys, base, Order::Increasing).sorted.counts
        });
    }
}

#[test]
fn bead_sort_stated_rows_match_every_order_up_to_the_check_limit() {
    stated_rows_match(Method::Bead, |keys| {
        bead_counts(keys, Order::Increasing).counts
    });
}

fn bitonic_counts(entries: &[usize], order: Order) -> Counts {
    let sorted = sort(
        entries,
        Method::Bitonic,
        order,
        |left, right| Ok::<_, Infallible>(left.cmp(right)),
        StepRecording::Skip,
    )
    .unwrap();
    let values: Vec<usize> = sorted
        .arrangement
        .iter()
        .map(|&index| entries[index])
        .collect();
    let mut expected = entries.to_vec();
    expected.sort_unstable();
    if order == Order::Decreasing {
        expected.reverse();
    }
    assert_eq!(values, expected, "{entries:?}");
    sorted.counts
}

#[test]
fn bitonic_sort_stated_rows_match_every_order_of_every_power_of_two_up_to_the_check_limit() {
    let mut length = 1;
    while length <= EXHAUSTIVE_CHECK_LIMIT {
        for order in [Order::Increasing, Order::Decreasing] {
            let all: Vec<Counts> = every_order(usize::try_from(length).unwrap())
                .iter()
                .map(|entries| bitonic_counts(entries, order))
                .collect();
            for cost in costs(Method::Bitonic) {
                let Some(value) = cost.value_at(length) else {
                    continue;
                };
                assert_eq!(
                    value.unwrap(),
                    observed(&all, cost.measure, cost.case),
                    "{cost:?} at {length}"
                );
            }
        }
        length *= 2;
    }
}

#[test]
fn bitonic_sort_sorts_repeated_keys() {
    for keys in every_key_sequence(4, 3) {
        bitonic_counts(&keys, Order::Increasing);
        bitonic_counts(&keys, Order::Decreasing);
    }
}

#[test]
fn bitonic_sort_refuses_a_length_that_is_not_a_power_of_two() {
    for length in [3_usize, 5, 6, 7, 12] {
        let entries: Vec<usize> = (0..length).collect();
        let refused = sort(
            &entries,
            Method::Bitonic,
            Order::Increasing,
            |left, right| Ok::<_, Infallible>(left.cmp(right)),
            StepRecording::Skip,
        );
        assert!(
            matches!(refused, Err(calc_sort::Refusal::LengthNotAPowerOfTwo { length: refused }) if refused == length),
            "{length}"
        );
    }
}

fn bogo(
    entries: &[usize],
    seed: u64,
    limit: u64,
) -> Result<calc_sort::Sorted, calc_sort::Refusal<Infallible>> {
    sort(
        entries,
        Method::Bogo { seed, limit },
        Order::Increasing,
        |left, right| Ok::<_, Infallible>(left.cmp(right)),
        StepRecording::Skip,
    )
}

#[test]
fn bogo_sort_sorts_every_order_and_its_fewest_counts_are_the_stated_rows() {
    for length in 0..=calc_sort::BOGO_CHECK_LIMIT {
        let mut fewest: Option<Counts> = None;
        for (index, entries) in every_order(usize::try_from(length).unwrap())
            .iter()
            .enumerate()
        {
            let sorted = bogo(entries, index as u64, u64::MAX).unwrap();
            let values: Vec<usize> = sorted.arrangement.iter().map(|&at| entries[at]).collect();
            assert_eq!(
                values,
                (0..entries.len()).collect::<Vec<_>>(),
                "{entries:?}"
            );
            let exchanges = length.saturating_sub(1) * sorted.counts.shuffles;
            assert_eq!(sorted.counts.writes, 2 * exchanges, "{entries:?}");
            assert!(
                sorted.counts.comparisons >= length.saturating_sub(1),
                "{entries:?}"
            );
            fewest = Some(match fewest {
                None => sorted.counts,
                Some(least) => Counts {
                    comparisons: least.comparisons.min(sorted.counts.comparisons),
                    writes: least.writes.min(sorted.counts.writes),
                    ..least
                },
            });
        }
        let fewest = fewest.unwrap();
        for cost in costs(Method::Bogo { seed: 0, limit: 0 }) {
            if cost.case != Case::Best {
                continue;
            }
            let value = cost.value_at(length).map(Result::unwrap);
            let observed = match cost.measure {
                Measure::Comparisons => fewest.comparisons,
                _ => fewest.writes,
            };
            if let Some(value) = value {
                assert_eq!(
                    value,
                    Number::Integer(Integer::from(observed)),
                    "{cost:?} at {length}"
                );
            }
        }
    }
}

#[test]
fn bogo_sort_stops_at_its_limit_without_sorting() {
    let refused = bogo(&[2, 1, 0], 1, 0);
    match refused {
        Err(calc_sort::Refusal::LimitReached { counts, .. }) => {
            assert_eq!(counts.shuffles, 0);
            assert_eq!(counts.comparisons, 1);
            assert_eq!(counts.writes, 0);
        }
        other => panic!("{other:?}"),
    }
    let sorted = bogo(&[2, 1, 0], 1, 1_000_000).unwrap();
    assert!(sorted.counts.shuffles >= 1);
}

#[test]
fn bogo_sort_gives_the_same_counts_for_the_same_seed() {
    let first = bogo(&[4, 2, 0, 3, 1], 7, u64::MAX).unwrap();
    let second = bogo(&[4, 2, 0, 3, 1], 7, u64::MAX).unwrap();
    assert_eq!(first, second);
}
