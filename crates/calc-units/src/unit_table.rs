use std::collections::HashMap;

use crate::definitions::{
    AMBIGUOUS_NAMES, BASE_UNITS, BINARY_PREFIXES, BYTE, DEGREE, DEGREE_CELSIUS_DIFFERENCE,
    DEGREE_FAHRENHEIT_DIFFERENCE, DERIVED_UNITS, GRAM, INTERNATIONAL_YARD_AND_POUND_UNITS,
    KINDS_THAT_SHARE_A_UNIT, NamedUnitDefinition, PREFIXES, PrefixDefinition,
    SMALLEST_INFORMATION_PREFIX_POWER, TORR, UNITS_ACCEPTED_FOR_USE_WITH_THE_SI, UNITS_OF_A_128,
    UNITS_OF_A_252, UNITS_OF_A_253, UNITS_OF_A_273, WEEK,
};
use crate::dimension::{BASE_DIMENSION_COUNT, BaseDimension, Dimension};
use crate::scale_factor::ScaleFactor;

const DIMENSIONLESS_UNIT: UnitId = UnitId(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnitId(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NamedUnitId(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnitFactor {
    named_unit: NamedUnitId,
    exponent: i8,
}

impl UnitFactor {
    pub fn named_unit(&self) -> NamedUnitId {
        self.named_unit
    }

    pub fn exponent(&self) -> i8 {
        self.exponent
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitLookupError {
    UnknownName,
    Ambiguous,
    TableFull,
}

enum PrefixScale {
    Decimal(i8),
    Binary(u32),
}

pub fn ambiguous_readings(name: &str) -> Option<Vec<String>> {
    if let Some(readings) = named_ambiguity(name) {
        return Some(readings);
    }
    if name == "KB" {
        return Some(vec!["kB".to_owned(), "KiB".to_owned()]);
    }
    if name == "Kbit" {
        return Some(vec!["kbit".to_owned(), "Kibit".to_owned()]);
    }
    let prefix = name.strip_suffix('b')?;
    let written = if prefix == "K" { "k" } else { prefix };
    let known = written.is_empty()
        || BINARY_PREFIXES.iter().any(|(symbol, _)| *symbol == written)
        || PREFIXES.iter().any(|candidate| {
            candidate.symbol == written
                && candidate.power_of_ten >= SMALLEST_INFORMATION_PREFIX_POWER
        });
    known.then(|| vec![format!("{written}bit"), format!("{written}B")])
}

fn named_ambiguity(name: &str) -> Option<Vec<String>> {
    AMBIGUOUS_NAMES.iter().find_map(|ambiguous| {
        let prefix = if name == ambiguous.name {
            ""
        } else if ambiguous.takes_prefix {
            let prefix = name.strip_suffix(ambiguous.name)?;
            PREFIXES
                .iter()
                .find(|candidate| candidate.symbol == prefix)?
                .symbol
        } else {
            return None;
        };
        Some(
            ambiguous
                .readings
                .iter()
                .map(|reading| format!("{prefix}{reading}"))
                .collect(),
        )
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitProductError {
    UnknownUnitId(UnitId),
    ExponentOutOfRange,
    PiExponentOutOfRange,
    TableFull,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitAccessError {
    UnknownUnitId(UnitId),
    UnknownNamedUnitId(NamedUnitId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConversionError {
    UnknownUnitId(UnitId),
    DimensionMismatch { from: Dimension, to: Dimension },
    PiExponentOutOfRange,
}

struct TableFull;

struct NamedUnit {
    family: Option<NamedUnitId>,
    symbol: String,
    display_symbol: String,
    dimension: Dimension,
    scale_factor: ScaleFactor,
    accepts_prefix: bool,
}

struct UnitEntry {
    factors: Vec<UnitFactor>,
    dimension: Dimension,
    scale_factor: ScaleFactor,
}

const KILOGRAM_SYMBOL: &str = "kg";
const GRAM_SYMBOL: &str = "g";

pub struct UnitTable {
    named_units: Vec<NamedUnit>,
    named_unit_by_symbol: HashMap<String, NamedUnitId>,
    units: Vec<UnitEntry>,
    unit_by_factors: HashMap<Vec<UnitFactor>, UnitId>,
    entry_limit: u32,
}

impl Default for UnitTable {
    fn default() -> Self {
        Self::new()
    }
}

const AMBIGUOUS_NAMED_DIMENSIONS: [[i8; BASE_DIMENSION_COUNT]; 1] = [[2, 1, -2, 0, 0, 0, 0, 0]];

impl UnitTable {
    pub fn new() -> Self {
        let mut table = Self {
            named_units: Vec::new(),
            named_unit_by_symbol: HashMap::new(),
            units: vec![UnitEntry {
                factors: Vec::new(),
                dimension: Dimension::DIMENSIONLESS,
                scale_factor: ScaleFactor::one(),
            }],
            unit_by_factors: HashMap::from([(Vec::new(), DIMENSIONLESS_UNIT)]),
            entry_limit: u32::MAX,
        };
        let definitions = BASE_UNITS
            .iter()
            .chain(DERIVED_UNITS.iter())
            .chain([&GRAM, &DEGREE_CELSIUS_DIFFERENCE, &DEGREE])
            .chain(UNITS_ACCEPTED_FOR_USE_WITH_THE_SI.iter())
            .chain([&WEEK])
            .chain(UNITS_OF_A_128.iter())
            .chain(UNITS_OF_A_252.iter())
            .chain(UNITS_OF_A_253.iter())
            .chain(INTERNATIONAL_YARD_AND_POUND_UNITS.iter())
            .chain([&DEGREE_FAHRENHEIT_DIFFERENCE, &BYTE, &TORR])
            .chain(UNITS_OF_A_273.iter());
        for (definition, index) in definitions.zip(0_u32..) {
            table.register_definition(definition, NamedUnitId(index));
        }
        table
    }

    fn register_definition(&mut self, definition: &NamedUnitDefinition, named_unit: NamedUnitId) {
        self.named_units.push(NamedUnit {
            family: None,
            symbol: definition.symbol.to_string(),
            dimension: definition.dimension(),
            display_symbol: definition.display_symbol.to_string(),
            scale_factor: ScaleFactor::with_pi_exponent(
                definition.scale_numerator,
                definition.scale_denominator,
                definition.scale_pi_exponent,
            )
            .scaled_by_power_of_ten(definition.scale_power_of_ten),
            accepts_prefix: definition.accepts_prefix,
        });
        for symbol in std::iter::once(&definition.symbol).chain(definition.aliases) {
            self.named_unit_by_symbol
                .insert((*symbol).to_string(), named_unit);
        }
    }

    pub fn accepts_prefix(&self, named_unit: NamedUnitId) -> Result<bool, UnitAccessError> {
        self.named_units
            .get(named_unit.0 as usize)
            .map(|named| named.accepts_prefix)
            .ok_or(UnitAccessError::UnknownNamedUnitId(named_unit))
    }

    pub fn decimal_prefixes() -> Vec<(&'static str, i32)> {
        crate::definitions::PREFIXES
            .iter()
            .map(|prefix| (prefix.symbol, i32::from(prefix.power_of_ten)))
            .collect()
    }

    pub fn named_units(&self) -> Vec<NamedUnitId> {
        (0..self.named_units.len())
            .map(|index| NamedUnitId(u32::try_from(index).unwrap_or(u32::MAX)))
            .collect()
    }

    pub fn dimensionless(&self) -> UnitId {
        DIMENSIONLESS_UNIT
    }

    pub fn lookup(&mut self, name: &str) -> Result<UnitId, UnitLookupError> {
        let named_unit = match self.named_unit_by_symbol.get(name) {
            Some(named_unit) => *named_unit,
            None if ambiguous_readings(name).is_some() => {
                return Err(UnitLookupError::Ambiguous);
            }
            None => self.prefixed_named_unit(name)?,
        };
        let factors = vec![UnitFactor {
            named_unit,
            exponent: 1,
        }];
        let entry = self.entry_from_named_unit(named_unit, factors);
        self.intern(entry)
            .map_err(|TableFull| UnitLookupError::TableFull)
    }

    fn entry_from_named_unit(
        &self,
        named_unit: NamedUnitId,
        factors: Vec<UnitFactor>,
    ) -> UnitEntry {
        let named = self.named_unit(named_unit);
        UnitEntry {
            factors,
            dimension: named.dimension,
            scale_factor: named.scale_factor.clone(),
        }
    }

    fn prefixed_named_unit(&mut self, name: &str) -> Result<NamedUnitId, UnitLookupError> {
        let (prefix_symbol, scale, base) = match self.split_prefix(name) {
            Some((prefix, base)) => (
                prefix.symbol,
                PrefixScale::Decimal(prefix.power_of_ten),
                base,
            ),
            None => {
                let (symbol, power_of_two, base) = self
                    .split_binary_prefix(name)
                    .ok_or(UnitLookupError::UnknownName)?;
                (symbol, PrefixScale::Binary(power_of_two), base)
            }
        };
        let base_unit = self.named_unit(base);
        let symbol = format!("{}{}", prefix_symbol, base_unit.symbol);
        let display_symbol = format!("{}{}", prefix_symbol, base_unit.display_symbol);
        if let Some(existing) = self.named_unit_by_symbol.get(&symbol) {
            return Ok(*existing);
        }
        let scale_factor = match scale {
            PrefixScale::Decimal(power) => base_unit.scale_factor.scaled_by_power_of_ten(power),
            PrefixScale::Binary(power) => base_unit.scale_factor.scaled_by_power_of_two(power),
        };
        let dimension = base_unit.dimension;
        let named_unit = self
            .push_named_unit(NamedUnit {
                family: Some(base),
                symbol: symbol.clone(),
                display_symbol,
                dimension,
                scale_factor,
                accepts_prefix: false,
            })
            .map_err(|TableFull| UnitLookupError::TableFull)?;
        self.named_unit_by_symbol.insert(symbol, named_unit);
        Ok(named_unit)
    }

    fn split_prefix(&self, name: &str) -> Option<(&'static PrefixDefinition, NamedUnitId)> {
        PREFIXES.iter().find_map(|prefix| {
            std::iter::once(&prefix.symbol)
                .chain(prefix.aliases)
                .find_map(|prefix_symbol| {
                    let rest = name.strip_prefix(prefix_symbol)?;
                    let base = *self.named_unit_by_symbol.get(rest)?;
                    let named = self.named_unit(base);
                    let information =
                        named.dimension == Dimension::of_base(BaseDimension::Information);
                    (named.accepts_prefix
                        && (!information
                            || prefix.power_of_ten >= SMALLEST_INFORMATION_PREFIX_POWER))
                        .then_some((prefix, base))
                })
        })
    }

    fn split_binary_prefix(&self, name: &str) -> Option<(&'static str, u32, NamedUnitId)> {
        BINARY_PREFIXES.iter().find_map(|(symbol, power_of_two)| {
            let rest = name.strip_prefix(symbol)?;
            let base = *self.named_unit_by_symbol.get(rest)?;
            let named = self.named_unit(base);
            (named.accepts_prefix
                && named.dimension == Dimension::of_base(BaseDimension::Information))
            .then_some((*symbol, *power_of_two, base))
        })
    }

    fn named_unit(&self, named_unit: NamedUnitId) -> &NamedUnit {
        &self.named_units[position(named_unit.0)]
    }

    fn push_named_unit(&mut self, named_unit: NamedUnit) -> Result<NamedUnitId, TableFull> {
        let index = next_index(self.named_units.len(), self.entry_limit)?;
        self.named_units.push(named_unit);
        Ok(NamedUnitId(index))
    }

    fn intern(&mut self, entry: UnitEntry) -> Result<UnitId, TableFull> {
        if let Some(existing) = self.unit_by_factors.get(&entry.factors) {
            return Ok(*existing);
        }
        let unit = UnitId(next_index(self.units.len(), self.entry_limit)?);
        self.unit_by_factors.insert(entry.factors.clone(), unit);
        self.units.push(entry);
        Ok(unit)
    }

    pub fn named_coherent_unit_names_one_quantity(dimension: &Dimension) -> bool {
        !AMBIGUOUS_NAMED_DIMENSIONS
            .iter()
            .any(|exponents| Dimension::from_exponents(*exponents) == *dimension)
    }

    pub fn coherent_unit(&mut self, dimension: &Dimension) -> Result<UnitId, UnitProductError> {
        let factors = BaseDimension::ALL
            .iter()
            .zip(0_u32..)
            .filter(|(base, _)| dimension.exponent(**base) != 0)
            .map(|(base, index)| UnitFactor {
                named_unit: NamedUnitId(index),
                exponent: dimension.exponent(*base),
            })
            .collect();
        self.intern_product(factors)
    }

    pub fn named_coherent_unit(
        &mut self,
        dimension: &Dimension,
    ) -> Result<UnitId, UnitProductError> {
        let built_in = BASE_UNITS.len() + DERIVED_UNITS.len();
        let mut candidates = self
            .named_units
            .iter()
            .take(built_in)
            .zip(0_u32..)
            .filter(|(named, _)| named.dimension == *dimension && named.scale_factor.is_one())
            .map(|(_, index)| NamedUnitId(index));
        match (candidates.next(), candidates.next()) {
            (Some(named_unit), None) if !dimension.is_dimensionless() => {
                let factors = vec![UnitFactor {
                    named_unit,
                    exponent: 1,
                }];
                let entry = self.entry_from_named_unit(named_unit, factors);
                self.intern(entry)
                    .map_err(|TableFull| UnitProductError::TableFull)
            }
            _ => self.coherent_unit(dimension),
        }
    }

    pub fn multiply(&mut self, left: UnitId, right: UnitId) -> Result<UnitId, UnitProductError> {
        let mut factors = self.product_factors(left)?.to_vec();
        for right_factor in self.product_factors(right)? {
            match factors
                .iter_mut()
                .find(|factor| factor.named_unit == right_factor.named_unit)
            {
                Some(factor) => {
                    factor.exponent = factor
                        .exponent
                        .checked_add(right_factor.exponent)
                        .ok_or(UnitProductError::ExponentOutOfRange)?;
                }
                None => factors.push(*right_factor),
            }
        }
        self.intern_product(factors)
    }

    pub fn divide(&mut self, left: UnitId, right: UnitId) -> Result<UnitId, UnitProductError> {
        let inverse = self.power(right, -1)?;
        self.multiply(left, inverse)
    }

    pub fn power(&mut self, unit: UnitId, exponent: i8) -> Result<UnitId, UnitProductError> {
        let factors = self
            .product_factors(unit)?
            .iter()
            .map(|factor| {
                factor
                    .exponent
                    .checked_mul(exponent)
                    .map(|product| UnitFactor {
                        named_unit: factor.named_unit,
                        exponent: product,
                    })
                    .ok_or(UnitProductError::ExponentOutOfRange)
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.intern_product(factors)
    }

    fn product_factors(&self, unit: UnitId) -> Result<&[UnitFactor], UnitProductError> {
        self.entry(unit)
            .map(|entry| entry.factors.as_slice())
            .ok_or(UnitProductError::UnknownUnitId(unit))
    }

    fn intern_product(&mut self, mut factors: Vec<UnitFactor>) -> Result<UnitId, UnitProductError> {
        factors.retain(|factor| factor.exponent != 0);
        factors.sort_by(|left, right| {
            self.named_unit(left.named_unit)
                .symbol
                .cmp(&self.named_unit(right.named_unit).symbol)
        });
        let mut dimension = Dimension::DIMENSIONLESS;
        let mut scale_factor = ScaleFactor::one();
        for factor in &factors {
            let named = self.named_unit(factor.named_unit);
            dimension = named
                .dimension
                .power(factor.exponent)
                .and_then(|powered| dimension.multiply(&powered))
                .map_err(|_| UnitProductError::ExponentOutOfRange)?;
            scale_factor = named
                .scale_factor
                .power(factor.exponent)
                .and_then(|powered| scale_factor.multiply(&powered))
                .map_err(|_| UnitProductError::PiExponentOutOfRange)?;
        }
        self.intern(UnitEntry {
            factors,
            dimension,
            scale_factor,
        })
        .map_err(|TableFull| UnitProductError::TableFull)
    }

    fn entry(&self, unit: UnitId) -> Option<&UnitEntry> {
        self.units.get(position(unit.0))
    }

    pub fn dimension(&self, unit: UnitId) -> Result<Dimension, UnitAccessError> {
        self.entry(unit)
            .map(|entry| entry.dimension)
            .ok_or(UnitAccessError::UnknownUnitId(unit))
    }

    pub fn scale_factor(&self, unit: UnitId) -> Result<&ScaleFactor, UnitAccessError> {
        self.entry(unit)
            .map(|entry| &entry.scale_factor)
            .ok_or(UnitAccessError::UnknownUnitId(unit))
    }

    pub fn factors(&self, unit: UnitId) -> Result<&[UnitFactor], UnitAccessError> {
        self.entry(unit)
            .map(|entry| entry.factors.as_slice())
            .ok_or(UnitAccessError::UnknownUnitId(unit))
    }

    pub fn symbol(&self, named_unit: NamedUnitId) -> Result<&str, UnitAccessError> {
        self.named_units
            .get(position(named_unit.0))
            .map(|named| named.symbol.as_str())
            .ok_or(UnitAccessError::UnknownNamedUnitId(named_unit))
    }

    pub fn prefix_family(&self, named_unit: NamedUnitId) -> Result<NamedUnitId, UnitAccessError> {
        let named = self
            .named_units
            .get(position(named_unit.0))
            .ok_or(UnitAccessError::UnknownNamedUnitId(named_unit))?;
        if let Some(family) = named.family {
            return Ok(family);
        }
        if named.symbol == KILOGRAM_SYMBOL {
            return Ok(self
                .named_unit_by_symbol
                .get(GRAM_SYMBOL)
                .copied()
                .unwrap_or(named_unit));
        }
        Ok(named_unit)
    }

    pub fn display_symbol(&self, named_unit: NamedUnitId) -> Result<&str, UnitAccessError> {
        self.named_units
            .get(position(named_unit.0))
            .map(|named| named.display_symbol.as_str())
            .ok_or(UnitAccessError::UnknownNamedUnitId(named_unit))
    }

    pub fn kind_sharing_the_unit(&self, unit: UnitId) -> Option<&'static str> {
        let [factor] = self.factors(unit).ok()? else {
            return None;
        };
        if factor.exponent() != 1 {
            return None;
        }
        let family = self.prefix_family(factor.named_unit()).ok()?;
        let symbol = self.symbol(family).ok()?;
        KINDS_THAT_SHARE_A_UNIT
            .iter()
            .find(|(named, _)| *named == symbol)
            .map(|(_, kind)| *kind)
    }

    pub fn has_binary_prefix(&self, unit: UnitId) -> Result<bool, UnitAccessError> {
        Ok(self.factors(unit)?.iter().any(|factor| {
            let named = self.named_unit(factor.named_unit());
            named.family.is_some_and(|family| {
                let base = &self.named_unit(family).symbol;
                BINARY_PREFIXES.iter().any(|(prefix, _)| {
                    named
                        .symbol
                        .strip_prefix(prefix)
                        .is_some_and(|rest| rest == base)
                })
            })
        }))
    }

    pub fn conversion_factor(
        &self,
        from: UnitId,
        to: UnitId,
    ) -> Result<ScaleFactor, ConversionError> {
        let from_entry = self
            .entry(from)
            .ok_or(ConversionError::UnknownUnitId(from))?;
        let to_entry = self.entry(to).ok_or(ConversionError::UnknownUnitId(to))?;
        if from_entry.dimension != to_entry.dimension {
            return Err(ConversionError::DimensionMismatch {
                from: from_entry.dimension,
                to: to_entry.dimension,
            });
        }
        from_entry
            .scale_factor
            .divide(&to_entry.scale_factor)
            .map_err(|_| ConversionError::PiExponentOutOfRange)
    }

    #[cfg(test)]
    fn register_test_unit(
        &mut self,
        symbol: &str,
        dimension: Dimension,
        scale_factor: ScaleFactor,
    ) {
        let named_unit = self
            .push_named_unit(NamedUnit {
                family: None,
                symbol: symbol.to_string(),
                display_symbol: symbol.to_string(),
                dimension,
                scale_factor,
                accepts_prefix: false,
            })
            .ok()
            .unwrap();
        self.named_unit_by_symbol
            .insert(symbol.to_string(), named_unit);
    }
}

fn position(index: u32) -> usize {
    usize::try_from(index).unwrap_or(usize::MAX)
}

fn next_index(length: usize, entry_limit: u32) -> Result<u32, TableFull> {
    u32::try_from(length)
        .ok()
        .filter(|index| *index < entry_limit)
        .ok_or(TableFull)
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_numbers::Integer;

    fn ratio(numerator: u64, denominator: u64) -> ScaleFactor {
        ScaleFactor::from_positive_ratio(numerator, denominator)
    }

    fn unit(table: &mut UnitTable, name: &str) -> UnitId {
        table.lookup(name).unwrap()
    }

    fn exponents(table: &UnitTable, unit_id: UnitId) -> [i8; 8] {
        table.dimension(unit_id).unwrap().exponents()
    }

    #[test]
    fn a_gibibyte_is_two_to_the_thirty_three_bits() {
        let mut table = UnitTable::new();
        let gibibyte = unit(&mut table, "GiB");
        let bit = unit(&mut table, "bit");

        assert_eq!(
            table.conversion_factor(gibibyte, bit).unwrap(),
            ratio(1 << 33, 1)
        );
    }

    #[test]
    fn a_gigabyte_is_eight_billion_bits() {
        let mut table = UnitTable::new();
        let gigabyte = unit(&mut table, "GB");
        let bit = unit(&mut table, "bit");

        assert_eq!(
            table.conversion_factor(gigabyte, bit).unwrap(),
            ratio(8_000_000_000, 1)
        );
    }

    #[test]
    fn a_byte_has_the_dimension_information() {
        let mut table = UnitTable::new();
        let byte = unit(&mut table, "B");

        assert_eq!(exponents(&table, byte), [0, 0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn a_gibibyte_has_a_binary_prefix() {
        let mut table = UnitTable::new();
        let unit = table.lookup("GiB").unwrap();

        assert_eq!(table.has_binary_prefix(unit), Ok(true));
    }

    #[test]
    fn a_terabyte_has_no_binary_prefix() {
        let mut table = UnitTable::new();
        let unit = table.lookup("TB").unwrap();

        assert_eq!(table.has_binary_prefix(unit), Ok(false));
    }

    #[test]
    fn a_binary_prefix_does_not_attach_to_the_metre() {
        let mut table = UnitTable::new();

        assert_eq!(table.lookup("Kim"), Err(UnitLookupError::UnknownName));
    }

    #[test]
    fn a_small_decimal_prefix_does_not_attach_to_the_byte() {
        let mut table = UnitTable::new();

        assert_eq!(table.lookup("mB"), Err(UnitLookupError::UnknownName));
    }

    #[test]
    fn kb_is_ambiguous() {
        let mut table = UnitTable::new();

        assert_eq!(table.lookup("KB"), Err(UnitLookupError::Ambiguous));
        assert_eq!(
            ambiguous_readings("KB"),
            Some(vec!["kB".to_owned(), "KiB".to_owned()])
        );
    }

    #[test]
    fn b_behind_a_prefix_is_bit_or_byte() {
        assert_eq!(
            ambiguous_readings("Gb"),
            Some(vec!["Gbit".to_owned(), "GB".to_owned()])
        );
    }

    #[test]
    fn a_millibar_is_not_ambiguous() {
        assert_eq!(ambiguous_readings("mbar"), None);
    }

    #[test]
    fn a_week_is_seven_days() {
        let mut table = UnitTable::new();
        let week = unit(&mut table, "wk");
        let day = unit(&mut table, "d");

        assert_eq!(table.conversion_factor(week, day).unwrap(), ratio(7, 1));
    }

    #[test]
    fn a_week_is_six_hundred_and_four_thousand_eight_hundred_seconds() {
        let mut table = UnitTable::new();
        let week = unit(&mut table, "wk");
        let second = unit(&mut table, "s");

        assert_eq!(
            table.conversion_factor(week, second).unwrap(),
            ratio(604_800, 1)
        );
    }

    #[test]
    fn the_long_spelling_is_the_same_unit() {
        let mut table = UnitTable::new();

        assert_eq!(unit(&mut table, "week"), unit(&mut table, "wk"));
    }

    #[test]
    fn a_week_shows_as_its_short_symbol() {
        let mut table = UnitTable::new();
        let week = unit(&mut table, "week");

        let named = table.factors(week).unwrap()[0].named_unit();

        assert_eq!(table.display_symbol(named).unwrap(), "wk");
    }

    #[test]
    fn a_week_takes_no_prefix() {
        let mut table = UnitTable::new();

        assert!(table.lookup("kwk").is_err());
    }

    #[test]
    fn an_electronvolt_is_the_exact_joule_of_the_brochure() {
        let mut table = UnitTable::new();
        let electronvolt = unit(&mut table, "eV");
        let joule = unit(&mut table, "J");

        let factor = table.conversion_factor(electronvolt, joule).unwrap();

        let expected_denominator = &Integer::from(10_u64).pow(27) * &Integer::from(5_u64);
        assert_eq!(
            (
                factor.numerator().clone(),
                factor.denominator().clone(),
                factor.pi_exponent()
            ),
            (Integer::from(801_088_317_u64), expected_denominator, 0)
        );
    }

    #[test]
    fn a_kiloelectronvolt_is_a_thousand_electronvolts() {
        let mut table = UnitTable::new();
        let kilo = unit(&mut table, "keV");
        let electronvolt = unit(&mut table, "eV");

        let factor = table.conversion_factor(kilo, electronvolt).unwrap();

        assert_eq!(factor, ratio(1000, 1));
    }

    #[test]
    fn a_bar_is_a_hundred_thousand_pascal() {
        let mut table = UnitTable::new();
        let bar = unit(&mut table, "bar");
        let pascal = unit(&mut table, "Pa");

        assert_eq!(
            table.conversion_factor(bar, pascal).unwrap(),
            ratio(100_000, 1)
        );
    }

    #[test]
    fn a_standard_atmosphere_is_the_resolution_of_1954() {
        let mut table = UnitTable::new();
        let atmosphere = unit(&mut table, "atm");
        let pascal = unit(&mut table, "Pa");

        assert_eq!(
            table.conversion_factor(atmosphere, pascal).unwrap(),
            ratio(101_325, 1)
        );
    }

    #[test]
    fn a_bare_calorie_names_its_readings() {
        let mut table = UnitTable::new();

        assert_eq!(table.lookup("cal"), Err(UnitLookupError::Ambiguous));
        assert_eq!(
            ambiguous_readings("kcal"),
            Some(vec!["kcal_th".to_owned(), "kcal_IT".to_owned()])
        );
    }

    #[test]
    fn a_thermochemical_calorie_is_four_point_one_eight_four_joules() {
        let mut table = UnitTable::new();
        let calorie = unit(&mut table, "cal_th");
        let joule = unit(&mut table, "J");

        assert_eq!(
            table.conversion_factor(calorie, joule).unwrap(),
            ratio(4184, 1000)
        );
    }

    #[test]
    fn a_kilocalorie_is_a_thousand_calories() {
        let mut table = UnitTable::new();
        let kilocalorie = unit(&mut table, "kcal_th");
        let joule = unit(&mut table, "J");

        assert_eq!(
            table.conversion_factor(kilocalorie, joule).unwrap(),
            ratio(4184, 1)
        );
    }

    #[test]
    fn a_standard_atmosphere_takes_no_prefix() {
        let mut table = UnitTable::new();

        assert!(table.lookup("katm").is_err());
    }

    #[test]
    fn base_units_are_coherent_units_of_their_base_dimension() {
        let mut table = UnitTable::new();
        let names = ["m", "kg", "s", "A", "K", "mol", "cd", "bit"];

        let results: Vec<(Dimension, bool)> = names
            .iter()
            .map(|name| {
                let id = unit(&mut table, name);
                (
                    table.dimension(id).unwrap(),
                    table.scale_factor(id).unwrap().is_one(),
                )
            })
            .collect();

        let expected: Vec<(Dimension, bool)> = BaseDimension::ALL
            .iter()
            .map(|base| (Dimension::of_base(*base), true))
            .collect();
        assert_eq!(results, expected);
    }

    #[test]
    fn coherent_unit_of_base_dimension_is_the_base_unit() {
        let mut table = UnitTable::new();
        let second = unit(&mut table, "s");

        let coherent = table
            .coherent_unit(&Dimension::of_base(BaseDimension::Time))
            .unwrap();

        assert_eq!(coherent, second);
    }

    #[test]
    fn derived_units_equal_their_defining_relations() {
        let mut table = UnitTable::new();
        let relations: [(&str, &[(&str, i8)]); 21] = [
            ("rad", &[("m", 1), ("m", -1)]),
            ("sr", &[("m", 2), ("m", -2)]),
            ("Hz", &[("s", -1)]),
            ("N", &[("kg", 1), ("m", 1), ("s", -2)]),
            ("Pa", &[("N", 1), ("m", -2)]),
            ("J", &[("N", 1), ("m", 1)]),
            ("W", &[("J", 1), ("s", -1)]),
            ("C", &[("A", 1), ("s", 1)]),
            ("V", &[("W", 1), ("A", -1)]),
            ("F", &[("C", 1), ("V", -1)]),
            ("\u{03A9}", &[("V", 1), ("A", -1)]),
            ("S", &[("A", 1), ("V", -1)]),
            ("Wb", &[("V", 1), ("s", 1)]),
            ("T", &[("Wb", 1), ("m", -2)]),
            ("H", &[("Wb", 1), ("A", -1)]),
            ("lm", &[("cd", 1), ("sr", 1)]),
            ("lx", &[("lm", 1), ("m", -2)]),
            ("Bq", &[("s", -1)]),
            ("Gy", &[("J", 1), ("kg", -1)]),
            ("Sv", &[("J", 1), ("kg", -1)]),
            ("kat", &[("mol", 1), ("s", -1)]),
        ];

        let mismatches: Vec<&str> = relations
            .iter()
            .filter(|(name, relation)| {
                let named = unit(&mut table, name);
                let mut product = table.dimensionless();
                for (factor_name, exponent) in relation.iter() {
                    let factor = unit(&mut table, factor_name);
                    let powered = table.power(factor, *exponent).unwrap();
                    product = table.multiply(product, powered).unwrap();
                }
                table.conversion_factor(named, product) != Ok(ScaleFactor::one())
            })
            .map(|(name, _)| *name)
            .collect();

        assert!(mismatches.is_empty(), "{mismatches:?}");
    }

    #[test]
    fn same_name_resolves_to_same_id() {
        let mut table = UnitTable::new();

        let first = unit(&mut table, "km");
        let second = unit(&mut table, "km");

        assert_eq!(first, second);
    }

    #[test]
    fn unknown_name_is_an_error() {
        let mut table = UnitTable::new();

        let result = table.lookup("furlong");

        assert_eq!(result, Err(UnitLookupError::UnknownName));
    }

    #[test]
    fn kilometre_is_one_thousand_metres() {
        let mut table = UnitTable::new();
        let kilometre = unit(&mut table, "km");
        let metre = unit(&mut table, "m");

        let factor = table.conversion_factor(kilometre, metre);

        assert_eq!(factor, Ok(ratio(1000, 1)));
    }

    #[test]
    fn gram_is_one_thousandth_kilogram() {
        let mut table = UnitTable::new();
        let gram = unit(&mut table, "g");

        let factor = table.scale_factor(gram).unwrap();

        assert_eq!(factor, &ratio(1, 1000));
    }

    #[test]
    fn milligram_is_prefixed_gram() {
        let mut table = UnitTable::new();
        let milligram = unit(&mut table, "mg");

        let factor = table.scale_factor(milligram).unwrap();

        assert_eq!(factor, &ratio(1, 1_000_000));
    }

    #[test]
    fn kilogram_symbol_is_the_base_unit_not_a_prefixed_gram() {
        let mut table = UnitTable::new();
        let kilogram = unit(&mut table, "kg");

        let factor = table.factors(kilogram).unwrap()[0];

        assert_eq!(
            (
                table.symbol(factor.named_unit()),
                table.scale_factor(kilogram).unwrap().is_one(),
            ),
            (Ok("kg"), true)
        );
    }

    #[test]
    fn prefix_on_kilogram_is_unknown() {
        let mut table = UnitTable::new();

        let result = table.lookup("mkg");

        assert_eq!(result, Err(UnitLookupError::UnknownName));
    }

    #[test]
    fn double_prefix_is_unknown() {
        let mut table = UnitTable::new();
        unit(&mut table, "km");

        let result = table.lookup("Mkm");

        assert_eq!(result, Err(UnitLookupError::UnknownName));
    }

    #[test]
    fn prefix_on_degree_celsius_is_unknown() {
        let mut table = UnitTable::new();

        let result = table.lookup("mdegC");

        assert_eq!(result, Err(UnitLookupError::UnknownName));
    }

    #[test]
    fn full_unit_names_take_precedence_over_prefix_readings() {
        let mut table = UnitTable::new();
        let names = [
            "cd", "Pa", "T", "Gy", "kat", "mol", "min", "h", "d", "t", "ft", "mi", "yd",
        ];

        let symbols: Vec<String> = names
            .iter()
            .map(|name| {
                let id = unit(&mut table, name);
                let factor = table.factors(id).unwrap()[0];
                table.symbol(factor.named_unit()).unwrap().to_string()
            })
            .collect();

        assert_eq!(symbols, names);
    }

    #[test]
    fn every_prefix_on_every_prefixable_unit_scales_by_its_corpus_power_of_ten() {
        let mut table = UnitTable::new();
        let corpus_prefixes: [(&str, i8); 25] = [
            ("Q", 30),
            ("R", 27),
            ("Y", 24),
            ("Z", 21),
            ("E", 18),
            ("P", 15),
            ("T", 12),
            ("G", 9),
            ("M", 6),
            ("k", 3),
            ("h", 2),
            ("da", 1),
            ("d", -1),
            ("c", -2),
            ("m", -3),
            ("\u{00B5}", -6),
            ("\u{03BC}", -6),
            ("n", -9),
            ("p", -12),
            ("f", -15),
            ("a", -18),
            ("z", -21),
            ("y", -24),
            ("r", -27),
            ("q", -30),
        ];
        let prefixable: Vec<&NamedUnitDefinition> = BASE_UNITS
            .iter()
            .chain(DERIVED_UNITS.iter())
            .chain([&GRAM])
            .chain(UNITS_ACCEPTED_FOR_USE_WITH_THE_SI.iter())
            .filter(|definition| definition.accepts_prefix)
            .filter(|definition| {
                definition.dimension() != Dimension::of_base(BaseDimension::Information)
            })
            .collect();

        let mut wrong = Vec::new();
        for (prefix_symbol, power) in corpus_prefixes {
            let ten = Integer::from(10_u64).pow(u32::from(power.unsigned_abs()));
            for definition in &prefixable {
                let name = format!("{prefix_symbol}{}", definition.symbol);
                let prefixed = unit(&mut table, &name);
                let plain = unit(&mut table, definition.symbol);
                let factor = table.conversion_factor(prefixed, plain).unwrap();
                let (numerator, denominator) = if power < 0 {
                    (Integer::one(), ten.clone())
                } else {
                    (ten.clone(), Integer::one())
                };
                if (factor.numerator(), factor.denominator()) != (&numerator, &denominator) {
                    wrong.push(name);
                }
            }
        }

        assert!(wrong.is_empty(), "{wrong:?}");
    }

    #[test]
    fn micro_sign_and_greek_mu_give_the_same_unit() {
        let mut table = UnitTable::new();

        let micro_sign = unit(&mut table, "\u{00B5}m");
        let greek_mu = unit(&mut table, "\u{03BC}m");

        assert_eq!(micro_sign, greek_mu);
    }

    #[test]
    fn ohm_sign_and_greek_omega_give_the_same_unit() {
        let mut table = UnitTable::new();

        let ohm_sign = unit(&mut table, "k\u{2126}");
        let omega = unit(&mut table, "k\u{03A9}");

        assert_eq!(ohm_sign, omega);
    }

    #[test]
    fn degree_is_a_dimensionless_plane_angle_unit() {
        let mut table = UnitTable::new();
        let degree = unit(&mut table, "deg");

        let dimension = table.dimension(degree).unwrap();

        assert!(dimension.is_dimensionless());
    }

    #[test]
    fn degree_is_pi_over_one_hundred_eighty_radians() {
        let mut table = UnitTable::new();
        let degree = unit(&mut table, "deg");
        let radian = unit(&mut table, "rad");

        let factor = table.conversion_factor(degree, radian).unwrap();

        assert_eq!(factor, ScaleFactor::with_pi_exponent(1, 180, 1));
    }

    #[test]
    fn radian_is_one_hundred_eighty_over_pi_degrees() {
        let mut table = UnitTable::new();
        let degree = unit(&mut table, "deg");
        let radian = unit(&mut table, "rad");

        let factor = table.conversion_factor(radian, degree).unwrap();

        assert_eq!(factor, ScaleFactor::with_pi_exponent(180, 1, -1));
    }

    #[test]
    fn square_degree_squares_the_scale_factor() {
        let mut table = UnitTable::new();
        let degree = unit(&mut table, "deg");

        let square = table.power(degree, 2).unwrap();

        assert_eq!(
            table.scale_factor(square).unwrap(),
            &ScaleFactor::with_pi_exponent(1, 32_400, 2)
        );
    }

    #[test]
    fn prefix_on_degree_is_unknown() {
        let mut table = UnitTable::new();

        let result = table.lookup("mdeg");

        assert_eq!(result, Err(UnitLookupError::UnknownName));
    }

    #[test]
    fn degree_is_not_read_as_a_prefixed_unit() {
        let mut table = UnitTable::new();
        let degree = unit(&mut table, "deg");

        let factor = table.factors(degree).unwrap()[0];

        assert_eq!(table.symbol(factor.named_unit()), Ok("deg"));
    }

    #[test]
    fn degree_celsius_is_a_temperature_difference_equal_to_kelvin() {
        let mut table = UnitTable::new();
        let celsius = unit(&mut table, "degC");
        let kelvin = unit(&mut table, "K");

        let factor = table.conversion_factor(celsius, kelvin);

        assert_eq!(factor, Ok(ScaleFactor::one()));
    }

    #[test]
    fn degree_celsius_keeps_its_own_id() {
        let mut table = UnitTable::new();

        let celsius = unit(&mut table, "degC");
        let kelvin = unit(&mut table, "K");

        assert_ne!(celsius, kelvin);
    }

    #[test]
    fn hertz_and_becquerel_are_separate_units_with_factor_one() {
        let mut table = UnitTable::new();
        let hertz = unit(&mut table, "Hz");
        let becquerel = unit(&mut table, "Bq");

        let result = (
            hertz != becquerel,
            table.conversion_factor(hertz, becquerel),
        );

        assert_eq!(result, (true, Ok(ScaleFactor::one())));
    }

    #[test]
    fn product_of_same_unit_adds_exponents() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");

        let square = table.multiply(metre, metre).unwrap();

        assert_eq!(table.factors(square).unwrap()[0].exponent(), 2);
    }

    #[test]
    fn product_with_inverse_of_same_unit_is_dimensionless_unit() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");

        let ratio_unit = table.divide(metre, metre).unwrap();

        assert_eq!(ratio_unit, table.dimensionless());
    }

    #[test]
    fn ratio_of_different_length_units_is_dimensionless_but_not_one() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");
        let kilometre = unit(&mut table, "km");

        let ratio_unit = table.divide(metre, kilometre).unwrap();

        assert_eq!(
            (
                ratio_unit == table.dimensionless(),
                table.dimension(ratio_unit).unwrap().is_dimensionless(),
                table.scale_factor(ratio_unit).unwrap().clone(),
            ),
            (false, true, ratio(1, 1000))
        );
    }

    #[test]
    fn factor_order_does_not_change_the_unit() {
        let mut table = UnitTable::new();
        let newton = unit(&mut table, "N");
        let metre = unit(&mut table, "m");

        let newton_metre = table.multiply(newton, metre).unwrap();
        let metre_newton = table.multiply(metre, newton).unwrap();

        assert_eq!(newton_metre, metre_newton);
    }

    #[test]
    fn factors_are_sorted_by_symbol() {
        let mut table = UnitTable::new();
        let second = unit(&mut table, "s");
        let metre = unit(&mut table, "m");
        let kilogram = unit(&mut table, "kg");
        let per_second = table.power(second, -2).unwrap();
        let mass_per_second = table.multiply(per_second, kilogram).unwrap();

        let product = table.multiply(mass_per_second, metre).unwrap();

        let symbols: Vec<&str> = table
            .factors(product)
            .unwrap()
            .iter()
            .map(|factor| table.symbol(factor.named_unit()).unwrap())
            .collect();
        assert_eq!(symbols, vec!["kg", "m", "s"]);
    }

    #[test]
    fn power_raises_scale_factor() {
        let mut table = UnitTable::new();
        let kilometre = unit(&mut table, "km");

        let square = table.power(kilometre, 2).unwrap();

        assert_eq!(table.scale_factor(square).unwrap(), &ratio(1_000_000, 1));
    }

    #[test]
    fn negative_power_inverts_dimension() {
        let mut table = UnitTable::new();
        let second = unit(&mut table, "s");

        let per_second = table.power(second, -1).unwrap();

        assert_eq!(exponents(&table, per_second), [0, 0, -1, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn power_zero_is_dimensionless_unit() {
        let mut table = UnitTable::new();
        let newton = unit(&mut table, "N");

        let result = table.power(newton, 0).unwrap();

        assert_eq!(result, table.dimensionless());
    }

    #[test]
    fn coherent_unit_of_force_is_base_unit_product_with_factor_one() {
        let mut table = UnitTable::new();
        let newton = unit(&mut table, "N");
        let force = table.dimension(newton).unwrap();

        let coherent = table.coherent_unit(&force).unwrap();

        assert_eq!(
            (
                table.factors(coherent).unwrap().len(),
                table.conversion_factor(newton, coherent),
            ),
            (3, Ok(ScaleFactor::one()))
        );
    }

    #[test]
    fn named_coherent_unit_of_force_is_the_newton() {
        let mut table = UnitTable::new();
        let newton = unit(&mut table, "N");
        let force = table.dimension(newton).unwrap();

        let named = table.named_coherent_unit(&force).unwrap();

        assert_eq!(named, newton);
    }

    #[test]
    fn named_coherent_unit_of_a_base_dimension_is_the_base_unit() {
        let mut table = UnitTable::new();
        let kilogram = unit(&mut table, "kg");

        let named = table
            .named_coherent_unit(&Dimension::of_base(BaseDimension::Mass))
            .unwrap();

        assert_eq!(named, kilogram);
    }

    #[test]
    fn named_coherent_unit_shared_by_two_names_is_the_base_unit_product() {
        let mut table = UnitTable::new();
        let hertz = unit(&mut table, "Hz");
        let frequency = table.dimension(hertz).unwrap();
        let product = table.coherent_unit(&frequency).unwrap();

        let named = table.named_coherent_unit(&frequency).unwrap();

        assert_eq!(named, product);
    }

    #[test]
    fn named_coherent_unit_without_a_name_is_the_base_unit_product() {
        let mut table = UnitTable::new();
        let velocity = Dimension::from_exponents([1, 0, -1, 0, 0, 0, 0, 0]);
        let product = table.coherent_unit(&velocity).unwrap();

        let named = table.named_coherent_unit(&velocity).unwrap();

        assert_eq!(named, product);
    }

    #[test]
    fn named_coherent_unit_of_dimensionless_is_dimensionless_unit() {
        let mut table = UnitTable::new();

        let named = table
            .named_coherent_unit(&Dimension::DIMENSIONLESS)
            .unwrap();

        assert_eq!(named, table.dimensionless());
    }

    #[test]
    fn coherent_unit_of_dimensionless_is_dimensionless_unit() {
        let mut table = UnitTable::new();

        let coherent = table.coherent_unit(&Dimension::DIMENSIONLESS).unwrap();

        assert_eq!(coherent, table.dimensionless());
    }

    #[test]
    fn conversion_between_prefixed_units_is_ratio_of_scales() {
        let mut table = UnitTable::new();
        let millimetre = unit(&mut table, "mm");
        let kilometre = unit(&mut table, "km");

        let factor = table.conversion_factor(millimetre, kilometre);

        assert_eq!(factor, Ok(ratio(1, 1_000_000)));
    }

    #[test]
    fn conversion_between_different_dimensions_is_an_error() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");
        let second = unit(&mut table, "s");

        let result = table.conversion_factor(metre, second);

        assert_eq!(
            result,
            Err(ConversionError::DimensionMismatch {
                from: Dimension::of_base(BaseDimension::Length),
                to: Dimension::of_base(BaseDimension::Time),
            })
        );
    }

    #[test]
    fn conversion_with_unknown_unit_id_is_an_error() {
        let table = UnitTable::new();
        let foreign = UnitId(u32::MAX - 1);

        let result = table.conversion_factor(table.dimensionless(), foreign);

        assert_eq!(result, Err(ConversionError::UnknownUnitId(foreign)));
    }

    #[test]
    fn conversion_past_pi_exponent_range_is_an_error() {
        let mut table = UnitTable::new();
        let pi_scale = ScaleFactor::with_pi_exponent(1, 1, i32::MAX);
        let reciprocal_scale = ScaleFactor::with_pi_exponent(1, 1, -1);
        table.register_test_unit("high", Dimension::DIMENSIONLESS, pi_scale);
        table.register_test_unit("low", Dimension::DIMENSIONLESS, reciprocal_scale);
        let high = unit(&mut table, "high");
        let low = unit(&mut table, "low");

        let result = table.conversion_factor(high, low);

        assert_eq!(result, Err(ConversionError::PiExponentOutOfRange));
    }

    #[test]
    fn access_with_unknown_unit_id_is_an_error() {
        let table = UnitTable::new();
        let foreign = UnitId(u32::MAX - 1);

        let result = table.dimension(foreign);

        assert_eq!(result, Err(UnitAccessError::UnknownUnitId(foreign)));
    }

    #[test]
    fn symbol_with_unknown_named_unit_id_is_an_error() {
        let table = UnitTable::new();
        let foreign = NamedUnitId(u32::MAX - 1);

        let result = table.symbol(foreign);

        assert_eq!(result, Err(UnitAccessError::UnknownNamedUnitId(foreign)));
    }

    #[test]
    fn product_with_unknown_unit_id_is_an_error() {
        let mut table = UnitTable::new();
        let foreign = UnitId(u32::MAX - 1);

        let result = table.multiply(table.dimensionless(), foreign);

        assert_eq!(result, Err(UnitProductError::UnknownUnitId(foreign)));
    }

    #[test]
    fn product_exponent_past_i8_range_is_an_error() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");
        let high = table.power(metre, i8::MAX).unwrap();

        let result = table.multiply(high, metre);

        assert_eq!(result, Err(UnitProductError::ExponentOutOfRange));
    }

    #[test]
    fn power_exponent_past_i8_range_is_an_error() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");
        let high = table.power(metre, 100).unwrap();

        let result = table.power(high, 2);

        assert_eq!(result, Err(UnitProductError::ExponentOutOfRange));
    }

    #[test]
    fn dimension_exponent_past_i8_range_is_an_error_even_when_factor_exponents_fit() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");
        let kilometre = unit(&mut table, "km");
        let high = table.power(metre, i8::MAX).unwrap();

        let result = table.multiply(high, kilometre);

        assert_eq!(result, Err(UnitProductError::ExponentOutOfRange));
    }

    #[test]
    fn product_past_pi_exponent_range_is_an_error() {
        let mut table = UnitTable::new();
        let pi_scale = ScaleFactor::with_pi_exponent(1, 1, i32::MAX);
        table.register_test_unit("high", Dimension::DIMENSIONLESS, pi_scale);
        let high = unit(&mut table, "high");

        let result = table.multiply(high, high);

        assert_eq!(result, Err(UnitProductError::PiExponentOutOfRange));
    }

    #[test]
    fn product_on_full_table_is_an_error() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");
        table.entry_limit = u32::try_from(table.units.len()).unwrap();

        let result = table.power(metre, 2);

        assert_eq!(result, Err(UnitProductError::TableFull));
    }

    #[test]
    fn lookup_of_new_unit_on_full_table_is_an_error() {
        let mut table = UnitTable::new();
        table.entry_limit = u32::try_from(table.named_units.len()).unwrap();

        let result = table.lookup("km");

        assert_eq!(result, Err(UnitLookupError::TableFull));
    }

    #[test]
    fn existing_unit_is_found_on_full_table() {
        let mut table = UnitTable::new();
        let metre = unit(&mut table, "m");
        table.entry_limit = 0;

        let result = table.lookup("m");

        assert_eq!(result, Ok(metre));
    }

    #[test]
    fn scale_factor_numerator_of_quettametre_is_ten_to_thirty() {
        let mut table = UnitTable::new();
        let quettametre = unit(&mut table, "Qm");

        let factor = table.scale_factor(quettametre).unwrap();

        assert_eq!(factor.numerator(), &Integer::from(10_u64).pow(30));
    }
    fn factor_between(table: &mut UnitTable, from: &str, to: &str) -> ScaleFactor {
        let from = unit(table, from);
        let to = unit(table, to);
        table.conversion_factor(from, to).unwrap()
    }

    #[test]
    fn minute_is_sixty_seconds() {
        let mut table = UnitTable::new();

        let factor = factor_between(&mut table, "min", "s");

        assert_eq!(factor, ratio(60, 1));
    }

    #[test]
    fn hour_is_sixty_minutes() {
        let mut table = UnitTable::new();

        let factor = factor_between(&mut table, "h", "min");

        assert_eq!(factor, ratio(60, 1));
    }

    #[test]
    fn day_is_twenty_four_hours() {
        let mut table = UnitTable::new();

        let factor = factor_between(&mut table, "d", "h");

        assert_eq!(factor, ratio(24, 1));
    }

    #[test]
    fn litre_is_one_cubic_decimetre() {
        let mut table = UnitTable::new();
        let litre = unit(&mut table, "L");
        let decimetre = unit(&mut table, "dm");
        let cubic_decimetre = table.power(decimetre, 3).unwrap();

        let factor = table.conversion_factor(litre, cubic_decimetre).unwrap();

        assert!(factor.is_one());
    }

    #[test]
    fn lower_case_l_is_the_litre() {
        let mut table = UnitTable::new();

        let lower = unit(&mut table, "l");
        let upper = unit(&mut table, "L");

        assert_eq!(lower, upper);
    }

    #[test]
    fn millilitre_is_one_cubic_centimetre() {
        let mut table = UnitTable::new();
        let millilitre = unit(&mut table, "mL");
        let centimetre = unit(&mut table, "cm");
        let cubic_centimetre = table.power(centimetre, 3).unwrap();

        let factor = table
            .conversion_factor(millilitre, cubic_centimetre)
            .unwrap();

        assert!(factor.is_one());
    }

    #[test]
    fn prefix_on_lower_case_litre_gives_the_prefixed_litre() {
        let mut table = UnitTable::new();

        let lower = unit(&mut table, "ml");
        let upper = unit(&mut table, "mL");

        assert_eq!(lower, upper);
    }

    #[test]
    fn tonne_is_one_megagram() {
        let mut table = UnitTable::new();

        let factor = factor_between(&mut table, "t", "Mg");

        assert!(factor.is_one());
    }

    #[test]
    fn inch_is_two_point_five_four_centimetres() {
        let mut table = UnitTable::new();

        let factor = factor_between(&mut table, "in", "cm");

        assert_eq!(factor, ratio(254, 100));
    }

    #[test]
    fn foot_is_twelve_inches() {
        let mut table = UnitTable::new();

        let factor = factor_between(&mut table, "ft", "in");

        assert_eq!(factor, ratio(12, 1));
    }

    #[test]
    fn yard_is_three_feet() {
        let mut table = UnitTable::new();

        let factor = factor_between(&mut table, "yd", "ft");

        assert_eq!(factor, ratio(3, 1));
    }

    #[test]
    fn mile_is_one_thousand_seven_hundred_sixty_yards() {
        let mut table = UnitTable::new();

        let factor = factor_between(&mut table, "mi", "yd");

        assert_eq!(factor, ratio(1760, 1));
    }

    #[test]
    fn pound_is_four_hundred_fifty_three_point_five_nine_two_three_seven_grams() {
        let mut table = UnitTable::new();

        let factor = factor_between(&mut table, "lb", "g");

        assert_eq!(factor, ratio(45_359_237, 100_000));
    }

    #[test]
    fn nine_degrees_fahrenheit_are_five_kelvin() {
        let mut table = UnitTable::new();

        let nine = factor_between(&mut table, "degF", "K")
            .multiply(&ratio(9, 1))
            .unwrap();

        assert_eq!(nine, ratio(5, 1));
    }

    #[test]
    fn degree_fahrenheit_difference_has_the_dimension_of_temperature() {
        let mut table = UnitTable::new();
        let fahrenheit = unit(&mut table, "degF");

        let exponents = exponents(&table, fahrenheit);

        assert_eq!(exponents, [0, 0, 0, 0, 1, 0, 0, 0]);
    }

    #[test]
    fn units_outside_the_si_except_the_litre_take_no_prefix() {
        let mut table = UnitTable::new();
        let names = [
            "kmin", "mh", "kd", "kt", "kin", "kft", "kyd", "kmi", "klb", "mdegF",
        ];

        let results: Vec<_> = names.iter().map(|name| table.lookup(name)).collect();

        assert_eq!(results, vec![Err(UnitLookupError::UnknownName); 10]);
    }

    #[test]
    fn minute_is_not_milli_inch() {
        let mut table = UnitTable::new();
        let minute = unit(&mut table, "min");

        let exponents = exponents(&table, minute);

        assert_eq!(exponents, [0, 0, 1, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn display_symbols_differ_only_for_the_degrees() {
        let mut table = UnitTable::new();
        let names = ["degC", "degF", "deg", "min", "L", "l", "\u{2126}", "km"];

        let symbols: Vec<String> = names
            .iter()
            .map(|name| {
                let id = unit(&mut table, name);
                let factor = table.factors(id).unwrap()[0];
                table
                    .display_symbol(factor.named_unit())
                    .unwrap()
                    .to_string()
            })
            .collect();

        assert_eq!(
            symbols,
            [
                "\u{00B0}C",
                "\u{00B0}F",
                "\u{00B0}",
                "min",
                "L",
                "L",
                "\u{03A9}",
                "km"
            ]
        );
    }

    #[test]
    fn display_symbol_of_prefixed_unit_carries_the_prefix_symbol() {
        let mut table = UnitTable::new();
        let millilitre = unit(&mut table, "\u{03BC}l");
        let factor = table.factors(millilitre).unwrap()[0];

        let symbol = table.display_symbol(factor.named_unit());

        assert_eq!(symbol, Ok("\u{00B5}L"));
    }

    #[test]
    fn display_symbol_with_unknown_named_unit_id_is_an_error() {
        let table = UnitTable::new();

        let result = table.display_symbol(NamedUnitId(u32::MAX));

        assert_eq!(
            result,
            Err(UnitAccessError::UnknownNamedUnitId(NamedUnitId(u32::MAX)))
        );
    }

    fn family_of(table: &mut UnitTable, symbol: &str) -> String {
        let unit = table.lookup(symbol).unwrap();
        let named = table.factors(unit).unwrap()[0].named_unit();
        let family = table.prefix_family(named).unwrap();
        table.symbol(family).unwrap().to_string()
    }

    #[test]
    fn kilometre_belongs_to_the_metre_family() {
        assert_eq!(family_of(&mut UnitTable::new(), "km"), "m");
    }

    #[test]
    fn unprefixed_metre_is_its_own_family() {
        assert_eq!(family_of(&mut UnitTable::new(), "m"), "m");
    }

    #[test]
    fn kilogram_belongs_to_the_gram_family() {
        assert_eq!(family_of(&mut UnitTable::new(), "kg"), "g");
    }
}
