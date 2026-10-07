use std::f64::consts::PI;

use calc_numbers::{Number, cos_f64, ln_f64, sin_f64, tan_f64};
use calc_viz::{Camera, Column, Projection, Scale, ViewAxis};

const MANTISSA_BITS: u32 = 52;
const EXPONENT_MASK: u64 = 0x7ff;
const EXPONENT_BIAS: u64 = 1_023;
const LARGEST_PIXEL: f64 = 65_535.0;
const CUBE_HALF_DIAGONAL_SQUARED: f64 = 3.0;
const EYE_DISTANCE_IN_HALF_DIAGONALS: f64 = 4.0;
const DEGREES_PER_HALF_TURN: f64 = 180.0;

pub(crate) type ScreenPoint = (f64, f64);

pub(crate) fn to_f32(value: f64) -> f32 {
    Number::F64(value).round_to_f32_ties_even()
}

pub(crate) fn pixel_floor(value: f64) -> u16 {
    if value.is_nan() || value < 1.0 {
        return 0;
    }
    if value >= LARGEST_PIXEL {
        return u16::MAX;
    }
    let bits = value.to_bits();
    let biased = (bits >> MANTISSA_BITS) & EXPONENT_MASK;
    let Some(exponent) = biased.checked_sub(EXPONENT_BIAS) else {
        return 0;
    };
    let mantissa = (bits & ((1_u64 << MANTISSA_BITS) - 1)) | (1_u64 << MANTISSA_BITS);
    let shift = u64::from(MANTISSA_BITS).saturating_sub(exponent);
    let whole = mantissa
        .checked_shr(u32::try_from(shift).unwrap_or(u32::MAX))
        .unwrap_or(0);
    u16::try_from(whole).unwrap_or(u16::MAX)
}

pub(crate) fn pixel_round(value: f64) -> u16 {
    pixel_floor((value + 0.5).floor())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AxisMap {
    lower_f64: f64,
    lower_f32: f32,
    width: f64,
    start: f64,
    length: f64,
    is_inverted: bool,
    logarithm: Option<(f64, f64)>,
}

impl AxisMap {
    pub(crate) fn new(
        axis: &ViewAxis,
        start: f64,
        length: f64,
        is_inverted: bool,
    ) -> Option<AxisMap> {
        let lower_f64 = axis.range.lower.round_to_f64_ties_even();
        let width = axis
            .range
            .upper
            .sub_exact(&axis.range.lower)
            .ok()?
            .round_to_f64_ties_even();
        let logarithm = match axis.scale {
            Scale::Linear => None,
            Scale::Logarithmic => {
                let upper = axis.range.upper.round_to_f64_ties_even();
                if lower_f64 <= 0.0 || !lower_f64.is_finite() || !upper.is_finite() {
                    return None;
                }
                let lower_log = ln_f64(lower_f64);
                Some((lower_log, ln_f64(upper) - lower_log))
            }
        };
        let is_usable = width > 0.0 && width.is_finite() && lower_f64.is_finite();
        is_usable.then_some(AxisMap {
            lower_f64,
            lower_f32: axis.range.lower.round_to_f32_ties_even(),
            width,
            start,
            length,
            is_inverted,
            logarithm,
        })
    }

    fn place(&self, fraction: f64) -> Option<f64> {
        let along = fraction * self.length;
        let position = if self.is_inverted {
            self.start + self.length - along
        } else {
            self.start + along
        };
        position.is_finite().then_some(position)
    }

    fn logarithmic_fraction(&self, value: f64) -> Option<f64> {
        let (lower_log, span) = self.logarithm?;
        (value > 0.0 && value.is_finite()).then(|| (ln_f64(value) - lower_log) / span)
    }

    pub(crate) fn of_f64(&self, value: f64) -> Option<f64> {
        if !value.is_finite() {
            return None;
        }
        let fraction = match self.logarithm {
            Some(_) => self.logarithmic_fraction(value)?,
            None => (value - self.lower_f64) / self.width,
        };
        self.place(fraction)
    }

    pub(crate) fn of_f32(&self, value: f32) -> Option<f64> {
        if !value.is_finite() {
            return None;
        }
        let fraction = match self.logarithm {
            Some(_) => self.logarithmic_fraction(f64::from(value))?,
            None => f64::from(value - self.lower_f32) / self.width,
        };
        self.place(fraction)
    }

    pub(crate) fn of_column(&self, column: &Column, index: usize) -> Option<f64> {
        match column {
            Column::F32(values) => self.of_f32(*values.get(index)?),
            Column::F64(values) => self.of_f64(*values.get(index)?),
        }
    }

    pub(crate) fn of_exact(&self, axis: &ViewAxis, value: &Number) -> Option<f64> {
        let fraction = match self.logarithm {
            Some(_) => self.logarithmic_fraction(value.round_to_f64_ties_even())?,
            None => value
                .sub_exact(&axis.range.lower)
                .ok()?
                .div_exact(&axis.range.upper.sub_exact(&axis.range.lower).ok()?)
                .ok()?
                .round_to_f64_ties_even(),
        };
        self.place(fraction)
    }
}

pub(crate) fn column_value(column: &Column, index: usize) -> Option<f64> {
    match column {
        Column::F32(values) => values.get(index).map(|value| f64::from(*value)),
        Column::F64(values) => values.get(index).copied(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SpaceProjection {
    right: [f64; 3],
    up: [f64; 3],
    forward: [f64; 3],
    eye: [f64; 3],
    perspective: Option<f64>,
    centre: ScreenPoint,
    half: f64,
}

fn dot(first: [f64; 3], second: [f64; 3]) -> f64 {
    first[0] * second[0] + first[1] * second[1] + first[2] * second[2]
}

fn radians(degrees: &Number) -> f64 {
    degrees.round_to_f64_ties_even() * (PI / DEGREES_PER_HALF_TURN)
}

impl SpaceProjection {
    pub(crate) fn new(camera: &Camera, centre: ScreenPoint, half: f64) -> SpaceProjection {
        let azimuth = radians(&camera.azimuth_degrees);
        let elevation = radians(&camera.elevation_degrees);
        let (azimuth_sine, azimuth_cosine) = (sin_f64(azimuth), cos_f64(azimuth));
        let (elevation_sine, elevation_cosine) = (sin_f64(elevation), cos_f64(elevation));
        let distance = EYE_DISTANCE_IN_HALF_DIAGONALS * CUBE_HALF_DIAGONAL_SQUARED.sqrt();
        let direction = [
            elevation_cosine * azimuth_cosine,
            elevation_cosine * azimuth_sine,
            elevation_sine,
        ];
        let perspective = match &camera.projection {
            Projection::Orthographic => None,
            Projection::Perspective {
                field_of_view_degrees,
            } => Some(1.0 / tan_f64(radians(field_of_view_degrees) / 2.0)),
        };
        SpaceProjection {
            right: [-azimuth_sine, azimuth_cosine, 0.0],
            up: [
                -elevation_sine * azimuth_cosine,
                -elevation_sine * azimuth_sine,
                elevation_cosine,
            ],
            forward: direction.map(|component| -component),
            eye: direction.map(|component| component * distance),
            perspective,
            centre,
            half,
        }
    }

    pub(crate) fn project(&self, point: [f64; 3]) -> Option<(f64, f64, f64)> {
        if point.iter().any(|component| !component.is_finite()) {
            return None;
        }
        let relative = [
            point[0] - self.eye[0],
            point[1] - self.eye[1],
            point[2] - self.eye[2],
        ];
        let depth = dot(relative, self.forward);
        let (across, upward) = match self.perspective {
            None => {
                let scale = self.half / CUBE_HALF_DIAGONAL_SQUARED.sqrt();
                (dot(point, self.right) * scale, dot(point, self.up) * scale)
            }
            Some(focal) => {
                if depth <= 0.0 {
                    return None;
                }
                let scale = focal * self.half / depth;
                (
                    dot(relative, self.right) * scale,
                    dot(relative, self.up) * scale,
                )
            }
        };
        let x = self.centre.0 + across;
        let y = self.centre.1 - upward;
        (x.is_finite() && y.is_finite()).then_some((x, y, depth))
    }

    pub(crate) fn faces_viewer(&self, normal: [f64; 3], point: [f64; 3]) -> bool {
        let towards_eye = match self.perspective {
            None => self.forward.map(|component| -component),
            Some(_) => [
                self.eye[0] - point[0],
                self.eye[1] - point[1],
                self.eye[2] - point[2],
            ],
        };
        dot(normal, towards_eye) > 0.0
    }
}

#[cfg(test)]
mod tests {
    use calc_viz::{AxisUnit, Dimension, Interval};

    use super::*;

    fn axis(lower: i64, upper: i64, scale: Scale) -> ViewAxis {
        ViewAxis {
            range: Interval {
                lower: Number::from(lower),
                upper: Number::from(upper),
            },
            scale,
            dimension: Dimension::DIMENSIONLESS,
            unit: AxisUnit::dimensionless(),
            divisions: 100,
        }
    }

    fn camera(azimuth: i64, elevation: i64) -> Camera {
        Camera {
            azimuth_degrees: Number::from(azimuth),
            elevation_degrees: Number::from(elevation),
            projection: Projection::Orthographic,
        }
    }

    #[test]
    fn pixel_floor_drops_the_fraction() {
        assert_eq!(pixel_floor(37.99), 37);
    }

    #[test]
    fn pixel_floor_of_a_power_of_two_is_exact() {
        assert_eq!(pixel_floor(1024.0), 1024);
    }

    #[test]
    fn pixel_floor_clamps_negative_values_to_zero() {
        assert_eq!(pixel_floor(-3.5), 0);
    }

    #[test]
    fn pixel_floor_clamps_large_values_to_the_largest_pixel() {
        assert_eq!(pixel_floor(1e300), u16::MAX);
    }

    #[test]
    fn pixel_floor_of_nan_is_zero() {
        assert_eq!(pixel_floor(f64::NAN), 0);
    }

    #[test]
    fn pixel_round_takes_the_nearest_pixel() {
        assert_eq!(pixel_round(4.5), 5);
    }

    #[test]
    fn linear_axis_maps_its_range_onto_its_pixels() {
        let map = AxisMap::new(&axis(-1, 3, Scale::Linear), 10.0, 400.0, false).unwrap();

        assert_eq!(map.of_f64(1.0), Some(210.0));
    }

    #[test]
    fn inverted_axis_puts_the_lower_bound_at_the_far_end() {
        let map = AxisMap::new(&axis(0, 2, Scale::Linear), 0.0, 100.0, true).unwrap();

        assert_eq!(map.of_f64(0.0), Some(100.0));
    }

    #[test]
    fn f64_column_near_one_keeps_its_offset_from_the_view_origin() {
        let range = Interval {
            lower: Number::F64(1.0).to_exact().unwrap(),
            upper: Number::F64(1.0 + 1024.0 * f64::EPSILON).to_exact().unwrap(),
        };
        let deep = ViewAxis {
            range,
            ..axis(0, 1, Scale::Linear)
        };
        let map = AxisMap::new(&deep, 0.0, 1024.0, false).unwrap();

        assert_eq!(map.of_f64(1.0 + f64::EPSILON), Some(1.0));
    }

    #[test]
    fn f32_column_subtracts_the_origin_in_f32() {
        let map = AxisMap::new(&axis(1, 2, Scale::Linear), 0.0, 100.0, false).unwrap();

        assert_eq!(map.of_f32(1.5), Some(50.0));
    }

    #[test]
    fn non_finite_value_has_no_position() {
        let map = AxisMap::new(&axis(0, 1, Scale::Linear), 0.0, 100.0, false).unwrap();

        assert_eq!(map.of_f64(f64::INFINITY), None);
    }

    #[test]
    fn logarithmic_axis_places_the_geometric_midpoint_in_the_middle() {
        let map = AxisMap::new(&axis(1, 100, Scale::Logarithmic), 0.0, 200.0, false).unwrap();

        let position = map.of_f64(10.0).unwrap();

        assert!((position - 100.0).abs() < 1e-9);
    }

    #[test]
    fn logarithmic_axis_over_zero_has_no_map() {
        assert_eq!(
            AxisMap::new(&axis(0, 100, Scale::Logarithmic), 0.0, 200.0, false),
            None
        );
    }

    #[test]
    fn exact_value_is_placed_from_its_exact_fraction() {
        let view_axis = axis(0, 3, Scale::Linear);
        let map = AxisMap::new(&view_axis, 0.0, 300.0, false).unwrap();

        assert_eq!(map.of_exact(&view_axis, &Number::from(1_i64)), Some(100.0));
    }

    #[test]
    fn camera_in_front_of_the_x_axis_shows_y_to_the_right() {
        let projection = SpaceProjection::new(&camera(0, 0), (100.0, 100.0), 50.0);

        let (x, _, _) = projection.project([0.0, 1.0, 0.0]).unwrap();

        assert!(x > 100.0);
    }

    #[test]
    fn camera_shows_z_upward() {
        let projection = SpaceProjection::new(&camera(30, 20), (100.0, 100.0), 50.0);

        let (_, y, _) = projection.project([0.0, 0.0, 1.0]).unwrap();

        assert!(y < 100.0);
    }

    #[test]
    fn nearer_point_has_the_smaller_depth() {
        let projection = SpaceProjection::new(&camera(0, 0), (0.0, 0.0), 50.0);

        let (_, _, near) = projection.project([1.0, 0.0, 0.0]).unwrap();
        let (_, _, far) = projection.project([-1.0, 0.0, 0.0]).unwrap();

        assert!(near < far);
    }

    #[test]
    fn cube_fits_inside_the_half_size() {
        let projection = SpaceProjection::new(&camera(45, 35), (0.0, 0.0), 50.0);

        let (x, y, _) = projection.project([1.0, 1.0, 1.0]).unwrap();

        assert!((x * x + y * y).sqrt() <= 50.0 + 1e-9);
    }

    #[test]
    fn face_turned_towards_the_eye_faces_the_viewer() {
        let projection = SpaceProjection::new(&camera(0, 0), (0.0, 0.0), 50.0);

        assert!(projection.faces_viewer([1.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
    }
}
