pub mod canvas;
pub mod colour;
pub mod digits;
pub mod draw;
pub mod geometry;
pub mod image;
pub mod text;
pub mod theme;

mod arc;
mod axis_units;
mod clip;
mod deflate;
mod depth;
mod figure_draw;
mod key_rows;
mod layout;
mod legend;
mod mapping;
mod picture_text;
mod plane;
mod render;
mod roles;
mod space;
mod ticks;

#[cfg(test)]
mod test_scenes;

pub use axis_units::axis_unit_rects;
pub use calc_numbers::Number;
pub use calc_viz::{ColourLegend, layer_legend, shape_legend};
pub use calc_viz::{Interval, Scale, Scene, View};
pub use layout::{
    AxisRange, LayoutError, LayoutRequest, PictureLayout, ViewKind, ViewLayout, ViewRequest,
    plot_layout,
};
pub use legend::{LegendLayout, LegendRequest};
pub use picture_text::{
    DesignationText, DomainLegendText, EscapeTimeText, MarkKeyText, PictureText, PrecisionText,
    RoleText, UndecidedShare, undecided_share,
};
pub use render::{DrawnScene, RenderError, RenderRequest, Rendered, draw_scene, render_scene};
pub use text::font::FontSet;
pub use ticks::{TICK_LABEL_CELLS, TickLabelError, tick_label};
