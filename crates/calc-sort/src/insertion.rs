use std::cmp::Ordering;

use crate::run::{Located, Place, Run, Stopped};

pub(crate) fn sort<T, E, C>(run: &mut Run<'_, T, E, C>) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    for next in 1..run.len() {
        let key = run.entry_at(next);
        let held = Place::Held { taken_from: next };
        let mut gap = next;
        while gap > 0 {
            let neighbour = run.entry_at(gap - 1);
            let key_first = run.precedes(
                Located {
                    entry: key,
                    place: held,
                },
                Located {
                    entry: neighbour,
                    place: Place::List(gap - 1),
                },
            )?;
            if !key_first {
                break;
            }
            run.write(neighbour, Place::List(gap - 1), gap);
            gap -= 1;
        }
        run.write(key, held, gap);
    }
    Ok(())
}
