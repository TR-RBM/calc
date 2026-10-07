use std::cmp::Ordering;

use crate::quick::draw_below;
use crate::run::{Located, Place, Run, Stopped};

const SHUFFLE_STREAM: u64 = 1;

fn at<T, E, C>(run: &Run<'_, T, E, C>, position: usize) -> Located
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    Located {
        entry: run.entry_at(position),
        place: Place::List(position),
    }
}

fn is_sorted<T, E, C>(run: &mut Run<'_, T, E, C>) -> Result<bool, Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    for position in 1..run.len() {
        let (earlier, later) = (at(run, position - 1), at(run, position));
        if run.precedes(later, earlier)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn sort<T, E, C>(
    run: &mut Run<'_, T, E, C>,
    seed: u64,
    limit: u64,
) -> Result<bool, Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let mut next_index = 0;
    loop {
        if is_sorted(run)? {
            return Ok(true);
        }
        if run.shuffles() == limit {
            return Ok(false);
        }
        run.shuffled();
        for position in (1..run.len()).rev() {
            let (chosen, draws) = draw_below(seed, SHUFFLE_STREAM, &mut next_index, position + 1);
            run.drew_partner(position, chosen, draws);
            run.exchange(chosen, position);
        }
    }
}
