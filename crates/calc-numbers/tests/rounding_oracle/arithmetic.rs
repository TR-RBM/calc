use std::cmp::Ordering;

use calc_numbers::{Integer, Number};

pub const ZERO_MAGNITUDE_EXPONENT: i32 = -100_000;
pub const UNBOUNDED_MAGNITUDE_EXPONENT: i32 = 100_000;

#[derive(Clone, Debug)]
pub struct Point {
    pub machine: Option<f64>,
    pub exact: Option<Number>,
}

impl Point {
    pub fn machine(value: f64) -> Self {
        Self {
            machine: Some(value),
            exact: None,
        }
    }

    pub fn exact(value: Number) -> Self {
        Self {
            machine: None,
            exact: Some(value),
        }
    }
}

pub trait Arithmetic {
    type Value: Clone + std::fmt::Debug;

    fn exact(&self, value: f64) -> Self::Value;
    fn integer(&self, value: &Integer) -> Self::Value;
    fn add(&self, left: &Self::Value, right: &Self::Value) -> Self::Value;
    fn sub(&self, left: &Self::Value, right: &Self::Value) -> Self::Value;
    fn mul(&self, left: &Self::Value, right: &Self::Value) -> Self::Value;
    fn div(&self, numerator: &Self::Value, denominator: &Self::Value) -> Option<Self::Value>;
    fn negate(&self, value: &Self::Value) -> Self::Value;
    fn square_root(&self, value: &Self::Value) -> Option<Self::Value>;
    fn times_power_of_two(&self, value: &Self::Value, exponent: i32) -> Self::Value;
    fn widen(&self, value: &Self::Value, radius_exponent: i32) -> Self::Value;
    fn magnitude_exponent(&self, value: &Self::Value) -> i32;
    fn nearest_small_integer(&self, value: &Self::Value) -> Option<i64>;
    fn quarter_turns(&self, value: f64) -> Option<(Self::Value, u8)>;
    fn pi(&self) -> Self::Value;
    fn ln2(&self) -> Self::Value;
    fn truncation_exponent(&self) -> i32;
    fn compare_lower(&self, value: &Self::Value, point: &Point) -> Option<Ordering>;
    fn compare_upper(&self, value: &Self::Value, point: &Point) -> Option<Ordering>;

    fn small_integer(&self, value: i64) -> Self::Value {
        self.integer(&Integer::from(value))
    }
}
