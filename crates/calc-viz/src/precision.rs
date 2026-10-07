use std::cmp::Ordering;

use calc_exec::Domain;
use calc_numbers::{Integer, Number};

use crate::exact_order::compare_exact;
use crate::primitive::{Column, Primitive};
use crate::record::Interval;
use crate::style::StyleRole;
use crate::view::{View, ViewAxis};

const COLOUR_STEPS: i64 = 256;

pub(crate) fn grid_spacing(magnitude: &Number, domain: Domain) -> Option<Number> {
    match domain {
        Domain::F32 => {
            let rounded = magnitude.round_to_f32_ties_even().abs();
            Number::F32(rounded.next_up() - rounded).to_exact().ok()
        }
        Domain::F64 => {
            let rounded = magnitude.round_to_f64_ties_even().abs();
            Number::F64(rounded.next_up() - rounded).to_exact().ok()
        }
    }
}

fn absolute(number: &Number) -> Option<Number> {
    match compare_exact(number, &Number::from(0_i64))? {
        Ordering::Less => number.negate_exact().ok(),
        Ordering::Equal | Ordering::Greater => Some(number.clone()),
    }
}

pub(crate) fn is_grid_exhausted(interval: &Interval, fineness: &Number, domain: Domain) -> bool {
    let larger = match (absolute(&interval.lower), absolute(&interval.upper)) {
        (Some(lower), Some(upper)) => match compare_exact(&lower, &upper) {
            Some(Ordering::Greater) => lower,
            _ => upper,
        },
        _ => return false,
    };
    grid_spacing(&larger, domain)
        .and_then(|spacing| compare_exact(fineness, &spacing))
        .is_some_and(|order| order == Ordering::Less)
}

fn width(range: &Interval) -> Option<Number> {
    range.upper.sub_exact(&range.lower).ok()
}

pub(crate) fn is_unresolved_against(bound: f64, step: &Number) -> bool {
    Number::F64(bound)
        .to_exact()
        .ok()
        .and_then(|bound| bound.mul_exact(&Number::from(2_i64)).ok())
        .is_some_and(|twice| compare_exact(&twice, step) != Some(Ordering::Less))
}

pub(crate) fn axis_step(axis: &ViewAxis) -> Option<Number> {
    width(&axis.range)?
        .div_exact(&Number::Integer(Integer::from(u64::from(axis.divisions))))
        .ok()
}

pub(crate) fn colour_step(style: &StyleRole) -> Option<Number> {
    width(&style.colour_map.as_ref()?.range)?
        .div_exact(&Number::from(COLOUR_STEPS))
        .ok()
}

fn f64_values(column: &Column) -> Vec<f64> {
    match column {
        Column::F32(values) => values.iter().map(|value| f64::from(*value)).collect(),
        Column::F64(values) => values.clone(),
    }
}

#[derive(Default)]
pub(crate) struct ResolutionCounts {
    pub(crate) unresolved: u64,
    pub(crate) unknown: u64,
}

impl ResolutionCounts {
    fn count_unknown(&mut self) {
        self.unknown = self.unknown.saturating_add(1);
    }

    fn count_unresolved(&mut self) {
        self.unresolved = self.unresolved.saturating_add(1);
    }

    fn add_against_step(&mut self, bound: f64, step: Option<&Number>) {
        let Ok(exact_bound) = Number::F64(bound).to_exact() else {
            self.count_unknown();
            return;
        };
        let Some(step) = step else {
            return;
        };
        let twice = exact_bound.mul_exact(&Number::from(2_i64)).ok();
        if twice.is_some_and(|twice| compare_exact(&twice, step) != Some(Ordering::Less)) {
            self.count_unresolved();
        }
    }
}

fn view_axes(view: &View) -> Vec<&ViewAxis> {
    view.axes()
}

pub(crate) fn count_resolution(
    primitive: &Primitive,
    bounds: &[Column],
    view: &View,
    style: &StyleRole,
) -> ResolutionCounts {
    let mut counts = ResolutionCounts::default();
    let axes = view_axes(view);
    let last_axis_step = axes.last().and_then(|axis| axis_step(axis));
    let bound_values: Vec<Vec<f64>> = bounds.iter().map(f64_values).collect();
    match primitive {
        Primitive::Polyline(_)
        | Primitive::Points(_)
        | Primitive::Band(_)
        | Primitive::TriangleMesh(_) => {
            for bound in bound_values.iter().flatten() {
                counts.add_against_step(*bound, last_axis_step.as_ref());
            }
        }
        Primitive::Arrows(_) => {
            for (component, column) in bound_values.iter().enumerate() {
                let step = axes.get(component).and_then(|axis| axis_step(axis));
                for bound in column {
                    counts.add_against_step(*bound, step.as_ref());
                }
            }
        }
        Primitive::ScalarGrid(_) => {
            let step = colour_step(style);
            for bound in bound_values.iter().flatten() {
                counts.add_against_step(*bound, step.as_ref());
            }
        }
        Primitive::ComplexGrid(_) => {
            let step = colour_step(style);
            if let [real, imaginary] = bound_values.as_slice() {
                for (real, imaginary) in real.iter().zip(imaginary) {
                    let larger = if real.is_nan() || imaginary.is_nan() {
                        f64::NAN
                    } else if real >= imaginary {
                        *real
                    } else {
                        *imaginary
                    };
                    counts.add_against_step(larger, step.as_ref());
                }
            }
        }
        Primitive::Voxels(voxels) => {
            let values = voxels.scalar.as_ref().map(f64_values).unwrap_or_default();
            for (bound, value) in bound_values.iter().flatten().zip(values) {
                match (
                    Number::F64(*bound).to_exact(),
                    Number::F64(value.abs()).to_exact(),
                ) {
                    (Ok(bound), Ok(magnitude)) => {
                        if compare_exact(&bound, &magnitude) != Some(Ordering::Less) {
                            counts.count_unresolved();
                        }
                    }
                    (Err(_), _) => counts.count_unknown(),
                    (Ok(_), Err(_)) => counts.count_unresolved(),
                }
            }
        }
        Primitive::Graph(_)
        | Primitive::Formula(_)
        | Primitive::BitLayout(_)
        | Primitive::Figure(_) => {}
    }
    counts
}
