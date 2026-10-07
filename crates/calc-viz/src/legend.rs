use crate::primitive::Primitive;
use crate::sampling::SampledShape;
use crate::scene::Layer;
use crate::style::ColourMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColourLegend {
    Sequential,
    Diverging,
    EscapeTime,
    DomainColouring,
}

pub fn shape_legend(shape: &SampledShape) -> Option<ColourLegend> {
    match shape {
        SampledShape::ScalarGrid(_) | SampledShape::Surface(_) | SampledShape::Occupancy(_) => {
            Some(ColourLegend::Sequential)
        }
        SampledShape::EscapeTime => Some(ColourLegend::EscapeTime),
        SampledShape::ComplexGrid(_) => Some(ColourLegend::DomainColouring),
        SampledShape::Curve(_)
        | SampledShape::Band { .. }
        | SampledShape::Points(_)
        | SampledShape::VectorField { .. } => None,
    }
}

pub fn layer_legend(layer: &Layer) -> Option<ColourLegend> {
    let mapping = layer.style.colour_map.as_ref()?;
    let has_classes =
        matches!(&layer.primitive, Primitive::ScalarGrid(grid) if grid.classes.is_some());
    match mapping.map {
        ColourMap::Sequential if has_classes => Some(ColourLegend::EscapeTime),
        ColourMap::Sequential => Some(ColourLegend::Sequential),
        ColourMap::Diverging => Some(ColourLegend::Diverging),
        ColourMap::DomainColouring => Some(ColourLegend::DomainColouring),
        ColourMap::Categorical => None,
    }
}

#[cfg(test)]
mod tests {
    use calc_numbers::Number;

    use super::*;
    use crate::primitive::{CellGrid, Column, ScalarGrid};
    use crate::record::Interval;
    use crate::scene::{InputIndex, ViewIndex};
    use crate::style::{ColourMapping, Emphasis, KindColour, LinePattern, Marker, StyleRole};

    fn layer(map: Option<ColourMap>, classes: Option<Vec<u8>>) -> Layer {
        let unit = Interval {
            lower: Number::from(0_i64),
            upper: Number::from(1_i64),
        };
        Layer {
            view: ViewIndex(0),
            input: InputIndex(0),
            primitive: Primitive::ScalarGrid(ScalarGrid {
                cells: CellGrid {
                    region: vec![unit.clone(), unit.clone()],
                    counts: vec![1, 1],
                },
                scalar: Column::F64(vec![0.5]),
                classes,
            }),
            style: StyleRole {
                kind: KindColour::Sampled,
                colour_map: map.map(|map| ColourMapping {
                    map,
                    range: unit.clone(),
                }),
                line: LinePattern::Solid,
                marker: Marker::None,
                emphasis: Emphasis::Normal,
            },
            value_bounds: Vec::new(),
            precision: None,
            columns: None,
            readings: Vec::new(),
        }
    }

    #[test]
    fn layer_without_a_colour_map_has_no_legend() {
        assert_eq!(layer_legend(&layer(None, None)), None);
    }

    #[test]
    fn sequential_grid_with_classes_has_the_escape_time_legend() {
        assert_eq!(
            layer_legend(&layer(Some(ColourMap::Sequential), Some(vec![0]))),
            Some(ColourLegend::EscapeTime)
        );
    }

    #[test]
    fn diverging_layer_has_the_diverging_legend() {
        assert_eq!(
            layer_legend(&layer(Some(ColourMap::Diverging), None)),
            Some(ColourLegend::Diverging)
        );
    }

    #[test]
    fn categorical_layer_has_no_legend_yet() {
        assert_eq!(
            layer_legend(&layer(Some(ColourMap::Categorical), None)),
            None
        );
    }
}
