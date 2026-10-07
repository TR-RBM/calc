use calc_expr::{BuildError, ExprId, ExprPool, Head, Operator};
use calc_numbers::{Integer, Number};
use calc_units::UnitId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalConstant {
    pub symbol: &'static str,
    pub bare_symbol: &'static str,
    pub numerator: i128,
    pub power_of_ten: i32,
    pub uncertainty_numerator: Option<i128>,
    pub uncertainty_power_of_ten: i32,
    pub unit: &'static [(&'static str, i8)],
}

pub const PHYSICAL_CONSTANTS: [PhysicalConstant; 7] = [
    PhysicalConstant {
        symbol: "c_0",
        bare_symbol: "c",
        numerator: 299_792_458,
        power_of_ten: 0,
        uncertainty_numerator: None,
        uncertainty_power_of_ten: 0,
        unit: &[("m", 1), ("s", -1)],
    },
    PhysicalConstant {
        symbol: "h_P",
        bare_symbol: "h",
        numerator: 662_607_015,
        power_of_ten: -42,
        uncertainty_numerator: None,
        uncertainty_power_of_ten: 0,
        unit: &[("J", 1), ("s", 1)],
    },
    PhysicalConstant {
        symbol: "k_B",
        bare_symbol: "k",
        numerator: 1_380_649,
        power_of_ten: -29,
        uncertainty_numerator: None,
        uncertainty_power_of_ten: 0,
        unit: &[("J", 1), ("K", -1)],
    },
    PhysicalConstant {
        symbol: "N_A",
        bare_symbol: "N",
        numerator: 602_214_076,
        power_of_ten: 15,
        uncertainty_numerator: None,
        uncertainty_power_of_ten: 0,
        unit: &[("mol", -1)],
    },
    PhysicalConstant {
        symbol: "G_N",
        bare_symbol: "G",
        numerator: 667_430,
        power_of_ten: -16,
        uncertainty_numerator: Some(15),
        uncertainty_power_of_ten: -16,
        unit: &[("m", 3), ("kg", -1), ("s", -2)],
    },
    PhysicalConstant {
        symbol: "m_e",
        bare_symbol: "m_e",
        numerator: 91_093_837_139,
        power_of_ten: -41,
        uncertainty_numerator: Some(28),
        uncertainty_power_of_ten: -41,
        unit: &[("kg", 1)],
    },
    PhysicalConstant {
        symbol: "m_u",
        bare_symbol: "m_u",
        numerator: 166_053_906_892,
        power_of_ten: -38,
        uncertainty_numerator: Some(52),
        uncertainty_power_of_ten: -38,
        unit: &[("kg", 1)],
    },
];

pub fn physical_constant(symbol: &str) -> Option<&'static PhysicalConstant> {
    PHYSICAL_CONSTANTS
        .iter()
        .find(|constant| constant.symbol == symbol)
}

pub fn constant_of_bare_symbol(symbol: &str) -> Option<&'static PhysicalConstant> {
    PHYSICAL_CONSTANTS
        .iter()
        .find(|constant| constant.bare_symbol == symbol)
}

fn scaled(numerator: i128, power_of_ten: i32) -> Option<Number> {
    let magnitude = Integer::from(10_u64).pow(power_of_ten.unsigned_abs());
    let numerator = Integer::from(i64::try_from(numerator).ok()?);
    if power_of_ten >= 0 {
        Some(Number::Integer(&numerator * &magnitude))
    } else {
        Number::fraction(&numerator, &magnitude).ok()
    }
}

fn unit_of(pool: &mut ExprPool, constant: &PhysicalConstant) -> Option<UnitId> {
    let mut unit: Option<UnitId> = None;
    for (symbol, exponent) in constant.unit {
        let named = pool.units_mut().lookup(symbol).ok()?;
        let factor = pool.units_mut().power(named, *exponent).ok()?;
        unit = Some(match unit {
            None => factor,
            Some(unit) => pool.units_mut().multiply(unit, factor).ok()?,
        });
    }
    unit
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstantError {
    Build(BuildError),
    ValueOutOfRange,
    UnitNotInTable,
}

pub fn constant_expression(
    pool: &mut ExprPool,
    constant: &PhysicalConstant,
) -> Result<ExprId, ConstantError> {
    let number = |pool: &mut ExprPool, numerator, power_of_ten| {
        scaled(numerator, power_of_ten)
            .ok_or(ConstantError::ValueOutOfRange)
            .and_then(|number| pool.number(number).map_err(ConstantError::Build))
    };
    let value = number(pool, constant.numerator, constant.power_of_ten)?;
    let value = match constant.uncertainty_numerator {
        None => value,
        Some(numerator) => {
            let uncertainty = number(pool, numerator, constant.uncertainty_power_of_ten)?;
            let mark = pool.anonymous_measurement().map_err(ConstantError::Build)?;
            pool.apply(
                Head::Operator(Operator::Uncertain),
                &[value, uncertainty, mark],
            )
            .map_err(ConstantError::Build)?
        }
    };
    let unit = unit_of(pool, constant).ok_or(ConstantError::UnitNotInTable)?;
    pool.quantity(value, unit).map_err(ConstantError::Build)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value_text(pool: &mut ExprPool, symbol: &str) -> String {
        let constant = physical_constant(symbol).expect("a constant");
        let expression = constant_expression(pool, constant).expect("an expression");
        calc_syntax::print_expression(pool, expression, calc_syntax::PrintMode::Ascii)
            .expect("prints")
    }

    #[test]
    fn the_speed_of_light_is_the_defining_value_in_metres_per_second() {
        let mut pool = ExprPool::new();

        assert_eq!(value_text(&mut pool, "c_0"), "299792458 m/s");
    }

    #[test]
    fn the_planck_constant_is_the_defining_value_in_joule_seconds() {
        let mut pool = ExprPool::new();

        assert_eq!(
            value_text(&mut pool, "h_P"),
            "0.000000000000000000000000000000000662607015 J*s"
        );
    }

    #[test]
    fn the_gravitational_constant_carries_its_standard_uncertainty() {
        let mut pool = ExprPool::new();

        assert_eq!(
            value_text(&mut pool, "G_N"),
            "0.000000000066743 +- 0.0000000000000015 m^3/kg/s^2"
        );
    }

    #[test]
    fn the_electron_mass_is_codata_2022_with_its_standard_uncertainty() {
        let mut pool = ExprPool::new();

        assert_eq!(
            value_text(&mut pool, "m_e"),
            "0.00000000000000000000000000000091093837139 +- 0.00000000000000000000000000000000000000028 kg"
        );
    }

    #[test]
    fn the_atomic_mass_constant_is_codata_2022_with_its_standard_uncertainty() {
        let mut pool = ExprPool::new();

        assert_eq!(
            value_text(&mut pool, "m_u"),
            "0.00000000000000000000000000166053906892 +- 0.00000000000000000000000000000000000052 kg"
        );
    }

    #[test]
    fn a_mass_constant_gives_no_bare_letter_a_hint() {
        assert_eq!(
            constant_of_bare_symbol("m").map(|constant| constant.symbol),
            None
        );
    }

    #[test]
    fn the_four_defining_constants_carry_no_uncertainty() {
        let exact: Vec<&str> = PHYSICAL_CONSTANTS
            .iter()
            .filter(|constant| constant.uncertainty_numerator.is_none())
            .map(|constant| constant.symbol)
            .collect();

        assert_eq!(exact, ["c_0", "h_P", "k_B", "N_A"]);
    }

    #[test]
    fn a_bare_letter_names_the_constant_that_carries_it() {
        assert_eq!(
            constant_of_bare_symbol("c").map(|constant| constant.symbol),
            Some("c_0")
        );
    }

    #[test]
    fn a_name_that_is_no_constant_has_none() {
        assert_eq!(physical_constant("c").map(|constant| constant.symbol), None);
    }
}
