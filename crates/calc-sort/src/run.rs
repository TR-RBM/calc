use std::cmp::Ordering;

use crate::method::{Method, Order};
use crate::{
    binary_insertion, bitonic, bogo, bubble, heap, insertion, merge, neighbours, placing, quick,
    selection, shell,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub comparisons: u64,
    pub writes: u64,
    pub tallies: u64,
    pub draws: u64,
    pub flips: u64,
    pub shuffles: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    List(usize),
    Held { taken_from: usize },
    Scratch(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Located {
    pub entry: usize,
    pub place: Place,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Compare {
        left: Located,
        right: Located,
        outcome: Option<Ordering>,
    },
    Write {
        entry: usize,
        from: Place,
        to: Place,
    },
    Exchange {
        left: Located,
        right: Located,
    },
    Tally {
        entry: usize,
        counter: usize,
    },
    PrefixSum {
        counter: usize,
    },
    Decrement {
        counter: usize,
    },
    Draw {
        from: usize,
        to: usize,
        chosen: usize,
        draws: u64,
    },
    Flip {
        length: usize,
    },
    Pass {
        gap: usize,
    },
    DigitPass {
        place: usize,
        least_entry: usize,
    },
    DigitTally {
        entry: usize,
        digit: usize,
        counter: usize,
    },
    BeadFalls {
        entry: usize,
        pole: usize,
    },
    BeadRead {
        pole: usize,
        row: usize,
    },
    Rebuild {
        row: usize,
        position: usize,
        beads: usize,
    },
    BitonicMerge {
        block: usize,
        distance: usize,
    },
    Shuffle {
        number: u64,
    },
    ShuffleDraw {
        position: usize,
        chosen: usize,
        draws: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal<E> {
    Stopped(Stopped<E>),
    WholeNumbersNeeded,
    LengthNotAPowerOfTwo { length: usize },
    LimitReached { counts: Counts, steps: Vec<Step> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepRecording {
    Record,
    Skip,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sorted {
    pub arrangement: Vec<usize>,
    pub counts: Counts,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stopped<E> {
    pub error: E,
    pub left: usize,
    pub right: usize,
    pub counts: Counts,
    pub steps: Vec<Step>,
}

pub(crate) struct Run<'a, T, E, C>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    entries: &'a [T],
    compare: C,
    order: Order,
    recording: StepRecording,
    pub(crate) arrangement: Vec<usize>,
    counts: Counts,
    steps: Vec<Step>,
}

impl<T, E, C> Run<'_, T, E, C>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    pub(crate) fn len(&self) -> usize {
        self.arrangement.len()
    }

    pub(crate) fn entry_at(&self, position: usize) -> usize {
        self.arrangement[position]
    }

    pub(crate) fn precedes(&mut self, left: Located, right: Located) -> Result<bool, Stopped<E>> {
        Ok(self.ordering(left, right)? == Ordering::Less)
    }

    pub(crate) fn ordering(
        &mut self,
        left: Located,
        right: Located,
    ) -> Result<Ordering, Stopped<E>> {
        self.counts.comparisons += 1;
        let outcome = (self.compare)(&self.entries[left.entry], &self.entries[right.entry]);
        match outcome {
            Ok(ordering) => {
                self.record(Step::Compare {
                    left,
                    right,
                    outcome: Some(ordering),
                });
                Ok(match self.order {
                    Order::Increasing => ordering,
                    Order::Decreasing => ordering.reverse(),
                })
            }
            Err(error) => {
                self.record(Step::Compare {
                    left,
                    right,
                    outcome: None,
                });
                Err(Stopped {
                    error,
                    left: left.entry,
                    right: right.entry,
                    counts: self.counts,
                    steps: std::mem::take(&mut self.steps),
                })
            }
        }
    }

    pub(crate) fn merged_bitonic(&mut self, block: usize, distance: usize) {
        self.record(Step::BitonicMerge { block, distance });
    }

    pub(crate) fn shuffles(&self) -> u64 {
        self.counts.shuffles
    }

    pub(crate) fn shuffled(&mut self) {
        self.counts.shuffles += 1;
        self.record(Step::Shuffle {
            number: self.counts.shuffles,
        });
    }

    pub(crate) fn drew_partner(&mut self, position: usize, chosen: usize, draws: u64) {
        self.counts.draws += draws;
        self.record(Step::ShuffleDraw {
            position,
            chosen,
            draws,
        });
    }

    pub(crate) fn passed(&mut self, gap: usize) {
        self.record(Step::Pass { gap });
    }

    pub(crate) fn flipped(&mut self, length: usize) {
        self.counts.flips += 1;
        self.record(Step::Flip { length });
    }

    pub(crate) fn drew(&mut self, low: usize, high: usize, chosen: usize, draws: u64) {
        self.counts.draws += draws;
        self.record(Step::Draw {
            from: low,
            to: high - 1,
            chosen,
            draws,
        });
    }

    pub(crate) fn copy_to_scratch(&mut self, from: usize, slot: usize, scratch: &mut [usize]) {
        self.counts.writes += 1;
        let entry = self.arrangement[from];
        scratch[slot] = entry;
        self.record(Step::Write {
            entry,
            from: Place::List(from),
            to: Place::Scratch(slot),
        });
    }

    pub(crate) fn copy_back_from_scratch(&mut self, slot: usize, scratch: &[usize]) {
        self.write(scratch[slot], Place::Scratch(slot), slot);
    }

    pub(crate) fn write(&mut self, entry: usize, from: Place, to: usize) {
        self.counts.writes += 1;
        self.arrangement[to] = entry;
        self.record(Step::Write {
            entry,
            from,
            to: Place::List(to),
        });
    }

    pub(crate) fn exchange(&mut self, left: usize, right: usize) {
        self.counts.writes += 2;
        let (left_entry, right_entry) = (self.arrangement[left], self.arrangement[right]);
        self.arrangement.swap(left, right);
        self.record(Step::Exchange {
            left: Located {
                entry: left_entry,
                place: Place::List(left),
            },
            right: Located {
                entry: right_entry,
                place: Place::List(right),
            },
        });
    }

    fn record(&mut self, step: Step) {
        if self.recording == StepRecording::Record {
            self.steps.push(step);
        }
    }
}

pub fn sort<T, E, C>(
    entries: &[T],
    method: Method,
    order: Order,
    compare: C,
    recording: StepRecording,
) -> Result<Sorted, Refusal<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let mut run = Run {
        entries,
        compare,
        order,
        recording,
        arrangement: (0..entries.len()).collect(),
        counts: Counts::default(),
        steps: Vec::new(),
    };
    match method {
        Method::Insertion => insertion::sort(&mut run).map_err(Refusal::Stopped)?,
        Method::BinaryInsertion => binary_insertion::sort(&mut run).map_err(Refusal::Stopped)?,
        Method::Selection => selection::sort(&mut run).map_err(Refusal::Stopped)?,
        Method::Bubble(form) => bubble::sort(&mut run, form).map_err(Refusal::Stopped)?,
        Method::Merge => merge::sort(&mut run).map_err(Refusal::Stopped)?,
        Method::Heap => heap::sort(&mut run).map_err(Refusal::Stopped)?,
        Method::Quick(partition) => quick::sort(&mut run, partition).map_err(Refusal::Stopped)?,
        Method::DoubleSelection => placing::double_selection(&mut run).map_err(Refusal::Stopped)?,
        Method::CocktailShaker(form) => {
            neighbours::cocktail_shaker(&mut run, form).map_err(Refusal::Stopped)?;
        }
        Method::Gnome => neighbours::gnome(&mut run).map_err(Refusal::Stopped)?,
        Method::OddEven(form) => neighbours::odd_even(&mut run, form).map_err(Refusal::Stopped)?,
        Method::Comb(_) => neighbours::comb(&mut run).map_err(Refusal::Stopped)?,
        Method::Cycle => placing::cycle(&mut run).map_err(Refusal::Stopped)?,
        Method::Pancake => placing::pancake(&mut run).map_err(Refusal::Stopped)?,
        Method::Shell(gaps) => shell::sort(&mut run, gaps).map_err(Refusal::Stopped)?,
        Method::BottomUpMerge => merge::bottom_up(&mut run).map_err(Refusal::Stopped)?,
        Method::NaturalMerge => merge::natural(&mut run).map_err(Refusal::Stopped)?,
        Method::Bogo { seed, limit } => {
            if !bogo::sort(&mut run, seed, limit).map_err(Refusal::Stopped)? {
                return Err(Refusal::LimitReached {
                    counts: run.counts,
                    steps: run.steps,
                });
            }
        }
        Method::Bitonic => {
            let length = run.len();
            if length > 1 && !length.is_power_of_two() {
                return Err(Refusal::LengthNotAPowerOfTwo { length });
            }
            bitonic::sort(&mut run).map_err(Refusal::Stopped)?;
        }
        Method::Counting | Method::Radix { .. } | Method::Bead => {
            return Err(Refusal::WholeNumbersNeeded);
        }
    }
    Ok(Sorted {
        arrangement: run.arrangement,
        counts: run.counts,
        steps: run.steps,
    })
}

pub fn quick_sort_with_chosen_pivots<T, E, C>(
    entries: &[T],
    order: Order,
    compare: C,
    choose: &mut dyn FnMut(usize, usize) -> usize,
) -> Result<Sorted, Stopped<E>>
where
    C: FnMut(&T, &T) -> Result<Ordering, E>,
{
    let mut run = Run {
        entries,
        compare,
        order,
        recording: StepRecording::Skip,
        arrangement: (0..entries.len()).collect(),
        counts: Counts::default(),
        steps: Vec::new(),
    };
    quick::sort_with_chosen_pivots(&mut run, choose)?;
    Ok(Sorted {
        arrangement: run.arrangement,
        counts: run.counts,
        steps: run.steps,
    })
}
