use calc_concepts::{ActivityDefinition, ActivityShape, ActivityVariation};

use crate::solve::concept_set;

const SQUARE_TURN_DEGREES: u16 = 45;
const TRIANGLE_TURN_DEGREES: u16 = 60;
const RESIZED_SCALE: Scale = Scale {
    numerator: 1,
    denominator: 2,
};
const UNSCALED: Scale = Scale {
    numerator: 1,
    denominator: 1,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scale {
    pub numerator: u32,
    pub denominator: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapePiece {
    pub shape: ActivityShape,
    pub turn_degrees: u16,
    pub scale: Scale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    MovedIn,
    WentBack,
    AlreadyHome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapeMatching {
    activity: ActivityDefinition,
    round: usize,
    pieces: Vec<ShapePiece>,
    homes: Vec<Option<usize>>,
}

pub fn fits(piece: &ShapePiece, house: ActivityShape) -> bool {
    piece.shape == house
}

fn piece_of(shape: ActivityShape, variation: ActivityVariation) -> ShapePiece {
    let turn_degrees = match (variation, shape) {
        (ActivityVariation::Orientation, ActivityShape::Square) => SQUARE_TURN_DEGREES,
        (ActivityVariation::Orientation, ActivityShape::Triangle) => TRIANGLE_TURN_DEGREES,
        (ActivityVariation::Orientation, ActivityShape::Circle)
        | (ActivityVariation::Identical | ActivityVariation::Size, _) => 0,
    };
    let scale = match variation {
        ActivityVariation::Size => RESIZED_SCALE,
        ActivityVariation::Identical | ActivityVariation::Orientation => UNSCALED,
    };
    ShapePiece {
        shape,
        turn_degrees,
        scale,
    }
}

fn permutations(count: usize) -> Vec<Vec<usize>> {
    if count == 0 {
        return vec![Vec::new()];
    }
    let mut all = Vec::new();
    for shorter in permutations(count - 1) {
        for at in 0..=shorter.len() {
            let mut longer = shorter.clone();
            longer.insert(at, count - 1);
            all.push(longer);
        }
    }
    all.sort();
    all
}

fn piece_order(count: usize, round: usize) -> Vec<usize> {
    let moved: Vec<Vec<usize>> = permutations(count)
        .into_iter()
        .filter(|order| order.iter().enumerate().all(|(at, house)| at != *house))
        .collect();
    match moved.len() {
        0 => (0..count).collect(),
        choices => moved[round % choices].clone(),
    }
}

impl ShapeMatching {
    pub fn new(activity: ActivityDefinition) -> Self {
        let mut matching = Self {
            activity,
            round: 0,
            pieces: Vec::new(),
            homes: Vec::new(),
        };
        matching.deal();
        matching
    }

    fn deal(&mut self) {
        let shapes = &self.activity.shapes;
        self.pieces = piece_order(shapes.len(), self.round)
            .into_iter()
            .map(|house| piece_of(shapes[house], self.activity.variation))
            .collect();
        self.homes = vec![None; self.pieces.len()];
    }

    pub fn activity(&self) -> &ActivityDefinition {
        &self.activity
    }

    pub fn houses(&self) -> &[ActivityShape] {
        &self.activity.shapes
    }

    pub fn pieces(&self) -> &[ShapePiece] {
        &self.pieces
    }

    pub fn home_of(&self, piece: usize) -> Option<usize> {
        self.homes.get(piece).copied().flatten()
    }

    pub fn place(&mut self, piece: usize, house: usize) -> Placement {
        let (Some(moving), Some(target)) = (self.pieces.get(piece), self.houses().get(house))
        else {
            return Placement::WentBack;
        };
        if self.home_of(piece).is_some() {
            return Placement::AlreadyHome;
        }
        if !fits(moving, *target) {
            return Placement::WentBack;
        }
        self.homes[piece] = Some(house);
        Placement::MovedIn
    }

    pub fn is_complete(&self) -> bool {
        self.homes.iter().all(Option::is_some)
    }

    pub fn next_round(&mut self) {
        self.round = self.round.wrapping_add(1);
        self.deal();
    }
}

pub fn concept_activities(identifier: &str) -> Vec<ActivityDefinition> {
    concept_set()
        .ok()
        .and_then(|concepts| {
            concepts
                .concepts
                .iter()
                .find(|node| node.identifier == identifier)
                .map(|node| node.activities.clone())
        })
        .unwrap_or_default()
}

pub fn beginnings_with_activities() -> Vec<String> {
    concept_set()
        .map(|concepts| {
            concepts
                .concepts
                .iter()
                .filter(|node| node.beginning && !node.activities.is_empty())
                .map(|node| node.identifier.clone())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_concepts::ActivityKind;

    fn activity(shapes: &[ActivityShape], variation: ActivityVariation) -> ActivityDefinition {
        ActivityDefinition {
            identifier: "shape-matching/test".to_owned(),
            kind: ActivityKind::ShapeMatching,
            shapes: shapes.to_vec(),
            variation,
            sources: vec!["learning-trajectories-lt2".to_owned()],
        }
    }

    const THREE: [ActivityShape; 3] = [
        ActivityShape::Circle,
        ActivityShape::Square,
        ActivityShape::Triangle,
    ];

    #[test]
    fn a_round_has_one_house_and_one_piece_per_shape() {
        let matching = ShapeMatching::new(activity(&THREE, ActivityVariation::Identical));

        let mut shapes: Vec<ActivityShape> =
            matching.pieces().iter().map(|piece| piece.shape).collect();
        shapes.sort();

        assert_eq!(matching.houses(), &THREE);
        assert_eq!(shapes, THREE.to_vec());
    }

    #[test]
    fn no_piece_starts_beside_its_own_house() {
        for round in 0..6 {
            let mut matching = ShapeMatching::new(activity(&THREE, ActivityVariation::Identical));
            for _ in 0..round {
                matching.next_round();
            }

            assert!(
                matching
                    .pieces()
                    .iter()
                    .zip(matching.houses())
                    .all(|(piece, house)| piece.shape != *house)
            );
        }
    }

    #[test]
    fn a_piece_moves_into_the_house_of_its_shape() {
        let mut matching = ShapeMatching::new(activity(
            &[ActivityShape::Circle, ActivityShape::Square],
            ActivityVariation::Identical,
        ));
        let circle = matching
            .pieces()
            .iter()
            .position(|piece| piece.shape == ActivityShape::Circle)
            .expect("a circle piece");

        assert_eq!(matching.place(circle, 0), Placement::MovedIn);
        assert_eq!(matching.home_of(circle), Some(0));
    }

    #[test]
    fn a_piece_goes_back_from_a_house_of_another_shape() {
        let mut matching = ShapeMatching::new(activity(
            &[ActivityShape::Circle, ActivityShape::Square],
            ActivityVariation::Identical,
        ));
        let circle = matching
            .pieces()
            .iter()
            .position(|piece| piece.shape == ActivityShape::Circle)
            .expect("a circle piece");

        assert_eq!(matching.place(circle, 1), Placement::WentBack);
        assert_eq!(matching.home_of(circle), None);
    }

    #[test]
    fn a_round_is_complete_when_every_piece_is_home_and_the_next_one_starts_empty() {
        let mut matching = ShapeMatching::new(activity(&THREE, ActivityVariation::Size));
        for piece in 0..3 {
            let shape = matching.pieces()[piece].shape;
            let house = THREE
                .iter()
                .position(|house| *house == shape)
                .expect("a house");
            matching.place(piece, house);
        }
        let complete = matching.is_complete();

        matching.next_round();

        assert!(complete);
        assert!(!matching.is_complete());
    }

    #[test]
    fn a_turned_square_and_triangle_are_turned_where_the_turn_shows() {
        let matching = ShapeMatching::new(activity(&THREE, ActivityVariation::Orientation));
        let turn = |shape| {
            matching
                .pieces()
                .iter()
                .find(|piece| piece.shape == shape)
                .map(|piece| piece.turn_degrees)
        };

        assert_eq!(turn(ActivityShape::Square), Some(45));
        assert_eq!(turn(ActivityShape::Triangle), Some(60));
        assert_eq!(turn(ActivityShape::Circle), Some(0));
    }

    #[test]
    fn the_turn_of_each_shape_is_as_far_as_can_be_from_every_turn_that_does_not_show() {
        let farthest = |symmetry: u16| {
            (0..symmetry)
                .max_by_key(|angle| (*angle).min(symmetry - angle))
                .expect("an angle")
        };

        assert_eq!(farthest(90), SQUARE_TURN_DEGREES);
        assert_eq!(farthest(120), TRIANGLE_TURN_DEGREES);
    }

    #[test]
    fn a_resized_piece_is_smaller_than_its_house_and_keeps_its_orientation() {
        let matching = ShapeMatching::new(activity(&THREE, ActivityVariation::Size));

        assert!(matching.pieces().iter().all(
            |piece| piece.turn_degrees == 0 && piece.scale.numerator < piece.scale.denominator
        ));
    }

    #[test]
    fn the_shipped_beginnings_with_activities_include_shape_matching() {
        assert!(beginnings_with_activities().contains(&"shape-matching".to_owned()));
        assert_eq!(concept_activities("shape-matching").len(), 3);
    }
}
