use std::num::NonZeroU64;

use calc_numbers::{ExactArithmeticError, Integer, Number};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemperatureScaleError {
    MachineReading,
}

const ONE: NonZeroU64 = NonZeroU64::MIN;
const FIVE: NonZeroU64 = NonZeroU64::new(5).unwrap();
const NINE: NonZeroU64 = NonZeroU64::new(9).unwrap();
const TWENTY: NonZeroU64 = NonZeroU64::new(20).unwrap();
const ONE_HUNDRED: NonZeroU64 = NonZeroU64::new(100).unwrap();

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NonNegativeRatio {
    pub numerator: u64,
    pub denominator: NonZeroU64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PositiveRatio {
    pub numerator: NonZeroU64,
    pub denominator: NonZeroU64,
}

impl NonNegativeRatio {
    pub fn to_number(self) -> Number {
        fraction(&Integer::from(self.numerator), self.denominator)
    }
}

impl PositiveRatio {
    pub fn to_number(self) -> Number {
        fraction(&Integer::from(self.numerator.get()), self.denominator)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TemperatureScale {
    Kelvin,
    Celsius,
    Fahrenheit,
}

impl TemperatureScale {
    pub const ALL: [Self; 3] = [Self::Kelvin, Self::Celsius, Self::Fahrenheit];

    pub fn name(self) -> &'static str {
        match self {
            Self::Kelvin => "kelvin",
            Self::Celsius => "celsius",
            Self::Fahrenheit => "fahrenheit",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|scale| scale.name() == name)
    }

    pub fn display_symbol(self) -> &'static str {
        match self {
            Self::Kelvin => "K",
            Self::Celsius => "\u{00B0}C",
            Self::Fahrenheit => "\u{00B0}F",
        }
    }

    pub fn offset_ratio(self) -> NonNegativeRatio {
        let (numerator, denominator) = match self {
            Self::Kelvin => (0, ONE),
            Self::Celsius => (5463, TWENTY),
            Self::Fahrenheit => (45967, ONE_HUNDRED),
        };
        NonNegativeRatio {
            numerator,
            denominator,
        }
    }

    pub fn factor_ratio(self) -> PositiveRatio {
        let (numerator, denominator) = match self {
            Self::Kelvin | Self::Celsius => (ONE, ONE),
            Self::Fahrenheit => (FIVE, NINE),
        };
        PositiveRatio {
            numerator,
            denominator,
        }
    }

    pub fn offset(self) -> Number {
        self.offset_ratio().to_number()
    }

    pub fn factor(self) -> Number {
        self.factor_ratio().to_number()
    }

    pub fn to_kelvin(self, reading: &Number) -> Result<Number, TemperatureScaleError> {
        reading
            .add_exact(&self.offset())
            .and_then(|shifted| shifted.mul_exact(&self.factor()))
            .map_err(machine_reading)
    }

    pub fn from_kelvin(self, kelvin: &Number) -> Result<Number, TemperatureScaleError> {
        kelvin
            .div_exact(&self.factor())
            .and_then(|scaled| scaled.sub_exact(&self.offset()))
            .map_err(machine_reading)
    }
}

fn fraction(numerator: &Integer, denominator: NonZeroU64) -> Number {
    match Number::fraction(numerator, &Integer::from(denominator.get())) {
        Ok(number) => number,
        Err(ExactArithmeticError::DivisionByZero | ExactArithmeticError::MachineOperand) => {
            unreachable!()
        }
    }
}

fn machine_reading(_: ExactArithmeticError) -> TemperatureScaleError {
    TemperatureScaleError::MachineReading
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    #[test]
    fn kelvin_reading_is_the_thermodynamic_temperature() {
        let kelvin = TemperatureScale::Kelvin.to_kelvin(&number(29315, 100));

        assert_eq!(kelvin, Ok(number(29315, 100)));
    }

    #[test]
    fn celsius_reading_zero_is_two_hundred_seventy_three_point_one_five_kelvin() {
        let kelvin = TemperatureScale::Celsius.to_kelvin(&number(0, 1));

        assert_eq!(kelvin, Ok(number(27315, 100)));
    }

    #[test]
    fn fahrenheit_reading_thirty_two_is_the_celsius_zero() {
        let fahrenheit = TemperatureScale::Fahrenheit.to_kelvin(&number(32, 1));

        assert_eq!(
            fahrenheit,
            TemperatureScale::Celsius.to_kelvin(&number(0, 1))
        );
    }

    #[test]
    fn fahrenheit_reading_two_hundred_twelve_is_celsius_one_hundred() {
        let fahrenheit = TemperatureScale::Fahrenheit.to_kelvin(&number(212, 1));

        assert_eq!(
            fahrenheit,
            TemperatureScale::Celsius.to_kelvin(&number(100, 1))
        );
    }

    #[test]
    fn minus_forty_is_the_same_reading_in_celsius_and_fahrenheit() {
        let celsius = TemperatureScale::Celsius
            .to_kelvin(&number(-40, 1))
            .unwrap();

        let fahrenheit = TemperatureScale::Fahrenheit.from_kelvin(&celsius);

        assert_eq!(fahrenheit, Ok(number(-40, 1)));
    }

    #[test]
    fn absolute_zero_is_minus_four_hundred_fifty_nine_point_six_seven_fahrenheit() {
        let reading = TemperatureScale::Fahrenheit.from_kelvin(&number(0, 1));

        assert_eq!(reading, Ok(number(-45967, 100)));
    }

    #[test]
    fn from_kelvin_inverts_to_kelvin_on_every_scale() {
        let reading = number(1013, 7);

        let round_trips: Vec<_> = TemperatureScale::ALL
            .into_iter()
            .map(|scale| scale.from_kelvin(&scale.to_kelvin(&reading).unwrap()))
            .collect();

        assert_eq!(round_trips, vec![Ok(reading.clone()); 3]);
    }

    #[test]
    fn factor_is_the_kelvin_per_step_of_one_reading() {
        let steps: Vec<_> = TemperatureScale::ALL
            .into_iter()
            .map(|scale| {
                let one = scale.to_kelvin(&number(1, 1)).unwrap();
                let zero = scale.to_kelvin(&number(0, 1)).unwrap();
                one.sub_exact(&zero).unwrap() == scale.factor()
            })
            .collect();

        assert_eq!(steps, [true, true, true]);
    }

    #[test]
    fn machine_reading_is_an_error() {
        let kelvin = TemperatureScale::Celsius.to_kelvin(&Number::F64(20.0));

        assert_eq!(kelvin, Err(TemperatureScaleError::MachineReading));
    }

    #[test]
    fn scale_names_resolve_to_their_scales() {
        let scales: Vec<_> = ["kelvin", "celsius", "fahrenheit"]
            .into_iter()
            .map(TemperatureScale::from_name)
            .collect();

        assert_eq!(
            scales,
            [
                Some(TemperatureScale::Kelvin),
                Some(TemperatureScale::Celsius),
                Some(TemperatureScale::Fahrenheit)
            ]
        );
    }

    #[test]
    fn unit_symbol_is_not_a_scale_name() {
        let scale = TemperatureScale::from_name("degC");

        assert_eq!(scale, None);
    }

    #[test]
    fn display_symbols_are_kelvin_and_degree_signs() {
        let symbols: Vec<_> = TemperatureScale::ALL
            .into_iter()
            .map(TemperatureScale::display_symbol)
            .collect();

        assert_eq!(symbols, ["K", "\u{00B0}C", "\u{00B0}F"]);
    }
}
