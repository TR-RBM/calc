use calc_numbers::Number;

use crate::primitive::Column;
use crate::view::Dimension;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PointIndex(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SegmentIndex(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AngleIndex(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ElementIndex {
    Point(PointIndex),
    Segment(SegmentIndex),
    Angle(AngleIndex),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stroke {
    Solid,
    Auxiliary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Segment {
    pub from: PointIndex,
    pub to: PointIndex,
    pub stroke: Stroke,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Angle {
    pub vertex: PointIndex,
    pub first_arm: PointIndex,
    pub second_arm: PointIndex,
    pub is_oriented: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RightAngleMark {
    Square,
    ArcWithDot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkKind {
    AngleArc { count: u8 },
    RightAngle(RightAngleMark),
    EqualTicks { count: u8 },
    Direction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Mark {
    pub element: ElementIndex,
    pub kind: MarkKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NameKind {
    Vertex,
    Side,
    Angle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoherentValue {
    Rational(Number),
    PiMultiple(Number),
    Enclosure { lower: Number, upper: Number },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LabelValue {
    pub value: CoherentValue,
    pub dimension: Dimension,
    pub printed: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Quantity {
    Named,
    Given(LabelValue),
    Sought { step: Option<u32> },
    Answered(LabelValue),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Designation {
    Hypotenuse,
    Leg,
    Height,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub element: ElementIndex,
    pub names: NameKind,
    pub name: String,
    pub quantity: Quantity,
    pub designation: Option<Designation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FigureScale {
    Sketch,
    ToScale { view_units_per_metre: Number },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Figure {
    pub points: [Column; 2],
    pub segments: Vec<Segment>,
    pub angles: Vec<Angle>,
    pub marks: Vec<Mark>,
    pub labels: Vec<Label>,
    pub scale: FigureScale,
}

impl ElementIndex {
    pub fn kind(self) -> NameKind {
        match self {
            ElementIndex::Point(_) => NameKind::Vertex,
            ElementIndex::Segment(_) => NameKind::Side,
            ElementIndex::Angle(_) => NameKind::Angle,
        }
    }
}

impl Figure {
    pub fn point_count(&self) -> usize {
        self.points[0].len()
    }

    pub fn segment(&self, index: SegmentIndex) -> Option<&Segment> {
        usize::try_from(index.0)
            .ok()
            .and_then(|index| self.segments.get(index))
    }

    pub fn angle(&self, index: AngleIndex) -> Option<&Angle> {
        usize::try_from(index.0)
            .ok()
            .and_then(|index| self.angles.get(index))
    }
}
