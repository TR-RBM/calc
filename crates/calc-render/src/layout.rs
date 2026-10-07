use calc_viz::{Interval, Scale};

use crate::geometry::{PhysicalRect, ScaleError, ScaleFactor};
use crate::legend::{self, LegendLayout, LegendRequest, plan, row_heights};
use crate::plane::{TICK_LABEL_SPACING_CELLS, Y_TICK_SPACING_ROWS};
use crate::text::cells::cells_of;
use crate::text::font::FontSet;
use crate::text::grid::{GridError, GridMetrics, grid_metrics};
use crate::ticks::TICK_LABEL_CELLS;
use crate::ticks::{Tick, TickRoom, linear_ticks, logarithmic_ticks};

const MARGIN_LOGICAL: u16 = 8;
const TICK_LENGTH_LOGICAL: u16 = 4;
const GAP_LOGICAL: u16 = 4;
const SPACE_AXES: usize = 3;
const ACROSS_AXIS: usize = 0;
const VALUE_AXIS: usize = 1;
const MEASURED_SEPARATOR: char = '.';

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ViewKind {
    Plane,
    Space,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AxisRange {
    pub range: Interval,
    pub scale: Scale,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewRequest {
    pub kind: ViewKind,
    pub axes: Vec<Option<AxisRange>>,
}

impl ViewRequest {
    pub fn plane(axes: Vec<Option<AxisRange>>) -> Self {
        Self {
            kind: ViewKind::Plane,
            axes,
        }
    }

    pub fn of_kind(kind: ViewKind) -> Self {
        Self {
            kind,
            axes: Vec::new(),
        }
    }

    fn axis(&self, index: usize) -> Option<&AxisRange> {
        self.axes.get(index).and_then(Option::as_ref)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutRequest {
    pub width: u16,
    pub height: u16,
    pub text_size: u16,
    pub scale: ScaleFactor,
    pub views: Vec<ViewRequest>,
    pub legends: Vec<LegendRequest>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewLayout {
    pub kind: ViewKind,
    pub column: PhysicalRect,
    pub title_row: PhysicalRect,
    pub plot_area: PhysicalRect,
    pub tick_label_row: Option<PhysicalRect>,
    pub axis_title_row: Option<PhysicalRect>,
    pub gutter: Option<PhysicalRect>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PictureLayout {
    pub width: u16,
    pub height: u16,
    pub scale: ScaleFactor,
    pub metrics: GridMetrics,
    pub tick_length: u16,
    pub gap: u16,
    pub views: Vec<ViewLayout>,
    pub key_row: PhysicalRect,
    pub legends: Vec<LegendLayout>,
    pub footer_row: PhysicalRect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutError {
    NoView,
    Scale(ScaleError),
    Grid(GridError),
    TooSmall { width: u16, height: u16 },
}

fn moved(rect: PhysicalRect, origin: (u16, u16)) -> PhysicalRect {
    PhysicalRect::new(
        rect.x.saturating_add(origin.0),
        rect.y.saturating_add(origin.1),
        rect.width,
        rect.height,
    )
}

impl PictureLayout {
    pub fn move_to(&mut self, origin: (u16, u16)) {
        for view in &mut self.views {
            view.column = moved(view.column, origin);
            view.title_row = moved(view.title_row, origin);
            view.plot_area = moved(view.plot_area, origin);
            view.tick_label_row = view.tick_label_row.map(|row| moved(row, origin));
            view.axis_title_row = view.axis_title_row.map(|row| moved(row, origin));
            view.gutter = view.gutter.map(|gutter| moved(gutter, origin));
        }
        self.key_row = moved(self.key_row, origin);
        self.footer_row = moved(self.footer_row, origin);
        for legend in &mut self.legends {
            for row in &mut legend.rows {
                *row = moved(*row, origin);
            }
        }
    }
}

impl ViewLayout {
    pub fn divisions(&self) -> Vec<u32> {
        let width = u32::from(self.plot_area.width);
        let height = u32::from(self.plot_area.height);
        match self.kind {
            ViewKind::Plane => vec![width, height],
            ViewKind::Space => vec![width.min(height); SPACE_AXES],
        }
    }
}

struct Sizes {
    margin: u32,
    tick_length: u32,
    gap: u32,
    row: u32,
    cell: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Reserved {
    value: u16,
    last_across: u16,
}

fn widest_cells(ticks: &[Tick]) -> u16 {
    ticks
        .iter()
        .map(|tick| cells_of(&tick.label))
        .max()
        .unwrap_or(0)
}

fn axis_ticks(axis: &AxisRange, room: &TickRoom) -> Vec<Tick> {
    match axis.scale {
        Scale::Linear => linear_ticks(&axis.range, room, MEASURED_SEPARATOR),
        Scale::Logarithmic => logarithmic_ticks(&axis.range, room, MEASURED_SEPARATOR),
    }
}

fn reserved_cells(view: &ViewRequest, sizes: &Sizes, [width, height]: [u32; 2]) -> Reserved {
    let worst = TICK_LABEL_CELLS;
    let widest = u32::from(worst) * sizes.cell;
    let label_offset = (sizes.tick_length + sizes.gap).max(sizes.row / 2 + sizes.gap);
    let area_height = height.saturating_sub(sizes.row + label_offset + 2 * sizes.row);
    let area_width = width
        .saturating_sub(widest + sizes.gap + sizes.tick_length)
        .saturating_sub(widest / 2);
    let value = match view.axis(VALUE_AXIS) {
        Some(down) => {
            let room = TickRoom::spaced(area_height, Y_TICK_SPACING_ROWS * sizes.row);
            widest_cells(&axis_ticks(down, &room)).min(worst)
        }
        None => worst,
    };
    let last_across = match view.axis(ACROSS_AXIS) {
        Some(across) => {
            let room = TickRoom::beside_labels(
                area_width,
                u16::try_from(sizes.cell).unwrap_or(1),
                TICK_LABEL_SPACING_CELLS,
            );
            widest_cells(&axis_ticks(across, &room)).min(worst)
        }
        None => worst,
    };
    Reserved { value, last_across }
}

fn to_u16(value: u32, request: &LayoutRequest) -> Result<u16, LayoutError> {
    u16::try_from(value).map_err(|_| too_small(request))
}

fn too_small(request: &LayoutRequest) -> LayoutError {
    LayoutError::TooSmall {
        width: request.width,
        height: request.height,
    }
}

fn rect(
    request: &LayoutRequest,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<PhysicalRect, LayoutError> {
    Ok(PhysicalRect::new(
        to_u16(x, request)?,
        to_u16(y, request)?,
        to_u16(width, request)?,
        to_u16(height, request)?,
    ))
}

pub fn plot_layout(request: &LayoutRequest, fonts: &FontSet) -> Result<PictureLayout, LayoutError> {
    let view_count = u32::try_from(request.views.len()).map_err(|_| too_small(request))?;
    if view_count == 0 {
        return Err(LayoutError::NoView);
    }
    let physical = |logical: u16| {
        request
            .scale
            .to_physical_size(logical)
            .map(u32::from)
            .map_err(LayoutError::Scale)
    };
    let font_size = request
        .scale
        .to_physical_size(request.text_size)
        .map_err(LayoutError::Scale)?;
    let metrics = grid_metrics(fonts, font_size).map_err(LayoutError::Grid)?;
    let sizes = Sizes {
        margin: physical(MARGIN_LOGICAL)?,
        tick_length: physical(TICK_LENGTH_LOGICAL)?,
        gap: physical(GAP_LOGICAL)?,
        row: u32::from(metrics.cell.height()),
        cell: u32::from(metrics.cell.width()),
    };
    let width = u32::from(request.width);
    let height = u32::from(request.height);
    let row_width = width.saturating_sub(2 * sizes.margin);
    let legend_sizes = legend::sizes(metrics);
    let legend_heights: Vec<Vec<u32>> = request
        .legends
        .iter()
        .map(|legend| {
            row_heights(
                &plan(legend, &legend_sizes, row_width),
                &legend_sizes,
                sizes.row,
            )
        })
        .collect();
    let legend_total: u32 = legend_heights.iter().flatten().sum();
    let rows_below = 2 * sizes.row + legend_total + sizes.margin;
    let column_space = view_count
        .checked_add(1)
        .and_then(|gaps| sizes.margin.checked_mul(gaps))
        .and_then(|gaps| width.checked_sub(gaps))
        .ok_or_else(|| too_small(request))?;
    let column_width = column_space / view_count;
    let column_height = height
        .checked_sub(sizes.margin + rows_below)
        .ok_or_else(|| too_small(request))?;
    let mut views = Vec::with_capacity(request.views.len());
    let mut left = sizes.margin;
    for view in &request.views {
        views.push(view_layout(
            request,
            view,
            &sizes,
            [left, sizes.margin, column_width, column_height],
        )?);
        left += column_width + sizes.margin;
    }
    let key_top = sizes.margin + column_height;
    let mut top = key_top + sizes.row;
    let mut legends = Vec::with_capacity(legend_heights.len());
    for heights in &legend_heights {
        let mut rows = Vec::with_capacity(heights.len());
        for row_height in heights {
            rows.push(rect(request, sizes.margin, top, row_width, *row_height)?);
            top += row_height;
        }
        legends.push(LegendLayout {
            rows,
            bar_height: to_u16(legend_sizes.bar_height, request)?,
        });
    }
    Ok(PictureLayout {
        width: request.width,
        height: request.height,
        scale: request.scale,
        metrics,
        tick_length: to_u16(sizes.tick_length, request)?,
        gap: to_u16(sizes.gap, request)?,
        views,
        key_row: rect(request, sizes.margin, key_top, row_width, sizes.row)?,
        legends,
        footer_row: rect(request, sizes.margin, top, row_width, sizes.row)?,
    })
}

fn view_layout(
    request: &LayoutRequest,
    view: &ViewRequest,
    sizes: &Sizes,
    [left, top, width, height]: [u32; 4],
) -> Result<ViewLayout, LayoutError> {
    let kind = view.kind;
    let column = rect(request, left, top, width, height)?;
    let title_row = rect(request, left, top, width, sizes.row)?;
    let area_top = top + sizes.row;
    match kind {
        ViewKind::Plane => {
            let cell = sizes.cell;
            let reserved = reserved_cells(view, sizes, [width, height]);
            let label_width = u32::from(reserved.value) * cell;
            let gutter_width = label_width + sizes.gap + sizes.tick_length;
            let right_pad = u32::from(reserved.last_across) * cell / 2;
            let label_offset = (sizes.tick_length + sizes.gap).max(sizes.row / 2 + sizes.gap);
            let below = label_offset + 2 * sizes.row;
            let area_width = width
                .checked_sub(gutter_width + right_pad)
                .filter(|area| *area > 0)
                .ok_or_else(|| too_small(request))?;
            let area_height = height
                .checked_sub(sizes.row + below)
                .filter(|area| *area > 0)
                .ok_or_else(|| too_small(request))?;
            let area_left = left + gutter_width;
            let area_bottom = area_top + area_height;
            Ok(ViewLayout {
                kind,
                column,
                title_row,
                plot_area: rect(request, area_left, area_top, area_width, area_height)?,
                tick_label_row: Some(rect(
                    request,
                    left,
                    area_bottom + label_offset,
                    width,
                    sizes.row,
                )?),
                axis_title_row: Some(rect(
                    request,
                    left,
                    area_bottom + label_offset + sizes.row,
                    width,
                    sizes.row,
                )?),
                gutter: Some(rect(request, left, area_top, label_width, area_height)?),
            })
        }
        ViewKind::Space => {
            let area_height = height
                .checked_sub(sizes.row)
                .filter(|area| *area > 0)
                .ok_or_else(|| too_small(request))?;
            if width == 0 {
                return Err(too_small(request));
            }
            Ok(ViewLayout {
                kind,
                column,
                title_row,
                plot_area: rect(request, left, area_top, width, area_height)?,
                tick_label_row: None,
                axis_title_row: None,
                gutter: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(width: u16, height: u16, views: Vec<ViewKind>) -> LayoutRequest {
        LayoutRequest {
            width,
            height,
            text_size: 14,
            scale: ScaleFactor::from_percent(100).unwrap(),
            views: views.into_iter().map(ViewRequest::of_kind).collect(),
            legends: Vec::new(),
        }
    }

    fn interval(lower: i64, upper: i64) -> Interval {
        Interval {
            lower: calc_numbers::Number::from(lower),
            upper: calc_numbers::Number::from(upper),
        }
    }

    fn plane_with(across: Interval, down: Interval) -> ViewRequest {
        ViewRequest::plane(
            [across, down]
                .into_iter()
                .map(|range| {
                    Some(AxisRange {
                        range,
                        scale: Scale::Linear,
                    })
                })
                .collect(),
        )
    }

    fn layout(request: &LayoutRequest) -> Result<PictureLayout, LayoutError> {
        plot_layout(request, &FontSet::bundled().unwrap())
    }

    fn with_views(width: u16, height: u16, views: Vec<ViewRequest>) -> LayoutRequest {
        LayoutRequest {
            width,
            height,
            text_size: 14,
            scale: ScaleFactor::from_percent(100).unwrap(),
            views,
            legends: Vec::new(),
        }
    }

    #[test]
    fn a_settled_range_with_narrow_labels_gives_a_wider_plot_area() {
        let narrow = layout(&with_views(
            800,
            600,
            vec![plane_with(interval(0, 1), interval(0, 1))],
        ))
        .unwrap();
        let wide = layout(&with_views(
            800,
            600,
            vec![plane_with(
                interval(-1_000_000, 1_000_000),
                interval(-1_000_000, 1_000_000),
            )],
        ))
        .unwrap();

        assert!(
            narrow.views[0].plot_area.width > wide.views[0].plot_area.width,
            "narrow {:?} wide {:?}",
            narrow.views[0].plot_area,
            wide.views[0].plot_area
        );
    }

    #[test]
    fn an_axis_without_a_settled_range_keeps_the_widest_gutter() {
        let unknown = layout(&with_views(
            800,
            600,
            vec![ViewRequest::plane(vec![None, None])],
        ))
        .unwrap();

        let gutter = unknown.views[0].gutter.expect("a plane view has a gutter");
        assert_eq!(
            gutter.width,
            TICK_LABEL_CELLS * unknown.metrics.cell.width()
        );
    }

    #[test]
    fn plane_plot_area_leaves_the_gutter_for_the_widest_tick_label() {
        let picture = layout(&request(800, 600, vec![ViewKind::Plane])).unwrap();
        let view = picture.views[0];
        let label_width = TICK_LABEL_CELLS * picture.metrics.cell.width();

        assert_eq!(
            view.plot_area.x,
            MARGIN_LOGICAL + label_width + GAP_LOGICAL + TICK_LENGTH_LOGICAL
        );
    }

    #[test]
    fn plot_area_depends_only_on_size_text_size_and_view_kind() {
        let first = layout(&request(800, 600, vec![ViewKind::Plane])).unwrap();
        let second = layout(&request(800, 600, vec![ViewKind::Plane])).unwrap();

        assert_eq!(first.views[0].plot_area, second.views[0].plot_area);
    }

    #[test]
    fn larger_text_makes_the_plot_area_smaller() {
        let small = layout(&request(800, 600, vec![ViewKind::Plane])).unwrap();
        let mut larger_request = request(800, 600, vec![ViewKind::Plane]);
        larger_request.text_size = 20;
        let large = layout(&larger_request).unwrap();

        assert!(large.views[0].plot_area.width < small.views[0].plot_area.width);
    }

    #[test]
    fn key_and_footer_rows_sit_above_the_bottom_margin() {
        let picture = layout(&request(800, 600, vec![ViewKind::Plane])).unwrap();

        assert_eq!(picture.footer_row.bottom(), 600 - MARGIN_LOGICAL);
    }

    #[test]
    fn plane_divisions_are_the_plot_area_pixel_counts() {
        let picture = layout(&request(800, 600, vec![ViewKind::Plane])).unwrap();
        let area = picture.views[0].plot_area;

        assert_eq!(
            picture.views[0].divisions(),
            vec![u32::from(area.width), u32::from(area.height)]
        );
    }

    #[test]
    fn space_divisions_take_the_smaller_pixel_count_on_every_axis() {
        let picture = layout(&request(800, 600, vec![ViewKind::Space])).unwrap();
        let area = picture.views[0].plot_area;
        let smaller = u32::from(area.width.min(area.height));

        assert_eq!(picture.views[0].divisions(), vec![smaller; 3]);
    }

    #[test]
    fn two_views_share_the_width_in_equal_columns() {
        let picture = layout(&request(800, 600, vec![ViewKind::Plane, ViewKind::Plane])).unwrap();

        assert_eq!(
            picture.views[0].plot_area.width,
            picture.views[1].plot_area.width
        );
    }

    fn escape_legend(inside: &str) -> LegendRequest {
        LegendRequest {
            legend: calc_viz::ColourLegend::EscapeTime,
            title: String::from("count"),
            class_names: [String::from(inside), String::from("undecided")],
            bar_words: Default::default(),
        }
    }

    #[test]
    fn legend_row_makes_the_plot_area_shorter() {
        let without = layout(&request(800, 600, vec![ViewKind::Plane])).unwrap();
        let mut with_legend = request(800, 600, vec![ViewKind::Plane]);
        with_legend.legends = vec![escape_legend("inside")];

        let with = layout(&with_legend).unwrap();

        let row = with.legends[0].rows[0].height;
        assert_eq!(
            with.views[0].plot_area.height,
            without.views[0].plot_area.height - row
        );
    }

    #[test]
    fn legend_rows_sit_between_key_and_footer() {
        let mut with_legend = request(800, 600, vec![ViewKind::Plane]);
        with_legend.legends = vec![escape_legend("inside")];

        let picture = layout(&with_legend).unwrap();

        let legend = picture.legends[0].rows[0];
        assert_eq!(
            (legend.y, legend.bottom()),
            (picture.key_row.bottom(), picture.footer_row.y)
        );
    }

    #[test]
    fn long_class_names_wrap_onto_a_reserved_row() {
        let mut narrow = request(400, 600, vec![ViewKind::Plane]);
        narrow.legends = vec![escape_legend(&"inside ".repeat(4))];

        let picture = layout(&narrow).unwrap();

        assert_eq!(picture.legends[0].rows.len(), 2);
    }

    #[test]
    fn layout_without_a_view_is_rejected() {
        assert_eq!(
            layout(&request(800, 600, Vec::new())),
            Err(LayoutError::NoView)
        );
    }

    #[test]
    fn picture_too_narrow_for_the_gutter_is_rejected() {
        assert_eq!(
            layout(&request(60, 600, vec![ViewKind::Plane])),
            Err(LayoutError::TooSmall {
                width: 60,
                height: 600
            })
        );
    }

    #[test]
    fn text_size_beyond_the_physical_range_is_a_scale_error() {
        let mut oversized = request(800, 600, vec![ViewKind::Plane]);
        oversized.scale = ScaleFactor::from_percent(400).unwrap();
        oversized.text_size = u16::MAX;

        assert_eq!(
            layout(&oversized),
            Err(LayoutError::Scale(ScaleError::PhysicalSizeTooLarge {
                logical: u16::MAX
            }))
        );
    }

    #[test]
    fn zero_text_size_is_a_grid_error() {
        let mut empty_text = request(800, 600, vec![ViewKind::Plane]);
        empty_text.text_size = 0;

        assert!(matches!(layout(&empty_text), Err(LayoutError::Grid(_))));
    }
}
