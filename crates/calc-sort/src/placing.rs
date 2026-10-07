use std::cmp::Ordering;

use crate::run::{Located, Place, Run, Stopped};

fn at<T, E, C>(run: &Run<'_, T, E, C>, position: usize) -> Located
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    Located {
        entry: run.entry_at(position),
        place: Place::List(position),
    }
}

fn precedes_at<T, E, C>(
    run: &mut Run<'_, T, E, C>,
    first: usize,
    second: usize,
) -> Result<bool, Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let (first, second) = (at(run, first), at(run, second));
    run.precedes(first, second)
}

pub(crate) fn double_selection<T, E, C>(run: &mut Run<'_, T, E, C>) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let (mut low_end, mut high_end) = (0, run.len().saturating_sub(1));
    while low_end < high_end {
        let (mut least, mut greatest, start) = if (high_end - low_end + 1) % 2 == 1 {
            (low_end, low_end, low_end + 1)
        } else if precedes_at(run, low_end + 1, low_end)? {
            (low_end + 1, low_end, low_end + 2)
        } else {
            (low_end, low_end + 1, low_end + 2)
        };
        for first in (start..high_end).step_by(2) {
            let (smaller, larger) = if precedes_at(run, first + 1, first)? {
                (first + 1, first)
            } else {
                (first, first + 1)
            };
            if precedes_at(run, smaller, least)? {
                least = smaller;
            }
            if precedes_at(run, greatest, larger)? {
                greatest = larger;
            }
        }
        if least != low_end {
            run.exchange(low_end, least);
        }
        if greatest == low_end {
            greatest = least;
        }
        if greatest != high_end {
            run.exchange(high_end, greatest);
        }
        low_end += 1;
        high_end -= 1;
    }
    Ok(())
}

pub(crate) fn cycle<T, E, C>(run: &mut Run<'_, T, E, C>) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let length = run.len();
    for start in 0..length.saturating_sub(1) {
        let mut held = Located {
            entry: run.entry_at(start),
            place: Place::Held { taken_from: start },
        };
        let mut position = place_of(run, held, start)?;
        if position == start {
            continue;
        }
        loop {
            position = past_equals(run, held, position)?;
            let displaced = run.entry_at(position);
            run.write(held.entry, held.place, position);
            held = Located {
                entry: displaced,
                place: Place::Held {
                    taken_from: position,
                },
            };
            if position == start {
                break;
            }
            position = place_of(run, held, start)?;
        }
    }
    Ok(())
}

fn place_of<T, E, C>(
    run: &mut Run<'_, T, E, C>,
    held: Located,
    start: usize,
) -> Result<usize, Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let mut position = start;
    for other in start + 1..run.len() {
        let other = at(run, other);
        if run.precedes(other, held)? {
            position += 1;
        }
    }
    Ok(position)
}

fn past_equals<T, E, C>(
    run: &mut Run<'_, T, E, C>,
    held: Located,
    mut position: usize,
) -> Result<usize, Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    loop {
        let there = at(run, position);
        if run.ordering(held, there)? != Ordering::Equal {
            return Ok(position);
        }
        position += 1;
    }
}

pub(crate) fn pancake<T, E, C>(run: &mut Run<'_, T, E, C>) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let mut unsorted = run.len();
    while unsorted > 1 {
        let mut top = 0;
        for other in 1..unsorted {
            if precedes_at(run, top, other)? {
                top = other;
            }
        }
        if top != unsorted - 1 {
            if top != 0 {
                flip(run, top);
            }
            flip(run, unsorted - 1);
        }
        unsorted -= 1;
    }
    Ok(())
}

fn flip<T, E, C>(run: &mut Run<'_, T, E, C>, last: usize)
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    run.flipped(last + 1);
    let (mut left, mut right) = (0, last);
    while left < right {
        run.exchange(left, right);
        left += 1;
        right -= 1;
    }
}
