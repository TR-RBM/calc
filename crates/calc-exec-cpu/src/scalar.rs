use std::ops::{Add, Div, Mul, Neg, Sub};

use calc_exec::{Batch, Constant, PlanOp};

pub(crate) trait Scalar:
    Copy
    + Send
    + Sync
    + PartialEq
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
{
    const ZERO: Self;

    fn from_constant(constant: Constant) -> Option<Self>;
    fn channel(batch: &Batch, channel: usize) -> Option<&[Self]>;
    fn channel_mut(batch: &mut Batch, channel: usize) -> Option<&mut [Self]>;

    fn abs(self) -> Self;
    fn sqrt(self) -> Self;
    fn fused_multiply_add(self, factor: Self, addend: Self) -> Self;
    fn minimum(self, other: Self) -> Self;
    fn maximum(self, other: Self) -> Self;
    fn floor(self) -> Self;
    fn ceil(self) -> Self;
    fn trunc(self) -> Self;
    fn round_ties_even(self) -> Self;
    fn copysign(self, sign: Self) -> Self;
    fn unary_reference(operation: PlanOp) -> Option<fn(Self) -> Self>;
    fn binary_reference(operation: PlanOp) -> Option<fn(Self, Self) -> Self>;
}

impl Scalar for f32 {
    const ZERO: f32 = 0.0;

    fn from_constant(constant: Constant) -> Option<f32> {
        match constant {
            Constant::F32(value) => Some(value),
            Constant::F64(_) => None,
        }
    }

    fn channel(batch: &Batch, channel: usize) -> Option<&[f32]> {
        batch.f32_channel(channel)
    }

    fn channel_mut(batch: &mut Batch, channel: usize) -> Option<&mut [f32]> {
        batch.f32_channel_mut(channel)
    }

    fn abs(self) -> f32 {
        f32::abs(self)
    }

    fn sqrt(self) -> f32 {
        f32::sqrt(self)
    }

    fn fused_multiply_add(self, factor: f32, addend: f32) -> f32 {
        f32::mul_add(self, factor, addend)
    }

    fn minimum(self, other: f32) -> f32 {
        calc_numbers::minimum_f32(self, other)
    }

    fn maximum(self, other: f32) -> f32 {
        calc_numbers::maximum_f32(self, other)
    }

    fn floor(self) -> f32 {
        f32::floor(self)
    }

    fn ceil(self) -> f32 {
        f32::ceil(self)
    }

    fn trunc(self) -> f32 {
        f32::trunc(self)
    }

    fn round_ties_even(self) -> f32 {
        f32::round_ties_even(self)
    }

    fn copysign(self, sign: f32) -> f32 {
        f32::copysign(self, sign)
    }

    fn unary_reference(operation: PlanOp) -> Option<fn(f32) -> f32> {
        match operation {
            PlanOp::Exp => Some(calc_numbers::exp_f32),
            PlanOp::Ln => Some(calc_numbers::ln_f32),
            PlanOp::Sin => Some(calc_numbers::sin_f32),
            PlanOp::Cos => Some(calc_numbers::cos_f32),
            PlanOp::Tan => Some(calc_numbers::tan_f32),
            PlanOp::Asin => Some(calc_numbers::asin_f32),
            PlanOp::Acos => Some(calc_numbers::acos_f32),
            PlanOp::Atan => Some(calc_numbers::atan_f32),
            _ => None,
        }
    }

    fn binary_reference(operation: PlanOp) -> Option<fn(f32, f32) -> f32> {
        match operation {
            PlanOp::Atan2 => Some(calc_numbers::atan2_f32),
            PlanOp::Pow => Some(calc_numbers::pow_f32),
            _ => None,
        }
    }
}

impl Scalar for f64 {
    const ZERO: f64 = 0.0;

    fn from_constant(constant: Constant) -> Option<f64> {
        match constant {
            Constant::F64(value) => Some(value),
            Constant::F32(_) => None,
        }
    }

    fn channel(batch: &Batch, channel: usize) -> Option<&[f64]> {
        batch.f64_channel(channel)
    }

    fn channel_mut(batch: &mut Batch, channel: usize) -> Option<&mut [f64]> {
        batch.f64_channel_mut(channel)
    }

    fn abs(self) -> f64 {
        f64::abs(self)
    }

    fn sqrt(self) -> f64 {
        f64::sqrt(self)
    }

    fn fused_multiply_add(self, factor: f64, addend: f64) -> f64 {
        f64::mul_add(self, factor, addend)
    }

    fn minimum(self, other: f64) -> f64 {
        calc_numbers::minimum_f64(self, other)
    }

    fn maximum(self, other: f64) -> f64 {
        calc_numbers::maximum_f64(self, other)
    }

    fn floor(self) -> f64 {
        f64::floor(self)
    }

    fn ceil(self) -> f64 {
        f64::ceil(self)
    }

    fn trunc(self) -> f64 {
        f64::trunc(self)
    }

    fn round_ties_even(self) -> f64 {
        f64::round_ties_even(self)
    }

    fn copysign(self, sign: f64) -> f64 {
        f64::copysign(self, sign)
    }

    fn unary_reference(operation: PlanOp) -> Option<fn(f64) -> f64> {
        match operation {
            PlanOp::Exp => Some(calc_numbers::exp_f64),
            PlanOp::Ln => Some(calc_numbers::ln_f64),
            PlanOp::Sin => Some(calc_numbers::sin_f64),
            PlanOp::Cos => Some(calc_numbers::cos_f64),
            PlanOp::Tan => Some(calc_numbers::tan_f64),
            PlanOp::Asin => Some(calc_numbers::asin_f64),
            PlanOp::Acos => Some(calc_numbers::acos_f64),
            PlanOp::Atan => Some(calc_numbers::atan_f64),
            _ => None,
        }
    }

    fn binary_reference(operation: PlanOp) -> Option<fn(f64, f64) -> f64> {
        match operation {
            PlanOp::Atan2 => Some(calc_numbers::atan2_f64),
            PlanOp::Pow => Some(calc_numbers::pow_f64),
            _ => None,
        }
    }
}
