use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::mpsc::Sender;

use calc_core::{Diagnostic, evaluate_f32, evaluate_f64};
use calc_exec::Backend;
use calc_exec::Domain;
use calc_expr::ExprPool;
use calc_numbers::{Integer, Number};
use calc_syntax::parse_expression;
use calc_viz::{Orbit, OrbitRequest, read_orbit};

use crate::picture::{PictureAddress, PictureSlot, PlotError};
use crate::platform::{Clock, Job, JobState};
use crate::result_record::{CalculatorVersion, LineId, ResultRecord};
use crate::session::{
    ExactOutcome, Precision, SessionError, Settings, evaluate_exactly, evaluate_in_machine,
};

const READING_NOT_PARSED_CODE: &str = "reading_not_parsed";
const ORBIT_NOT_READ_CODE: &str = "orbit_not_read";

#[derive(Clone, Debug, PartialEq)]
pub enum ReadCoordinate {
    Pointer(f64),
    Exact(Number),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReadRequest {
    pub line: LineId,
    pub slot: PictureSlot,
    pub generation: u64,
    pub layer: usize,
    pub at: Vec<ReadCoordinate>,
    pub commit: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ReadError {
    SceneNotComplete,
    LayerHasNoReadings { layer: usize },
    CoordinateCount { expected: usize, found: usize },
    CoordinateNotExact { axis: usize },
    ReadingNotBuilt,
    ReadingNeedsFunctionForm(LineId),
    Plot(PlotError),
    NotEntered(Box<SessionError>),
}

#[derive(Debug)]
pub enum ReadEvent {
    OrbitRead {
        line: LineId,
        slot: PictureSlot,
        orbit: Orbit,
    },
    ReadingFinished {
        line: LineId,
        slot: PictureSlot,
        value: Box<ResultRecord>,
    },
    ReadingFailed {
        line: LineId,
        slot: PictureSlot,
        error: Box<Diagnostic>,
    },
}

enum ReadWork {
    Expression {
        text: String,
        settings: Settings,
        clock: Arc<dyn Clock>,
    },
    Orbit(Box<OrbitRequest>),
}

pub struct ReadJob {
    work: ReadWork,
    backends: Arc<Vec<Arc<dyn Backend>>>,
    address: PictureAddress,
    events: Sender<ReadEvent>,
    cancellation: Arc<AtomicBool>,
}

impl ReadJob {
    pub(crate) fn new(
        text: String,
        backends: Arc<Vec<Arc<dyn Backend>>>,
        settings: Settings,
        clock: Arc<dyn Clock>,
        address: PictureAddress,
        events: Sender<ReadEvent>,
        cancellation: Arc<AtomicBool>,
    ) -> Self {
        Self {
            work: ReadWork::Expression {
                text,
                settings,
                clock,
            },
            backends,
            address,
            events,
            cancellation,
        }
    }

    pub(crate) fn orbit(
        request: Box<OrbitRequest>,
        backends: Arc<Vec<Arc<dyn Backend>>>,
        address: PictureAddress,
        events: Sender<ReadEvent>,
        cancellation: Arc<AtomicBool>,
    ) -> Self {
        Self {
            work: ReadWork::Orbit(request),
            backends,
            address,
            events,
            cancellation,
        }
    }

    pub fn generation(&self) -> u64 {
        self.address.generation
    }

    fn is_cancelled(&self) -> bool {
        self.cancellation.load(AtomicOrdering::SeqCst)
    }
}

impl Job for ReadJob {
    fn step(&mut self) -> JobState {
        if self.is_cancelled() {
            return JobState::Finished;
        }
        let (text, settings, clock) = match &self.work {
            ReadWork::Orbit(request) => {
                let backends: Vec<&dyn Backend> = self.backends.iter().map(AsRef::as_ref).collect();
                let event = match read_orbit(&backends, request) {
                    Ok(orbit) => ReadEvent::OrbitRead {
                        line: self.address.line,
                        slot: self.address.slot,
                        orbit,
                    },
                    Err(_) => ReadEvent::ReadingFailed {
                        line: self.address.line,
                        slot: self.address.slot,
                        error: Box::new(Diagnostic {
                            code: ORBIT_NOT_READ_CODE.to_owned(),
                            data: BTreeMap::new(),
                        }),
                    },
                };
                if !self.is_cancelled() {
                    let _ = self.events.send(event);
                }
                return JobState::Finished;
            }
            ReadWork::Expression {
                text,
                settings,
                clock,
            } => (text.clone(), *settings, Arc::clone(clock)),
        };
        let event = match evaluated_reading(&text, &self.backends, settings, clock.as_ref()) {
            Ok(record) => ReadEvent::ReadingFinished {
                line: self.address.line,
                slot: self.address.slot,
                value: Box::new(record),
            },
            Err(error) => ReadEvent::ReadingFailed {
                line: self.address.line,
                slot: self.address.slot,
                error: Box::new(error),
            },
        };
        if !self.is_cancelled() {
            let _ = self.events.send(event);
        }
        JobState::Finished
    }
}

fn evaluated_reading(
    text: &str,
    backends: &[Arc<dyn Backend>],
    settings: Settings,
    clock: &dyn Clock,
) -> Result<ResultRecord, Diagnostic> {
    let started = clock.monotonic_nanoseconds();
    let mut pool = ExprPool::new();
    let expression = parse_expression(&mut pool, text).map_err(|_| Diagnostic {
        code: READING_NOT_PARSED_CODE.to_owned(),
        data: BTreeMap::new(),
    })?;
    let (computed, backend) = match (
        evaluate_exactly(&mut pool, backends, settings, expression),
        settings.precision,
    ) {
        (ExactOutcome::Rational(evaluated), _) => *evaluated,
        (ExactOutcome::Failed(error), _) => return Err(error),
        (ExactOutcome::NeedsMachine(error), Precision::Exact) => return Err(error),
        (ExactOutcome::NeedsMachine(_), Precision::F64) => evaluate_in_machine(
            &mut pool,
            backends,
            settings,
            expression,
            evaluate_f64,
            Domain::F64,
        )?,
        (ExactOutcome::NeedsMachine(_), Precision::F32) => evaluate_in_machine(
            &mut pool,
            backends,
            settings,
            expression,
            evaluate_f32,
            Domain::F32,
        )?,
    };
    let finished = clock.monotonic_nanoseconds();
    Ok(ResultRecord::new(
        computed,
        backend,
        Vec::new(),
        clock.now_utc(),
        std::time::Duration::from_nanos(finished.saturating_sub(started)),
        CalculatorVersion::current(),
    ))
}

const SMALLEST_SNAP_EXPONENT: i32 = -60;
const LARGEST_SNAP_EXPONENT: i32 = 60;

pub fn snapped_coordinate(exact: &Number, step: &Number) -> Option<Number> {
    let half = step.div_exact(&Number::from(2_i64)).ok()?;
    if exact_sign(&half)? != Ordering::Greater {
        return None;
    }
    for exponent in (SMALLEST_SNAP_EXPONENT..=LARGEST_SNAP_EXPONENT).rev() {
        let power = power_of_ten(exponent)?;
        let candidate = round_to_multiple(exact, &power)?;
        let distance = candidate.sub_exact(exact).ok()?;
        if exact_order(&absolute(&distance)?, &half)? != Ordering::Greater {
            return Some(candidate);
        }
    }
    None
}

fn absolute(number: &Number) -> Option<Number> {
    match exact_sign(number)? {
        Ordering::Less => number.negate_exact().ok(),
        Ordering::Equal | Ordering::Greater => Some(number.clone()),
    }
}

fn power_of_ten(exponent: i32) -> Option<Number> {
    let magnitude = Integer::from(10_i64).pow(exponent.unsigned_abs());
    if exponent >= 0 {
        Some(Number::Integer(magnitude))
    } else {
        Number::fraction(&Integer::one(), &magnitude).ok()
    }
}

fn exact_sign(number: &Number) -> Option<Ordering> {
    let numerator = match number {
        Number::Integer(integer) => integer,
        Number::Rational(rational) => rational.numerator(),
        Number::F32(_) | Number::F64(_) => return None,
    };
    Some(if numerator.is_zero() {
        Ordering::Equal
    } else if numerator.is_negative() {
        Ordering::Less
    } else {
        Ordering::Greater
    })
}

fn exact_order(left: &Number, right: &Number) -> Option<Ordering> {
    left.sub_exact(right).ok().as_ref().and_then(exact_sign)
}

fn round_to_multiple(value: &Number, step: &Number) -> Option<Number> {
    let quotient = value.div_exact(step).ok()?;
    let (numerator, denominator) = match &quotient {
        Number::Integer(_) => return Some(value.clone()),
        Number::Rational(rational) => {
            (rational.numerator().clone(), rational.denominator().clone())
        }
        Number::F32(_) | Number::F64(_) => return None,
    };
    let (floor, remainder) = numerator.div_rem_euclid(&denominator).ok()?;
    let twice_remainder = &remainder * &Integer::from(2_i64);
    let rounded = match twice_remainder.cmp(&denominator) {
        Ordering::Less => floor,
        Ordering::Greater => &floor + &Integer::one(),
        Ordering::Equal => {
            let (_, parity) = floor.div_rem_euclid(&Integer::from(2_i64)).ok()?;
            if parity.is_zero() {
                floor
            } else {
                &floor + &Integer::one()
            }
        }
    };
    Number::Integer(rounded).mul_exact(step).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fraction(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator))
            .expect("a denominator that is not zero")
    }

    #[test]
    fn a_coordinate_is_snapped_to_the_decimal_with_the_fewest_digits_in_reach() {
        let snapped = snapped_coordinate(&fraction(7351, 10_000), &fraction(1, 100));

        assert_eq!(snapped, Some(fraction(74, 100)));
    }

    #[test]
    fn a_step_that_reaches_a_whole_number_snaps_to_it() {
        let snapped = snapped_coordinate(&fraction(21, 10), &Number::from(1_i64));

        assert_eq!(snapped, Some(Number::from(2_i64)));
    }

    #[test]
    fn zero_is_reached_before_any_other_decimal() {
        let snapped = snapped_coordinate(&fraction(-3, 100), &Number::from(1_i64));

        assert_eq!(snapped, Some(Number::from(0_i64)));
    }

    #[test]
    fn two_decimals_equally_near_give_the_one_with_the_even_last_digit() {
        let snapped = snapped_coordinate(&fraction(25, 10), &Number::from(1_i64));

        assert_eq!(snapped, Some(Number::from(2_i64)));
    }

    #[test]
    fn a_large_coordinate_keeps_only_the_digits_the_step_reaches() {
        let snapped = snapped_coordinate(&Number::from(1_234_567_i64), &Number::from(100_000_i64));

        assert_eq!(snapped, Some(Number::from(1_200_000_i64)));
    }

    #[test]
    fn a_machine_coordinate_is_not_snapped() {
        let snapped = snapped_coordinate(&Number::F64(0.5), &Number::from(1_i64));

        assert_eq!(snapped, None);
    }

    #[test]
    fn a_step_of_zero_snaps_nothing() {
        let snapped = snapped_coordinate(&fraction(1, 2), &Number::from(0_i64));

        assert_eq!(snapped, None);
    }
}
