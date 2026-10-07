use calc_numbers::Integer;

use crate::method::Order;
use crate::run::{Counts, Place, Sorted, Step, StepRecording};

pub const RADIX_BASE_LIMIT: u64 = 1 << 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseRefused {
    TooSmall,
    TooLarge { limit: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadixSorted {
    pub sorted: Sorted,
    pub passes: u64,
}

pub fn radix_sort_whole_numbers(
    keys: &[Integer],
    base: u64,
    order: Order,
    recording: StepRecording,
) -> Result<RadixSorted, BaseRefused> {
    if base < 2 {
        return Err(BaseRefused::TooSmall);
    }
    if base > RADIX_BASE_LIMIT {
        return Err(BaseRefused::TooLarge {
            limit: RADIX_BASE_LIMIT,
        });
    }
    let width = usize::try_from(base).map_err(|_| BaseRefused::TooLarge {
        limit: RADIX_BASE_LIMIT,
    })?;
    let divisor = Integer::from(base);
    let mut counts = Counts::default();
    let mut steps = Vec::new();
    let mut record = |step: Step| {
        if recording == StepRecording::Record {
            steps.push(step);
        }
    };
    let mut arrangement: Vec<usize> = (0..keys.len()).collect();
    let Some((least_entry, least)) = keys
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| left.cmp(right))
    else {
        return Ok(RadixSorted {
            sorted: Sorted {
                arrangement,
                counts,
                steps,
            },
            passes: 0,
        });
    };
    let mut rest: Vec<Integer> = keys.iter().map(|key| key - least).collect();
    let mut passes = 0;
    while rest.iter().any(|value| !value.is_zero()) {
        record(Step::DigitPass {
            place: usize::try_from(passes).unwrap_or(usize::MAX),
            least_entry,
        });
        let mut values = vec![0_usize; keys.len()];
        let mut digits = vec![0_usize; keys.len()];
        for (entry, value) in rest.iter_mut().enumerate() {
            let (quotient, remainder) = value
                .div_rem_euclid(&divisor)
                .unwrap_or((Integer::zero(), Integer::zero()));
            let digit = remainder
                .to_i64()
                .and_then(|digit| usize::try_from(digit).ok())
                .unwrap_or(0);
            values[entry] = digit;
            digits[entry] = match order {
                Order::Increasing => digit,
                Order::Decreasing => width - 1 - digit,
            };
            *value = quotient;
        }
        let mut tallies = vec![0_usize; width];
        for &entry in &arrangement {
            tallies[digits[entry]] += 1;
            counts.tallies += 1;
            record(Step::DigitTally {
                entry,
                digit: values[entry],
                counter: digits[entry],
            });
        }
        for counter in 1..width {
            tallies[counter] += tallies[counter - 1];
            counts.tallies += 1;
            record(Step::PrefixSum { counter });
        }
        let mut output = vec![0_usize; keys.len()];
        for (position, &entry) in arrangement.iter().enumerate().rev() {
            let counter = digits[entry];
            tallies[counter] -= 1;
            counts.tallies += 1;
            record(Step::Decrement { counter });
            output[tallies[counter]] = entry;
            counts.writes += 1;
            record(Step::Write {
                entry,
                from: Place::List(position),
                to: Place::Scratch(tallies[counter]),
            });
        }
        for (place, &entry) in output.iter().enumerate() {
            counts.writes += 1;
            record(Step::Write {
                entry,
                from: Place::Scratch(place),
                to: Place::List(place),
            });
        }
        arrangement = output;
        passes += 1;
    }
    Ok(RadixSorted {
        sorted: Sorted {
            arrangement,
            counts,
            steps,
        },
        passes,
    })
}
