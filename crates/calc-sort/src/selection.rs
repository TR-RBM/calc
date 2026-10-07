use std::cmp::Ordering;

use crate::run::{Located, Place, Run, Stopped};

pub(crate) fn sort<T, E, C>(run: &mut Run<'_, T, E, C>) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let length = run.len();
    for front in 0..length.saturating_sub(1) {
        let mut least = front;
        for candidate in front + 1..length {
            let is_smaller = run.precedes(
                Located {
                    entry: run.entry_at(candidate),
                    place: Place::List(candidate),
                },
                Located {
                    entry: run.entry_at(least),
                    place: Place::List(least),
                },
            )?;
            if is_smaller {
                least = candidate;
            }
        }
        if least != front {
            run.exchange(front, least);
        }
    }
    Ok(())
}
