use calc_core::{
    ExactCurve, SampleEnclosure, SampleInput, SamplePosition, attained_over, enclose_over,
    enclose_sample, enclose_slope_over, exact_curve, is_continuous_where_enclosed, slope_of,
};
use calc_expr::{ExprId, ExprPool, SymbolId};
use calc_numbers::{Integer, Interval as MachineInterval, Number};

use crate::precision::is_unresolved_against;
use crate::scene::ColumnEnclosures;

const EXACT_SUBDIVISIONS: [u32; 2] = [4, 16];
const MACHINE_SUBDIVISIONS: [u32; 2] = [1, 8];

struct Edge {
    lower: f64,
    upper: f64,
}

pub(crate) struct ColumnWork {
    exact: Option<ExactCurve>,
    slope: Option<ExprId>,
    is_continuous: bool,
    positions: Vec<Number>,
    edges: Vec<Option<Edge>>,
    clipped_edges: Vec<bool>,
    attained: Vec<Option<(f64, f64)>>,
    enclosures: ColumnEnclosures,
    exact_values: Option<Vec<f64>>,
    exact_bounds: Option<Vec<f64>>,
}

fn outward(value: &Number) -> Option<Edge> {
    let interval = MachineInterval::from_exact(value)?;
    Some(Edge {
        lower: interval.lower(),
        upper: interval.upper(),
    })
}

fn nearest(value: &Number) -> f64 {
    value.round_to_f64_ties_even()
}

fn machine_edge(value: f64, bound: f64) -> Option<Edge> {
    (value.is_finite() && bound.is_finite()).then(|| Edge {
        lower: (value - bound).next_down(),
        upper: (value + bound).next_up(),
    })
}

fn point_edge(pool: &ExprPool, root: ExprId, symbol: SymbolId, at: &Number) -> Option<Edge> {
    let input = SampleInput {
        symbol,
        position: SamplePosition::Real(at.clone()),
    };
    match enclose_sample(pool, root, &[input]) {
        SampleEnclosure::Real(Some(interval)) => Some(Edge {
            lower: interval.lower(),
            upper: interval.upper(),
        }),
        _ => None,
    }
}

fn subdivided(low: &Number, high: &Number, parts: u32) -> Option<Vec<Number>> {
    let width = high.sub_exact(low).ok()?;
    (0..=parts)
        .map(|part| {
            let share = Number::fraction(
                &Integer::from(i64::from(part)),
                &Integer::from(i64::from(parts)),
            )
            .ok()?;
            low.add_exact(&width.mul_exact(&share).ok()?).ok()
        })
        .collect()
}

fn slope_span(
    pool: &ExprPool,
    slope: ExprId,
    pair: &[Number],
    (start, end): (&Edge, &Edge),
) -> Option<(f64, f64)> {
    let (low, high) = enclose_slope_over(pool, slope, &pair[0], &pair[1])?;
    let steepness = MachineInterval::point(low)?.hull(&MachineInterval::point(high)?);
    let width = MachineInterval::from_exact(&pair[1].sub_exact(&pair[0]).ok()?)?;
    let reach = steepness.mul(&MachineInterval::point(0.0)?.hull(&width))?;
    let start = MachineInterval::point(start.lower)?.hull(&MachineInterval::point(start.upper)?);
    let end = MachineInterval::point(end.lower)?.hull(&MachineInterval::point(end.upper)?);
    let forward = start.add(&reach)?;
    let backward = end.sub(&reach)?;
    let lower = forward.lower().max(backward.lower());
    let upper = forward.upper().min(backward.upper());
    (lower <= upper).then_some((lower, upper))
}

fn widest(proven: (f64, f64), attained: Option<(f64, f64)>) -> (f64, f64) {
    match attained {
        Some((lower, upper)) if proven.0.is_nan() => (lower, upper),
        Some((lower, upper)) => (proven.0.min(lower), proven.1.max(upper)),
        None => proven,
    }
}

fn proven_span(edges: &[&Edge], is_continuous: bool) -> (f64, f64) {
    if !is_continuous || edges.is_empty() {
        return (f64::NAN, f64::NAN);
    }
    let lowest_upper = edges
        .iter()
        .map(|edge| edge.upper)
        .fold(f64::INFINITY, f64::min);
    let highest_lower = edges
        .iter()
        .map(|edge| edge.lower)
        .fold(f64::NEG_INFINITY, f64::max);
    if lowest_upper <= highest_lower {
        (lowest_upper, highest_lower)
    } else {
        (f64::NAN, f64::NAN)
    }
}

impl ColumnWork {
    pub(crate) fn new(
        pool: &mut ExprPool,
        root: ExprId,
        symbol: SymbolId,
        positions: &[Number],
        values: &[f64],
        bounds: &[f64],
    ) -> ColumnWork {
        let exact = exact_curve(pool, root, symbol);
        let is_continuous = exact.is_some() || is_continuous_where_enclosed(pool, root);
        let slope = if exact.is_none() {
            slope_of(pool, root, symbol)
        } else {
            None
        };
        let mut exact_values = None;
        let mut exact_bounds = None;
        let mut clipped_edges = vec![false; positions.len()];
        let edges: Vec<Option<Edge>> = match &exact {
            Some(curve) => {
                let exact_edges: Vec<Option<Number>> =
                    positions.iter().map(|at| curve.value_at(at)).collect();
                let shown: Vec<f64> = exact_edges
                    .iter()
                    .map(|value| value.as_ref().map_or(f64::NAN, nearest))
                    .collect();
                exact_bounds = Some(
                    exact_edges
                        .iter()
                        .zip(&shown)
                        .map(|(value, shown)| {
                            value
                                .as_ref()
                                .and_then(MachineInterval::from_exact)
                                .and_then(|interval| interval.distance_bound(*shown))
                                .unwrap_or(f64::NAN)
                        })
                        .collect(),
                );
                exact_values = Some(shown);
                exact_edges
                    .iter()
                    .map(|value| value.as_ref().and_then(outward))
                    .collect()
            }
            None => values
                .iter()
                .zip(bounds)
                .zip(positions)
                .enumerate()
                .map(|(index, ((value, bound), at))| {
                    machine_edge(*value, *bound).or_else(|| {
                        let (lower, upper, clipped) = enclose_over(pool, root, symbol, at, at)?;
                        clipped_edges[index] = clipped;
                        Some(Edge { lower, upper })
                    })
                })
                .collect(),
        };
        let attained = if exact.is_none() && is_continuous {
            positions
                .windows(2)
                .map(|pair| attained_over(pool, root, symbol, &pair[0], &pair[1]))
                .collect()
        } else {
            Vec::new()
        };
        let mut work = ColumnWork {
            exact,
            slope,
            is_continuous,
            positions: positions.to_vec(),
            edges,
            clipped_edges,
            attained,
            enclosures: ColumnEnclosures::default(),
            exact_values,
            exact_bounds,
        };
        let count = positions.len().saturating_sub(1);
        for column in 0..count {
            let (enclosed, proven) = work.enclose(pool, root, symbol, column, 1, false);
            work.enclosures.lower.push(enclosed.0);
            work.enclosures.upper.push(enclosed.1);
            work.enclosures.hit_lower.push(proven.0);
            work.enclosures.hit_upper.push(proven.1);
            work.enclosures.marked.push(false);
        }
        work
    }

    fn enclose(
        &mut self,
        pool: &ExprPool,
        root: ExprId,
        symbol: SymbolId,
        column: usize,
        parts: u32,
        uses_slope: bool,
    ) -> ((f64, f64), (f64, f64)) {
        let none = ((f64::NAN, f64::NAN), (f64::NAN, f64::NAN));
        let (Some(low), Some(high)) = (self.positions.get(column), self.positions.get(column + 1))
        else {
            return none;
        };
        let Some(points) = subdivided(low, high, parts) else {
            return none;
        };
        let mut edges: Vec<Edge> = Vec::with_capacity(points.len());
        let mut is_clipped = self.clipped_edges.get(column) == Some(&true)
            || self.clipped_edges.get(column + 1) == Some(&true);
        for (index, at) in points.iter().enumerate() {
            let known = match index {
                0 => self.edges.get(column).and_then(|edge| {
                    edge.as_ref().map(|edge| Edge {
                        lower: edge.lower,
                        upper: edge.upper,
                    })
                }),
                last if last == points.len() - 1 => self.edges.get(column + 1).and_then(|edge| {
                    edge.as_ref().map(|edge| Edge {
                        lower: edge.lower,
                        upper: edge.upper,
                    })
                }),
                _ => match &self.exact {
                    Some(curve) => curve.value_at(at).as_ref().and_then(outward),
                    None => point_edge(pool, root, symbol, at),
                },
            };
            let clipped_edge = || {
                enclose_over(pool, root, symbol, at, at)
                    .map(|(lower, upper, clipped)| (Edge { lower, upper }, clipped))
            };
            match (
                known,
                self.exact.is_none() && index > 0 && index + 1 < points.len(),
            ) {
                (Some(edge), _) => edges.push(edge),
                (None, true) => match clipped_edge() {
                    Some((edge, clipped)) => {
                        is_clipped |= clipped;
                        edges.push(edge);
                    }
                    None => return none,
                },
                (None, false) => return none,
            }
        }
        let mut lowest = f64::INFINITY;
        let mut highest = f64::NEG_INFINITY;
        for (index, pair) in points.windows(2).enumerate() {
            let span = match &mut self.exact {
                Some(curve) => curve
                    .enclose(&pair[0], &pair[1])
                    .and_then(|(low, high)| Some((outward(&low)?.lower, outward(&high)?.upper))),
                None => enclose_over(pool, root, symbol, &pair[0], &pair[1]).map(
                    |(low, high, clipped)| {
                        is_clipped |= clipped;
                        (low, high)
                    },
                ),
            };
            let Some((mut low, mut high)) = span else {
                return none;
            };
            if uses_slope
                && let Some(slope) = self.slope
                && let Some((from_low, from_high)) =
                    slope_span(pool, slope, pair, (&edges[index], &edges[index + 1]))
            {
                low = low.max(from_low);
                high = high.min(from_high);
            }
            lowest = lowest.min(low);
            highest = highest.max(high);
        }
        let references: Vec<&Edge> = edges.iter().collect();
        let is_proven_continuous = self.is_continuous && !is_clipped;
        let proven = proven_span(&references, is_proven_continuous);
        let attained = self
            .attained
            .get(column)
            .copied()
            .flatten()
            .filter(|_| is_proven_continuous);
        ((lowest, highest), widest(proven, attained))
    }

    fn slack(&self, column: usize) -> f64 {
        let enclosed = self.enclosures.upper[column] - self.enclosures.lower[column];
        let proven = self.enclosures.hit_upper[column] - self.enclosures.hit_lower[column];
        if proven.is_nan() {
            enclosed
        } else {
            enclosed - proven
        }
    }

    pub(crate) fn refine(&mut self, pool: &ExprPool, root: ExprId, symbol: SymbolId, step: f64) {
        let subdivisions: &[u32] = if self.exact.is_some() {
            &EXACT_SUBDIVISIONS
        } else {
            &MACHINE_SUBDIVISIONS
        };
        for column in 0..self.enclosures.lower.len() {
            if self.enclosures.marked[column] {
                continue;
            }
            for parts in subdivisions {
                if self.slack(column) <= step {
                    break;
                }
                let (enclosed, proven) = self.enclose(pool, root, symbol, column, *parts, true);
                if enclosed.0.is_nan() {
                    break;
                }
                let lower = self.enclosures.lower[column].max(enclosed.0);
                let upper = self.enclosures.upper[column].min(enclosed.1);
                self.enclosures.lower[column] = lower;
                self.enclosures.upper[column] = upper;
                if !proven.0.is_nan() {
                    self.enclosures.hit_lower[column] = proven.0;
                    self.enclosures.hit_upper[column] = proven.1;
                }
            }
        }
    }

    pub(crate) fn mark(&mut self, step: &Number) {
        if self.exact.is_some() {
            return;
        }
        let unresolved = |index: usize| match self.edges.get(index) {
            Some(Some(edge)) => is_unresolved_against((edge.upper - edge.lower) / 2.0, step),
            _ => true,
        };
        for column in 0..self.enclosures.marked.len() {
            let is_enclosed = self.enclosures.lower[column].is_finite()
                && self.enclosures.upper[column].is_finite();
            self.enclosures.marked[column] =
                is_enclosed && (unresolved(column) || unresolved(column + 1));
        }
    }

    pub(crate) fn exact_values(&self) -> Option<(&[f64], &[f64])> {
        Some((self.exact_values.as_deref()?, self.exact_bounds.as_deref()?))
    }

    pub(crate) fn note_spread(&mut self, values: &[f64], bounds: &[f64]) {
        if self.exact.is_some() {
            return;
        }
        let finite: Vec<(f64, f64)> = values
            .iter()
            .zip(bounds)
            .filter(|(value, bound)| value.is_finite() && bound.is_finite())
            .map(|(value, bound)| (*value, *bound))
            .collect();
        let lowest = finite
            .iter()
            .map(|(value, _)| *value)
            .fold(f64::INFINITY, f64::min);
        let highest = finite
            .iter()
            .map(|(value, _)| *value)
            .fold(f64::NEG_INFINITY, f64::max);
        let mut sorted: Vec<f64> = finite.iter().map(|(_, bound)| *bound).collect();
        sorted.sort_by(f64::total_cmp);
        let middle = sorted.get(sorted.len() / 2).copied().unwrap_or(0.0);
        self.enclosures.varies_below_bounds = middle > 0.0 && highest - lowest < 2.0 * middle;
    }

    pub(crate) fn into_enclosures(self) -> ColumnEnclosures {
        self.enclosures
    }
}
