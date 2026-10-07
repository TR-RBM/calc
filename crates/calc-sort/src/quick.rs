use std::cmp::Ordering;

use crate::method::Partition;
use crate::run::{Located, Place, Run, Stopped};

fn precedes_at<T, E, C>(
    run: &mut Run<'_, T, E, C>,
    first: usize,
    second: usize,
) -> Result<bool, Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    run.precedes(
        Located {
            entry: run.entry_at(first),
            place: Place::List(first),
        },
        Located {
            entry: run.entry_at(second),
            place: Place::List(second),
        },
    )
}

pub(crate) fn sort<T, E, C>(
    run: &mut Run<'_, T, E, C>,
    partition: Partition,
) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let length = run.len();
    match partition {
        Partition::LomutoLast => lomuto(run, 0, length, &mut Pivots::Last),
        Partition::HoareFirst => hoare(run, 0, length),
        Partition::LomutoRandom { seed } => lomuto(
            run,
            0,
            length,
            &mut Pivots::Drawn {
                seed,
                next_index: 0,
            },
        ),
    }
}

pub(crate) fn sort_with_chosen_pivots<T, E, C>(
    run: &mut Run<'_, T, E, C>,
    choose: &mut dyn FnMut(usize, usize) -> usize,
) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let length = run.len();
    lomuto(run, 0, length, &mut Pivots::Chosen(choose))
}

enum Pivots<'choose> {
    Last,
    Drawn { seed: u64, next_index: u64 },
    Chosen(&'choose mut dyn FnMut(usize, usize) -> usize),
}

impl Pivots<'_> {
    fn chosen(&mut self, low: usize, high: usize) -> Option<(usize, u64)> {
        match self {
            Pivots::Last => None,
            Pivots::Drawn { seed, next_index } => {
                let (offset, draws) = draw_below(*seed, PIVOT_STREAM, next_index, high - low);
                Some((low + offset, draws))
            }
            Pivots::Chosen(choose) => Some((choose(low, high).clamp(low, high - 1), 0)),
        }
    }
}

pub(crate) fn draw_below(
    seed: u64,
    stream: u64,
    next_index: &mut u64,
    bound: usize,
) -> (usize, u64) {
    let bound = u64::try_from(bound).unwrap_or(u64::MAX);
    let remainder = (u64::MAX % bound + 1) % bound;
    let mut draws = 0;
    loop {
        let block = calc_kernels::philox4x32_10_at(seed, stream, *next_index);
        *next_index = next_index.wrapping_add(1);
        draws += 1;
        let drawn = u64::from(block[0]) | (u64::from(block[1]) << 32);
        if remainder == 0 || drawn < remainder.wrapping_neg() {
            let offset = usize::try_from(drawn % bound).unwrap_or(0);
            return (offset, draws);
        }
    }
}

const PIVOT_STREAM: u64 = 0;

fn lomuto<T, E, C>(
    run: &mut Run<'_, T, E, C>,
    low: usize,
    high: usize,
    pivots: &mut Pivots<'_>,
) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    if high - low < 2 {
        return Ok(());
    }
    let pivot = high - 1;
    if let Some((chosen, draws)) = pivots.chosen(low, high) {
        run.drew(low, high, chosen, draws);
        if chosen != pivot {
            run.exchange(chosen, pivot);
        }
    }
    let mut boundary = low;
    for scanned in low..pivot {
        let pivot_first = precedes_at(run, pivot, scanned)?;
        if !pivot_first {
            if boundary != scanned {
                run.exchange(boundary, scanned);
            }
            boundary += 1;
        }
    }
    if boundary != pivot {
        run.exchange(boundary, pivot);
    }
    lomuto(run, low, boundary, pivots)?;
    lomuto(run, boundary + 1, high, pivots)
}

fn hoare<T, E, C>(run: &mut Run<'_, T, E, C>, low: usize, high: usize) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    if high - low < 2 {
        return Ok(());
    }
    let pivot_entry = run.entry_at(low);
    let mut pivot_place = low;
    let mut left = low;
    let mut right = high - 1;
    let split = loop {
        while against_pivot(run, (pivot_entry, pivot_place), right, true)? {
            right -= 1;
        }
        while against_pivot(run, (pivot_entry, pivot_place), left, false)? {
            left += 1;
        }
        if left < right {
            run.exchange(left, right);
            if pivot_place == left {
                pivot_place = right;
            } else if pivot_place == right {
                pivot_place = left;
            }
            left += 1;
            right -= 1;
        } else {
            break right;
        }
    };
    hoare(run, low, split + 1)?;
    hoare(run, split + 1, high)
}

fn against_pivot<T, E, C>(
    run: &mut Run<'_, T, E, C>,
    pivot: (usize, usize),
    at: usize,
    pivot_first: bool,
) -> Result<bool, Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let pivot = Located {
        entry: pivot.0,
        place: Place::List(pivot.1),
    };
    let other = Located {
        entry: run.entry_at(at),
        place: Place::List(at),
    };
    if pivot_first {
        run.precedes(pivot, other)
    } else {
        run.precedes(other, pivot)
    }
}
