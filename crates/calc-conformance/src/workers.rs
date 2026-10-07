use std::num::NonZeroUsize;

use calc_exec::{Backend, Parallelism, SharedParallelism};

use crate::cases::Case;
use crate::check::{ConformanceError, compare_plans};

pub const WORKER_COUNTS: [usize; 4] = [1, 2, 3, 8];

const PERMUTATION_MULTIPLIER: u64 = 6_364_136_223_846_793_005;
const PERMUTATION_INCREMENT: u64 = 1_442_695_040_888_963_407;

#[derive(Clone, Copy, Debug)]
pub struct PermutedParallelism {
    worker_count: NonZeroUsize,
    seed: u64,
}

impl PermutedParallelism {
    pub fn new(worker_count: NonZeroUsize, seed: u64) -> Self {
        Self { worker_count, seed }
    }
}

fn permutation(length: usize, seed: u64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..length).collect();
    let mut state = seed;
    for index in (1..length).rev() {
        state = state
            .wrapping_mul(PERMUTATION_MULTIPLIER)
            .wrapping_add(PERMUTATION_INCREMENT);
        let bound = u64::try_from(index + 1).unwrap_or(u64::MAX);
        let other = usize::try_from((state >> 33) % bound).unwrap_or(0);
        order.swap(index, other);
    }
    order
}

impl Parallelism for PermutedParallelism {
    fn worker_count(&self) -> NonZeroUsize {
        self.worker_count
    }

    fn run_all<'scope>(&self, tasks: Vec<Box<dyn FnOnce() + Send + 'scope>>) {
        let mut slots: Vec<Option<Box<dyn FnOnce() + Send + 'scope>>> =
            tasks.into_iter().map(Some).collect();
        for index in permutation(slots.len(), self.seed) {
            if let Some(task) = slots.get_mut(index).and_then(Option::take) {
                task();
            }
        }
    }
}

pub fn check_worker_counts(
    build: &dyn Fn(SharedParallelism) -> Box<dyn Backend>,
    case: &Case,
    seed: u64,
) -> Result<(), ConformanceError> {
    let reference = build(SharedParallelism::sequential());
    for workers in WORKER_COUNTS {
        let worker_count = NonZeroUsize::new(workers).unwrap_or(NonZeroUsize::MIN);
        let backend = build(SharedParallelism::new(PermutedParallelism::new(
            worker_count,
            seed,
        )));
        compare_plans(
            backend.as_ref(),
            Some(reference.as_ref()),
            &case.plan,
            &case.plan,
            &case.inputs,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permutation_contains_every_index_once() {
        let mut order = permutation(9, 7);

        order.sort_unstable();

        assert_eq!(order, (0..9).collect::<Vec<_>>());
    }

    #[test]
    fn different_seeds_give_different_orders() {
        let first = permutation(9, 1);

        let second = permutation(9, 2);

        assert_ne!(first, second);
    }
}
