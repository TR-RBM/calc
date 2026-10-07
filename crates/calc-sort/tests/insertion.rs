use std::cmp::Ordering;
use std::convert::Infallible;

use calc_sort::{
    Counts, Located, Method, Order, Place, Refusal, Step, StepRecording, Stopped, is_stable, sort,
};

fn by_value(left: &i64, right: &i64) -> Result<Ordering, Infallible> {
    Ok(left.cmp(right))
}

#[test]
fn three_one_two_comes_from_positions_two_three_one_with_three_comparisons_and_four_writes() {
    let sorted = sort(
        &[3, 1, 2],
        Method::Insertion,
        Order::Increasing,
        by_value,
        StepRecording::Skip,
    )
    .unwrap();

    assert_eq!(sorted.arrangement, vec![1, 2, 0]);
    assert_eq!(
        sorted.counts,
        Counts {
            comparisons: 3,
            writes: 4,
            tallies: 0,
            draws: 0,
            flips: 0,
            shuffles: 0,
        }
    );
}

#[test]
fn three_one_two_is_traced_in_seven_steps() {
    let sorted = sort(
        &[3, 1, 2],
        Method::Insertion,
        Order::Increasing,
        by_value,
        StepRecording::Record,
    )
    .unwrap();
    let held_one = Located {
        entry: 1,
        place: Place::Held { taken_from: 1 },
    };
    let held_two = Located {
        entry: 2,
        place: Place::Held { taken_from: 2 },
    };

    assert_eq!(
        sorted.steps,
        vec![
            Step::Compare {
                left: held_one,
                right: Located {
                    entry: 0,
                    place: Place::List(0)
                },
                outcome: Some(Ordering::Less),
            },
            Step::Write {
                entry: 0,
                from: Place::List(0),
                to: Place::List(1)
            },
            Step::Write {
                entry: 1,
                from: Place::Held { taken_from: 1 },
                to: Place::List(0)
            },
            Step::Compare {
                left: held_two,
                right: Located {
                    entry: 0,
                    place: Place::List(1)
                },
                outcome: Some(Ordering::Less),
            },
            Step::Write {
                entry: 0,
                from: Place::List(1),
                to: Place::List(2)
            },
            Step::Compare {
                left: held_two,
                right: Located {
                    entry: 1,
                    place: Place::List(0)
                },
                outcome: Some(Ordering::Greater),
            },
            Step::Write {
                entry: 2,
                from: Place::Held { taken_from: 2 },
                to: Place::List(1)
            },
        ]
    );
}

#[test]
fn equal_keys_keep_their_input_order() {
    let sorted = sort(
        &[2, 1, 2],
        Method::Insertion,
        Order::Increasing,
        by_value,
        StepRecording::Skip,
    )
    .unwrap();

    assert_eq!(sorted.arrangement, vec![1, 0, 2]);
    assert!(is_stable(Method::Insertion));
}

#[test]
fn decreasing_order_keeps_equal_keys_in_their_input_order() {
    let sorted = sort(
        &[2, 3, 2],
        Method::Insertion,
        Order::Decreasing,
        by_value,
        StepRecording::Skip,
    )
    .unwrap();

    assert_eq!(sorted.arrangement, vec![1, 0, 2]);
}

#[test]
fn a_comparison_that_fails_stops_the_sort_with_the_pair_and_the_counts() {
    let undecided = |left: &i64, right: &i64| {
        if *left == 10 || *right == 10 {
            Err("undecided")
        } else {
            Ok(left.cmp(right))
        }
    };

    let stopped = sort(
        &[5, 1, 10],
        Method::Insertion,
        Order::Increasing,
        undecided,
        StepRecording::Skip,
    )
    .unwrap_err();

    assert_eq!(
        stopped,
        Refusal::Stopped(Stopped {
            error: "undecided",
            left: 2,
            right: 0,
            counts: Counts {
                comparisons: 2,
                writes: 2,
                tallies: 0,
                draws: 0,
                flips: 0,
                shuffles: 0,
            },
            steps: Vec::new(),
        })
    );
}

#[test]
fn an_empty_list_makes_no_comparisons() {
    let sorted = sort(
        &[],
        Method::Insertion,
        Order::Increasing,
        by_value,
        StepRecording::Record,
    )
    .unwrap();

    assert_eq!(sorted.counts, Counts::default());
    assert!(sorted.steps.is_empty());
}

#[test]
fn counting_sort_through_the_comparison_entry_is_refused() {
    let refused = sort(
        &[2_i64, 1],
        Method::Counting,
        Order::Increasing,
        by_value,
        StepRecording::Skip,
    )
    .unwrap_err();

    assert_eq!(refused, Refusal::WholeNumbersNeeded);
}
