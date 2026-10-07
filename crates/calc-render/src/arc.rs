use std::f64::consts::FRAC_PI_4;

use calc_numbers::{cos_f64, sin_f64};
use tiny_skia::PathBuilder;

use crate::mapping::{ScreenPoint, to_f32};

const LAYOUT_POINTS_PER_PIECE: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CircularArc {
    pub centre: ScreenPoint,
    pub radius: f64,
    pub start: f64,
    pub sweep: f64,
}

impl CircularArc {
    fn piece_count(&self) -> u32 {
        let mut count = 1_u32;
        while self.sweep.abs() / f64::from(count) > FRAC_PI_4 && count < u32::from(u8::MAX) {
            count += 1;
        }
        count
    }

    pub(crate) fn point_at(&self, angle: f64) -> ScreenPoint {
        (
            self.centre.0 + self.radius * cos_f64(angle),
            self.centre.1 + self.radius * sin_f64(angle),
        )
    }

    pub(crate) fn push_to(&self, builder: &mut PathBuilder, starts_contour: bool) {
        let count = self.piece_count();
        let piece = self.sweep / f64::from(count);
        let (start_x, start_y) = self.point_at(self.start);
        if starts_contour {
            builder.move_to(to_f32(start_x), to_f32(start_y));
        } else {
            builder.line_to(to_f32(start_x), to_f32(start_y));
        }
        for index in 0..count {
            let from = self.start + piece * f64::from(index);
            let middle = from + piece / 2.0;
            let control_distance = self.radius / cos_f64(piece / 2.0);
            let control = (
                self.centre.0 + control_distance * cos_f64(middle),
                self.centre.1 + control_distance * sin_f64(middle),
            );
            let end = self.point_at(from + piece);
            builder.quad_to(
                to_f32(control.0),
                to_f32(control.1),
                to_f32(end.0),
                to_f32(end.1),
            );
        }
    }

    pub(crate) fn layout_points(&self) -> Vec<ScreenPoint> {
        let steps = self.piece_count() * LAYOUT_POINTS_PER_PIECE;
        (0..=steps)
            .map(|index| {
                self.point_at(self.start + self.sweep * f64::from(index) / f64::from(steps))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    fn quarter() -> CircularArc {
        CircularArc {
            centre: (10.0, 10.0),
            radius: 5.0,
            start: 0.0,
            sweep: PI / 2.0,
        }
    }

    #[test]
    fn quarter_arc_is_built_from_two_quadratic_pieces() {
        assert_eq!(quarter().piece_count(), 2);
    }

    #[test]
    fn full_circle_is_built_from_eight_pieces() {
        let circle = CircularArc {
            sweep: 2.0 * PI,
            ..quarter()
        };

        assert_eq!(circle.piece_count(), 8);
    }

    #[test]
    fn layout_points_end_at_the_arc_end() {
        let points = quarter().layout_points();
        let (x, y) = points[points.len() - 1];

        assert!((x - 10.0).abs() < 1e-9 && (y - 15.0).abs() < 1e-9);
    }

    #[test]
    fn path_of_an_arc_has_its_start_point_first() {
        let mut builder = PathBuilder::new();

        quarter().push_to(&mut builder, true);
        let path = builder.finish().unwrap();

        assert_eq!(path.points()[0], tiny_skia::Point::from_xy(15.0, 10.0));
    }
}
