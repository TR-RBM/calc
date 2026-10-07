use std::cmp::Ordering;

use crate::method::Gaps;
use crate::run::{Located, Place, Run, Stopped};

const CIURA_GAPS: [usize; 8] = [701, 301, 132, 57, 23, 10, 4, 1];

pub fn gaps_for(gaps: Gaps, length: usize) -> Vec<usize> {
    let mut sequence = match gaps {
        Gaps::Shell => {
            let mut halved = Vec::new();
            let mut gap = length / 2;
            while gap > 0 {
                halved.push(gap);
                gap /= 2;
            }
            halved
        }
        Gaps::Knuth => {
            let ceiling = length.div_ceil(3);
            let mut rising = Vec::new();
            let mut gap = 1;
            while gap <= ceiling {
                rising.push(gap);
                gap = 3 * gap + 1;
            }
            rising.reverse();
            rising
        }
        Gaps::Ciura => CIURA_GAPS.to_vec(),
    };
    sequence.retain(|gap| *gap < length);
    sequence
}

pub(crate) fn sort<T, E, C>(run: &mut Run<'_, T, E, C>, gaps: Gaps) -> Result<(), Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    for gap in gaps_for(gaps, run.len()) {
        run.passed(gap);
        for next in gap..run.len() {
            let key = run.entry_at(next);
            let held = Place::Held { taken_from: next };
            let mut hole = next;
            while hole >= gap {
                let neighbour = run.entry_at(hole - gap);
                let key_first = run.precedes(
                    Located {
                        entry: key,
                        place: held,
                    },
                    Located {
                        entry: neighbour,
                        place: Place::List(hole - gap),
                    },
                )?;
                if !key_first {
                    break;
                }
                run.write(neighbour, Place::List(hole - gap), hole);
                hole -= gap;
            }
            run.write(key, held, hole);
        }
    }
    Ok(())
}
