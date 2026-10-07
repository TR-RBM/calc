#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BaseDimension {
    Length,
    Mass,
    Time,
    ElectricCurrent,
    ThermodynamicTemperature,
    AmountOfSubstance,
    LuminousIntensity,
    Information,
}

impl BaseDimension {
    pub const ALL: [Self; BASE_DIMENSION_COUNT] = [
        Self::Length,
        Self::Mass,
        Self::Time,
        Self::ElectricCurrent,
        Self::ThermodynamicTemperature,
        Self::AmountOfSubstance,
        Self::LuminousIntensity,
        Self::Information,
    ];

    fn position(self) -> usize {
        match self {
            Self::Length => 0,
            Self::Mass => 1,
            Self::Time => 2,
            Self::ElectricCurrent => 3,
            Self::ThermodynamicTemperature => 4,
            Self::AmountOfSubstance => 5,
            Self::LuminousIntensity => 6,
            Self::Information => 7,
        }
    }
}

pub const BASE_DIMENSION_COUNT: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DimensionError {
    ExponentOutOfRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Dimension {
    exponents: [i8; BASE_DIMENSION_COUNT],
}

impl Dimension {
    pub const DIMENSIONLESS: Self = Self {
        exponents: [0; BASE_DIMENSION_COUNT],
    };

    pub const fn from_exponents(exponents: [i8; BASE_DIMENSION_COUNT]) -> Self {
        Self { exponents }
    }

    pub fn of_base(base: BaseDimension) -> Self {
        let mut exponents = [0; BASE_DIMENSION_COUNT];
        exponents[base.position()] = 1;
        Self { exponents }
    }

    pub fn exponent(&self, base: BaseDimension) -> i8 {
        self.exponents[base.position()]
    }

    pub fn exponents(&self) -> [i8; BASE_DIMENSION_COUNT] {
        self.exponents
    }

    pub fn is_dimensionless(&self) -> bool {
        *self == Self::DIMENSIONLESS
    }

    pub fn multiply(&self, other: &Self) -> Result<Self, DimensionError> {
        let mut exponents = [0; BASE_DIMENSION_COUNT];
        for (result, (left, right)) in exponents
            .iter_mut()
            .zip(self.exponents.iter().zip(other.exponents.iter()))
        {
            *result = left
                .checked_add(*right)
                .ok_or(DimensionError::ExponentOutOfRange)?;
        }
        Ok(Self { exponents })
    }

    pub fn power(&self, exponent: i8) -> Result<Self, DimensionError> {
        let mut exponents = [0; BASE_DIMENSION_COUNT];
        for (result, base_exponent) in exponents.iter_mut().zip(self.exponents.iter()) {
            *result = base_exponent
                .checked_mul(exponent)
                .ok_or(DimensionError::ExponentOutOfRange)?;
        }
        Ok(Self { exponents })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_dimension_has_exponent_one_only_at_its_own_position() {
        let mass = Dimension::of_base(BaseDimension::Mass);

        let exponents: Vec<i8> = BaseDimension::ALL
            .iter()
            .map(|base| mass.exponent(*base))
            .collect();

        assert_eq!(exponents, vec![0, 1, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn product_adds_exponents_per_base_dimension() {
        let velocity = Dimension::from_exponents([1, 0, -1, 0, 0, 0, 0, 0]);
        let momentum_per_velocity = Dimension::from_exponents([0, 1, 0, 0, 0, 0, 0, 0]);

        let momentum = velocity.multiply(&momentum_per_velocity).unwrap();

        assert_eq!(momentum.exponents(), [1, 1, -1, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn product_with_inverse_is_dimensionless() {
        let force = Dimension::from_exponents([1, 1, -2, 0, 0, 0, 0, 0]);
        let inverse = force.power(-1).unwrap();

        let product = force.multiply(&inverse).unwrap();

        assert!(product.is_dimensionless());
    }

    #[test]
    fn power_multiplies_every_exponent() {
        let length_per_time = Dimension::from_exponents([1, 0, -1, 0, 0, 0, 0, 0]);

        let squared = length_per_time.power(2).unwrap();

        assert_eq!(squared.exponents(), [2, 0, -2, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn power_zero_is_dimensionless() {
        let energy = Dimension::from_exponents([2, 1, -2, 0, 0, 0, 0, 0]);

        let result = energy.power(0).unwrap();

        assert!(result.is_dimensionless());
    }

    #[test]
    fn product_exponent_past_i8_range_is_an_error() {
        let high = Dimension::from_exponents([i8::MAX, 0, 0, 0, 0, 0, 0, 0]);
        let length = Dimension::of_base(BaseDimension::Length);

        let result = high.multiply(&length);

        assert_eq!(result, Err(DimensionError::ExponentOutOfRange));
    }

    #[test]
    fn power_exponent_past_i8_range_is_an_error() {
        let length = Dimension::from_exponents([64, 0, 0, 0, 0, 0, 0, 0]);

        let result = length.power(2);

        assert_eq!(result, Err(DimensionError::ExponentOutOfRange));
    }

    #[test]
    fn negating_minimum_exponent_is_an_error() {
        let length = Dimension::from_exponents([i8::MIN, 0, 0, 0, 0, 0, 0, 0]);

        let result = length.power(-1);

        assert_eq!(result, Err(DimensionError::ExponentOutOfRange));
    }
}
