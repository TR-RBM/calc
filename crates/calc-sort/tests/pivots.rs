use std::cmp::Ordering;
use std::convert::Infallible;

use calc_numbers::{Integer, Number};
use calc_sort::{
    Case, Counts, Measure, Method, Order, Over, PIVOT_CHOICE_CHECK_LIMIT, Partition, StepRecording,
    costs, quick_sort_with_chosen_pivots, sort,
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

struct Outcome {
    counts: Counts,
    weight: Number,
}

fn every_pivot_sequence(entries: &[usize]) -> Vec<Outcome> {
    let mut outcomes = Vec::new();
    let mut script: Vec<(usize, usize)> = Vec::new();
    loop {
        let mut position = 0;
        let sorted = quick_sort_with_chosen_pivots(
            entries,
            Order::Increasing,
            |left: &usize, right: &usize| Ok::<Ordering, Infallible>(left.cmp(right)),
            &mut |low, high| {
                if position == script.len() {
                    script.push((0, high - low));
                }
                let (choice, _) = script[position];
                position += 1;
                low + choice
            },
        )
        .unwrap();
        let values: Vec<usize> = sorted
            .arrangement
            .iter()
            .map(|&index| entries[index])
            .collect();
        assert_eq!(values, (0..entries.len()).collect::<Vec<_>>());
        let mut weight = Number::Integer(Integer::from(1_u64));
        for (_, width) in &script {
            let share = Number::fraction(
                &Integer::from(1_u64),
                &Integer::from(u64::try_from(*width).unwrap()),
            )
            .unwrap();
            weight = weight.mul_exact(&share).unwrap();
        }
        outcomes.push(Outcome {
            counts: sorted.counts,
            weight,
        });
        while script
            .last()
            .is_some_and(|(choice, width)| choice + 1 == *width)
        {
            script.pop();
        }
        match script.last_mut() {
            Some((choice, _)) => *choice += 1,
            None => return outcomes,
        }
    }
}

fn measured(counts: &Counts, measure: Measure) -> u64 {
    match measure {
        Measure::Comparisons => counts.comparisons,
        Measure::Writes => counts.writes,
        Measure::Flips => counts.flips,
    }
}

fn whole(value: u64) -> Number {
    Number::Integer(Integer::from(value))
}

#[test]
fn every_formula_holds_over_every_order_and_every_sequence_of_pivot_choices() {
    let method = Method::Quick(Partition::LomutoRandom { seed: 0 });
    for length in 0..=PIVOT_CHOICE_CHECK_LIMIT {
        let per_order: Vec<Vec<Outcome>> = every_order(usize::try_from(length).unwrap())
            .iter()
            .map(|entries| every_pivot_sequence(entries))
            .collect();
        for cost in costs(method) {
            assert_eq!(cost.over, Over::OrdersAndPivotChoices);
            assert_eq!(cost.checked_up_to, PIVOT_CHOICE_CHECK_LIMIT);
            let Some(value) = cost.value_at(length) else {
                continue;
            };
            let value = value.unwrap();
            let all = per_order
                .iter()
                .flatten()
                .map(|outcome| measured(&outcome.counts, cost.measure));
            match cost.case {
                Case::Best => assert_eq!(value, whole(all.min().unwrap()), "{cost:?} at {length}"),
                Case::Worst => assert_eq!(value, whole(all.max().unwrap()), "{cost:?} at {length}"),
                Case::Average => {
                    for outcomes in &per_order {
                        let mut expected = whole(0);
                        for outcome in outcomes {
                            let term = whole(measured(&outcome.counts, cost.measure))
                                .mul_exact(&outcome.weight)
                                .unwrap();
                            expected = expected.add_exact(&term).unwrap();
                        }
                        assert_eq!(value, expected, "{cost:?} at {length}");
                    }
                }
            }
        }
    }
}

#[test]
fn a_wrong_expectation_is_noticed_for_some_order() {
    let method = Method::Quick(Partition::LomutoRandom { seed: 0 });
    let expected = costs(method)
        .into_iter()
        .find(|cost| cost.measure == Measure::Comparisons && cost.case == Case::Average)
        .unwrap()
        .value_at(3)
        .unwrap()
        .unwrap();

    let outcomes = every_pivot_sequence(&[2, 0, 1]);
    let mut total = whole(0);
    for outcome in &outcomes {
        total = total
            .add_exact(
                &whole(outcome.counts.comparisons + 1)
                    .mul_exact(&outcome.weight)
                    .unwrap(),
            )
            .unwrap();
    }

    assert_ne!(total, expected);
}

#[test]
fn drawn_pivots_sort_every_key_sequence_and_count_their_draws() {
    for seed in [0_u64, 7, u64::MAX] {
        for keys in [
            vec![3_usize, 1, 2],
            vec![2, 2, 1, 0, 2],
            vec![5, 4, 3, 2, 1, 0],
        ] {
            let sorted = sort(
                &keys,
                Method::Quick(Partition::LomutoRandom { seed }),
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
            assert_eq!(values, expected, "{seed} {keys:?}");
            assert!(sorted.counts.draws >= 1, "{seed} {keys:?}");
        }
    }
}

#[test]
fn the_same_seed_gives_the_same_steps() {
    let run = |seed| {
        sort(
            &[4_usize, 1, 3, 0, 2],
            Method::Quick(Partition::LomutoRandom { seed }),
            Order::Increasing,
            |left: &usize, right: &usize| Ok::<Ordering, Infallible>(left.cmp(right)),
            StepRecording::Record,
        )
        .unwrap()
    };

    assert_eq!(run(7), run(7));
}
