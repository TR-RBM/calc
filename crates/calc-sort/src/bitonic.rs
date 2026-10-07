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

pub(crate) fn sort<T, E, C>(run: &mut Run<'_, T, E, C>) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let length = run.len();
    let mut block = 2;
    while block <= length {
        let mut distance = block / 2;
        while distance > 0 {
            run.merged_bitonic(block, distance);
            for position in 0..length {
                let partner = position ^ distance;
                if partner <= position {
                    continue;
                }
                let rising = position & block == 0;
                let (low, high) = (at(run, position), at(run, partner));
                let out_of_order = if rising {
                    run.precedes(high, low)?
                } else {
                    run.precedes(low, high)?
                };
                if out_of_order {
                    run.exchange(position, partner);
                }
            }
            distance /= 2;
        }
        block *= 2;
    }
    Ok(())
}
