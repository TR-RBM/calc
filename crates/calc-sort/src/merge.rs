use std::cmp::Ordering;

use crate::run::{Located, Place, Run, Stopped};

pub(crate) fn sort<T, E, C>(run: &mut Run<'_, T, E, C>) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let mut scratch = vec![0; run.len()];
    sort_range(run, 0, run.len(), &mut scratch)
}

fn sort_range<T, E, C>(
    run: &mut Run<'_, T, E, C>,
    low: usize,
    high: usize,
    scratch: &mut [usize],
) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    if high - low < 2 {
        return Ok(());
    }
    let middle = low + (high - low).div_ceil(2);
    sort_range(run, low, middle, scratch)?;
    sort_range(run, middle, high, scratch)?;
    merge_runs(run, low, middle, high, scratch)?;
    for place in low..high {
        run.copy_back_from_scratch(place, scratch);
    }
    Ok(())
}

fn merge_runs<T, E, C>(
    run: &mut Run<'_, T, E, C>,
    low: usize,
    middle: usize,
    high: usize,
    scratch: &mut [usize],
) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let (mut left, mut right, mut slot) = (low, middle, low);
    while left < middle && right < high {
        let right_first = run.precedes(
            Located {
                entry: run.entry_at(right),
                place: Place::List(right),
            },
            Located {
                entry: run.entry_at(left),
                place: Place::List(left),
            },
        )?;
        if right_first {
            run.copy_to_scratch(right, slot, scratch);
            right += 1;
        } else {
            run.copy_to_scratch(left, slot, scratch);
            left += 1;
        }
        slot += 1;
    }
    for rest in (left..middle).chain(right..high) {
        run.copy_to_scratch(rest, slot, scratch);
        slot += 1;
    }
    Ok(())
}

pub(crate) fn bottom_up<T, E, C>(run: &mut Run<'_, T, E, C>) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let length = run.len();
    let mut scratch = vec![0; length];
    let mut width = 1;
    while width < length {
        for low in (0..length).step_by(2 * width) {
            let middle = (low + width).min(length);
            let high = (low + 2 * width).min(length);
            merge_runs(run, low, middle, high, &mut scratch)?;
        }
        for place in 0..length {
            run.copy_back_from_scratch(place, &scratch);
        }
        width *= 2;
    }
    Ok(())
}

pub(crate) fn natural<T, E, C>(run: &mut Run<'_, T, E, C>) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let length = run.len();
    if length < 2 {
        return Ok(());
    }
    let mut bounds = vec![0];
    for next in 1..length {
        let descends = run.precedes(
            Located {
                entry: run.entry_at(next),
                place: Place::List(next),
            },
            Located {
                entry: run.entry_at(next - 1),
                place: Place::List(next - 1),
            },
        )?;
        if descends {
            bounds.push(next);
        }
    }
    bounds.push(length);
    let mut scratch = vec![0; length];
    while bounds.len() > 2 {
        let mut merged = vec![0];
        for pair in (0..bounds.len() - 1).step_by(2) {
            let (low, middle) = (bounds[pair], bounds[pair + 1]);
            let high = bounds.get(pair + 2).copied().unwrap_or(middle);
            merge_runs(run, low, middle, high, &mut scratch)?;
            merged.push(high);
        }
        for place in 0..length {
            run.copy_back_from_scratch(place, &scratch);
        }
        bounds = merged;
    }
    Ok(())
}
