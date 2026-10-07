use std::cmp::Ordering;

use crate::method::BubbleForm;
use crate::run::{Located, Place, Run, Stopped};

fn out_of_order<T, E, C>(run: &mut Run<'_, T, E, C>, left: usize) -> Result<bool, Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    run.precedes(
        Located {
            entry: run.entry_at(left + 1),
            place: Place::List(left + 1),
        },
        Located {
            entry: run.entry_at(left),
            place: Place::List(left),
        },
    )
}

pub(crate) fn sort<T, E, C>(run: &mut Run<'_, T, E, C>, form: BubbleForm) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let length = run.len();
    match form {
        BubbleForm::Full => {
            for _ in 1..length {
                for left in 0..length - 1 {
                    if out_of_order(run, left)? {
                        run.exchange(left, left + 1);
                    }
                }
            }
        }
        BubbleForm::Shrinking | BubbleForm::EarlyExit => {
            for pass in 1..length {
                let mut exchanged = false;
                for left in 0..length - pass {
                    if out_of_order(run, left)? {
                        run.exchange(left, left + 1);
                        exchanged = true;
                    }
                }
                if form == BubbleForm::EarlyExit && !exchanged {
                    break;
                }
            }
        }
        BubbleForm::LastExchange => {
            let mut bound = length.saturating_sub(1);
            while bound > 0 {
                let mut last = 0;
                for left in 0..bound {
                    if out_of_order(run, left)? {
                        run.exchange(left, left + 1);
                        last = left;
                    }
                }
                bound = last;
            }
        }
    }
    Ok(())
}
