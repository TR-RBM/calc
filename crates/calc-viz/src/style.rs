use crate::record::Interval;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KindColour {
    Exact,
    Symbolic,
    Numeric,
    Sampled,
    Measured,
    Derived,
    Running,
    Differs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColourMap {
    Sequential,
    Diverging,
    Categorical,
    DomainColouring,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColourMapping {
    pub map: ColourMap,
    pub range: Interval,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LinePattern {
    None,
    Solid,
    Dashed,
    Dotted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Marker {
    None,
    Dot,
    Cross,
    Circle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Emphasis {
    Normal,
    Provisional,
    Unresolved,
    Inside,
    Undecided,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleRole {
    pub kind: KindColour,
    pub colour_map: Option<ColourMapping>,
    pub line: LinePattern,
    pub marker: Marker,
    pub emphasis: Emphasis,
}
