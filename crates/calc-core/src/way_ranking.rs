use std::cmp::Ordering;

use calc_numbers::Number;

use crate::rule_search::{ErrorBound, Exactness, Way};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Criterion {
    FewestMeasurements,
    FewestSteps,
    Exactness,
    ErrorBound,
    GateCount,
    SmallestUncertainty,
    Conditioning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Combine {
    Order,
    Front,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Score {
    Count(u64),
    Exactness(Exactness),
    ErrorBound(ErrorBound),
    Rational(Number),
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EvaluatedScores {
    pub smallest_uncertainty: Option<Number>,
    pub conditioning: Option<Number>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankedWay {
    pub way: usize,
    pub layer: Option<u32>,
    pub scores: Vec<Score>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RankError {
    NoCriterion,
    RepeatedCriterion(Criterion),
    ScoreCountMismatch { ways: usize, scores: usize },
    ScoreNotExact(usize),
}

fn exact_order(left: &Number, right: &Number) -> Ordering {
    match left.sub_exact(right) {
        Ok(Number::Integer(difference)) if difference.is_zero() => Ordering::Equal,
        Ok(Number::Integer(difference)) if difference.is_negative() => Ordering::Less,
        Ok(Number::Rational(difference)) if difference.numerator().is_negative() => Ordering::Less,
        _ => Ordering::Greater,
    }
}

fn compare_scores(left: &Score, right: &Score) -> Ordering {
    match (left, right) {
        (Score::Unknown, Score::Unknown) => Ordering::Equal,
        (Score::Unknown, _) => Ordering::Greater,
        (_, Score::Unknown) => Ordering::Less,
        (Score::Count(left), Score::Count(right)) => left.cmp(right),
        (Score::Exactness(left), Score::Exactness(right)) => left.cmp(right),
        (Score::ErrorBound(left), Score::ErrorBound(right)) => left.cmp(right),
        (Score::Rational(left), Score::Rational(right)) => exact_order(left, right),
        _ => Ordering::Equal,
    }
}

fn count(value: usize) -> Score {
    Score::Count(u64::try_from(value).unwrap_or(u64::MAX))
}

pub fn score(way: &Way, criterion: Criterion, evaluated: Option<&EvaluatedScores>) -> Score {
    match criterion {
        Criterion::FewestMeasurements => count(way.inputs.len()),
        Criterion::FewestSteps => Score::Count(u64::from(way.steps)),
        Criterion::Exactness => Score::Exactness(way.exactness),
        Criterion::ErrorBound => Score::ErrorBound(way.error_bound),
        Criterion::GateCount => way.gate_count.map_or(Score::Unknown, Score::Count),
        Criterion::SmallestUncertainty => evaluated
            .and_then(|scores| scores.smallest_uncertainty.clone())
            .map_or(Score::Unknown, Score::Rational),
        Criterion::Conditioning => evaluated
            .and_then(|scores| scores.conditioning.clone())
            .map_or(Score::Unknown, Score::Rational),
    }
}

fn tie_break(left: &Way, right: &Way) -> Ordering {
    left.exactness
        .cmp(&right.exactness)
        .then(left.error_bound.cmp(&right.error_bound))
        .then_with(|| {
            compare_scores(
                &score(left, Criterion::GateCount, None),
                &score(right, Criterion::GateCount, None),
            )
        })
        .then_with(|| left.last_rule.cmp(&right.last_rule))
        .then_with(|| left.inputs.len().cmp(&right.inputs.len()))
        .then_with(|| left.inputs.cmp(&right.inputs))
}

fn lexicographic(left: &[Score], right: &[Score]) -> Ordering {
    left.iter()
        .zip(right)
        .map(|(left, right)| compare_scores(left, right))
        .find(|ordering| *ordering != Ordering::Equal)
        .unwrap_or(Ordering::Equal)
}

fn dominates(left: &[Score], right: &[Score]) -> bool {
    let orderings: Vec<Ordering> = left
        .iter()
        .zip(right)
        .map(|(left, right)| compare_scores(left, right))
        .collect();
    orderings
        .iter()
        .all(|ordering| *ordering != Ordering::Greater)
        && orderings.contains(&Ordering::Less)
}

fn check_criteria(criteria: &[Criterion]) -> Result<(), RankError> {
    if criteria.is_empty() {
        return Err(RankError::NoCriterion);
    }
    for (position, criterion) in criteria.iter().enumerate() {
        if criteria[..position].contains(criterion) {
            return Err(RankError::RepeatedCriterion(*criterion));
        }
    }
    Ok(())
}

pub fn rank_ways(
    ways: &[Way],
    criteria: &[Criterion],
    combine: Combine,
    evaluated: &[EvaluatedScores],
) -> Result<Vec<RankedWay>, RankError> {
    check_criteria(criteria)?;
    if !evaluated.is_empty() && evaluated.len() != ways.len() {
        return Err(RankError::ScoreCountMismatch {
            ways: ways.len(),
            scores: evaluated.len(),
        });
    }
    for (position, scores) in evaluated.iter().enumerate() {
        let is_exact = |number: &Option<Number>| number.as_ref().is_none_or(Number::is_exact);
        if !is_exact(&scores.smallest_uncertainty) || !is_exact(&scores.conditioning) {
            return Err(RankError::ScoreNotExact(position));
        }
    }
    let scores: Vec<Vec<Score>> = ways
        .iter()
        .enumerate()
        .map(|(position, way)| {
            criteria
                .iter()
                .map(|criterion| score(way, *criterion, evaluated.get(position)))
                .collect()
        })
        .collect();
    let mut layers: Vec<Option<u32>> = vec![None; ways.len()];
    if combine == Combine::Front {
        let mut remaining: Vec<usize> = (0..ways.len()).collect();
        let mut layer = 1;
        while !remaining.is_empty() {
            let front: Vec<usize> = remaining
                .iter()
                .copied()
                .filter(|candidate| {
                    !remaining
                        .iter()
                        .any(|other| dominates(&scores[*other], &scores[*candidate]))
                })
                .collect();
            for way in &front {
                layers[*way] = Some(layer);
            }
            remaining.retain(|way| !front.contains(way));
            layer += 1;
        }
    }
    let mut order: Vec<usize> = (0..ways.len()).collect();
    order.sort_by(|left, right| {
        let primary = match combine {
            Combine::Order => lexicographic(&scores[*left], &scores[*right]),
            Combine::Front => layers[*left].cmp(&layers[*right]),
        };
        primary.then_with(|| tie_break(&ways[*left], &ways[*right]))
    });
    Ok(order
        .into_iter()
        .map(|way| RankedWay {
            way,
            layer: layers[way],
            scores: scores[way].clone(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_numbers::Integer;

    fn way(inputs: &[&str], steps: u32, exactness: Exactness, gates: u64, last_rule: &str) -> Way {
        Way {
            inputs: inputs.iter().map(|input| input.to_string()).collect(),
            steps,
            derivation: vec![last_rule.to_string()],
            last_rule: Some(last_rule.to_string()),
            obtainable: false,
            exactness,
            error_bound: match exactness {
                Exactness::Exact => ErrorBound::Zero,
                Exactness::Machine => ErrorBound::Documented,
            },
            gate_count: Some(gates),
            conditions: Vec::new(),
            sources: Vec::new(),
        }
    }

    fn fraction(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    fn order(ranked: &[RankedWay]) -> Vec<usize> {
        ranked.iter().map(|ranked| ranked.way).collect()
    }

    fn triangle() -> Vec<Way> {
        vec![
            way(
                &["a", "b", "c"],
                2,
                Exactness::Machine,
                12,
                "triangle-area/heron",
            ),
            way(
                &["a", "b", "gamma"],
                1,
                Exactness::Machine,
                6,
                "triangle-area/sas",
            ),
            way(
                &["b", "h"],
                1,
                Exactness::Exact,
                3,
                "triangle-area/base-height",
            ),
        ]
    }

    #[test]
    fn order_compares_the_criteria_lexicographically() {
        let ranked = rank_ways(
            &triangle(),
            &[Criterion::FewestSteps, Criterion::GateCount],
            Combine::Order,
            &[],
        )
        .unwrap();

        assert_eq!(order(&ranked), vec![2, 1, 0]);
    }

    #[test]
    fn tie_break_puts_exact_ways_first() {
        let ways = vec![
            way(&["x"], 1, Exactness::Machine, 3, "q/one"),
            way(&["y"], 1, Exactness::Exact, 9, "q/two"),
        ];

        let ranked = rank_ways(&ways, &[Criterion::FewestSteps], Combine::Order, &[]).unwrap();

        assert_eq!(order(&ranked), vec![1, 0]);
    }

    #[test]
    fn tie_break_uses_gate_count_before_rule_identifier() {
        let ways = vec![
            way(&["x"], 1, Exactness::Machine, 9, "q/a"),
            way(&["y"], 1, Exactness::Machine, 3, "q/b"),
        ];

        let ranked = rank_ways(&ways, &[Criterion::FewestSteps], Combine::Order, &[]).unwrap();

        assert_eq!(order(&ranked), vec![1, 0]);
    }

    #[test]
    fn ways_sharing_their_last_rule_are_ordered_by_input_set() {
        let ways = vec![
            way(&["y", "z"], 2, Exactness::Exact, 5, "q/same"),
            way(&["x"], 2, Exactness::Exact, 5, "q/same"),
            way(&["w", "z"], 2, Exactness::Exact, 5, "q/same"),
        ];

        let ranked = rank_ways(&ways, &[Criterion::FewestSteps], Combine::Order, &[]).unwrap();

        assert_eq!(order(&ranked), vec![1, 2, 0]);
    }

    #[test]
    fn front_keeps_the_ways_no_other_way_dominates() {
        let ranked = rank_ways(
            &triangle(),
            &[Criterion::FewestMeasurements, Criterion::GateCount],
            Combine::Front,
            &[],
        )
        .unwrap();

        let layers: Vec<(usize, Option<u32>)> = ranked
            .iter()
            .map(|ranked| (ranked.way, ranked.layer))
            .collect();
        assert_eq!(layers, vec![(2, Some(1)), (1, Some(2)), (0, Some(3))]);
    }

    #[test]
    fn conflicting_criteria_put_both_ways_on_the_front() {
        let ways = vec![
            way(&["a", "b"], 1, Exactness::Machine, 20, "q/cheap-inputs"),
            way(&["c", "d", "e"], 1, Exactness::Machine, 4, "q/cheap-gates"),
        ];

        let ranked = rank_ways(
            &ways,
            &[Criterion::FewestMeasurements, Criterion::GateCount],
            Combine::Front,
            &[],
        )
        .unwrap();

        assert!(ranked.iter().all(|ranked| ranked.layer == Some(1)));
    }

    #[test]
    fn order_gives_no_layers() {
        let ranked = rank_ways(&triangle(), &[Criterion::GateCount], Combine::Order, &[]).unwrap();

        assert!(ranked.iter().all(|ranked| ranked.layer.is_none()));
    }

    #[test]
    fn criterion_unknown_in_the_phase_ties_and_the_next_decides() {
        let ranked = rank_ways(
            &triangle(),
            &[Criterion::SmallestUncertainty, Criterion::GateCount],
            Combine::Order,
            &[],
        )
        .unwrap();

        assert_eq!(
            (order(&ranked), ranked[0].scores[0].clone()),
            (vec![2, 1, 0], Score::Unknown)
        );
    }

    #[test]
    fn evaluated_uncertainty_ranks_exactly_and_unknown_last() {
        let evaluated = vec![
            EvaluatedScores {
                smallest_uncertainty: Some(fraction(1, 3)),
                conditioning: None,
            },
            EvaluatedScores::default(),
            EvaluatedScores {
                smallest_uncertainty: Some(fraction(333, 1000)),
                conditioning: None,
            },
        ];

        let ranked = rank_ways(
            &triangle(),
            &[Criterion::SmallestUncertainty],
            Combine::Order,
            &evaluated,
        )
        .unwrap();

        assert_eq!(order(&ranked), vec![2, 0, 1]);
    }

    #[test]
    fn empty_criteria_are_an_error() {
        assert_eq!(
            rank_ways(&triangle(), &[], Combine::Order, &[]),
            Err(RankError::NoCriterion)
        );
    }

    #[test]
    fn repeated_criterion_is_an_error() {
        assert_eq!(
            rank_ways(
                &triangle(),
                &[Criterion::GateCount, Criterion::GateCount],
                Combine::Order,
                &[]
            ),
            Err(RankError::RepeatedCriterion(Criterion::GateCount))
        );
    }

    #[test]
    fn evaluated_scores_for_a_different_number_of_ways_are_an_error() {
        assert_eq!(
            rank_ways(
                &triangle(),
                &[Criterion::GateCount],
                Combine::Order,
                &[EvaluatedScores::default()]
            ),
            Err(RankError::ScoreCountMismatch { ways: 3, scores: 1 })
        );
    }

    #[test]
    fn machine_number_as_a_score_is_an_error() {
        let evaluated = vec![
            EvaluatedScores::default(),
            EvaluatedScores {
                smallest_uncertainty: Some(Number::F64(0.5)),
                conditioning: None,
            },
            EvaluatedScores::default(),
        ];

        assert_eq!(
            rank_ways(
                &triangle(),
                &[Criterion::SmallestUncertainty],
                Combine::Order,
                &evaluated
            ),
            Err(RankError::ScoreNotExact(1))
        );
    }
}
