use crate::canvas::Canvas;
use crate::colour::Colour;
use crate::geometry::PhysicalRect;
use crate::mapping::pixel_floor;

pub(crate) type DepthPoint = (f64, f64, f64);

const PIXEL_CENTRE: f64 = 0.5;

pub(crate) struct DepthBuffer {
    area: PhysicalRect,
    depths: Vec<f64>,
}

fn edge(from: DepthPoint, to: DepthPoint, x: f64, y: f64) -> f64 {
    (to.0 - from.0) * (y - from.1) - (to.1 - from.1) * (x - from.0)
}

impl DepthBuffer {
    pub(crate) fn new(area: PhysicalRect) -> DepthBuffer {
        DepthBuffer {
            area,
            depths: vec![f64::INFINITY; usize::from(area.width) * usize::from(area.height)],
        }
    }

    pub(crate) fn fill_triangle(
        &mut self,
        canvas: &mut Canvas,
        corners: [DepthPoint; 3],
        paint: &dyn Fn(u16, u16) -> Colour,
    ) {
        let [first, second, third] = corners;
        let area = edge(first, second, third.0, third.1);
        if area == 0.0 || !area.is_finite() {
            return;
        }
        let lowest = |values: [f64; 3]| {
            values.into_iter().fold(
                f64::INFINITY,
                |low, value| if value < low { value } else { low },
            )
        };
        let highest = |values: [f64; 3]| {
            values.into_iter().fold(
                f64::NEG_INFINITY,
                |high, value| if value > high { value } else { high },
            )
        };
        let left = pixel_floor(lowest([first.0, second.0, third.0])).max(self.area.x);
        let top = pixel_floor(lowest([first.1, second.1, third.1])).max(self.area.y);
        let right = pixel_floor(highest([first.0, second.0, third.0]))
            .saturating_add(1)
            .min(self.area.right());
        let bottom = pixel_floor(highest([first.1, second.1, third.1]))
            .saturating_add(1)
            .min(self.area.bottom());
        for y in top..bottom {
            let centre_y = f64::from(y) + PIXEL_CENTRE;
            for x in left..right {
                let centre_x = f64::from(x) + PIXEL_CENTRE;
                let weights = [
                    edge(second, third, centre_x, centre_y) / area,
                    edge(third, first, centre_x, centre_y) / area,
                    edge(first, second, centre_x, centre_y) / area,
                ];
                if weights.iter().any(|weight| *weight < 0.0) {
                    continue;
                }
                let depth = weights[0] * first.2 + weights[1] * second.2 + weights[2] * third.2;
                let index = usize::from(y - self.area.y) * usize::from(self.area.width)
                    + usize::from(x - self.area.x);
                let Some(stored) = self.depths.get_mut(index) else {
                    continue;
                };
                if depth < *stored {
                    *stored = depth;
                    canvas.set_pixels(PhysicalRect::new(x, y, 1, 1), paint(x, y));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GROUND: Colour = Colour::opaque(255, 255, 255);
    const NEAR: Colour = Colour::opaque(200, 0, 0);
    const FAR: Colour = Colour::opaque(0, 0, 200);

    fn square(depth: f64) -> [[DepthPoint; 3]; 2] {
        [
            [(0.0, 0.0, depth), (10.0, 0.0, depth), (10.0, 10.0, depth)],
            [(0.0, 0.0, depth), (10.0, 10.0, depth), (0.0, 10.0, depth)],
        ]
    }

    #[test]
    fn nearer_triangle_drawn_first_stays_in_front() {
        let mut canvas = Canvas::new(10, 10, GROUND).unwrap();
        let mut buffer = DepthBuffer::new(PhysicalRect::new(0, 0, 10, 10));

        for triangle in square(1.0) {
            buffer.fill_triangle(&mut canvas, triangle, &|_, _| NEAR);
        }
        for triangle in square(2.0) {
            buffer.fill_triangle(&mut canvas, triangle, &|_, _| FAR);
        }

        assert_eq!(canvas.pixel(5, 5), Some(NEAR));
    }

    #[test]
    fn nearer_triangle_drawn_later_covers_the_farther_one() {
        let mut canvas = Canvas::new(10, 10, GROUND).unwrap();
        let mut buffer = DepthBuffer::new(PhysicalRect::new(0, 0, 10, 10));

        for triangle in square(2.0) {
            buffer.fill_triangle(&mut canvas, triangle, &|_, _| FAR);
        }
        for triangle in square(1.0) {
            buffer.fill_triangle(&mut canvas, triangle, &|_, _| NEAR);
        }

        assert_eq!(canvas.pixel(5, 5), Some(NEAR));
    }

    #[test]
    fn triangle_of_either_winding_is_filled() {
        let mut canvas = Canvas::new(10, 10, GROUND).unwrap();
        let mut buffer = DepthBuffer::new(PhysicalRect::new(0, 0, 10, 10));

        buffer.fill_triangle(
            &mut canvas,
            [(0.0, 0.0, 1.0), (0.0, 10.0, 1.0), (10.0, 0.0, 1.0)],
            &|_, _| NEAR,
        );

        assert_eq!(canvas.pixel(2, 2), Some(NEAR));
    }

    #[test]
    fn triangle_leaves_pixels_outside_its_area_untouched() {
        let mut canvas = Canvas::new(20, 20, GROUND).unwrap();
        let mut buffer = DepthBuffer::new(PhysicalRect::new(0, 0, 10, 10));

        buffer.fill_triangle(
            &mut canvas,
            [(0.0, 0.0, 1.0), (20.0, 0.0, 1.0), (0.0, 20.0, 1.0)],
            &|_, _| NEAR,
        );

        assert_eq!(canvas.pixel(12, 2), Some(GROUND));
    }
}
