use calc_exec::Domain;
use calc_numbers::Number;

use crate::figure::Figure;
use crate::record::Interval;

#[derive(Clone, Debug)]
pub enum Column {
    F32(Vec<f32>),
    F64(Vec<f64>),
}

impl PartialEq for Column {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Column::F32(left), Column::F32(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(left, right)| left.to_bits() == right.to_bits())
            }
            (Column::F64(left), Column::F64(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(left, right)| left.to_bits() == right.to_bits())
            }
            (Column::F32(_), Column::F64(_)) | (Column::F64(_), Column::F32(_)) => false,
        }
    }
}

impl Column {
    pub fn len(&self) -> usize {
        match self {
            Column::F32(values) => values.len(),
            Column::F64(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn domain(&self) -> Domain {
        match self {
            Column::F32(_) => Domain::F32,
            Column::F64(_) => Domain::F64,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Polyline {
    pub coordinates: Vec<Column>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    pub abscissa: Column,
    pub lower: Column,
    pub upper: Column,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Points {
    pub coordinates: Vec<Column>,
    pub scalar: Option<Column>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Arrows {
    pub bases: Vec<Column>,
    pub components: Vec<Column>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellGrid {
    pub region: Vec<Interval>,
    pub counts: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScalarGrid {
    pub cells: CellGrid,
    pub scalar: Column,
    pub classes: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComplexGrid {
    pub cells: CellGrid,
    pub real: Column,
    pub imaginary: Column,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TriangleMesh {
    pub vertices: Vec<Column>,
    pub triangles: Vec<[u32; 3]>,
    pub scalar: Option<Column>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Voxels {
    pub occupied: Vec<[i64; 3]>,
    pub scalar: Option<Column>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    pub positions: Vec<Column>,
    pub edges: Vec<[u32; 2]>,
    pub is_directed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Formula {
    pub expression: String,
    pub anchor: Vec<Number>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitLayout {
    pub bits: Vec<bool>,
    pub exponent_start: u32,
    pub fraction_start: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Primitive {
    Polyline(Polyline),
    Band(Band),
    Points(Points),
    Arrows(Arrows),
    ScalarGrid(ScalarGrid),
    ComplexGrid(ComplexGrid),
    TriangleMesh(TriangleMesh),
    Voxels(Voxels),
    Graph(Graph),
    Formula(Formula),
    BitLayout(BitLayout),
    Figure(Box<Figure>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_with_the_same_nan_bits_are_equal() {
        let column = Column::F64(vec![f64::NAN, 1.0]);

        assert_eq!(column.clone(), column);
    }

    #[test]
    fn columns_with_opposite_zeros_differ() {
        assert_ne!(Column::F64(vec![0.0]), Column::F64(vec![-0.0]));
    }

    #[test]
    fn columns_of_different_domains_differ() {
        assert_ne!(Column::F32(vec![1.0]), Column::F64(vec![1.0]));
    }
}
