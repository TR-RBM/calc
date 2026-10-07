use std::f64::consts::TAU;

use calc_numbers::atan2_f64;

use crate::record::Interval;

const SECTORS: f64 = 6.0;
const CATEGORY_COUNT: f64 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colour {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
}

const fn colour(red: f64, green: f64, blue: f64) -> Colour {
    Colour { red, green, blue }
}

const SEQUENTIAL_STOPS: [Colour; 5] = [
    colour(0.267004, 0.004874, 0.329415),
    colour(0.229739, 0.322361, 0.545706),
    colour(0.127568, 0.566949, 0.550556),
    colour(0.369214, 0.788888, 0.382914),
    colour(0.993248, 0.906157, 0.143936),
];

const DIVERGING_STOPS: [Colour; 3] = [
    colour(0.230, 0.299, 0.754),
    colour(0.865, 0.865, 0.865),
    colour(0.706, 0.016, 0.150),
];

const CATEGORICAL_COLOURS: [Colour; 8] = [
    colour(0.902, 0.624, 0.0),
    colour(0.337, 0.706, 0.914),
    colour(0.0, 0.620, 0.451),
    colour(0.941, 0.894, 0.259),
    colour(0.0, 0.447, 0.698),
    colour(0.835, 0.369, 0.0),
    colour(0.800, 0.475, 0.655),
    colour(0.600, 0.600, 0.600),
];

fn range_position(range: &Interval, value: f64) -> Option<f64> {
    let lower = range.lower.round_to_f64_ties_even();
    let upper = range.upper.round_to_f64_ties_even();
    let width = upper - lower;
    if value.is_nan() || width <= 0.0 {
        return None;
    }
    let position = (value - lower) / width;
    if position.is_nan() {
        None
    } else if position < 0.0 {
        Some(0.0)
    } else if position > 1.0 {
        Some(1.0)
    } else {
        Some(position)
    }
}

fn blend(start: f64, end: f64, fraction: f64) -> f64 {
    start * (1.0 - fraction) + end * fraction
}

fn interpolate(stops: &[Colour], position: f64) -> Option<Colour> {
    let mut segments = stops.windows(2).peekable();
    let segment_count = f64::from(u32::try_from(stops.len().saturating_sub(1)).ok()?);
    let scaled = position * segment_count;
    let mut segment_start = 0.0;
    while let Some(pair) = segments.next() {
        let is_last = segments.peek().is_none();
        if scaled <= segment_start + 1.0 || is_last {
            let [start, end] = pair else {
                return None;
            };
            let fraction = scaled - segment_start;
            return Some(Colour {
                red: blend(start.red, end.red, fraction),
                green: blend(start.green, end.green, fraction),
                blue: blend(start.blue, end.blue, fraction),
            });
        }
        segment_start += 1.0;
    }
    None
}

pub fn sequential_colour(range: &Interval, value: f64) -> Option<Colour> {
    interpolate(&SEQUENTIAL_STOPS, range_position(range, value)?)
}

pub fn diverging_colour(range: &Interval, value: f64) -> Option<Colour> {
    interpolate(&DIVERGING_STOPS, range_position(range, value)?)
}

pub fn categorical_colour(value: f64) -> Option<Colour> {
    if !value.is_finite() {
        return None;
    }
    let remainder = value.floor() % CATEGORY_COUNT;
    let category = if remainder < 0.0 {
        remainder + CATEGORY_COUNT
    } else {
        remainder
    };
    let mut index = 0.0;
    for candidate in CATEGORICAL_COLOURS {
        if category == index {
            return Some(candidate);
        }
        index += 1.0;
    }
    None
}

pub fn domain_colour(range: &Interval, real: f64, imaginary: f64) -> Option<Colour> {
    let reference_modulus =
        range.upper.round_to_f64_ties_even() - range.lower.round_to_f64_ties_even();
    if real.is_nan() || imaginary.is_nan() || reference_modulus <= 0.0 {
        return None;
    }
    let modulus = (real * real + imaginary * imaginary).sqrt();
    let lightness = if modulus.is_infinite() {
        1.0
    } else {
        modulus / (modulus + reference_modulus)
    };
    let turns = atan2_f64(imaginary, real) / TAU;
    let hue = if turns < 0.0 { turns + 1.0 } else { turns };
    Some(hue_lightness_colour(hue, lightness))
}

fn hue_lightness_colour(hue: f64, lightness: f64) -> Colour {
    let chroma = 1.0 - (2.0 * lightness - 1.0).abs();
    let sector = hue * SECTORS;
    let secondary = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let base = lightness - chroma / 2.0;
    let (red, green, blue) = if sector < 1.0 {
        (chroma, secondary, 0.0)
    } else if sector < 2.0 {
        (secondary, chroma, 0.0)
    } else if sector < 3.0 {
        (0.0, chroma, secondary)
    } else if sector < 4.0 {
        (0.0, secondary, chroma)
    } else if sector < 5.0 {
        (secondary, 0.0, chroma)
    } else {
        (chroma, 0.0, secondary)
    };
    Colour {
        red: red + base,
        green: green + base,
        blue: blue + base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_numbers::Number;

    fn range(lower: i64, upper: i64) -> Interval {
        Interval {
            lower: Number::from(lower),
            upper: Number::from(upper),
        }
    }

    #[test]
    fn sequential_lower_bound_is_the_first_stop() {
        let result = sequential_colour(&range(-2, 2), -2.0);

        assert_eq!(result, Some(SEQUENTIAL_STOPS[0]));
    }

    #[test]
    fn sequential_value_between_stops_is_blended() {
        let result = sequential_colour(&range(0, 8), 1.0);

        assert_eq!(
            result.map(|colour| colour.red),
            Some(blend(SEQUENTIAL_STOPS[0].red, SEQUENTIAL_STOPS[1].red, 0.5))
        );
    }

    #[test]
    fn sequential_value_above_range_is_the_last_stop() {
        let result = sequential_colour(&range(0, 1), f64::INFINITY);

        assert_eq!(result, Some(SEQUENTIAL_STOPS[4]));
    }

    #[test]
    fn sequential_nan_is_missing() {
        let result = sequential_colour(&range(0, 1), f64::NAN);

        assert_eq!(result, None);
    }

    #[test]
    fn diverging_midpoint_is_the_neutral_stop() {
        let result = diverging_colour(&range(-3, 1), -1.0);

        assert_eq!(result, Some(DIVERGING_STOPS[1]));
    }

    #[test]
    fn categorical_negative_category_wraps_around() {
        let result = categorical_colour(-1.5);

        assert_eq!(result, Some(CATEGORICAL_COLOURS[6]));
    }

    #[test]
    fn categorical_infinity_is_missing() {
        let result = categorical_colour(f64::NEG_INFINITY);

        assert_eq!(result, None);
    }

    #[test]
    fn domain_positive_real_at_reference_modulus_is_pure_red() {
        let result = domain_colour(&range(0, 2), 2.0, 0.0);

        assert_eq!(result, Some(colour(1.0, 0.0, 0.0)));
    }

    #[test]
    fn domain_positive_imaginary_at_reference_modulus_is_between_yellow_and_green() {
        let result = domain_colour(&range(0, 1), 0.0, 1.0);

        assert_eq!(result, Some(colour(0.5, 1.0, 0.0)));
    }

    #[test]
    fn domain_zero_is_black() {
        let result = domain_colour(&range(0, 1), 0.0, 0.0);

        assert_eq!(result, Some(colour(0.0, 0.0, 0.0)));
    }

    #[test]
    fn domain_infinite_modulus_is_white() {
        let result = domain_colour(&range(0, 1), f64::INFINITY, 0.0);

        assert_eq!(result, Some(colour(1.0, 1.0, 1.0)));
    }

    #[test]
    fn domain_nan_part_is_missing() {
        let result = domain_colour(&range(0, 1), 1.0, f64::NAN);

        assert_eq!(result, None);
    }
}
