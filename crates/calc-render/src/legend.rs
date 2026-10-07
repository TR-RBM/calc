use std::f64::consts::PI;

use calc_numbers::{Number, cos_f64, sin_f64};
use calc_viz::{
    ColourLegend, ColourMapping, Interval, diverging_colour, domain_colour, sequential_colour,
};

use crate::geometry::PhysicalRect;
use crate::render::{Painter, RenderError, TextRow};
use crate::roles::{self, Fill, Patterns, Role};
use crate::text::cells::cells_of;
use crate::text::grid::GridMetrics;
use crate::ticks::{
    Outward, TICK_LABEL_CELLS, Tick, TickRoom, compare, exact_label, linear_ticks, outward_label,
};

const SEQUENTIAL_SWATCHES: u32 = 16;
const DIVERGING_SWATCHES: u32 = 17;
const HUE_SWATCHES: u32 = 12;
const LIGHTNESS_SWATCHES: u32 = 8;
const BAR_HEIGHT_EM: f64 = 0.8;
const NAMED_SWATCH_EM: f64 = 0.8;
const NAME_GAP_EM: f64 = 0.25;
const END_TICKS: u32 = 16;
const MINUS_PI: &str = "-\u{3c0}";
const PI_LABEL: &str = "\u{3c0}";
const ZERO_LABEL: &str = "0";
const AT_MOST: char = '\u{2264}';
const AT_LEAST: char = '\u{2265}';

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegendRequest {
    pub legend: ColourLegend,
    pub title: String,
    pub class_names: [String; 2],
    pub bar_words: [String; 2],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegendLayout {
    pub rows: Vec<PhysicalRect>,
    pub bar_height: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LegendPlan {
    pub bars: Vec<(usize, u32, u32)>,
    pub words: Vec<(usize, u32)>,
    pub named: Vec<(usize, u32)>,
    pub bar_rows: Vec<bool>,
    pub centre_row: bool,
    pub fits: bool,
}

pub(crate) struct Sizes {
    pub swatch: u32,
    pub bar_height: u32,
    pub named: u32,
    pub name_gap: u32,
    pub next_gap: u32,
    pub reserve: u32,
    pub cell: u32,
}

fn device(size: f64) -> u32 {
    u32::from(crate::mapping::pixel_round(size))
}

pub(crate) fn sizes(metrics: GridMetrics) -> Sizes {
    let em = f64::from(metrics.font_size_px);
    let cell = u32::from(metrics.cell.width());
    Sizes {
        swatch: device(em),
        bar_height: device(BAR_HEIGHT_EM * em),
        named: device(NAMED_SWATCH_EM * em),
        name_gap: device(NAME_GAP_EM * em),
        next_gap: device(em),
        reserve: u32::from(TICK_LABEL_CELLS) / 2 * cell + cell,
        cell,
    }
}

fn bar_counts(legend: ColourLegend) -> Vec<u32> {
    match legend {
        ColourLegend::Sequential | ColourLegend::EscapeTime => vec![SEQUENTIAL_SWATCHES],
        ColourLegend::Diverging => vec![DIVERGING_SWATCHES],
        ColourLegend::DomainColouring => vec![HUE_SWATCHES, LIGHTNESS_SWATCHES],
    }
}

pub(crate) fn plan(request: &LegendRequest, sizes: &Sizes, width: u32) -> LegendPlan {
    let mut x = u32::from(cells_of(&request.title)) * sizes.cell;
    let mut row = 0;
    let mut bars = Vec::new();
    let mut words = Vec::new();
    let mut bar_rows = vec![false];
    let mut fits = true;
    if request.legend == ColourLegend::DomainColouring {
        for (index, count) in bar_counts(request.legend).into_iter().enumerate() {
            let word = request.bar_words.get(index).map_or("", String::as_str);
            let word_width = u32::from(cells_of(word)) * sizes.cell;
            let word_gap = if word.is_empty() { 0 } else { sizes.next_gap };
            let segment = word_width + word_gap + count * sizes.swatch;
            let mut gap = match (x, index) {
                (0, _) => 0,
                (_, 0) => sizes.next_gap,
                _ => 2 * sizes.next_gap,
            };
            if x > 0 && x + gap + segment > width {
                row += 1;
                bar_rows.push(false);
                x = 0;
                gap = 0;
            }
            fits &= x + gap + segment <= width;
            let word_left = x + gap;
            if !word.is_empty() {
                words.push((row, word_left));
            }
            let bar_left = word_left + word_width + word_gap;
            bars.push((row, bar_left, count));
            if let Some(flag) = bar_rows.get_mut(row) {
                *flag = true;
            }
            x = bar_left + count * sizes.swatch;
        }
    } else {
        for count in bar_counts(request.legend) {
            let left = x + sizes.reserve;
            bars.push((0, left, count));
            x = left + count * sizes.swatch;
        }
        x += sizes.reserve;
        fits = x <= width;
        if let Some(flag) = bar_rows.first_mut() {
            *flag = true;
        }
    }
    let mut named = Vec::new();
    if request.legend == ColourLegend::EscapeTime {
        for name in &request.class_names {
            let needed = sizes.named + sizes.name_gap + u32::from(cells_of(name)) * sizes.cell;
            let mut left = x + sizes.next_gap;
            if left + needed > width {
                row += 1;
                bar_rows.push(false);
                left = 0;
                fits &= needed <= width;
            }
            named.push((row, left));
            x = left + needed;
        }
    }
    LegendPlan {
        bars,
        words,
        named,
        bar_rows,
        centre_row: request.legend == ColourLegend::Diverging,
        fits,
    }
}

pub(crate) fn row_heights(plan: &LegendPlan, sizes: &Sizes, row: u32) -> Vec<u32> {
    plan.bar_rows
        .iter()
        .enumerate()
        .map(|(index, has_bar)| {
            let centre = if index == 0 && plan.centre_row {
                row
            } else {
                0
            };
            if *has_bar {
                sizes.bar_height + row + centre
            } else {
                row + centre
            }
        })
        .collect()
}

fn fraction(range: &Interval, value: &Number) -> Option<f64> {
    Some(
        value
            .sub_exact(&range.lower)
            .ok()?
            .div_exact(&range.upper.sub_exact(&range.lower).ok()?)
            .ok()?
            .round_to_f64_ties_even(),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Alignment {
    Centred,
    FlushStart,
    FlushEnd,
}

#[derive(Clone, Debug, PartialEq)]
struct BarLabel {
    position: f64,
    text: String,
    alignment: Alignment,
    row: u16,
}

impl BarLabel {
    fn centred(position: f64, text: String) -> BarLabel {
        BarLabel {
            position,
            text,
            alignment: Alignment::Centred,
            row: 0,
        }
    }
}

fn end_labels(
    range: &Interval,
    with_centre: bool,
    beyond: [bool; 2],
    separator: char,
) -> Vec<BarLabel> {
    let ticks = linear_ticks(range, &TickRoom::at_most(END_TICKS), separator);
    let nearest = |target: &Number| {
        ticks.iter().min_by(|first, second| {
            let distance = |tick: &Tick| {
                tick.value
                    .sub_exact(target)
                    .map_or(f64::INFINITY, |difference| {
                        difference.round_to_f64_ties_even().abs()
                    })
            };
            distance(first).total_cmp(&distance(second))
        })
    };
    let centre = with_centre
        .then(|| {
            range
                .lower
                .add_exact(&range.upper)
                .ok()
                .and_then(|sum| sum.div_exact(&Number::from(2_i64)).ok())
        })
        .flatten()
        .and_then(|centre| {
            Some(BarLabel {
                row: 1,
                ..BarLabel::centred(
                    fraction(range, &centre)?,
                    exact_label(&centre, separator)
                        .or_else(|| nearest(&centre).map(|tick| tick.label.clone()))?,
                )
            })
        });
    let ends = [
        (
            &range.lower,
            beyond[0],
            AT_MOST,
            Outward::Down,
            0.0,
            Alignment::FlushStart,
        ),
        (
            &range.upper,
            beyond[1],
            AT_LEAST,
            Outward::Up,
            1.0,
            Alignment::FlushEnd,
        ),
    ];
    let mut labels: Vec<BarLabel> = Vec::new();
    for (end, is_exceeded, mark, direction, position, alignment) in ends {
        let exact = exact_label(end, separator);
        let label = if is_exceeded {
            exact
                .or_else(|| outward_label(end, direction, separator))
                .map(|bound| BarLabel {
                    position,
                    text: format!("{mark}{bound}"),
                    alignment,
                    row: 0,
                })
        } else {
            match exact {
                Some(text) => {
                    fraction(range, end).map(|position| BarLabel::centred(position, text))
                }
                None => nearest(end).and_then(|tick| {
                    Some(BarLabel::centred(
                        fraction(range, &tick.value)?,
                        tick.label.clone(),
                    ))
                }),
            }
        };
        if let Some(label) = label
            && !labels
                .iter()
                .chain(&centre)
                .any(|known| known.text == label.text)
        {
            labels.push(label);
        }
    }
    labels.extend(centre);
    labels
}

fn reference_modulus_label(range: &Interval, separator: char) -> Option<(f64, String)> {
    let reference = range.upper.sub_exact(&range.lower).ok()?;
    let doubled = reference.mul_exact(&Number::from(2_i64)).ok()?;
    let span = Interval {
        lower: Number::from(0_i64),
        upper: doubled,
    };
    let ticks = linear_ticks(&span, &TickRoom::at_most(LIGHTNESS_SWATCHES), separator);
    if let Some(label) = exact_label(&reference, separator) {
        return Some((0.5, label));
    }
    let tick = ticks
        .iter()
        .filter(|tick| {
            compare(&tick.value, &Number::from(0_i64)) == Some(std::cmp::Ordering::Greater)
        })
        .min_by(|first, second| {
            let distance = |tick: &Tick| {
                tick.value
                    .sub_exact(&reference)
                    .map_or(f64::INFINITY, |difference| {
                        difference.round_to_f64_ties_even().abs()
                    })
            };
            distance(first).total_cmp(&distance(second))
        })?;
    let value = tick.value.round_to_f64_ties_even();
    let reference = reference.round_to_f64_ties_even();
    Some((value / (value + reference), tick.label.clone()))
}

pub(crate) fn draw_legend(
    painter: &mut Painter<'_>,
    layout: &LegendLayout,
    (request, mapping, beyond): (&LegendRequest, &ColourMapping, [bool; 2]),
    layer: usize,
) -> Result<(), RenderError> {
    let slot = TextRow::Legend { layer };
    let Some(first) = layout.rows.first().copied() else {
        return Ok(());
    };
    let sizes = sizes(painter.metrics);
    let plan = plan(request, &sizes, u32::from(first.width));
    if !plan.fits {
        return Err(RenderError::TextDoesNotFit(slot));
    }
    let to_x = |offset: u32| {
        u16::try_from(u32::from(first.x) + offset).map_err(|_| RenderError::TextDoesNotFit(slot))
    };
    let tops = |row: usize| {
        let rect = layout.rows.get(row).copied().unwrap_or(first);
        let has_bar = plan.bar_rows.get(row).copied().unwrap_or(false);
        let band = if has_bar { layout.bar_height } else { 0 };
        (rect.y, rect.y + band)
    };
    let theme = painter.theme;
    painter.text(&request.title, first.x, tops(0).1, theme.text)?;
    let bar_words = request.bar_words.iter().filter(|word| !word.is_empty());
    for ((row, left), word) in plan.words.iter().zip(bar_words) {
        painter.text(word, to_x(*left)?, tops(*row).1, theme.text_secondary)?;
    }
    let range = &mapping.range;
    let lower = range.lower.round_to_f64_ties_even();
    let upper = range.upper.round_to_f64_ties_even();
    let mut labels: Vec<(u16, u16, String)> = Vec::new();
    for (bar_index, (row, left, count)) in plan.bars.iter().enumerate() {
        let (band_top, text_top) = tops(*row);
        let bar_left = u32::from(first.x) + left;
        let width = count * sizes.swatch;
        for swatch in 0..*count {
            let centre = (f64::from(swatch) + 0.5) / f64::from(*count);
            let colour = match (request.legend, bar_index) {
                (ColourLegend::Diverging, _) => {
                    diverging_colour(range, lower + centre * (upper - lower))
                }
                (ColourLegend::DomainColouring, 0) => {
                    let angle = -PI + centre * 2.0 * PI;
                    let modulus = upper - lower;
                    domain_colour(range, modulus * cos_f64(angle), modulus * sin_f64(angle))
                }
                (ColourLegend::DomainColouring, _) => {
                    let modulus = (upper - lower) * centre / (1.0 - centre);
                    domain_colour(range, modulus, 0.0)
                }
                _ => sequential_colour(range, lower + centre * (upper - lower)),
            };
            let x = u16::try_from(bar_left + swatch * sizes.swatch)
                .map_err(|_| RenderError::TextDoesNotFit(slot))?;
            let rect = PhysicalRect::new(
                x,
                band_top,
                u16::try_from(sizes.swatch).unwrap_or(u16::MAX),
                u16::try_from(sizes.bar_height).unwrap_or(u16::MAX),
            );
            let fill = colour.map_or(roles::Fill::Role(Role::Missing), |colour| {
                Fill::Solid(roles::from_map(colour))
            });
            let patterns = Patterns::new(theme, painter.scale, (rect.x, rect.y));
            painter
                .canvas
                .paint_rect(rect, &|x, y| patterns.covering(fill, x, y));
        }
        let first_centre = 0.5 / f64::from(*count);
        let last_centre = 1.0 - first_centre;
        let is_domain = request.legend == ColourLegend::DomainColouring;
        let positioned: Vec<BarLabel> = match (request.legend, bar_index) {
            (ColourLegend::DomainColouring, 0) => vec![
                BarLabel::centred(first_centre, String::from(MINUS_PI)),
                BarLabel::centred(0.5, String::from(ZERO_LABEL)),
                BarLabel::centred(last_centre, String::from(PI_LABEL)),
            ],
            (ColourLegend::DomainColouring, _) => {
                std::iter::once((first_centre, String::from(ZERO_LABEL)))
                    .chain(reference_modulus_label(range, painter.separator))
                    .map(|(position, text)| BarLabel::centred(position, text))
                    .collect()
            }
            (ColourLegend::Diverging, _) => end_labels(range, true, beyond, painter.separator),
            _ => end_labels(range, false, beyond, painter.separator),
        };
        for BarLabel {
            position,
            text: label,
            alignment,
            row: label_row,
        } in positioned
        {
            let centre = f64::from(bar_left) + position * f64::from(width);
            let label_width = painter.text_width(&label);
            let mut label_left = match alignment {
                Alignment::Centred => centre - f64::from(label_width) / 2.0,
                Alignment::FlushStart => f64::from(bar_left),
                Alignment::FlushEnd => f64::from(bar_left + width) - f64::from(label_width),
            };
            if is_domain {
                let rightmost = f64::from(bar_left + width) - f64::from(label_width);
                if label_left > rightmost {
                    label_left = rightmost;
                }
                if label_left < f64::from(bar_left) {
                    label_left = f64::from(bar_left);
                }
            }
            let label_top = text_top + label_row * painter.row_height();
            labels.push((crate::mapping::pixel_round(label_left), label_top, label));
        }
    }
    for (left, top, label) in labels {
        painter.text(&label, left, top, theme.text_secondary)?;
    }
    for ((row, left), (name, role)) in plan.named.iter().zip(
        request
            .class_names
            .iter()
            .zip([Role::Inside, Role::Undecided]),
    ) {
        let row_top = tops(*row).1;
        let size = u16::try_from(sizes.named).unwrap_or(u16::MAX);
        let x = to_x(*left)?;
        let y = row_top + painter.row_height().saturating_sub(size) / 2;
        let swatch = PhysicalRect::new(x, y, size, size);
        let patterns = Patterns::new(theme, painter.scale, (x, y));
        painter.show(role);
        painter.canvas.paint_rect(swatch, &|column, line| {
            patterns.covering(Fill::Role(role), column, line)
        });
        let name_x = u16::try_from(u32::from(x) + sizes.named + sizes.name_gap)
            .map_err(|_| RenderError::TextDoesNotFit(slot))?;
        painter.text(name, name_x, row_top, theme.text_secondary)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use calc_numbers::Integer;

    use super::*;

    fn range(lower: Number, upper: Number) -> Interval {
        Interval { lower, upper }
    }

    fn third(numerator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(3_i64)).unwrap()
    }

    fn texts(labels: &[BarLabel]) -> Vec<&str> {
        labels.iter().map(|label| label.text.as_str()).collect()
    }

    #[test]
    fn writable_ends_are_labelled_at_the_ends() {
        let labels = end_labels(
            &range(Number::from(0_i64), Number::from(32_i64)),
            false,
            [false, false],
            '.',
        );

        assert_eq!(
            labels,
            vec![
                BarLabel::centred(0.0, String::from("0")),
                BarLabel::centred(1.0, String::from("32"))
            ]
        );
    }

    #[test]
    fn unwritable_end_is_labelled_by_the_nearest_round_tick_at_its_own_position() {
        let labels = end_labels(&range(third(-10), third(10)), false, [false, false], '.');

        assert_eq!(labels[1].text, "3");
    }

    #[test]
    fn writable_end_with_values_beyond_it_is_labelled_as_a_bound() {
        let labels = end_labels(
            &range(Number::from(0_i64), Number::from(32_i64)),
            false,
            [false, true],
            '.',
        );

        assert_eq!(texts(&labels), vec!["0", "\u{2265}32"]);
    }

    #[test]
    fn unwritable_ends_with_values_beyond_them_are_labelled_by_bounds_rounded_outward() {
        let labels = end_labels(&range(third(-10), third(10)), false, [true, true], '.');

        assert_eq!(texts(&labels), vec!["\u{2264}-3.4", "\u{2265}3.4"]);
    }

    #[test]
    fn marked_lower_end_is_flush_with_the_start_of_the_bar() {
        let labels = end_labels(&range(third(-10), third(10)), false, [true, false], '.');

        assert_eq!(
            (labels[0].position, labels[0].alignment),
            (0.0, Alignment::FlushStart)
        );
    }

    #[test]
    fn marked_upper_end_is_flush_with_the_end_of_the_bar() {
        let labels = end_labels(
            &range(Number::from(0_i64), Number::from(32_i64)),
            false,
            [false, true],
            '.',
        );

        assert_eq!(
            (labels[1].position, labels[1].alignment),
            (1.0, Alignment::FlushEnd)
        );
    }

    #[test]
    fn diverging_centre_is_labelled_in_the_middle_of_the_second_row() {
        let labels = end_labels(&range(third(-10), third(10)), true, [true, true], '.');

        assert_eq!(
            labels[2],
            BarLabel {
                position: 0.5,
                text: String::from("0"),
                alignment: Alignment::Centred,
                row: 1
            }
        );
    }

    #[test]
    fn diverging_legend_reserves_a_second_label_row() {
        let metrics =
            crate::text::grid::grid_metrics(&crate::text::font::FontSet::bundled().unwrap(), 14)
                .unwrap();
        let sizes = sizes(metrics);
        let request = LegendRequest {
            legend: ColourLegend::Diverging,
            title: String::from("value"),
            class_names: Default::default(),
            bar_words: Default::default(),
        };

        let heights = row_heights(&plan(&request, &sizes, 2000), &sizes, 20);

        assert_eq!(heights, vec![sizes.bar_height + 40]);
    }

    #[test]
    fn escape_time_names_that_do_not_fit_wrap_onto_the_next_row() {
        let metrics =
            crate::text::grid::grid_metrics(&crate::text::font::FontSet::bundled().unwrap(), 14)
                .unwrap();
        let request = LegendRequest {
            legend: ColourLegend::EscapeTime,
            title: String::from("count"),
            class_names: [String::from("inside"), String::from("undecided")],
            bar_words: Default::default(),
        };

        let narrow = plan(&request, &sizes(metrics), 420);

        assert_eq!((narrow.bar_rows.len(), narrow.named[1].0), (2, 1));
    }

    #[test]
    fn domain_bars_that_do_not_fit_one_row_wrap_the_second_bar() {
        let metrics =
            crate::text::grid::grid_metrics(&crate::text::font::FontSet::bundled().unwrap(), 14)
                .unwrap();
        let request = LegendRequest {
            legend: ColourLegend::DomainColouring,
            title: String::from("colour value"),
            class_names: Default::default(),
            bar_words: [String::from("argument"), String::from("modulus")],
        };

        let narrow = plan(&request, &sizes(metrics), 500);

        assert_eq!(
            (narrow.bar_rows, narrow.bars[1].0, narrow.words[1]),
            (vec![true, true], 1, (1, 0))
        );
    }

    #[test]
    fn domain_bar_keeps_one_em_after_its_word_and_two_em_after_the_first_bar() {
        let metrics =
            crate::text::grid::grid_metrics(&crate::text::font::FontSet::bundled().unwrap(), 14)
                .unwrap();
        let sizes = sizes(metrics);
        let request = LegendRequest {
            legend: ColourLegend::DomainColouring,
            title: String::new(),
            class_names: Default::default(),
            bar_words: [String::from("argument"), String::from("modulus")],
        };

        let wide = plan(&request, &sizes, 2000);

        let first_bar_end = wide.bars[0].1 + 12 * sizes.swatch;
        assert_eq!(
            (
                wide.bars[0].1 - 8 * sizes.cell,
                wide.words[1].1 - first_bar_end
            ),
            (sizes.next_gap, 2 * sizes.next_gap)
        );
    }

    #[test]
    fn domain_colouring_has_a_hue_and_a_lightness_bar() {
        let metrics =
            crate::text::grid::grid_metrics(&crate::text::font::FontSet::bundled().unwrap(), 14)
                .unwrap();
        let request = LegendRequest {
            legend: ColourLegend::DomainColouring,
            title: String::new(),
            class_names: Default::default(),
            bar_words: [String::from("argument"), String::from("modulus")],
        };

        let wide = plan(&request, &sizes(metrics), 2000);

        assert_eq!(
            wide.bars
                .iter()
                .map(|(_, _, count)| *count)
                .collect::<Vec<_>>(),
            vec![12, 8]
        );
    }
}
