use std::cmp::Ordering;

use crate::run::{Located, Place, Run, Stopped};

pub(crate) fn sort<T, E, C>(run: &mut Run<'_, T, E, C>) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    for next in 1..run.len() {
        let key = run.entry_at(next);
        let held = Place::Held { taken_from: next };
        let (mut low, mut high) = (0, next);
        while low < high {
            let middle = low + (high - low) / 2;
            let key_first = run.precedes(
                Located {
                    entry: key,
                    place: held,
                },
                Located {
                    entry: run.entry_at(middle),
                    place: Place::List(middle),
                },
            )?;
            if key_first {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        for gap in (low + 1..=next).rev() {
            let neighbour = run.entry_at(gap - 1);
            run.write(neighbour, Place::List(gap - 1), gap);
        }
        if low != next {
            run.write(key, held, low);
        }
    }
    Ok(())
}
