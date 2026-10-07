use tiny_skia::{Path, PathBuilder};

use crate::geometry::PhysicalRect;
use crate::mapping::{ScreenPoint, to_f32};

type ClipEdge = (
    fn(ScreenPoint, f64) -> bool,
    fn(ScreenPoint, ScreenPoint, f64) -> ScreenPoint,
    f64,
);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Guard {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl Guard {
    pub(crate) fn around(area: PhysicalRect) -> Guard {
        let reach = f64::from(area.width.max(area.height));
        Guard {
            left: f64::from(area.x) - reach,
            top: f64::from(area.y) - reach,
            right: f64::from(area.right()) + reach,
            bottom: f64::from(area.bottom()) + reach,
        }
    }

    pub(crate) fn contains(&self, point: ScreenPoint) -> bool {
        point.0 >= self.left
            && point.0 <= self.right
            && point.1 >= self.top
            && point.1 <= self.bottom
    }

    pub(crate) fn clip_segment(
        &self,
        from: ScreenPoint,
        to: ScreenPoint,
    ) -> Option<(ScreenPoint, ScreenPoint)> {
        let delta = (to.0 - from.0, to.1 - from.1);
        let mut entering = 0.0;
        let mut leaving = 1.0;
        let limits = [
            (-delta.0, from.0 - self.left),
            (delta.0, self.right - from.0),
            (-delta.1, from.1 - self.top),
            (delta.1, self.bottom - from.1),
        ];
        for (direction, room) in limits {
            if direction == 0.0 {
                if room < 0.0 {
                    return None;
                }
                continue;
            }
            let ratio = room / direction;
            if direction < 0.0 {
                if ratio > leaving {
                    return None;
                }
                if ratio > entering {
                    entering = ratio;
                }
            } else {
                if ratio < entering {
                    return None;
                }
                if ratio < leaving {
                    leaving = ratio;
                }
            }
        }
        let at = |fraction: f64| (from.0 + delta.0 * fraction, from.1 + delta.1 * fraction);
        Some((at(entering), at(leaving)))
    }

    pub(crate) fn clip_polygon(&self, points: &[ScreenPoint]) -> Vec<ScreenPoint> {
        let edges: [ClipEdge; 4] = [
            (|point, bound| point.0 >= bound, cross_vertical, self.left),
            (|point, bound| point.0 <= bound, cross_vertical, self.right),
            (|point, bound| point.1 >= bound, cross_horizontal, self.top),
            (
                |point, bound| point.1 <= bound,
                cross_horizontal,
                self.bottom,
            ),
        ];
        let mut clipped = points.to_vec();
        for (is_inside, crossing, bound) in edges {
            let input = std::mem::take(&mut clipped);
            let Some(mut previous) = input.last().copied() else {
                return Vec::new();
            };
            for point in input {
                match (is_inside(previous, bound), is_inside(point, bound)) {
                    (true, true) => clipped.push(point),
                    (true, false) => clipped.push(crossing(previous, point, bound)),
                    (false, true) => {
                        clipped.push(crossing(previous, point, bound));
                        clipped.push(point);
                    }
                    (false, false) => {}
                }
                previous = point;
            }
        }
        clipped
    }
}

fn cross_vertical(from: ScreenPoint, to: ScreenPoint, x: f64) -> ScreenPoint {
    let fraction = (x - from.0) / (to.0 - from.0);
    (x, from.1 + (to.1 - from.1) * fraction)
}

fn cross_horizontal(from: ScreenPoint, to: ScreenPoint, y: f64) -> ScreenPoint {
    let fraction = (y - from.1) / (to.1 - from.1);
    (from.0 + (to.0 - from.0) * fraction, y)
}

pub(crate) fn polyline_path(guard: &Guard, runs: &[Vec<ScreenPoint>]) -> Option<Path> {
    let mut builder = PathBuilder::new();
    for run in runs {
        let mut pen: Option<ScreenPoint> = None;
        for pair in run.windows(2) {
            let [from, to] = pair else {
                continue;
            };
            let Some((start, end)) = guard.clip_segment(*from, *to) else {
                pen = None;
                continue;
            };
            if pen != Some(start) {
                builder.move_to(to_f32(start.0), to_f32(start.1));
            }
            builder.line_to(to_f32(end.0), to_f32(end.1));
            pen = (end == *to).then_some(end);
        }
    }
    builder.finish()
}

pub(crate) fn polygon_path(guard: &Guard, points: &[ScreenPoint]) -> Option<Path> {
    let clipped = guard.clip_polygon(points);
    let mut corners = clipped.iter();
    let first = corners.next()?;
    let mut builder = PathBuilder::new();
    builder.move_to(to_f32(first.0), to_f32(first.1));
    for corner in corners {
        builder.line_to(to_f32(corner.0), to_f32(corner.1));
    }
    builder.close();
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> Guard {
        Guard::around(PhysicalRect::new(10, 10, 10, 10))
    }

    #[test]
    fn guard_reaches_one_area_size_beyond_the_area() {
        assert!(guard().contains((0.0, 30.0)) && !guard().contains((-0.5, 15.0)));
    }

    #[test]
    fn segment_inside_the_guard_is_kept_whole() {
        assert_eq!(
            guard().clip_segment((5.0, 5.0), (25.0, 25.0)),
            Some(((5.0, 5.0), (25.0, 25.0)))
        );
    }

    #[test]
    fn segment_leaving_the_guard_ends_on_its_edge() {
        assert_eq!(
            guard().clip_segment((15.0, 15.0), (1e300, 15.0)),
            Some(((15.0, 15.0), (30.0, 15.0)))
        );
    }

    #[test]
    fn segment_outside_the_guard_is_dropped() {
        assert_eq!(guard().clip_segment((-50.0, -50.0), (-40.0, 100.0)), None);
    }

    #[test]
    fn polygon_is_cut_at_the_guard() {
        let clipped = guard().clip_polygon(&[(15.0, 15.0), (100.0, 15.0), (15.0, 25.0)]);

        assert!(clipped.iter().all(|point| guard().contains(*point)) && clipped.len() == 4);
    }

    #[test]
    fn polygon_outside_the_guard_vanishes() {
        assert!(
            guard()
                .clip_polygon(&[(-50.0, -50.0), (-40.0, -50.0), (-45.0, -40.0)])
                .is_empty()
        );
    }
}
