use std::cmp::Ordering;

use crate::method::{OddEvenForm, ShakerForm};
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

fn out_of_order<T, E, C>(run: &mut Run<'_, T, E, C>, left: usize) -> Result<bool, Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let (right, left) = (at(run, left + 1), at(run, left));
    run.precedes(right, left)
}

fn exchange_if_out_of_order<T, E, C>(
    run: &mut Run<'_, T, E, C>,
    left: usize,
) -> Result<bool, Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let exchanged = out_of_order(run, left)?;
    if exchanged {
        run.exchange(left, left + 1);
    }
    Ok(exchanged)
}

pub(crate) fn cocktail_shaker<T, E, C>(
    run: &mut Run<'_, T, E, C>,
    form: ShakerForm,
) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let (mut begin, mut end) = (0, run.len().saturating_sub(1));
    while begin < end {
        let mut last = None;
        for left in begin..end {
            if exchange_if_out_of_order(run, left)? {
                last = Some(left);
            }
        }
        let Some(last) = last else {
            return Ok(());
        };
        end = match form {
            ShakerForm::Full => end,
            ShakerForm::Shrinking => end - 1,
            ShakerForm::LastExchange => last,
        };
        let mut first = None;
        for left in (begin..end).rev() {
            if exchange_if_out_of_order(run, left)? {
                first = Some(left + 1);
            }
        }
        let Some(first) = first else {
            return Ok(());
        };
        begin = match form {
            ShakerForm::Full => begin,
            ShakerForm::Shrinking => begin + 1,
            ShakerForm::LastExchange => first,
        };
    }
    Ok(())
}

pub(crate) fn gnome<T, E, C>(run: &mut Run<'_, T, E, C>) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let mut position = 1;
    while position < run.len() {
        if position == 0 || !out_of_order(run, position - 1)? {
            position += 1;
        } else {
            run.exchange(position - 1, position);
            position -= 1;
        }
    }
    Ok(())
}

pub(crate) fn odd_even<T, E, C>(
    run: &mut Run<'_, T, E, C>,
    form: OddEvenForm,
) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let length = run.len();
    match form {
        OddEvenForm::FixedPasses => {
            for phase in 0..length {
                for left in (1 - phase % 2..length.saturating_sub(1)).step_by(2) {
                    exchange_if_out_of_order(run, left)?;
                }
            }
        }
        OddEvenForm::UntilSorted => loop {
            let mut exchanged = false;
            for start in [1, 0] {
                for left in (start..length.saturating_sub(1)).step_by(2) {
                    exchanged |= exchange_if_out_of_order(run, left)?;
                }
            }
            if !exchanged {
                break;
            }
        },
    }
    Ok(())
}

const COMB_SHRINK_NUMERATOR: usize = 10;
const COMB_SHRINK_DENOMINATOR: usize = 13;

pub(crate) fn comb<T, E, C>(run: &mut Run<'_, T, E, C>) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let length = run.len();
    let mut gap = length;
    let mut done = false;
    while !done {
        gap = gap * COMB_SHRINK_NUMERATOR / COMB_SHRINK_DENOMINATOR;
        if gap <= 1 {
            gap = 1;
            done = true;
        } else if gap == 9 || gap == 10 {
            gap = 11;
        }
        let mut left = 0;
        while left + gap < length {
            let (right_entry, left_entry) = (at(run, left + gap), at(run, left));
            if run.precedes(right_entry, left_entry)? {
                run.exchange(left, left + gap);
                done = false;
            }
            left += 1;
        }
    }
    Ok(())
}
