use tiny_skia::{
    Color, ColorU8, FillRule, LineCap, LineJoin, Mask, Paint, Path, PathBuilder, Pixmap, Rect,
    Stroke, StrokeDash, Transform,
};

use crate::colour::Colour;
use crate::draw::{DrawCommand, DrawList};
use crate::geometry::PhysicalRect;
use crate::image::Image;
use crate::mapping::pixel_floor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasError {
    EmptySize { width: u16, height: u16 },
}

pub struct Canvas {
    pixmap: Pixmap,
    width: u16,
    height: u16,
}

impl Canvas {
    pub fn new(width: u16, height: u16, background: Colour) -> Result<Self, CanvasError> {
        let mut pixmap = Pixmap::new(u32::from(width), u32::from(height))
            .ok_or(CanvasError::EmptySize { width, height })?;
        pixmap.fill(to_skia_colour(background));
        Ok(Self {
            pixmap,
            width,
            height,
        })
    }

    pub fn draw(&mut self, list: &DrawList) {
        for command in list.commands() {
            match *command {
                DrawCommand::FillRect { rect, colour } => self.fill_rect(rect, colour),
                DrawCommand::FrameRect {
                    rect,
                    line_width,
                    colour,
                } => self.frame_rect(rect, line_width, colour),
                DrawCommand::FillRoundedRect {
                    rect,
                    radius,
                    colour,
                } => self.fill_rounded_rect(rect, radius, colour),
                DrawCommand::FrameRoundedRect {
                    rect,
                    radius,
                    line_width,
                    colour,
                } => self.frame_rounded_rect(rect, radius, line_width, colour),
                DrawCommand::FillTriangle { corners, colour } => {
                    self.fill_triangle(corners, colour);
                }
                DrawCommand::FillQuadrilateral { corners, colour } => {
                    self.fill_polygon(&corners, colour);
                }
                DrawCommand::FillEllipse { rect, colour } => self.fill_ellipse(rect, colour),
                DrawCommand::FrameEllipse {
                    rect,
                    line_width,
                    colour,
                } => self.frame_ellipse(rect, line_width, colour),
            }
        }
    }

    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    pub fn premultiplied_rgba(&self) -> &[u8] {
        self.pixmap.data()
    }

    pub fn to_image(&self) -> Image {
        let rgba = self
            .pixmap
            .pixels()
            .iter()
            .flat_map(|pixel| {
                let straight = pixel.demultiply();
                [
                    straight.red(),
                    straight.green(),
                    straight.blue(),
                    straight.alpha(),
                ]
            })
            .collect();
        Image::from_canvas(self.width, self.height, rgba)
    }

    pub fn pixel(&self, x: u16, y: u16) -> Option<Colour> {
        let (x, y) = (u32::from(x), u32::from(y));
        if x >= self.pixmap.width() || y >= self.pixmap.height() {
            return None;
        }
        let pixel = self.pixmap.pixel(x, y)?.demultiply();
        Some(Colour {
            red: pixel.red(),
            green: pixel.green(),
            blue: pixel.blue(),
            alpha: pixel.alpha(),
        })
    }

    pub(crate) fn fill_antialiased_path(&mut self, path: &Path, colour: Colour) {
        self.pixmap.fill_path(
            path,
            &solid_paint(colour, true),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    pub(crate) fn set_pixels(&mut self, rect: PhysicalRect, colour: Colour) {
        let premultiplied =
            ColorU8::from_rgba(colour.red, colour.green, colour.blue, colour.alpha).premultiply();
        let width = usize::from(self.width);
        let right = rect.right().min(self.width);
        let bottom = rect.bottom().min(self.height);
        let pixels = self.pixmap.pixels_mut();
        for y in rect.y..bottom {
            let row = usize::from(y) * width;
            for x in rect.x..right {
                if let Some(pixel) = pixels.get_mut(row + usize::from(x)) {
                    *pixel = premultiplied;
                }
            }
        }
    }

    pub(crate) fn paint_rect(&mut self, rect: PhysicalRect, paint: &dyn Fn(u16, u16) -> Colour) {
        let width = usize::from(self.width);
        let right = rect.right().min(self.width);
        let bottom = rect.bottom().min(self.height);
        let pixels = self.pixmap.pixels_mut();
        for y in rect.y..bottom {
            let row = usize::from(y) * width;
            for x in rect.x..right {
                let colour = paint(x, y);
                if let Some(pixel) = pixels.get_mut(row + usize::from(x)) {
                    *pixel =
                        ColorU8::from_rgba(colour.red, colour.green, colour.blue, colour.alpha)
                            .premultiply();
                }
            }
        }
    }

    pub(crate) fn fill_path_pattern(
        &mut self,
        path: &Path,
        clip: Option<&Mask>,
        paint: &dyn Fn(u16, u16) -> Option<Colour>,
    ) {
        let Some(mut mask) = Mask::new(self.pixmap.width(), self.pixmap.height()) else {
            return;
        };
        mask.fill_path(path, FillRule::Winding, false, Transform::identity());
        let bounds = path.bounds();
        let left = pixel_floor(f64::from(bounds.left()));
        let top = pixel_floor(f64::from(bounds.top()));
        let right = pixel_floor(f64::from(bounds.right()))
            .saturating_add(1)
            .min(self.width);
        let bottom = pixel_floor(f64::from(bounds.bottom()))
            .saturating_add(1)
            .min(self.height);
        let width = usize::from(self.width);
        for y in top..bottom {
            for x in left..right {
                let index = usize::from(y) * width + usize::from(x);
                let covered = mask
                    .data()
                    .get(index)
                    .is_some_and(|coverage| *coverage != 0)
                    && clip.is_none_or(|clip| {
                        clip.data()
                            .get(index)
                            .is_some_and(|coverage| *coverage != 0)
                    });
                if covered && let Some(colour) = paint(x, y) {
                    self.set_pixels(PhysicalRect::new(x, y, 1, 1), colour);
                }
            }
        }
    }

    pub(crate) fn clip_mask(&self, rect: PhysicalRect) -> Option<Mask> {
        let mut mask = Mask::new(self.pixmap.width(), self.pixmap.height())?;
        let path = PathBuilder::from_rect(to_skia_rect(rect)?);
        mask.fill_path(&path, FillRule::Winding, false, Transform::identity());
        Some(mask)
    }

    pub(crate) fn fill_path_within(&mut self, path: &Path, colour: Colour, clip: Option<&Mask>) {
        self.pixmap.fill_path(
            path,
            &solid_paint(colour, true),
            FillRule::Winding,
            Transform::identity(),
            clip,
        );
    }

    pub(crate) fn stroke_path_within(
        &mut self,
        path: &Path,
        colour: Colour,
        width: f32,
        dash: Option<[f32; 2]>,
        clip: Option<&Mask>,
    ) {
        let stroke = Stroke {
            width,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Bevel,
            dash: dash.and_then(|pattern| StrokeDash::new(pattern.to_vec(), 0.0)),
            ..Stroke::default()
        };
        self.pixmap.stroke_path(
            path,
            &solid_paint(colour, true),
            &stroke,
            Transform::identity(),
            clip,
        );
    }

    fn fill_rect(&mut self, rect: PhysicalRect, colour: Colour) {
        let Some(skia_rect) = to_skia_rect(rect) else {
            return;
        };
        self.pixmap.fill_rect(
            skia_rect,
            &solid_paint(colour, false),
            Transform::identity(),
            None,
        );
    }

    fn frame_rect(&mut self, rect: PhysicalRect, line_width: u16, colour: Colour) {
        let fills_whole_rect = line_width.saturating_mul(2) >= rect.width.min(rect.height);
        if fills_whole_rect {
            self.fill_rect(rect, colour);
            return;
        }
        let inner_height = rect.height - 2 * line_width;
        let edges = [
            PhysicalRect::new(rect.x, rect.y, rect.width, line_width),
            PhysicalRect::new(rect.x, rect.bottom() - line_width, rect.width, line_width),
            PhysicalRect::new(rect.x, rect.y + line_width, line_width, inner_height),
            PhysicalRect::new(
                rect.right() - line_width,
                rect.y + line_width,
                line_width,
                inner_height,
            ),
        ];
        for edge in edges {
            self.fill_rect(edge, colour);
        }
    }

    fn fill_triangle(&mut self, corners: [(u16, u16); 3], colour: Colour) {
        let [(first_x, first_y), (second_x, second_y), (third_x, third_y)] = corners;
        let mut builder = PathBuilder::new();
        builder.move_to(f32::from(first_x), f32::from(first_y));
        builder.line_to(f32::from(second_x), f32::from(second_y));
        builder.line_to(f32::from(third_x), f32::from(third_y));
        builder.close();
        if let Some(path) = builder.finish() {
            self.fill_antialiased_path(&path, colour);
        }
    }

    fn fill_polygon(&mut self, corners: &[(u16, u16)], colour: Colour) {
        let mut builder = PathBuilder::new();
        for (index, (x, y)) in corners.iter().enumerate() {
            if index == 0 {
                builder.move_to(f32::from(*x), f32::from(*y));
            } else {
                builder.line_to(f32::from(*x), f32::from(*y));
            }
        }
        builder.close();
        if let Some(path) = builder.finish() {
            self.fill_antialiased_path(&path, colour);
        }
    }

    fn fill_ellipse(&mut self, rect: PhysicalRect, colour: Colour) {
        let oval = Rect::from_xywh(
            f32::from(rect.x),
            f32::from(rect.y),
            f32::from(rect.width),
            f32::from(rect.height),
        );
        if let Some(path) = oval.and_then(PathBuilder::from_oval) {
            self.fill_antialiased_path(&path, colour);
        }
    }

    fn frame_ellipse(&mut self, rect: PhysicalRect, line_width: u16, colour: Colour) {
        let width = f32::from(line_width.max(1));
        let oval = Rect::from_xywh(
            f32::from(rect.x) + width / 2.0,
            f32::from(rect.y) + width / 2.0,
            f32::from(rect.width) - width,
            f32::from(rect.height) - width,
        );
        let Some(path) = oval.and_then(PathBuilder::from_oval) else {
            return;
        };
        self.pixmap.stroke_path(
            &path,
            &solid_paint(colour, true),
            &Stroke {
                width,
                ..Stroke::default()
            },
            Transform::identity(),
            None,
        );
    }

    fn frame_rounded_rect(
        &mut self,
        rect: PhysicalRect,
        radius: u16,
        line_width: u16,
        colour: Colour,
    ) {
        let Some(path) = rounded_path(rect, radius) else {
            return;
        };
        self.pixmap.stroke_path(
            &path,
            &solid_paint(colour, true),
            &Stroke {
                width: f32::from(line_width.max(1)),
                line_cap: LineCap::Butt,
                line_join: LineJoin::Miter,
                ..Stroke::default()
            },
            Transform::identity(),
            None,
        );
    }

    fn fill_rounded_rect(&mut self, rect: PhysicalRect, radius: u16, colour: Colour) {
        let Some(path) = rounded_path(rect, radius) else {
            return;
        };
        self.pixmap.fill_path(
            &path,
            &solid_paint(colour, true),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn rounded_path(rect: PhysicalRect, radius: u16) -> Option<Path> {
    let radius = f32::from(radius.min(rect.width / 2).min(rect.height / 2));
    let left = f32::from(rect.x);
    let top = f32::from(rect.y);
    let right = f32::from(rect.right());
    let bottom = f32::from(rect.bottom());
    let mut builder = PathBuilder::new();
    builder.move_to(left + radius, top);
    builder.line_to(right - radius, top);
    builder.quad_to(right, top, right, top + radius);
    builder.line_to(right, bottom - radius);
    builder.quad_to(right, bottom, right - radius, bottom);
    builder.line_to(left + radius, bottom);
    builder.quad_to(left, bottom, left, bottom - radius);
    builder.line_to(left, top + radius);
    builder.quad_to(left, top, left + radius, top);
    builder.close();
    builder.finish()
}

fn to_skia_colour(colour: Colour) -> Color {
    Color::from_rgba8(colour.red, colour.green, colour.blue, colour.alpha)
}

fn to_skia_rect(rect: PhysicalRect) -> Option<Rect> {
    if rect.is_empty() {
        return None;
    }
    Rect::from_xywh(
        f32::from(rect.x),
        f32::from(rect.y),
        f32::from(rect.width),
        f32::from(rect.height),
    )
}

fn solid_paint(colour: Colour, anti_alias: bool) -> Paint<'static> {
    let mut paint = Paint {
        anti_alias,
        ..Paint::default()
    };
    paint.set_color_rgba8(colour.red, colour.green, colour.blue, colour.alpha);
    paint
}

#[cfg(test)]
mod tests {
    use super::*;

    const BACKGROUND: Colour = Colour::opaque(255, 255, 255);
    const INK: Colour = Colour::opaque(10, 20, 30);

    fn canvas_with(command: DrawCommand) -> Canvas {
        let mut canvas = Canvas::new(20, 20, BACKGROUND).expect("non-empty canvas");
        let mut list = DrawList::new();
        list.push(command);
        canvas.draw(&list);
        canvas
    }

    #[test]
    fn new_canvas_is_filled_with_background() {
        let canvas = Canvas::new(4, 4, BACKGROUND).expect("non-empty canvas");

        assert_eq!(canvas.pixel(3, 3), Some(BACKGROUND));
    }

    #[test]
    fn canvas_without_width_is_rejected() {
        let result = Canvas::new(0, 4, BACKGROUND);

        assert_eq!(
            result.err(),
            Some(CanvasError::EmptySize {
                width: 0,
                height: 4
            })
        );
    }

    #[test]
    fn image_of_a_canvas_holds_its_straight_pixels() {
        let canvas = Canvas::new(3, 2, INK).expect("non-empty canvas");

        let image = canvas.to_image();

        assert_eq!(
            (image.width(), image.height(), image.pixel(2, 1)),
            (3, 2, Some([10, 20, 30, 255]))
        );
    }

    #[test]
    fn set_pixels_replace_the_pixels_of_their_rect() {
        let mut canvas = Canvas::new(8, 8, BACKGROUND).expect("non-empty canvas");

        canvas.set_pixels(PhysicalRect::new(2, 2, 3, 1), INK);

        assert_eq!(
            [canvas.pixel(4, 2), canvas.pixel(5, 2), canvas.pixel(4, 3)],
            [Some(INK), Some(BACKGROUND), Some(BACKGROUND)]
        );
    }

    #[test]
    fn set_pixels_beyond_the_canvas_stop_at_its_edge() {
        let mut canvas = Canvas::new(4, 4, BACKGROUND).expect("non-empty canvas");

        canvas.set_pixels(PhysicalRect::new(3, 0, 10, 1), INK);

        assert_eq!(canvas.pixel(3, 0), Some(INK));
    }

    #[test]
    fn fill_outside_the_clip_leaves_pixels_untouched() {
        let mut canvas = Canvas::new(20, 20, BACKGROUND).expect("non-empty canvas");
        let clip = canvas
            .clip_mask(PhysicalRect::new(0, 0, 10, 20))
            .expect("clip mask");
        let path = PathBuilder::from_rect(Rect::from_xywh(0.0, 0.0, 20.0, 20.0).unwrap());

        canvas.fill_path_within(&path, INK, Some(&clip));

        assert_eq!(
            [canvas.pixel(5, 5), canvas.pixel(15, 5)],
            [Some(INK), Some(BACKGROUND)]
        );
    }

    #[test]
    fn stroke_covers_the_pixels_along_its_line() {
        let mut canvas = Canvas::new(20, 20, BACKGROUND).expect("non-empty canvas");
        let mut builder = PathBuilder::new();
        builder.move_to(0.0, 10.0);
        builder.line_to(20.0, 10.0);
        let path = builder.finish().unwrap();

        canvas.stroke_path_within(&path, INK, 2.0, None, None);

        assert_eq!(
            [canvas.pixel(10, 9), canvas.pixel(10, 12)],
            [Some(INK), Some(BACKGROUND)]
        );
    }

    #[test]
    fn dashed_stroke_leaves_gaps() {
        let mut canvas = Canvas::new(20, 20, BACKGROUND).expect("non-empty canvas");
        let mut builder = PathBuilder::new();
        builder.move_to(0.0, 10.0);
        builder.line_to(20.0, 10.0);
        let path = builder.finish().unwrap();

        canvas.stroke_path_within(&path, INK, 2.0, Some([4.0, 4.0]), None);

        assert_eq!(
            [canvas.pixel(2, 10), canvas.pixel(6, 10)],
            [Some(INK), Some(BACKGROUND)]
        );
    }

    #[test]
    fn pixel_outside_canvas_is_absent() {
        let canvas = Canvas::new(4, 4, BACKGROUND).expect("non-empty canvas");

        assert_eq!(canvas.pixel(4, 0), None);
    }

    #[test]
    fn filled_rect_covers_its_corner_pixels_exactly() {
        let canvas = canvas_with(DrawCommand::FillRect {
            rect: PhysicalRect::new(2, 3, 5, 4),
            colour: INK,
        });

        let corners = [(2, 3), (6, 3), (2, 6), (6, 6)];
        assert!(
            corners
                .iter()
                .all(|&(x, y)| canvas.pixel(x, y) == Some(INK))
        );
    }

    #[test]
    fn filled_rect_leaves_neighbouring_pixels_untouched() {
        let canvas = canvas_with(DrawCommand::FillRect {
            rect: PhysicalRect::new(2, 3, 5, 4),
            colour: INK,
        });

        let neighbours = [(1, 3), (7, 3), (2, 2), (2, 7)];
        assert!(
            neighbours
                .iter()
                .all(|&(x, y)| canvas.pixel(x, y) == Some(BACKGROUND))
        );
    }

    #[test]
    fn empty_rect_draws_nothing() {
        let canvas = canvas_with(DrawCommand::FillRect {
            rect: PhysicalRect::new(2, 2, 0, 5),
            colour: INK,
        });

        assert_eq!(canvas.pixel(2, 2), Some(BACKGROUND));
    }

    #[test]
    fn frame_edge_has_the_given_width() {
        let canvas = canvas_with(DrawCommand::FrameRect {
            rect: PhysicalRect::new(2, 2, 10, 10),
            line_width: 2,
            colour: INK,
        });

        let across_left_edge = [canvas.pixel(2, 6), canvas.pixel(3, 6), canvas.pixel(4, 6)];
        assert_eq!(across_left_edge, [Some(INK), Some(INK), Some(BACKGROUND)]);
    }

    #[test]
    fn frame_bottom_edge_ends_at_rect_bottom() {
        let canvas = canvas_with(DrawCommand::FrameRect {
            rect: PhysicalRect::new(2, 2, 10, 10),
            line_width: 1,
            colour: INK,
        });

        let across_bottom_edge = [
            canvas.pixel(6, 10),
            canvas.pixel(6, 11),
            canvas.pixel(6, 12),
        ];
        assert_eq!(
            across_bottom_edge,
            [Some(BACKGROUND), Some(INK), Some(BACKGROUND)]
        );
    }

    #[test]
    fn frame_leaves_interior_unpainted() {
        let canvas = canvas_with(DrawCommand::FrameRect {
            rect: PhysicalRect::new(2, 2, 10, 10),
            line_width: 1,
            colour: INK,
        });

        assert_eq!(canvas.pixel(6, 6), Some(BACKGROUND));
    }

    #[test]
    fn frame_wider_than_half_the_rect_fills_it() {
        let canvas = canvas_with(DrawCommand::FrameRect {
            rect: PhysicalRect::new(2, 2, 4, 4),
            line_width: 2,
            colour: INK,
        });

        assert_eq!(canvas.pixel(4, 4), Some(INK));
    }

    #[test]
    fn triangle_covers_a_pixel_inside_it() {
        let canvas = canvas_with(DrawCommand::FillTriangle {
            corners: [(2, 2), (18, 2), (10, 18)],
            colour: INK,
        });

        assert_eq!(canvas.pixel(10, 6), Some(INK));
    }

    #[test]
    fn triangle_leaves_a_pixel_outside_it_untouched() {
        let canvas = canvas_with(DrawCommand::FillTriangle {
            corners: [(2, 2), (18, 2), (10, 18)],
            colour: INK,
        });

        assert_eq!(canvas.pixel(3, 16), Some(BACKGROUND));
    }

    #[test]
    fn rounded_rect_centre_is_fully_covered() {
        let canvas = canvas_with(DrawCommand::FillRoundedRect {
            rect: PhysicalRect::new(2, 2, 12, 12),
            radius: 4,
            colour: INK,
        });

        assert_eq!(canvas.pixel(8, 8), Some(INK));
    }

    fn large_canvas_with(size: u16, command: DrawCommand) -> Canvas {
        let mut canvas = Canvas::new(size, size, BACKGROUND).expect("non-empty canvas");
        let mut list = DrawList::new();
        list.push(command);
        canvas.draw(&list);
        canvas
    }

    #[test]
    fn an_ellipse_in_a_square_is_a_circle_where_a_rounded_rect_bulges() {
        let square = PhysicalRect::new(0, 0, 200, 200);
        let circle = large_canvas_with(
            200,
            DrawCommand::FillEllipse {
                rect: square,
                colour: INK,
            },
        );
        let rounded = large_canvas_with(
            200,
            DrawCommand::FillRoundedRect {
                rect: square,
                radius: 100,
                colour: INK,
            },
        );

        assert_eq!(circle.pixel(100, 100), Some(INK));
        assert_eq!(circle.pixel(27, 27), Some(BACKGROUND));
        assert_eq!(rounded.pixel(27, 27), Some(INK));
    }

    #[test]
    fn a_framed_ellipse_keeps_its_middle_and_its_line_inside_the_rect() {
        let canvas = large_canvas_with(
            48,
            DrawCommand::FrameEllipse {
                rect: PhysicalRect::new(2, 2, 40, 40),
                line_width: 2,
                colour: INK,
            },
        );

        assert_eq!(canvas.pixel(22, 22), Some(BACKGROUND));
        assert_eq!(canvas.pixel(22, 2), Some(INK));
        assert_eq!(canvas.pixel(22, 1), Some(BACKGROUND));
    }

    #[test]
    fn a_quadrilateral_fills_without_a_seam_along_its_diagonal() {
        let canvas = large_canvas_with(
            48,
            DrawCommand::FillQuadrilateral {
                corners: [(2, 2), (42, 2), (42, 42), (2, 42)],
                colour: INK,
            },
        );

        assert!((3..41).all(|at| canvas.pixel(at, at) == Some(INK)));
    }

    #[test]
    fn rounded_rect_leaves_outer_corner_pixel_uncovered() {
        let canvas = canvas_with(DrawCommand::FillRoundedRect {
            rect: PhysicalRect::new(2, 2, 12, 12),
            radius: 4,
            colour: INK,
        });

        assert_eq!(canvas.pixel(2, 2), Some(BACKGROUND));
    }

    #[test]
    fn rounded_rect_edge_midpoint_is_fully_covered() {
        let canvas = canvas_with(DrawCommand::FillRoundedRect {
            rect: PhysicalRect::new(2, 2, 12, 12),
            radius: 4,
            colour: INK,
        });

        assert_eq!(canvas.pixel(8, 2), Some(INK));
    }
}
