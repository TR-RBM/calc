use calc_numbers::{
    Integer, Number, acos_f32, acos_f64, asin_f32, asin_f64, atan_f32, atan_f64, atan2_f32,
    atan2_f64, cos_f32, cos_f64, elementary_rounding_error_bound_f32,
    elementary_rounding_error_bound_f64, exp_f32, exp_f64, ln_f32, ln_f64, pow_f32, pow_f64,
    sin_f32, sin_f64, sqrt_f32, sqrt_f64, tan_f32, tan_f64,
};

use crate::arithmetic::Point;
use crate::bits::{binary_exponent, integral_to_i64, power_of_two};
use crate::dyadic::DyadicArithmetic;
use crate::knowledge::{ElementaryFunction, knowledge};
use crate::machine::MachineArithmetic;
use crate::verdict::{Claim, Verdict, exact_order, verdict};

const BASE_PRECISIONS: [u32; 6] = [96, 192, 384, 768, 1536, 3072];
const PRECISION_STEP: u32 = 64;
const LARGEST_PRECISION_EXTRA: u32 = 2400;
const F32_FRACTION_BITS: u32 = 23;
const F32_EXPONENT_MASK: u32 = 0xff;
const F32_EXPONENT_FIELDS: u32 = 255;
const F32_FRACTION_MASK: u32 = 0x7f_ffff;
const F32_OVERFLOW_EXPONENT: i32 = 128;
const F32_OVERFLOW_HALF_UNIT_EXPONENT: i32 = 103;
const F64_OVERFLOW_EXPONENT: u32 = 1024;
const F64_OVERFLOW_HALF_UNIT_EXPONENT: u32 = 970;
const EXPONENTIAL_PRECISION_FACTOR: f64 = 1.5;
const LOGARITHM_NEAR_ONE_LOW: f64 = 0.5;
const LOGARITHM_NEAR_ONE_HIGH: f64 = 2.0;
const ARCCOSINE_PRECISION_EXTRA: u32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Binary32,
    Binary64,
}

pub fn evaluate_f32(function: ElementaryFunction, arguments: &[f32]) -> f32 {
    let first = arguments.first().copied().unwrap_or(f32::NAN);
    let second = arguments.get(1).copied().unwrap_or(f32::NAN);
    match function {
        ElementaryFunction::Exponential => exp_f32(first),
        ElementaryFunction::Logarithm => ln_f32(first),
        ElementaryFunction::SquareRoot => sqrt_f32(first),
        ElementaryFunction::Sine => sin_f32(first),
        ElementaryFunction::Cosine => cos_f32(first),
        ElementaryFunction::Tangent => tan_f32(first),
        ElementaryFunction::Arcsine => asin_f32(first),
        ElementaryFunction::Arccosine => acos_f32(first),
        ElementaryFunction::Arctangent => atan_f32(first),
        ElementaryFunction::Power => pow_f32(first, second),
        ElementaryFunction::Arctangent2 => atan2_f32(first, second),
    }
}

pub fn evaluate_f64(function: ElementaryFunction, arguments: &[f64]) -> f64 {
    let first = arguments.first().copied().unwrap_or(f64::NAN);
    let second = arguments.get(1).copied().unwrap_or(f64::NAN);
    match function {
        ElementaryFunction::Exponential => exp_f64(first),
        ElementaryFunction::Logarithm => ln_f64(first),
        ElementaryFunction::SquareRoot => sqrt_f64(first),
        ElementaryFunction::Sine => sin_f64(first),
        ElementaryFunction::Cosine => cos_f64(first),
        ElementaryFunction::Tangent => tan_f64(first),
        ElementaryFunction::Arcsine => asin_f64(first),
        ElementaryFunction::Arccosine => acos_f64(first),
        ElementaryFunction::Arctangent => atan_f64(first),
        ElementaryFunction::Power => pow_f64(first, second),
        ElementaryFunction::Arctangent2 => atan2_f64(first, second),
    }
}

fn exponent_or_zero(value: f64) -> i64 {
    binary_exponent(value).map_or(0, i64::from)
}

fn clamped_extra(extra: i64) -> u32 {
    u32::try_from(extra.clamp(0, i64::from(LARGEST_PRECISION_EXTRA))).unwrap_or_default()
}

pub fn precision_extra(function: ElementaryFunction, arguments: &[f64]) -> u32 {
    let first = arguments.first().copied().unwrap_or(0.0);
    let second = arguments.get(1).copied().unwrap_or(0.0);
    let exponent = exponent_or_zero(first);
    let extra = match function {
        ElementaryFunction::Exponential if first.is_finite() => {
            integral_to_i64((-first * EXPONENTIAL_PRECISION_FACTOR).ceil()).unwrap_or(0)
        }
        ElementaryFunction::Logarithm
            if (LOGARITHM_NEAR_ONE_LOW..=LOGARITHM_NEAR_ONE_HIGH).contains(&first) =>
        {
            -exponent_or_zero(first - 1.0)
        }
        ElementaryFunction::SquareRoot => -exponent / 2,
        ElementaryFunction::Sine | ElementaryFunction::Tangent => exponent.abs(),
        ElementaryFunction::Cosine => exponent.max(0),
        ElementaryFunction::Arcsine | ElementaryFunction::Arctangent => -exponent,
        ElementaryFunction::Arccosine => i64::from(ARCCOSINE_PRECISION_EXTRA),
        ElementaryFunction::Power => {
            let estimate = second * (f64::from(i32::try_from(exponent).unwrap_or_default()) + 0.5);
            integral_to_i64(
                (-estimate)
                    .ceil()
                    .clamp(0.0, f64::from(LARGEST_PRECISION_EXTRA)),
            )
            .unwrap_or(0)
        }
        ElementaryFunction::Arctangent2 => exponent_or_zero(second) - exponent,
        _ => 0,
    };
    clamped_extra(extra)
}

fn f32_overflow_threshold() -> Point {
    let threshold =
        power_of_two(F32_OVERFLOW_EXPONENT) - power_of_two(F32_OVERFLOW_HALF_UNIT_EXPONENT);
    Point::machine(threshold)
}

fn f64_overflow_threshold() -> Point {
    let two = Integer::from(2i64);
    let threshold = &two.pow(F64_OVERFLOW_EXPONENT) - &two.pow(F64_OVERFLOW_HALF_UNIT_EXPONENT);
    Point::exact(Number::Integer(threshold))
}

fn power_of_two_claim(claim: &Number) -> Option<f64> {
    let rounded = claim.round_to_f64_ties_even();
    let exponent = binary_exponent(rounded)?;
    let is_power_of_two = rounded == power_of_two(exponent);
    let is_exact = Number::F64(rounded)
        .to_exact()
        .ok()
        .and_then(|exact| exact_order(&exact, claim))
        == Some(std::cmp::Ordering::Equal);
    (is_power_of_two && is_exact).then_some(rounded)
}

pub struct F32ClaimTable {
    bounds: Vec<Option<f64>>,
}

impl F32ClaimTable {
    pub fn new() -> Self {
        let bounds = (0..F32_EXPONENT_FIELDS)
            .map(|field| {
                let smallest = f32::from_bits(field << F32_FRACTION_BITS);
                let largest = f32::from_bits((field << F32_FRACTION_BITS) | F32_FRACTION_MASK);
                let smallest_claim = elementary_rounding_error_bound_f32(smallest)?;
                let largest_claim = elementary_rounding_error_bound_f32(largest)?;
                assert_eq!(
                    exact_order(&smallest_claim, &largest_claim),
                    Some(std::cmp::Ordering::Equal),
                    "the f32 claim differs within exponent field {field}"
                );
                let bound = power_of_two_claim(&smallest_claim);
                assert!(
                    bound.is_some(),
                    "the f32 claim of exponent field {field} is not a power of two"
                );
                bound
            })
            .collect();
        Self { bounds }
    }

    pub fn claim(&self, result: f32) -> Option<Claim> {
        if !result.is_finite() {
            return None;
        }
        let field = (result.to_bits() >> F32_FRACTION_BITS) & F32_EXPONENT_MASK;
        let bound = (*self.bounds.get(usize::try_from(field).ok()?)?)?;
        let value = f64::from(result);
        Some(Claim {
            low: Point::machine(value - bound),
            high: Point::machine(value + bound),
        })
    }
}

fn f64_claim(result: f64) -> Option<Claim> {
    let bound = elementary_rounding_error_bound_f64(result)?;
    let value = Number::F64(result).to_exact().ok()?;
    Some(Claim {
        low: Point::exact(value.sub_exact(&bound).ok()?),
        high: Point::exact(value.add_exact(&bound).ok()?),
    })
}

pub struct Oracle {
    machine: MachineArithmetic,
    dyadic: Vec<DyadicArithmetic>,
    f32_claims: F32ClaimTable,
    f32_overflow: Point,
    f64_overflow: Point,
}

impl Oracle {
    pub fn new() -> Self {
        Self {
            machine: MachineArithmetic::new(),
            dyadic: Vec::new(),
            f32_claims: F32ClaimTable::new(),
            f32_overflow: f32_overflow_threshold(),
            f64_overflow: f64_overflow_threshold(),
        }
    }

    fn dyadic_index(&mut self, precision: u32) -> usize {
        let rounded = precision.div_ceil(PRECISION_STEP) * PRECISION_STEP;
        if let Some(index) = self
            .dyadic
            .iter()
            .position(|arithmetic| arithmetic.precision() == rounded)
        {
            return index;
        }
        self.dyadic.push(DyadicArithmetic::new(rounded));
        self.dyadic.len() - 1
    }

    fn escalate(
        &mut self,
        function: ElementaryFunction,
        arguments: &[f64],
        result: f64,
        claim: Option<&Claim>,
        format: Format,
    ) -> Verdict {
        let extra = precision_extra(function, arguments);
        for base in BASE_PRECISIONS {
            let index = self.dyadic_index(base + extra);
            let arithmetic = &self.dyadic[index];
            let known = knowledge(arithmetic, function, arguments);
            let overflow = match format {
                Format::Binary32 => &self.f32_overflow,
                Format::Binary64 => &self.f64_overflow,
            };
            let found = verdict(arithmetic, &known, result, claim, overflow);
            if found != Verdict::Undecided {
                return found;
            }
        }
        Verdict::Undecided
    }

    pub fn check_f32_result(
        &mut self,
        function: ElementaryFunction,
        arguments: &[f32],
        result: f32,
    ) -> Verdict {
        let wide: Vec<f64> = arguments
            .iter()
            .map(|argument| f64::from(*argument))
            .collect();
        let claim = self.f32_claims.claim(result);
        let known = knowledge(&self.machine, function, &wide);
        let found = verdict(
            &self.machine,
            &known,
            f64::from(result),
            claim.as_ref(),
            &self.f32_overflow,
        );
        if found != Verdict::Undecided {
            return found;
        }
        self.escalate(
            function,
            &wide,
            f64::from(result),
            claim.as_ref(),
            Format::Binary32,
        )
    }

    pub fn check_f64_result(
        &mut self,
        function: ElementaryFunction,
        arguments: &[f64],
        result: f64,
    ) -> Verdict {
        let claim = f64_claim(result);
        self.escalate(
            function,
            arguments,
            result,
            claim.as_ref(),
            Format::Binary64,
        )
    }

    pub fn check_f32(&mut self, function: ElementaryFunction, arguments: &[f32]) -> (f32, Verdict) {
        let result = evaluate_f32(function, arguments);
        (result, self.check_f32_result(function, arguments, result))
    }

    pub fn check_f64(&mut self, function: ElementaryFunction, arguments: &[f64]) -> (f64, Verdict) {
        let result = evaluate_f64(function, arguments);
        (result, self.check_f64_result(function, arguments, result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next_f32_up(value: f32, steps: u32) -> f32 {
        f32::from_bits(value.to_bits() + steps)
    }

    #[test]
    fn f32_exponential_of_one_is_within_its_claim() {
        let mut oracle = Oracle::new();

        let (_, found) = oracle.check_f32(ElementaryFunction::Exponential, &[1.0]);

        assert_eq!(found, Verdict::WithinBound);
    }

    #[test]
    fn f32_result_two_ulps_high_exceeds_one_ulp_claim() {
        let mut oracle = Oracle::new();
        let correct = exp_f32(1.0);

        let found = oracle.check_f32_result(
            ElementaryFunction::Exponential,
            &[1.0],
            next_f32_up(correct, 2),
        );

        assert_eq!(found, Verdict::ExceedsBound);
    }

    #[test]
    fn f32_sine_of_largest_value_misrounded_by_two_ulps_exceeds_claim() {
        let mut oracle = Oracle::new();
        let correct = sin_f32(f32::MAX);

        let found = oracle.check_f32_result(
            ElementaryFunction::Sine,
            &[f32::MAX],
            next_f32_up(correct, 2),
        );

        assert_eq!(found, Verdict::ExceedsBound);
    }

    #[test]
    fn f32_logarithm_misrounded_near_one_exceeds_claim() {
        let mut oracle = Oracle::new();
        let argument = next_f32_up(1.0, 1);
        let correct = ln_f32(argument);

        let found = oracle.check_f32_result(
            ElementaryFunction::Logarithm,
            &[argument],
            next_f32_up(correct, 2),
        );

        assert_eq!(found, Verdict::ExceedsBound);
    }

    #[test]
    fn f32_square_root_one_ulp_off_stays_within_one_ulp_claim() {
        let mut oracle = Oracle::new();
        let correct = sqrt_f32(2.0);

        let found = oracle.check_f32_result(
            ElementaryFunction::SquareRoot,
            &[2.0],
            next_f32_up(correct, 1),
        );

        assert_eq!(found, Verdict::WithinBound);
    }

    #[test]
    fn f32_finite_result_where_exponential_overflows_exceeds_claim() {
        let mut oracle = Oracle::new();

        let found = oracle.check_f32_result(ElementaryFunction::Exponential, &[100.0], f32::MAX);

        assert_eq!(found, Verdict::ExceedsBound);
    }

    #[test]
    fn f32_infinity_just_below_overflow_threshold_exceeds_claim() {
        let mut oracle = Oracle::new();
        let argument = 88.72;

        let found =
            oracle.check_f32_result(ElementaryFunction::Exponential, &[argument], f32::INFINITY);

        assert_eq!(found, Verdict::ExceedsBound);
    }

    #[test]
    fn f64_exponential_of_one_is_within_its_claim() {
        let mut oracle = Oracle::new();

        let (_, found) = oracle.check_f64(ElementaryFunction::Exponential, &[1.0]);

        assert_eq!(found, Verdict::WithinBound);
    }

    #[test]
    fn f64_result_one_ulp_high_exceeds_half_ulp_claim() {
        let mut oracle = Oracle::new();
        let misrounded = exp_f64(1.0).next_up();

        let found = oracle.check_f64_result(ElementaryFunction::Exponential, &[1.0], misrounded);

        assert_eq!(found, Verdict::ExceedsBound);
    }

    #[test]
    fn f64_power_rounding_exact_midpoint_to_odd_neighbour_is_within_half_ulp() {
        let mut oracle = Oracle::new();
        let midpoint_neighbour = 16_677_181_699_666_568.0f64;

        let found = oracle.check_f64_result(
            ElementaryFunction::Power,
            &[3.0, 34.0],
            midpoint_neighbour + 2.0,
        );

        assert_eq!(found, Verdict::WithinBound);
    }

    #[test]
    fn f64_power_one_ulp_beyond_midpoint_neighbour_exceeds_claim() {
        let mut oracle = Oracle::new();
        let midpoint_neighbour = 16_677_181_699_666_568.0f64;

        let found = oracle.check_f64_result(
            ElementaryFunction::Power,
            &[3.0, 34.0],
            midpoint_neighbour + 4.0,
        );

        assert_eq!(found, Verdict::ExceedsBound);
    }

    #[test]
    fn f64_tangent_misrounded_near_pole_exceeds_claim() {
        let mut oracle = Oracle::new();
        let argument = std::f64::consts::FRAC_PI_2;
        let misrounded = tan_f64(argument).next_down();

        let found = oracle.check_f64_result(ElementaryFunction::Tangent, &[argument], misrounded);

        assert_eq!(found, Verdict::ExceedsBound);
    }

    #[test]
    fn f64_arctangent2_with_wrong_quadrant_exceeds_claim() {
        let mut oracle = Oracle::new();
        let wrong = atan2_f64(1.0, 1.0);

        let found = oracle.check_f64_result(ElementaryFunction::Arctangent2, &[-1.0, -1.0], wrong);

        assert_eq!(found, Verdict::ExceedsBound);
    }

    #[test]
    fn f64_not_a_number_for_valid_argument_exceeds_claim() {
        let mut oracle = Oracle::new();

        let found = oracle.check_f64_result(ElementaryFunction::Arcsine, &[0.5], f64::NAN);

        assert_eq!(found, Verdict::ExceedsBound);
    }

    #[test]
    fn precision_extra_grows_for_tiny_arctangent_argument() {
        let extra = precision_extra(ElementaryFunction::Arctangent, &[1.0e-300]);

        assert!(extra > 990);
    }
}
