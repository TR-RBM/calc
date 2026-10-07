use calc_numbers::Integer;

use crate::method::Order;
use crate::run::{Counts, Step, StepRecording};

pub const BEAD_LIMIT: u64 = 1 << 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BeadRefused {
    Negative { entry: usize },
    TooManyBeads { beads: Integer, limit: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeadSorted {
    pub values: Vec<u64>,
    pub counts: Counts,
    pub steps: Vec<Step>,
}

pub fn bead_sort_whole_numbers(
    keys: &[Integer],
    order: Order,
    recording: StepRecording,
) -> Result<BeadSorted, BeadRefused> {
    if let Some(entry) = keys.iter().position(Integer::is_negative) {
        return Err(BeadRefused::Negative { entry });
    }
    let beads = keys.iter().fold(Integer::zero(), |total, key| &total + key);
    let too_many = || BeadRefused::TooManyBeads {
        beads: beads.clone(),
        limit: BEAD_LIMIT,
    };
    if beads > Integer::from(BEAD_LIMIT) {
        return Err(too_many());
    }
    let rows: Vec<usize> = keys
        .iter()
        .map(|key| {
            key.to_i64()
                .and_then(|key| usize::try_from(key).ok())
                .ok_or_else(too_many)
        })
        .collect::<Result<_, _>>()?;
    let mut counts = Counts::default();
    let mut steps = Vec::new();
    let mut record = |step: Step| {
        if recording == StepRecording::Record {
            steps.push(step);
        }
    };
    let mut poles = vec![0_usize; rows.iter().copied().max().unwrap_or(0)];
    for (entry, &row) in rows.iter().enumerate() {
        for (pole, height) in poles.iter_mut().take(row).enumerate() {
            *height += 1;
            counts.tallies += 1;
            record(Step::BeadFalls { entry, pole });
        }
    }
    let mut from_the_bottom = vec![0_usize; rows.len()];
    for (pole, &height) in poles.iter().enumerate() {
        for (row, beads) in from_the_bottom.iter_mut().take(height).enumerate() {
            *beads += 1;
            counts.tallies += 1;
            record(Step::BeadRead { pole, row });
        }
    }
    let length = rows.len();
    let mut values = vec![0_u64; length];
    for (row, &beads) in from_the_bottom.iter().enumerate() {
        let position = match order {
            Order::Increasing => length - 1 - row,
            Order::Decreasing => row,
        };
        values[position] = u64::try_from(beads).unwrap_or(u64::MAX);
        counts.writes += 1;
        record(Step::Rebuild {
            row,
            position,
            beads,
        });
    }
    Ok(BeadSorted {
        values,
        counts,
        steps,
    })
}
