use calc_exec::{Batch, Constant, PlanOp};

pub(crate) trait Element: Copy + Send + Sync + PartialEq {
    const ZERO: Self;

    fn from_constant(constant: Constant) -> Option<Self>;
    fn channel(batch: &Batch, channel: usize) -> Option<&[Self]>;
    fn channel_mut(batch: &mut Batch, channel: usize) -> Option<&mut [Self]>;
    fn floor(self) -> Self;
    fn ceil(self) -> Self;
    fn trunc(self) -> Self;
    fn round_ties_even(self) -> Self;
    fn fused_multiply_add(self, factor: Self, addend: Self) -> Self;
    fn unary_reference(operation: PlanOp) -> Option<fn(Self) -> Self>;
    fn binary_reference(operation: PlanOp) -> Option<fn(Self, Self) -> Self>;
    fn combine(operation: calc_exec::ReduceOperation, left: Self, right: Self) -> Self;
}

pub(crate) trait Lanes: Copy {
    type Element: Element;
    type Mask: Copy;

    const WIDTH: usize;

    fn splat(value: Self::Element) -> Self;
    fn load(values: &[Self::Element]) -> Self;
    fn store(self, values: &mut [Self::Element]);

    fn add(self, other: Self) -> Self;
    fn sub(self, other: Self) -> Self;
    fn mul(self, other: Self) -> Self;
    fn div(self, other: Self) -> Self;
    fn sqrt(self) -> Self;
    fn neg(self) -> Self;
    fn abs(self) -> Self;
    fn copysign(self, sign: Self) -> Self;

    fn less(self, other: Self) -> Self::Mask;
    fn less_or_equal(self, other: Self) -> Self::Mask;
    fn equal(self, other: Self) -> Self::Mask;
    fn not_equal(self, other: Self) -> Self::Mask;
    fn unordered(self) -> Self::Mask;
    fn negative(self) -> Self::Mask;

    fn select(mask: Self::Mask, when_true: Self, when_false: Self) -> Self;
    fn mask_and(left: Self::Mask, right: Self::Mask) -> Self::Mask;
    fn mask_or(left: Self::Mask, right: Self::Mask) -> Self::Mask;
    fn mask_not(mask: Self::Mask) -> Self::Mask;
    fn mask_none(mask: Self::Mask) -> bool;
    fn mask_empty() -> Self::Mask;
    fn quiet_nan(self) -> Self;

    fn by_lane(self, operation: fn(Self::Element) -> Self::Element) -> Self {
        let mut values = [Self::Element::ZERO; 8];
        let width = Self::WIDTH;
        self.store(&mut values[..width]);
        for value in &mut values[..width] {
            *value = operation(*value);
        }
        Self::load(&values[..width])
    }

    fn by_lane_pair(
        self,
        other: Self,
        operation: fn(Self::Element, Self::Element) -> Self::Element,
    ) -> Self {
        let mut left = [Self::Element::ZERO; 8];
        let mut right = [Self::Element::ZERO; 8];
        let width = Self::WIDTH;
        self.store(&mut left[..width]);
        other.store(&mut right[..width]);
        for lane in 0..width {
            left[lane] = operation(left[lane], right[lane]);
        }
        Self::load(&left[..width])
    }

    fn minimum(self, other: Self) -> Self {
        let take_left = Self::mask_or(
            self.less(other),
            Self::mask_and(self.equal(other), self.negative()),
        );
        let chosen = Self::select(take_left, self, other);
        let with_right_nan = Self::select(other.unordered(), other.quiet_nan(), chosen);
        Self::select(self.unordered(), self.quiet_nan(), with_right_nan)
    }

    fn maximum(self, other: Self) -> Self {
        let take_left = Self::mask_or(
            other.less(self),
            Self::mask_and(self.equal(other), Self::mask_not(self.negative())),
        );
        let chosen = Self::select(take_left, self, other);
        let with_right_nan = Self::select(other.unordered(), other.quiet_nan(), chosen);
        Self::select(self.unordered(), self.quiet_nan(), with_right_nan)
    }
}
