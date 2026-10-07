use calc_numbers::Number;

use crate::record::Interval;

const BASE_DIMENSION_COUNT: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Dimension {
    pub exponents: [i8; BASE_DIMENSION_COUNT],
}

impl Dimension {
    pub const DIMENSIONLESS: Dimension = Dimension {
        exponents: [0; BASE_DIMENSION_COUNT],
    };

    pub const TEMPERATURE: Dimension = Dimension {
        exponents: [0, 0, 0, 0, 1, 0, 0, 0],
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scale {
    Linear,
    Logarithmic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AxisUnit {
    Coherent {
        symbol: String,
    },
    Unit {
        factor: Number,
        pi_exponent: i8,
        symbol: String,
    },
    TemperatureScale {
        factor: Number,
        offset: Number,
        symbol: String,
    },
}

impl AxisUnit {
    pub fn dimensionless() -> AxisUnit {
        AxisUnit::Coherent {
            symbol: String::new(),
        }
    }

    pub fn symbol(&self) -> &str {
        match self {
            AxisUnit::Coherent { symbol }
            | AxisUnit::Unit { symbol, .. }
            | AxisUnit::TemperatureScale { symbol, .. } => symbol,
        }
    }

    pub fn is_coherent(&self) -> bool {
        matches!(self, AxisUnit::Coherent { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewAxis {
    pub range: Interval,
    pub scale: Scale,
    pub dimension: Dimension,
    pub unit: AxisUnit,
    pub divisions: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Projection {
    Perspective { field_of_view_degrees: Number },
    Orthographic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Camera {
    pub azimuth_degrees: Number,
    pub elevation_degrees: Number,
    pub projection: Projection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View2 {
    pub x: ViewAxis,
    pub y: ViewAxis,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View3 {
    pub x: ViewAxis,
    pub y: ViewAxis,
    pub z: ViewAxis,
    pub camera: Camera,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum View {
    View2(Box<View2>),
    View3(Box<View3>),
}

impl View {
    pub fn axis_count(&self) -> usize {
        match self {
            View::View2(_) => 2,
            View::View3(_) => 3,
        }
    }

    pub fn axes(&self) -> Vec<&ViewAxis> {
        match self {
            View::View2(view) => vec![&view.x, &view.y],
            View::View3(view) => vec![&view.x, &view.y, &view.z],
        }
    }
}
