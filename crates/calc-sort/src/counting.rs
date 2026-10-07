use calc_numbers::Integer;

use crate::method::Order;
use crate::run::{Counts, Place, Sorted, Step, StepRecording};

pub const COUNTING_RANGE_LIMIT: u64 = 1 << 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RangeTooWide {
    pub range: Integer,
    pub limit: u64,
}

pub fn sort_whole_numbers(
    keys: &[Integer],
    order: Order,
    recording: StepRecording,
) -> Result<Sorted, RangeTooWide> {
    let mut counts = Counts::default();
    let mut steps = Vec::new();
    let mut record = |step: Step| {
        if recording == StepRecording::Record {
            steps.push(step);
        }
    };
    let (Some(least), Some(greatest)) = (keys.iter().min(), keys.iter().max()) else {
        return Ok(Sorted {
            arrangement: Vec::new(),
            counts,
            steps,
        });
    };
    let range = &(greatest - least) + &Integer::one();
    let width = range
        .to_i128()
        .and_then(|width| u64::try_from(width).ok())
        .filter(|width| *width <= COUNTING_RANGE_LIMIT)
        .and_then(|width| usize::try_from(width).ok())
        .ok_or_else(|| RangeTooWide {
            range: range.clone(),
            limit: COUNTING_RANGE_LIMIT,
        })?;
    let slot_of = |key: &Integer| {
        let offset = (key - least)
            .to_i128()
            .and_then(|offset| usize::try_from(offset).ok())
            .unwrap_or(0);
        match order {
            Order::Increasing => offset,
            Order::Decreasing => width - 1 - offset,
        }
    };
    let mut tallies = vec![0_usize; width];
    for (entry, key) in keys.iter().enumerate() {
        let counter = slot_of(key);
        tallies[counter] += 1;
        counts.tallies += 1;
        record(Step::Tally { entry, counter });
    }
    for counter in 1..width {
        tallies[counter] += tallies[counter - 1];
        counts.tallies += 1;
        record(Step::PrefixSum { counter });
    }
    let mut output = vec![0_usize; keys.len()];
    for (entry, key) in keys.iter().enumerate().rev() {
        let slot = slot_of(key);
        tallies[slot] -= 1;
        counts.tallies += 1;
        record(Step::Decrement { counter: slot });
        output[tallies[slot]] = entry;
        counts.writes += 1;
        record(Step::Write {
            entry,
            from: Place::List(entry),
            to: Place::Scratch(tallies[slot]),
        });
    }
    for (place, entry) in output.iter().enumerate() {
        counts.writes += 1;
        record(Step::Write {
            entry: *entry,
            from: Place::Scratch(place),
            to: Place::List(place),
        });
    }
    Ok(Sorted {
        arrangement: output,
        counts,
        steps,
    })
}
