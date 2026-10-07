use calc_core::ParameterValue;
use calc_expr::{ExprId, ExprPool, NodeView};
use calc_units::{BASE_DIMENSION_COUNT, Dimension, UnitId, UnitTable};

use crate::unit_display::input_text;

const LIST_SEPARATOR: &str = ", ";
const BASE_SYMBOLS: [&str; BASE_DIMENSION_COUNT] = ["m", "kg", "s", "A", "K", "mol", "cd", "bit"];
const PRODUCT_SIGN: &str = "*";
const POWER_SIGN: &str = "^";

const NAMED_QUANTITIES: [(&str, [i8; BASE_DIMENSION_COUNT]); 30] = [
    ("length", [1, 0, 0, 0, 0, 0, 0, 0]),
    ("mass", [0, 1, 0, 0, 0, 0, 0, 0]),
    ("time", [0, 0, 1, 0, 0, 0, 0, 0]),
    ("electric_current", [0, 0, 0, 1, 0, 0, 0, 0]),
    ("temperature", [0, 0, 0, 0, 1, 0, 0, 0]),
    ("amount_of_substance", [0, 0, 0, 0, 0, 1, 0, 0]),
    ("luminous_intensity", [0, 0, 0, 0, 0, 0, 1, 0]),
    ("information", [0, 0, 0, 0, 0, 0, 0, 1]),
    ("area", [2, 0, 0, 0, 0, 0, 0, 0]),
    ("volume", [3, 0, 0, 0, 0, 0, 0, 0]),
    ("speed", [1, 0, -1, 0, 0, 0, 0, 0]),
    ("acceleration", [1, 0, -2, 0, 0, 0, 0, 0]),
    ("force", [1, 1, -2, 0, 0, 0, 0, 0]),
    ("pressure", [-1, 1, -2, 0, 0, 0, 0, 0]),
    ("energy", [2, 1, -2, 0, 0, 0, 0, 0]),
    ("power", [2, 1, -3, 0, 0, 0, 0, 0]),
    ("frequency", [0, 0, -1, 0, 0, 0, 0, 0]),
    ("electric_charge", [0, 0, 1, 1, 0, 0, 0, 0]),
    ("voltage", [2, 1, -3, -1, 0, 0, 0, 0]),
    ("capacitance", [-2, -1, 4, 2, 0, 0, 0, 0]),
    ("resistance", [2, 1, -3, -2, 0, 0, 0, 0]),
    ("conductance", [-2, -1, 3, 2, 0, 0, 0, 0]),
    ("magnetic_flux", [2, 1, -2, -1, 0, 0, 0, 0]),
    ("magnetic_flux_density", [0, 1, -2, -1, 0, 0, 0, 0]),
    ("inductance", [2, 1, -2, -2, 0, 0, 0, 0]),
    ("data_rate", [0, 0, -1, 0, 0, 0, 0, 1]),
    ("density", [-3, 1, 0, 0, 0, 0, 0, 0]),
    ("dose", [2, 0, -2, 0, 0, 0, 0, 0]),
    ("illuminance", [-2, 0, 0, 0, 0, 0, 1, 0]),
    ("catalytic_activity", [0, 0, -1, 0, 0, 1, 0, 0]),
];

pub(crate) const PURE_NUMBER: &str = "pure_number";
pub(crate) const UNNAMED_QUANTITY: &str = "unnamed";

pub(crate) fn quantity_code(name: &str) -> u64 {
    if name == PURE_NUMBER {
        return 0;
    }
    NAMED_QUANTITIES
        .iter()
        .position(|(named, _)| *named == name)
        .and_then(|index| u64::try_from(index + 1).ok())
        .unwrap_or(u64::MAX)
}

fn quantity_name(dimension: &Dimension) -> &'static str {
    if dimension.is_dimensionless() {
        return PURE_NUMBER;
    }
    NAMED_QUANTITIES
        .iter()
        .find(|(_, exponents)| Dimension::from_exponents(*exponents) == *dimension)
        .map_or(UNNAMED_QUANTITY, |(name, _)| name)
}

fn written_units(pool: &ExprPool, root: ExprId) -> Vec<UnitId> {
    let mut found = Vec::new();
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        match pool.node(expression) {
            Ok(NodeView::Quantity { value, unit }) => {
                found.push(unit);
                pending.push(value);
            }
            Ok(NodeView::Apply { arguments, .. }) => pending.extend(arguments.iter().rev()),
            Ok(NodeView::Bind {
                arguments, body, ..
            }) => {
                pending.push(body);
                pending.extend(arguments.iter().rev());
            }
            Ok(NodeView::Array { elements, .. }) => pending.extend(elements.iter().rev()),
            _ => {}
        }
    }
    found
}

fn base_product(dimension: &Dimension) -> String {
    dimension
        .exponents()
        .iter()
        .zip(BASE_SYMBOLS)
        .filter(|(exponent, _)| **exponent != 0)
        .map(|(exponent, symbol)| match exponent {
            1 => symbol.to_owned(),
            _ => format!("{symbol}{POWER_SIGN}{exponent}"),
        })
        .collect::<Vec<_>>()
        .join(PRODUCT_SIGN)
}

fn named_units_of(dimension: &Dimension) -> Vec<String> {
    let mut table = UnitTable::new();
    let mut symbols: Vec<String> = table
        .named_units()
        .into_iter()
        .filter(|named| {
            table
                .prefix_family(*named)
                .is_ok_and(|family| family == *named)
                || table
                    .symbol(*named)
                    .is_ok_and(|symbol| BASE_SYMBOLS.contains(&symbol))
        })
        .filter_map(|named| table.symbol(named).ok().map(str::to_owned))
        .collect();
    symbols.retain(|symbol| {
        table
            .lookup(symbol)
            .ok()
            .and_then(|unit| table.dimension(unit).ok())
            .is_some_and(|found| found == *dimension)
    });
    symbols.sort_by_key(|symbol| symbol.to_lowercase());
    symbols
}

fn holds_a_coherent_unit(symbols: &[String]) -> bool {
    let mut table = UnitTable::new();
    symbols.iter().any(|symbol| {
        table
            .lookup(symbol)
            .ok()
            .and_then(|unit| table.scale_factor(unit).ok().cloned())
            .is_some_and(|scale| scale == calc_units::ScaleFactor::one())
    })
}

fn case_variants(symbol: &str) -> Vec<String> {
    let characters: Vec<char> = symbol.chars().collect();
    let mut variants = vec![symbol.to_lowercase(), symbol.to_uppercase()];
    for (index, character) in characters.iter().enumerate() {
        let flipped: String = if character.is_uppercase() {
            character.to_lowercase().collect()
        } else {
            character.to_uppercase().collect()
        };
        let mut variant: String = characters[..index].iter().collect();
        variant.push_str(&flipped);
        variant.extend(&characters[index + 1..]);
        variants.push(variant);
    }
    variants.retain(|variant| variant != symbol);
    variants.dedup();
    variants
}

fn suggestion(pool: &ExprPool, units: &[UnitId], left: &Dimension, right: &Dimension) -> String {
    let mut table = UnitTable::new();
    let written_symbols: Vec<String> = units
        .iter()
        .filter_map(|unit| input_text(pool, *unit))
        .collect();
    for unit in units {
        let Ok(dimension) = pool.units().dimension(*unit) else {
            continue;
        };
        let wanted = if dimension == *left {
            right
        } else if dimension == *right {
            left
        } else {
            continue;
        };
        let Some(written) = input_text(pool, *unit) else {
            continue;
        };
        for variant in case_variants(&written) {
            if written_symbols.contains(&variant) {
                continue;
            }
            let fits = table
                .lookup(&variant)
                .ok()
                .and_then(|found| table.dimension(found).ok())
                .is_some_and(|found| found == *wanted);
            if fits {
                return variant;
            }
        }
    }
    String::new()
}

fn side(pool: &ExprPool, units: &[UnitId], dimension: &Dimension) -> (String, String, String) {
    let quantity = quantity_name(dimension);
    let written = units
        .iter()
        .find(|unit| {
            pool.units()
                .dimension(**unit)
                .is_ok_and(|found| found == *dimension)
        })
        .and_then(|unit| input_text(pool, *unit))
        .unwrap_or_else(|| base_product(dimension));
    let mut list = named_units_of(dimension);
    if !dimension.is_dimensionless() && !holds_a_coherent_unit(&list) {
        list.insert(0, base_product(dimension));
    }
    if list.is_empty() {
        list.push(written.clone());
    }
    (quantity.to_owned(), written, list.join(LIST_SEPARATOR))
}

pub(crate) fn mismatch_data(
    pool: &ExprPool,
    expression: ExprId,
    left: &Dimension,
    right: &Dimension,
) -> Vec<(&'static str, ParameterValue)> {
    let units = written_units(pool, expression);
    let (left_quantity, left_unit, left_units) = side(pool, &units, left);
    let (right_quantity, right_unit, right_units) = side(pool, &units, right);
    let suggested = suggestion(pool, &units, left, right);
    [
        ("left_quantity", left_quantity),
        ("left_unit", left_unit),
        ("left_units", left_units),
        ("right_quantity", right_quantity),
        ("right_unit", right_unit),
        ("right_units", right_units),
        ("suggestion", suggested),
    ]
    .into_iter()
    .map(|(name, value)| (name, ParameterValue::Identifier(value)))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_quantity_has_its_own_dimension() {
        let mut seen: Vec<[i8; BASE_DIMENSION_COUNT]> = Vec::new();
        for (_, exponents) in NAMED_QUANTITIES {
            assert!(!seen.contains(&exponents), "{exponents:?}");
            seen.push(exponents);
        }
    }

    #[test]
    fn a_capital_t_has_the_lower_case_variant() {
        assert!(case_variants("T").contains(&"t".to_owned()));
    }

    #[test]
    fn the_units_of_mass_are_listed_without_prefixes() {
        let mass = Dimension::from_exponents([0, 1, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            named_units_of(&mass),
            ["g", "kg", "lb", "oz_av", "oz_t", "t"]
        );
    }

    #[test]
    fn a_pure_number_has_code_zero() {
        assert_eq!(quantity_code(PURE_NUMBER), 0);
        assert_eq!(quantity_code("mass"), 2);
    }
}
