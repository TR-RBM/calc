use std::cmp::Ordering;
use std::fmt;

use crate::ball::Ball;
use crate::integer::Integer;
use crate::number::Number;
use crate::series;

pub const SIGNIFICANT_DIGITS_LIMIT: u32 = 5_000;

pub const ENCLOSURE_BUDGET_BITS: usize = 17_032;

const INITIAL_PRECISION_BITS: usize = 64;
const GUARD_BITS: usize = 32;
const BITS_PER_TEN_DIGITS: usize = 34;
const ARGUMENT_WIDTH_BITS: usize = 4;
const EXPONENTIAL_BITS_PER_HUNDRED: i64 = 145;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecimalEnclosure {
    pub significant_digits: u32,
    pub lower: Number,
    pub upper: Number,
    pub width: Number,
    pub reached: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnclosureError {
    NotFinite,
    DigitsOutOfRange { digits: u32, limit: u32 },
    NoEnclosure { budget_bits: usize },
    OverBudget { budget_bits: usize },
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BallError {
    NoEnclosure,
    OverBudget { integer_bits: usize },
    Cancelled,
}

#[derive(Clone, Copy)]
pub struct EnclosureStep<'step> {
    pub precision_bits: usize,
    pub budget_bits: usize,
    is_cancelled: &'step dyn Fn() -> bool,
}

#[derive(Clone)]
pub struct RealBall<'step> {
    ball: Ball,
    budget_bits: usize,
    is_cancelled: &'step dyn Fn() -> bool,
}

impl fmt::Debug for RealBall<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RealBall")
            .field("ball", &self.ball)
            .field("budget_bits", &self.budget_bits)
            .finish()
    }
}

fn bit_length_of(value: &Integer) -> usize {
    usize::try_from(value.bit_length()).unwrap_or(usize::MAX)
}

impl<'step> EnclosureStep<'step> {
    fn ball(&self, ball: Ball) -> Result<RealBall<'step>, BallError> {
        RealBall {
            ball,
            budget_bits: self.budget_bits,
            is_cancelled: self.is_cancelled,
        }
        .within_budget()
    }

    fn missing(&self) -> BallError {
        if (self.is_cancelled)() {
            BallError::Cancelled
        } else {
            BallError::NoEnclosure
        }
    }

    pub fn exact(&self, value: &Number) -> Result<RealBall<'step>, BallError> {
        let exact = value.to_exact().map_err(|_| BallError::NoEnclosure)?;
        let integer_bits = match &exact {
            Number::Integer(integer) => bit_length_of(integer),
            Number::Rational(rational) => bit_length_of(rational.numerator())
                .saturating_sub(bit_length_of(rational.denominator())),
            Number::F32(_) | Number::F64(_) => 0,
        };
        if integer_bits.saturating_add(self.precision_bits) > self.budget_bits + 1 {
            return Err(BallError::OverBudget { integer_bits });
        }
        let ball = Ball::from_exact(&exact, self.precision_bits).ok_or(BallError::NoEnclosure)?;
        self.ball(ball)
    }

    pub fn between(&self, lower: &Number, upper: &Number) -> Result<RealBall<'step>, BallError> {
        let lower = self.exact(lower)?;
        let upper = self.exact(upper)?;
        self.ball(lower.ball.hull(&upper.ball))
    }

    pub fn pi(&self) -> Result<RealBall<'step>, BallError> {
        let ball = series::pi_until(self.precision_bits, self.is_cancelled)
            .ok_or_else(|| self.missing())?;
        self.ball(ball)
    }

    pub fn e(&self) -> Result<RealBall<'step>, BallError> {
        self.exact(&Number::Integer(Integer::one()))?.exp()
    }
}

impl<'step> RealBall<'step> {
    fn with(&self, ball: Ball) -> Result<RealBall<'step>, BallError> {
        RealBall {
            ball,
            budget_bits: self.budget_bits,
            is_cancelled: self.is_cancelled,
        }
        .within_budget()
    }

    fn within_budget(self) -> Result<RealBall<'step>, BallError> {
        let bits = self.ball.center_bit_length();
        if bits > self.budget_bits {
            return Err(BallError::OverBudget {
                integer_bits: bits.saturating_sub(self.ball.precision()),
            });
        }
        Ok(self)
    }

    fn missing(&self) -> BallError {
        if (self.is_cancelled)() {
            BallError::Cancelled
        } else {
            BallError::NoEnclosure
        }
    }

    pub fn precision_bits(&self) -> usize {
        self.ball.precision()
    }

    pub fn add(&self, other: &RealBall<'step>) -> Result<RealBall<'step>, BallError> {
        self.with(self.ball.add(&other.ball))
    }

    pub fn sub(&self, other: &RealBall<'step>) -> Result<RealBall<'step>, BallError> {
        self.with(self.ball.sub(&other.ball))
    }

    pub fn mul(&self, other: &RealBall<'step>) -> Result<RealBall<'step>, BallError> {
        let product_bits = (self.ball.center_bit_length() + other.ball.center_bit_length())
            .saturating_sub(self.ball.precision());
        if product_bits > self.budget_bits + 1 {
            return Err(BallError::OverBudget {
                integer_bits: product_bits.saturating_sub(self.ball.precision()),
            });
        }
        self.with(self.ball.mul(&other.ball))
    }

    pub fn div(&self, other: &RealBall<'step>) -> Result<RealBall<'step>, BallError> {
        let quotient_bits = (self.ball.center_bit_length() + self.ball.precision())
            .saturating_sub(other.ball.center_bit_length());
        if quotient_bits > self.budget_bits + 1 {
            return Err(BallError::OverBudget {
                integer_bits: quotient_bits.saturating_sub(self.ball.precision()),
            });
        }
        let quotient = self.ball.div(&other.ball).ok_or(BallError::NoEnclosure)?;
        self.with(quotient)
    }

    pub fn negated(&self) -> RealBall<'step> {
        RealBall {
            ball: self.ball.negated(),
            budget_bits: self.budget_bits,
            is_cancelled: self.is_cancelled,
        }
    }

    pub fn absolute(&self) -> RealBall<'step> {
        RealBall {
            ball: self.ball.absolute(),
            budget_bits: self.budget_bits,
            is_cancelled: self.is_cancelled,
        }
    }

    pub fn sqrt(&self) -> Result<RealBall<'step>, BallError> {
        if !self.ball.is_surely_nonnegative() {
            return Err(BallError::NoEnclosure);
        }
        let root = self.ball.sqrt().ok_or(BallError::NoEnclosure)?;
        self.with(root)
    }

    pub fn power(&self, exponent: i64) -> Result<RealBall<'step>, BallError> {
        let mut result = self.with(Ball::one(self.ball.precision()))?;
        let mut square = self.clone();
        let mut remaining = exponent.unsigned_abs();
        while remaining > 0 {
            if (self.is_cancelled)() {
                return Err(BallError::Cancelled);
            }
            if remaining % 2 == 1 {
                result = result.mul(&square)?;
            }
            remaining /= 2;
            if remaining > 0 {
                square = square.mul(&square)?;
            }
        }
        if exponent < 0 {
            return self.with(Ball::one(self.ball.precision()))?.div(&result);
        }
        Ok(result)
    }

    pub fn exp(&self) -> Result<RealBall<'step>, BallError> {
        if !self.ball.has_radius_below_power_of_two(ARGUMENT_WIDTH_BITS) {
            return Err(BallError::NoEnclosure);
        }
        let integer_part = self.ball.floor_of_center();
        let result_integer_bits = integer_part.to_i64().map_or(i64::MAX, |bits| {
            bits.saturating_mul(EXPONENTIAL_BITS_PER_HUNDRED) / 100
        });
        let precision = i64::try_from(self.ball.precision()).unwrap_or(i64::MAX);
        let budget = i64::try_from(self.budget_bits).unwrap_or(i64::MAX);
        if result_integer_bits.saturating_add(precision) > budget {
            return Err(BallError::OverBudget {
                integer_bits: usize::try_from(result_integer_bits).unwrap_or(usize::MAX),
            });
        }
        let result =
            series::exp_until(&self.ball, self.is_cancelled).ok_or_else(|| self.missing())?;
        self.with(result)
    }

    pub fn ln(&self) -> Result<RealBall<'step>, BallError> {
        let result =
            series::ln_of_ball(&self.ball, self.is_cancelled).ok_or_else(|| self.missing())?;
        self.with(result)
    }

    pub fn sin(&self) -> Result<RealBall<'step>, BallError> {
        let pair = series::sine_cosine_of_ball(&self.ball, self.budget_bits, self.is_cancelled)
            .ok_or_else(|| self.missing())?;
        self.with(pair.sine)
    }

    pub fn cos(&self) -> Result<RealBall<'step>, BallError> {
        let pair = series::sine_cosine_of_ball(&self.ball, self.budget_bits, self.is_cancelled)
            .ok_or_else(|| self.missing())?;
        self.with(pair.cosine)
    }

    pub fn tan(&self) -> Result<RealBall<'step>, BallError> {
        let pair = series::sine_cosine_of_ball(&self.ball, self.budget_bits, self.is_cancelled)
            .ok_or_else(|| self.missing())?;
        self.with(pair.sine)?.div(&self.with(pair.cosine)?)
    }

    pub fn atan(&self) -> Result<RealBall<'step>, BallError> {
        if !self.ball.has_radius_below_power_of_two(ARGUMENT_WIDTH_BITS) {
            return Err(BallError::NoEnclosure);
        }
        let result = series::arctangent_until(&self.ball, self.is_cancelled)
            .ok_or_else(|| self.missing())?;
        self.with(result)
    }

    pub fn atan2(&self, abscissa: &RealBall<'step>) -> Result<RealBall<'step>, BallError> {
        let (ordinate_lower, ordinate_upper) = self.bounds().ok_or(BallError::NoEnclosure)?;
        let (abscissa_lower, abscissa_upper) = abscissa.bounds().ok_or(BallError::NoEnclosure)?;
        if is_above_zero(&abscissa_lower) {
            return self.div(abscissa)?.atan();
        }
        if is_below_zero(&abscissa_upper) {
            let turned = self.div(abscissa)?.atan()?;
            if is_above_zero(&ordinate_lower) {
                return turned.add(&self.half_turn()?);
            }
            if is_below_zero(&ordinate_upper) {
                return turned.sub(&self.half_turn()?);
            }
            return Err(BallError::NoEnclosure);
        }
        let quarter = self.quarter_turn()?;
        if is_above_zero(&ordinate_lower) {
            return quarter.sub(&abscissa.div(self)?.atan()?);
        }
        if is_below_zero(&ordinate_upper) {
            return quarter.negated().sub(&abscissa.div(self)?.atan()?);
        }
        Err(BallError::NoEnclosure)
    }

    fn half_turn(&self) -> Result<RealBall<'step>, BallError> {
        let pi = series::pi_until(self.ball.precision(), self.is_cancelled)
            .ok_or_else(|| self.missing())?;
        self.with(pi)
    }

    fn quarter_turn(&self) -> Result<RealBall<'step>, BallError> {
        let two = self.with(
            Ball::from_exact(
                &Number::Integer(Integer::from(2_i64)),
                self.ball.precision(),
            )
            .ok_or(BallError::NoEnclosure)?,
        )?;
        self.half_turn()?.div(&two)
    }

    pub fn bounds(&self) -> Option<(Number, Number)> {
        self.ball.exact_bounds()
    }
}

fn is_above_zero(value: &Number) -> bool {
    !is_negative(value) && !is_zero(value)
}

fn is_below_zero(value: &Number) -> bool {
    is_negative(value)
}

fn is_negative(value: &Number) -> bool {
    match value {
        Number::Integer(integer) => integer.is_negative(),
        Number::Rational(rational) => rational.numerator().is_negative(),
        Number::F32(_) | Number::F64(_) => false,
    }
}

fn is_zero(value: &Number) -> bool {
    matches!(value, Number::Integer(integer) if integer.is_zero())
}

fn magnitude(value: &Number) -> Option<Number> {
    if is_negative(value) {
        value.negate_exact().ok()
    } else {
        Some(value.clone())
    }
}

fn compare(left: &Number, right: &Number) -> Option<Ordering> {
    let difference = left.sub_exact(right).ok()?;
    Some(if is_zero(&difference) {
        Ordering::Equal
    } else if is_negative(&difference) {
        Ordering::Less
    } else {
        Ordering::Greater
    })
}

fn power_of_ten(exponent: i64) -> Option<Number> {
    let magnitude = Integer::from(10_i64).pow(u32::try_from(exponent.unsigned_abs()).ok()?);
    if exponent >= 0 {
        Some(Number::Integer(magnitude))
    } else {
        Number::fraction(&Integer::one(), &magnitude).ok()
    }
}

fn fraction_parts(value: &Number) -> Option<(Integer, Integer)> {
    match value {
        Number::Integer(integer) => Some((integer.clone(), Integer::one())),
        Number::Rational(rational) => {
            Some((rational.numerator().clone(), rational.denominator().clone()))
        }
        Number::F32(_) | Number::F64(_) => None,
    }
}

fn significant_digits_asked(lower: &Number, upper: &Number, places: u32) -> Option<u32> {
    let larger = [lower, upper]
        .into_iter()
        .filter_map(magnitude)
        .reduce(|left, right| match compare(&left, &right) {
            Some(Ordering::Less) => right,
            _ => left,
        })?;
    if is_zero(&larger) {
        return Some(places);
    }
    let exponent = decade(&larger)?.saturating_add(1).max(0);
    u32::try_from(exponent).ok()?.checked_add(places)
}

fn decade(value: &Number) -> Option<i64> {
    let (numerator, denominator) = fraction_parts(value)?;
    let magnitude = Number::fraction(&numerator.absolute(), &denominator).ok()?;
    let bit_difference = i64::try_from(numerator.bit_length()).ok()?
        - i64::try_from(denominator.bit_length()).ok()?;
    let mut exponent = bit_difference.saturating_mul(3_010) / 10_000;
    while compare(&magnitude, &power_of_ten(exponent)?)? == Ordering::Less {
        exponent -= 1;
    }
    while compare(&magnitude, &power_of_ten(exponent + 1)?)? != Ordering::Less {
        exponent += 1;
    }
    Some(exponent)
}

fn grid_step(value: &Number, significant_digits: u32) -> Option<Number> {
    power_of_ten(decade(value)? - i64::from(significant_digits) + 1)
}

fn rounded_to_grid(value: &Number, significant_digits: u32, is_upward: bool) -> Option<Number> {
    if is_zero(value) {
        return Some(Number::Integer(Integer::zero()));
    }
    let step = grid_step(value, significant_digits)?;
    let (numerator, denominator) = fraction_parts(&value.div_exact(&step).ok()?)?;
    let (floor, remainder) = numerator.div_rem_euclid(&denominator).ok()?;
    let multiple = if is_upward && !remainder.is_zero() {
        &floor + &Integer::one()
    } else {
        floor
    };
    Number::Integer(multiple).mul_exact(&step).ok()
}

fn is_reached(lower: &Number, upper: &Number, significant_digits: u32) -> Option<bool> {
    if compare(lower, upper)? == Ordering::Equal {
        return Some(true);
    }
    if is_zero(lower) || is_zero(upper) {
        return Some(false);
    }
    let smaller = match compare(&magnitude(lower)?, &magnitude(upper)?)? {
        Ordering::Greater => upper,
        Ordering::Less | Ordering::Equal => lower,
    };
    let step = grid_step(smaller, significant_digits)?;
    Some(compare(&upper.sub_exact(lower).ok()?, &step)? != Ordering::Greater)
}

fn decimal_enclosure(
    lower: &Number,
    upper: &Number,
    significant_digits: u32,
) -> Option<DecimalEnclosure> {
    let lower = rounded_to_grid(lower, significant_digits, false)?;
    let upper = rounded_to_grid(upper, significant_digits, true)?;
    let reached = is_reached(&lower, &upper, significant_digits)?;
    let width = upper.sub_exact(&lower).ok()?;
    Some(DecimalEnclosure {
        significant_digits,
        lower,
        upper,
        width,
        reached,
    })
}

fn checked_digits(significant_digits: u32) -> Result<(), EnclosureError> {
    if significant_digits == 0 || significant_digits > SIGNIFICANT_DIGITS_LIMIT {
        return Err(EnclosureError::DigitsOutOfRange {
            digits: significant_digits,
            limit: SIGNIFICANT_DIGITS_LIMIT,
        });
    }
    Ok(())
}

pub fn enclose_between(
    lower: &Number,
    upper: &Number,
    significant_digits: u32,
) -> Result<DecimalEnclosure, EnclosureError> {
    checked_digits(significant_digits)?;
    let lower = lower.to_exact().map_err(|_| EnclosureError::NotFinite)?;
    let upper = upper.to_exact().map_err(|_| EnclosureError::NotFinite)?;
    decimal_enclosure(&lower, &upper, significant_digits).ok_or(EnclosureError::NotFinite)
}

pub fn enclose_exact(
    value: &Number,
    significant_digits: u32,
) -> Result<DecimalEnclosure, EnclosureError> {
    checked_digits(significant_digits)?;
    let exact = value.to_exact().map_err(|_| EnclosureError::NotFinite)?;
    decimal_enclosure(&exact, &exact, significant_digits).ok_or(EnclosureError::NotFinite)
}

fn bits_for_decades(decades: u64) -> usize {
    usize::try_from(decades)
        .unwrap_or(usize::MAX)
        .saturating_mul(BITS_PER_TEN_DIGITS)
        .saturating_div(10)
}

fn ends_by_magnitude(best: &(Number, Number)) -> Option<(&Number, &Number)> {
    let (lower, upper) = best;
    Some(match compare(&magnitude(lower)?, &magnitude(upper)?)? {
        Ordering::Greater => (upper, lower),
        Ordering::Less | Ordering::Equal => (lower, upper),
    })
}

fn required_precision(significant_digits: u32, best: &Option<(Number, Number)>) -> usize {
    let digit_bits = bits_for_decades(u64::from(significant_digits)).saturating_add(GUARD_BITS);
    let Some(best) = best else {
        return digit_bits;
    };
    let (lower, upper) = best;
    if is_zero(lower) || is_zero(upper) || is_negative(lower) != is_negative(upper) {
        return digit_bits;
    }
    let Some(exponent) = ends_by_magnitude(best).and_then(|(smaller, _)| decade(smaller)) else {
        return digit_bits;
    };
    if exponent < 0 {
        digit_bits.saturating_add(bits_for_decades(exponent.unsigned_abs()))
    } else {
        digit_bits
            .saturating_sub(bits_for_decades(exponent.unsigned_abs()))
            .max(INITIAL_PRECISION_BITS)
    }
}

fn integer_bits(best: &Option<(Number, Number)>) -> usize {
    let Some((_, larger)) = best.as_ref().and_then(ends_by_magnitude) else {
        return 0;
    };
    match larger {
        Number::Integer(integer) => bit_length_of(integer),
        Number::Rational(rational) => bit_length_of(rational.numerator())
            .saturating_sub(bit_length_of(rational.denominator()))
            .saturating_add(1),
        Number::F32(_) | Number::F64(_) => 0,
    }
}

fn tighter(
    best: Option<(Number, Number)>,
    lower: Number,
    upper: Number,
) -> Option<(Number, Number)> {
    let Some((best_lower, best_upper)) = best else {
        return Some((lower, upper));
    };
    let lower = match compare(&lower, &best_lower)? {
        Ordering::Greater => lower,
        Ordering::Less | Ordering::Equal => best_lower,
    };
    let upper = match compare(&upper, &best_upper)? {
        Ordering::Less => upper,
        Ordering::Greater | Ordering::Equal => best_upper,
    };
    Some((lower, upper))
}

fn finished(
    best: Option<(Number, Number)>,
    significant_digits: u32,
    missing: EnclosureError,
) -> Result<DecimalEnclosure, EnclosureError> {
    let (lower, upper) = best.ok_or(missing)?;
    decimal_enclosure(&lower, &upper, significant_digits).ok_or(EnclosureError::NotFinite)
}

pub fn enclose_to_significant_digits<'step>(
    significant_digits: u32,
    budget_bits: usize,
    is_cancelled: &'step dyn Fn() -> bool,
    evaluate: impl Fn(&EnclosureStep<'step>) -> Result<RealBall<'step>, BallError>,
) -> Result<DecimalEnclosure, EnclosureError> {
    checked_digits(significant_digits)?;
    let floor = INITIAL_PRECISION_BITS.min(budget_bits);
    let mut precision = floor;
    let mut best: Option<(Number, Number)> = None;
    let mut seen_integer_bits = 1_usize;
    loop {
        if is_cancelled() {
            return Err(EnclosureError::Cancelled);
        }
        let step = EnclosureStep {
            precision_bits: precision,
            budget_bits,
            is_cancelled,
        };
        match evaluate(&step) {
            Ok(ball) => {
                if let Some((lower, upper)) = ball.bounds() {
                    best = tighter(best, lower, upper);
                }
                if let Some((lower, upper)) = &best {
                    let enclosure = decimal_enclosure(lower, upper, significant_digits)
                        .ok_or(EnclosureError::NotFinite)?;
                    if enclosure.reached {
                        return Ok(enclosure);
                    }
                }
            }
            Err(BallError::Cancelled) => return Err(EnclosureError::Cancelled),
            Err(BallError::OverBudget { integer_bits }) => {
                seen_integer_bits = seen_integer_bits.max(integer_bits);
                let ceiling = budget_bits.saturating_sub(seen_integer_bits);
                if ceiling < floor || precision <= ceiling {
                    return finished(
                        best,
                        significant_digits,
                        EnclosureError::OverBudget { budget_bits },
                    );
                }
                precision = ceiling;
                continue;
            }
            Err(BallError::NoEnclosure) => {}
        }
        let ceiling = budget_bits
            .saturating_sub(integer_bits(&best).max(seen_integer_bits))
            .max(floor);
        if precision >= ceiling {
            return finished(
                best,
                significant_digits,
                EnclosureError::NoEnclosure { budget_bits },
            );
        }
        precision = precision
            .saturating_mul(2)
            .max(required_precision(significant_digits, &best))
            .min(ceiling);
    }
}

fn nearest_on_grid(value: &Number, significant_digits: u32) -> Option<Number> {
    if is_zero(value) {
        return Some(Number::Integer(Integer::zero()));
    }
    let step = grid_step(value, significant_digits)?;
    let (numerator, denominator) = fraction_parts(&value.div_exact(&step).ok()?)?;
    let (floor, remainder) = numerator.div_rem_euclid(&denominator).ok()?;
    let twice_remainder = &remainder * &Integer::from(2_i64);
    let rounds_up = match twice_remainder.cmp(&denominator) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => {
            let (_, parity) = floor.div_rem_euclid(&Integer::from(2_i64)).ok()?;
            !parity.is_zero()
        }
    };
    let multiple = if rounds_up {
        &floor + &Integer::one()
    } else {
        floor
    };
    Number::Integer(multiple).mul_exact(&step).ok()
}

pub fn round_to_significant_digits(
    value: &Number,
    significant_digits: u32,
) -> Result<Number, EnclosureError> {
    checked_digits(significant_digits)?;
    let exact = value.to_exact().map_err(|_| EnclosureError::NotFinite)?;
    nearest_on_grid(&exact, significant_digits).ok_or(EnclosureError::NotFinite)
}

pub fn truncated_at_places<'step>(
    places: u32,
    significant_digit_limit: u32,
    budget_bits: usize,
    is_cancelled: &'step dyn Fn() -> bool,
    evaluate: impl Fn(&EnclosureStep<'step>) -> Result<RealBall<'step>, BallError>,
) -> Result<Number, EnclosureError> {
    let floor = INITIAL_PRECISION_BITS.min(budget_bits);
    let mut precision = floor;
    let mut seen_integer_bits = 1_usize;
    loop {
        if is_cancelled() {
            return Err(EnclosureError::Cancelled);
        }
        let step = EnclosureStep {
            precision_bits: precision,
            budget_bits,
            is_cancelled,
        };
        match evaluate(&step) {
            Ok(ball) => {
                if let Some((lower, upper)) = ball.bounds() {
                    let asked = significant_digits_asked(&lower, &upper, places);
                    if let Some(asked) = asked
                        && asked > significant_digit_limit
                    {
                        return Err(EnclosureError::DigitsOutOfRange {
                            digits: asked,
                            limit: significant_digit_limit,
                        });
                    }
                    let lower = crate::decimal::truncated_to_places(&lower, places).ok();
                    let upper = crate::decimal::truncated_to_places(&upper, places).ok();
                    if let (Some(lower), Some(upper)) = (lower, upper)
                        && compare(&lower, &upper) == Some(Ordering::Equal)
                    {
                        return Ok(lower);
                    }
                }
            }
            Err(BallError::Cancelled) => return Err(EnclosureError::Cancelled),
            Err(BallError::OverBudget { integer_bits }) => {
                seen_integer_bits = seen_integer_bits.max(integer_bits);
                let ceiling = budget_bits.saturating_sub(seen_integer_bits);
                if ceiling < floor || precision <= ceiling {
                    return Err(EnclosureError::OverBudget { budget_bits });
                }
                precision = ceiling;
                continue;
            }
            Err(BallError::NoEnclosure) => {}
        }
        let ceiling = budget_bits.saturating_sub(seen_integer_bits).max(floor);
        if precision >= ceiling {
            return Err(EnclosureError::NoEnclosure { budget_bits });
        }
        precision = precision.saturating_mul(2).min(ceiling);
    }
}

pub fn correctly_rounded_to_significant_digits<'step>(
    significant_digits: u32,
    budget_bits: usize,
    is_cancelled: &'step dyn Fn() -> bool,
    evaluate: impl Fn(&EnclosureStep<'step>) -> Result<RealBall<'step>, BallError>,
) -> Result<Number, EnclosureError> {
    checked_digits(significant_digits)?;
    let floor = INITIAL_PRECISION_BITS.min(budget_bits);
    let mut precision = floor;
    let mut seen_integer_bits = 1_usize;
    loop {
        if is_cancelled() {
            return Err(EnclosureError::Cancelled);
        }
        let step = EnclosureStep {
            precision_bits: precision,
            budget_bits,
            is_cancelled,
        };
        match evaluate(&step) {
            Ok(ball) => {
                if let Some((lower, upper)) = ball.bounds() {
                    let lower = nearest_on_grid(&lower, significant_digits);
                    let upper = nearest_on_grid(&upper, significant_digits);
                    if let (Some(lower), Some(upper)) = (lower, upper)
                        && compare(&lower, &upper) == Some(Ordering::Equal)
                    {
                        return Ok(lower);
                    }
                }
            }
            Err(BallError::Cancelled) => return Err(EnclosureError::Cancelled),
            Err(BallError::OverBudget { integer_bits }) => {
                seen_integer_bits = seen_integer_bits.max(integer_bits);
                let ceiling = budget_bits.saturating_sub(seen_integer_bits);
                if ceiling < floor || precision <= ceiling {
                    return Err(EnclosureError::OverBudget { budget_bits });
                }
                precision = ceiling;
                continue;
            }
            Err(BallError::NoEnclosure) => {}
        }
        let ceiling = budget_bits.saturating_sub(seen_integer_bits).max(floor);
        if precision >= ceiling {
            return Err(EnclosureError::NoEnclosure { budget_bits });
        }
        precision = precision
            .saturating_mul(2)
            .max(required_precision(significant_digits, &None))
            .min(ceiling);
    }
}

pub fn smallest_f64_at_least(value: &Number) -> Option<f64> {
    let exact = value.to_exact().ok()?;
    let nearest = exact.round_to_f64_ties_even();
    if !nearest.is_finite() {
        return Some(nearest);
    }
    let nearest_exact = Number::F64(nearest).to_exact().ok()?;
    Some(match compare(&nearest_exact, &exact)? {
        Ordering::Less => nearest.next_up(),
        Ordering::Equal | Ordering::Greater => nearest,
    })
}

fn half_away_on_grid(value: &Number, significant_digits: u32) -> Option<Number> {
    if is_zero(value) {
        return Some(Number::Integer(Integer::zero()));
    }
    let size = magnitude(value)?;
    let step = grid_step(&size, significant_digits)?;
    let (numerator, denominator) = fraction_parts(&size.div_exact(&step).ok()?)?;
    let (floor, remainder) = numerator.div_rem_euclid(&denominator).ok()?;
    let twice_remainder = &remainder * &Integer::from(2_i64);
    let multiple = if twice_remainder >= denominator {
        &floor + &Integer::one()
    } else {
        floor
    };
    let rounded = Number::Integer(multiple).mul_exact(&step).ok()?;
    if is_negative(value) {
        rounded.negate_exact().ok()
    } else {
        Some(rounded)
    }
}

pub fn round_half_away_to_significant_digits(
    value: &Number,
    significant_digits: u32,
) -> Result<Number, EnclosureError> {
    checked_digits(significant_digits)?;
    let exact = value.to_exact().map_err(|_| EnclosureError::NotFinite)?;
    half_away_on_grid(&exact, significant_digits).ok_or(EnclosureError::NotFinite)
}

pub fn at_least_one<'step>(
    budget_bits: usize,
    is_cancelled: &'step dyn Fn() -> bool,
    evaluate: impl Fn(&EnclosureStep<'step>) -> Result<RealBall<'step>, BallError>,
) -> Result<Option<bool>, EnclosureError> {
    let one = Number::Integer(Integer::one());
    let floor = INITIAL_PRECISION_BITS.min(budget_bits);
    let mut precision = floor;
    let mut seen_integer_bits = 1_usize;
    loop {
        if is_cancelled() {
            return Err(EnclosureError::Cancelled);
        }
        let step = EnclosureStep {
            precision_bits: precision,
            budget_bits,
            is_cancelled,
        };
        match evaluate(&step) {
            Ok(ball) => {
                if let Some((lower, upper)) = ball.bounds() {
                    if compare(&lower, &one) != Some(Ordering::Less) {
                        return Ok(Some(true));
                    }
                    if compare(&upper, &one) == Some(Ordering::Less) {
                        return Ok(Some(false));
                    }
                }
            }
            Err(BallError::Cancelled) => return Err(EnclosureError::Cancelled),
            Err(BallError::OverBudget { integer_bits }) => {
                seen_integer_bits = seen_integer_bits.max(integer_bits);
                let ceiling = budget_bits.saturating_sub(seen_integer_bits);
                if ceiling < floor || precision <= ceiling {
                    return Ok(None);
                }
                precision = ceiling;
                continue;
            }
            Err(BallError::NoEnclosure) => {}
        }
        let ceiling = budget_bits.saturating_sub(seen_integer_bits).max(floor);
        if precision >= ceiling {
            return Ok(None);
        }
        precision = precision.saturating_mul(2).min(ceiling);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{decimal_reference, is_at_most};

    fn never_cancelled() -> bool {
        false
    }

    fn fraction(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    fn decimal(text: &str) -> Number {
        decimal_reference(text).value
    }

    fn enclosed(
        digits: u32,
        evaluate: impl Fn(&EnclosureStep<'static>) -> Result<RealBall<'static>, BallError>,
    ) -> DecimalEnclosure {
        enclose_to_significant_digits(digits, ENCLOSURE_BUDGET_BITS, &never_cancelled, evaluate)
            .unwrap()
    }

    fn assert_contains(enclosure: &DecimalEnclosure, reference: &str) {
        let value = decimal(reference);
        assert!(is_at_most(&enclosure.lower, &value) && is_at_most(&value, &enclosure.upper));
    }

    fn ball_of<'step>(
        step: &EnclosureStep<'step>,
        numerator: i64,
        denominator: i64,
    ) -> Result<RealBall<'step>, BallError> {
        step.exact(&fraction(numerator, denominator))
    }

    #[test]
    fn square_root_of_two_to_ten_digits_is_reached_one_step_wide() {
        let enclosure = enclosed(10, |step| ball_of(step, 2, 1)?.sqrt());

        assert_eq!(
            (enclosure.lower, enclosure.upper, enclosure.reached),
            (decimal("1.414213562"), decimal("1.414213563"), true)
        );
    }

    #[test]
    fn pi_to_forty_digits_contains_the_reference() {
        let enclosure = enclosed(40, |step| step.pi());

        assert!(enclosure.reached);
        assert_contains(
            &enclosure,
            "3.14159265358979323846264338327950288419716939937510",
        );
    }

    #[test]
    fn e_to_thirty_digits_contains_the_reference() {
        let enclosure = enclosed(30, |step| step.e());

        assert!(enclosure.reached);
        assert_contains(&enclosure, "2.71828182845904523536028747135266249775724709");
    }

    #[test]
    fn logarithm_of_two_to_thirty_digits_contains_the_reference() {
        let enclosure = enclosed(30, |step| ball_of(step, 2, 1)?.ln());

        assert!(enclosure.reached);
        assert_contains(
            &enclosure,
            "0.693147180559945309417232121458176568075500134",
        );
    }

    #[test]
    fn logarithm_of_a_small_value_is_negative_and_contains_the_reference() {
        let enclosure = enclosed(20, |step| ball_of(step, 1, 1000)?.ln());

        assert!(enclosure.reached);
        assert_contains(
            &enclosure,
            "-6.90775527898213705205397436405309262280330446",
        );
    }

    #[test]
    fn sine_of_one_to_thirty_digits_contains_the_reference() {
        let enclosure = enclosed(30, |step| ball_of(step, 1, 1)?.sin());

        assert!(enclosure.reached);
        assert_contains(
            &enclosure,
            "0.841470984807896506652502321630298999622563060",
        );
    }

    #[test]
    fn cosine_of_ten_to_twenty_digits_contains_the_reference() {
        let enclosure = enclosed(20, |step| ball_of(step, 10, 1)?.cos());

        assert!(enclosure.reached);
        assert_contains(
            &enclosure,
            "-0.839071529076452452258863947824064834519930165",
        );
    }

    #[test]
    fn tangent_of_one_half_to_twenty_digits_contains_the_reference() {
        let enclosure = enclosed(20, |step| ball_of(step, 1, 2)?.tan());

        assert!(enclosure.reached);
        assert_contains(
            &enclosure,
            "0.546302489843790513255179465780285383297551720",
        );
    }

    #[test]
    fn an_arctangent_of_two_arguments_encloses_each_quadrant() {
        let first = enclosed(12, |step| ball_of(step, 1, 2)?.atan2(&ball_of(step, 1, 1)?));
        let second = enclosed(12, |step| {
            ball_of(step, 1, 1)?.atan2(&ball_of(step, -1, 1)?)
        });
        let third = enclosed(12, |step| {
            ball_of(step, -1, 1)?.atan2(&ball_of(step, -1, 1)?)
        });
        let fourth = enclosed(12, |step| {
            ball_of(step, -1, 2)?.atan2(&ball_of(step, 1, 1)?)
        });

        assert_contains(&first, "0.46364760900080611621425623146121440202853705");
        assert_contains(&second, "2.35619449019234492884698253745962716314787704");
        assert_contains(&third, "-2.35619449019234492884698253745962716314787704");
        assert_contains(&fourth, "-0.46364760900080611621425623146121440202853705");
    }

    #[test]
    fn an_arctangent_of_two_arguments_encloses_the_quarter_turns() {
        let up = enclosed(12, |step| ball_of(step, 1, 1)?.atan2(&ball_of(step, 0, 1)?));
        let down = enclosed(12, |step| {
            ball_of(step, -1, 1)?.atan2(&ball_of(step, 0, 1)?)
        });

        assert_contains(&up, "1.57079632679489661923132169163975144209858469");
        assert_contains(&down, "-1.57079632679489661923132169163975144209858469");
    }

    #[test]
    fn an_arctangent_of_two_arguments_refuses_the_cut() {
        let across =
            enclose_to_significant_digits(12, ENCLOSURE_BUDGET_BITS, &never_cancelled, |step| {
                let zero = ball_of(step, 0, 1)?;
                zero.atan2(&ball_of(step, -1, 1)?)
            });

        assert!(across.is_err(), "{across:?}");
    }

    #[test]
    fn four_arctangents_of_one_enclose_pi() {
        let enclosure = enclosed(25, |step| {
            ball_of(step, 1, 1)?.atan()?.mul(&ball_of(step, 4, 1)?)
        });

        assert!(enclosure.reached);
        assert_contains(
            &enclosure,
            "3.14159265358979323846264338327950288419716939937510",
        );
    }

    #[test]
    fn negative_power_of_three_encloses_one_ninth() {
        let enclosure = enclosed(6, |step| ball_of(step, 3, 1)?.power(-2));

        assert_eq!(
            (enclosure.lower, enclosure.upper),
            (decimal("0.111111"), decimal("0.111112"))
        );
    }

    #[test]
    fn large_value_takes_its_integer_bits_from_the_budget() {
        let enclosure = enclosed(30, |step| ball_of(step, 1000, 1)?.mul(&step.pi()?));

        assert!(enclosure.reached);
        assert_contains(
            &enclosure,
            "3141.59265358979323846264338327950288419716939937510",
        );
    }

    #[test]
    fn square_root_of_a_surely_negative_ball_has_no_enclosure() {
        let result = enclose_to_significant_digits(5, 256, &never_cancelled, |step| {
            ball_of(step, -1, 1)?.sqrt()
        });

        assert_eq!(
            result,
            Err(EnclosureError::NoEnclosure { budget_bits: 256 })
        );
    }

    #[test]
    fn value_that_never_separates_from_the_grid_stops_at_the_budget_unreached() {
        let enclosure = enclose_to_significant_digits(5, 512, &never_cancelled, |step| {
            let root = ball_of(step, 2, 1)?.sqrt()?;
            root.mul(&root)
        })
        .unwrap();

        assert!(!enclosure.reached);
        assert!(is_at_most(&enclosure.lower, &fraction(2, 1)));
        assert!(is_at_most(&fraction(2, 1), &enclosure.upper));
    }

    #[test]
    fn value_too_far_below_one_keeps_a_proven_interval_unreached() {
        let enclosure = enclose_to_significant_digits(20, 1024, &never_cancelled, |step| {
            ball_of(step, -2000, 1)?.exp()
        })
        .unwrap();

        assert!(!enclosure.reached);
        assert!(is_at_most(&Number::from(0_i64), &enclosure.upper));
    }

    #[test]
    fn exponential_of_a_large_value_is_over_budget() {
        let result = enclose_to_significant_digits(5, 1024, &never_cancelled, |step| {
            ball_of(step, 100_000, 1)?.exp()
        });

        assert_eq!(
            result,
            Err(EnclosureError::OverBudget { budget_bits: 1024 })
        );
    }

    #[test]
    fn exact_value_larger_than_the_budget_is_never_built() {
        let result = enclose_to_significant_digits(5, 1024, &never_cancelled, |step| {
            step.exact(&Number::Integer(Integer::from(10_i64).pow(1000)))
        });

        assert_eq!(
            result,
            Err(EnclosureError::OverBudget { budget_bits: 1024 })
        );
    }

    #[test]
    fn refinement_stops_when_cancelled_between_precision_steps() {
        let steps = std::cell::Cell::new(0_u32);
        let is_cancelled = || steps.get() >= 2;

        let result = enclose_to_significant_digits(5, 4096, &is_cancelled, |step| {
            steps.set(steps.get() + 1);
            let root = ball_of(step, 2, 1)?.sqrt()?;
            root.mul(&root)
        });

        assert_eq!((result, steps.get()), (Err(EnclosureError::Cancelled), 2));
    }

    #[test]
    fn series_stops_between_terms_when_cancelled_within_a_step() {
        let checks = std::cell::Cell::new(0_u32);
        let steps = std::cell::Cell::new(0_u32);
        let is_cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() > 3
        };

        let result = enclose_to_significant_digits(40, 4096, &is_cancelled, |step| {
            steps.set(steps.get() + 1);
            step.pi()
        });

        assert_eq!((result, steps.get()), (Err(EnclosureError::Cancelled), 1));
    }

    #[test]
    fn required_precision_grows_with_the_decades_below_one() {
        let tiny = Number::fraction(&Integer::one(), &Integer::from(10_i64).pow(300)).unwrap();
        let best = Some((tiny.clone(), tiny.mul_exact(&fraction(2, 1)).unwrap()));

        assert_eq!(required_precision(1, &best), 3 + 1020 + 32);
    }

    #[test]
    fn required_precision_shrinks_with_the_decades_above_one() {
        let best = Some((fraction(1000, 1), fraction(1001, 1)));

        assert_eq!(required_precision(100, &best), 340 + 32 - 10);
    }

    #[test]
    fn exact_one_third_rounds_outward_one_step_wide() {
        let enclosure = enclose_exact(&fraction(1, 3), 5).unwrap();

        assert_eq!(
            (enclosure.lower, enclosure.upper, enclosure.reached),
            (decimal("0.33333"), decimal("0.33334"), true)
        );
    }

    #[test]
    fn exact_value_on_the_grid_has_equal_ends() {
        let enclosure = enclose_exact(&fraction(1, 2), 3).unwrap();

        assert_eq!(
            (enclosure.lower, enclosure.upper, enclosure.reached),
            (fraction(1, 2), fraction(1, 2), true)
        );
    }

    #[test]
    fn exact_zero_is_reached_with_zero_ends() {
        let enclosure = enclose_exact(&Number::from(0_i64), 3).unwrap();

        assert_eq!(
            (enclosure.lower, enclosure.upper, enclosure.reached),
            (Number::from(0_i64), Number::from(0_i64), true)
        );
    }

    #[test]
    fn negative_exact_value_rounds_away_below_and_toward_zero_above() {
        let enclosure = enclose_exact(&fraction(-2, 3), 3).unwrap();

        assert_eq!(
            (enclosure.lower, enclosure.upper),
            (decimal("-0.667"), decimal("-0.666"))
        );
    }

    #[test]
    fn grid_step_follows_the_smaller_endpoint_across_a_decade() {
        let enclosure = enclose_exact(&fraction(9_999_951, 10_000_000), 5).unwrap();

        assert_eq!(
            (enclosure.lower, enclosure.upper, enclosure.reached),
            (decimal("0.99999"), Number::from(1_i64), true)
        );
    }

    #[test]
    fn zero_significant_digits_are_out_of_range() {
        assert_eq!(
            enclose_exact(&fraction(1, 3), 0),
            Err(EnclosureError::DigitsOutOfRange {
                digits: 0,
                limit: 5_000
            })
        );
    }

    #[test]
    fn significant_digits_above_the_limit_are_out_of_range() {
        assert_eq!(
            enclose_exact(&fraction(1, 3), 5_001),
            Err(EnclosureError::DigitsOutOfRange {
                digits: 5_001,
                limit: 5_000
            })
        );
    }

    #[test]
    fn machine_value_is_enclosed_as_its_exact_binary_number() {
        let enclosure = enclose_exact(&Number::F64(0.1), 20).unwrap();

        assert_eq!(
            (enclosure.lower, enclosure.upper),
            (
                decimal("0.10000000000000000555"),
                decimal("0.10000000000000000556")
            )
        );
    }

    #[test]
    fn rounding_to_significant_digits_takes_a_tie_to_even() {
        assert_eq!(
            round_to_significant_digits(&fraction(125, 1000), 2),
            Ok(decimal("0.12"))
        );
    }

    #[test]
    fn rounding_to_significant_digits_rounds_above_a_tie_up() {
        assert_eq!(
            round_to_significant_digits(&fraction(1251, 10000), 2),
            Ok(decimal("0.13"))
        );
    }

    #[test]
    fn correct_rounding_of_pi_agrees_on_both_ends() {
        let rounded = correctly_rounded_to_significant_digits(
            20,
            ENCLOSURE_BUDGET_BITS,
            &never_cancelled,
            |step| step.pi(),
        );

        assert_eq!(rounded, Ok(decimal("3.1415926535897932385")));
    }

    #[test]
    fn truncating_a_third_of_pi_gives_its_places() {
        let truncated = truncated_at_places(
            10,
            SIGNIFICANT_DIGITS_LIMIT,
            ENCLOSURE_BUDGET_BITS,
            &never_cancelled,
            |step| step.pi()?.div(&step.exact(&Number::from(3_i64))?),
        );

        assert_eq!(truncated, Ok(decimal("1.0471975511")));
    }

    #[test]
    fn a_value_sitting_on_the_grid_is_never_decided() {
        let truncated =
            truncated_at_places(0, SIGNIFICANT_DIGITS_LIMIT, 256, &never_cancelled, |step| {
                let two = step.exact(&Number::from(2_i64))?;
                let three = step.exact(&Number::from(3_i64))?;
                two.div(&three)?.mul(&three)
            });

        assert!(
            matches!(
                truncated,
                Err(EnclosureError::NoEnclosure { .. } | EnclosureError::OverBudget { .. })
            ),
            "{truncated:?}"
        );
    }

    #[test]
    fn places_beyond_the_significant_digits_asked_for_are_refused() {
        let truncated =
            truncated_at_places(8, 4, ENCLOSURE_BUDGET_BITS, &never_cancelled, |step| {
                step.pi()
            });

        assert_eq!(
            truncated,
            Err(EnclosureError::DigitsOutOfRange {
                digits: 9,
                limit: 4
            })
        );
    }

    #[test]
    fn smallest_double_at_least_a_value_between_doubles_is_above_it() {
        let tenth = fraction(1, 10);

        let above = smallest_f64_at_least(&tenth).unwrap();

        assert!(is_at_most(&tenth, &Number::F64(above).to_exact().unwrap()));
        assert!(is_at_most(
            &Number::F64(above.next_down()).to_exact().unwrap(),
            &tenth
        ));
    }

    #[test]
    fn smallest_double_at_least_a_double_is_that_double() {
        assert_eq!(smallest_f64_at_least(&fraction(1, 2)), Some(0.5));
    }

    #[test]
    fn school_rounding_takes_a_tie_away_from_zero() {
        assert_eq!(
            round_half_away_to_significant_digits(&fraction(125, 1000), 2),
            Ok(decimal("0.13"))
        );
    }

    #[test]
    fn school_rounding_takes_a_negative_tie_away_from_zero() {
        assert_eq!(
            round_half_away_to_significant_digits(&fraction(-125, 1000), 2),
            Ok(decimal("-0.13"))
        );
    }

    #[test]
    fn school_rounding_of_1250_ninths_is_138_9() {
        assert_eq!(
            round_half_away_to_significant_digits(&fraction(1250, 9), 4),
            Ok(decimal("138.9"))
        );
    }

    #[test]
    fn pi_is_at_least_one() {
        assert_eq!(
            at_least_one(ENCLOSURE_BUDGET_BITS, &never_cancelled, |step| step.pi()),
            Ok(Some(true))
        );
    }

    #[test]
    fn reciprocal_of_pi_is_below_one() {
        assert_eq!(
            at_least_one(ENCLOSURE_BUDGET_BITS, &never_cancelled, |step| {
                step.exact(&fraction(1, 1))?.div(&step.pi()?)
            }),
            Ok(Some(false))
        );
    }

    #[test]
    fn one_that_never_separates_is_undecided() {
        assert_eq!(
            at_least_one(512, &never_cancelled, |step| {
                let root = ball_of(step, 1, 1)?.sqrt()?;
                root.mul(&root)
            }),
            Ok(None)
        );
    }
}
