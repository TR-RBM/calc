use std::cmp::Ordering;

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

fn sift_down<T, E, C>(
    run: &mut Run<'_, T, E, C>,
    mut parent: usize,
    size: usize,
) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    loop {
        let (left, right) = (2 * parent + 1, 2 * parent + 2);
        let mut largest = parent;
        if left < size && precedes_at(run, largest, left)? {
            largest = left;
        }
        if right < size && precedes_at(run, largest, right)? {
            largest = right;
        }
        if largest == parent {
            return Ok(());
        }
        run.exchange(parent, largest);
        parent = largest;
    }
}

pub(crate) fn sort<T, E, C>(run: &mut Run<'_, T, E, C>) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let length = run.len();
    for parent in (0..length / 2).rev() {
        sift_down(run, parent, length)?;
    }
    for end in (1..length).rev() {
        run.exchange(0, end);
        sift_down(run, 0, end)?;
    }
    Ok(())
}
