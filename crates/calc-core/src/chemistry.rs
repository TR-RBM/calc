use std::cmp::Ordering;

use calc_expr::{COEFFICIENT_NOT_WRITTEN, ExprId, ExprPool, Head, NodeView, Operator};
use calc_numbers::{Integer, Number};
use calc_units::UnitId;

use crate::atomic_weights_2024::STANDARD_ATOMIC_WEIGHTS_2024;
use crate::exact_rational::ExactRational;

const GRAM: &str = "g";
const MOLE: &str = "mol";
const PRODUCT_SIDE: i64 = 1;

fn small_integer(pool: &ExprPool, expression: ExprId) -> Option<i64> {
    let NodeView::Number(number) = pool.node(expression).ok()? else {
        return None;
    };
    match pool.number_value(number).ok()? {
        Number::Integer(integer) => integer.to_i64(),
        _ => None,
    }
}

fn row(pool: &ExprPool, expression: ExprId) -> Option<Vec<ExprId>> {
    match pool.node(expression).ok()? {
        NodeView::Array { elements, .. } => Some(elements.to_vec()),
        _ => None,
    }
}

fn integers(pool: &ExprPool, expression: ExprId) -> Option<Vec<i64>> {
    row(pool, expression)?
        .iter()
        .map(|element| small_integer(pool, *element))
        .collect()
}

pub fn composition_of(pool: &ExprPool, expression: ExprId) -> Option<Composition> {
    let NodeView::Apply {
        head: Head::Operator(Operator::Substance),
        arguments: [_, composition],
    } = pool.node(expression).ok()?
    else {
        return None;
    };
    let values = integers(pool, *composition)?;
    let (charge, pairs) = values.split_first()?;
    let (pairs, rest) = pairs.as_chunks::<2>();
    if !rest.is_empty() {
        return None;
    }
    let elements = pairs
        .iter()
        .map(|[element, count]| Some((u8::try_from(*element).ok()?, u64::try_from(*count).ok()?)))
        .collect::<Option<Vec<_>>>()?;
    Some(Composition {
        elements,
        charge: *charge,
        ..Composition::default()
    })
}

pub fn reaction_of(pool: &ExprPool, expression: ExprId) -> Option<Vec<ReactionSpecies>> {
    let NodeView::Apply {
        head: Head::Operator(Operator::Reaction),
        arguments: [_, species, sides, coefficients],
    } = pool.node(expression).ok()?
    else {
        return None;
    };
    let nodes = row(pool, *species)?;
    let sides = integers(pool, *sides)?;
    let coefficients = integers(pool, *coefficients)?;
    nodes
        .iter()
        .zip(sides.iter().zip(&coefficients))
        .map(|(node, (side, coefficient))| {
            Some(ReactionSpecies {
                composition: composition_of(pool, *node)?,
                is_product: *side == PRODUCT_SIDE,
                written: if *coefficient == COEFFICIENT_NOT_WRITTEN {
                    None
                } else {
                    Some(u64::try_from(*coefficient).ok()?)
                },
            })
        })
        .collect()
}

pub fn written_text(pool: &ExprPool, expression: ExprId) -> Option<String> {
    let NodeView::Apply {
        head:
            Head::Operator(
                Operator::Substance
                | Operator::Reaction
                | Operator::Nuclide
                | Operator::NuclearReaction,
            ),
        arguments,
    } = pool.node(expression).ok()?
    else {
        return None;
    };
    let codes = integers(pool, *arguments.first()?)?;
    codes
        .iter()
        .map(|code| u32::try_from(*code).ok().and_then(char::from_u32))
        .collect()
}

pub fn species_nodes(pool: &ExprPool, expression: ExprId) -> Option<Vec<ExprId>> {
    let NodeView::Apply {
        head: Head::Operator(Operator::Reaction | Operator::NuclearReaction),
        arguments: [_, species, _, _],
    } = pool.node(expression).ok()?
    else {
        return None;
    };
    row(pool, *species)
}

pub fn molar_mass_unit(pool: &mut ExprPool) -> Option<UnitId> {
    let units = pool.units_mut();
    let gram = units.lookup(GRAM).ok()?;
    let mole = units.lookup(MOLE).ok()?;
    let per_mole = units.power(mole, -1).ok()?;
    units.multiply(gram, per_mole).ok()
}

pub const ATOMIC_WEIGHT_TABLE: &str = "CIAAW 2024";

pub const MOLAR_MASS_CONSTANT_TABLE: &str = "CODATA 2022";

pub const EXTREME_RAY_SPECIES_LIMIT: usize = 16;

const MOLAR_MASS_CONSTANT_DIGITS: u64 = 100_000_000_105;
const MOLAR_MASS_CONSTANT_UNCERTAINTY_DIGITS: u64 = 31;
const MOLAR_MASS_CONSTANT_PLACES: u32 = 11;
const MOLAR_MASS_CONSTANT_COVERAGE: u64 = 2;
const DECIMAL_BASE: u64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomicWeightEntry {
    Interval {
        low: u64,
        high: u64,
        places: u32,
    },
    Expanded {
        value: u64,
        uncertainty: u64,
        places: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomicWeightKind {
    NaturalInterval,
    ExpandedUncertainty,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtomicWeight {
    pub kind: AtomicWeightKind,
    pub low: Number,
    pub high: Number,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Composition {
    pub elements: Vec<(u8, u64)>,
    pub charge: i64,
    pub nucleons: i64,
    pub electron_leptons: i64,
}

impl Composition {
    pub fn count_of(&self, atomic_number: u8) -> u64 {
        self.elements
            .iter()
            .filter(|(element, _)| *element == atomic_number)
            .map(|(_, count)| *count)
            .sum()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MolarMassRange {
    pub low: Number,
    pub high: Number,
    pub natural_interval: Vec<u8>,
    pub expanded_uncertainty: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MolarMassRefusal {
    NoStandardAtomicWeight(u8),
    Charged,
    NoElements,
}

fn decimal(digits: u64, places: u32) -> ExactRational {
    let mut denominator = Integer::one();
    for _ in 0..places {
        denominator = &denominator * &Integer::from(DECIMAL_BASE);
    }
    ExactRational::fraction(&Integer::from(digits), &denominator)
        .unwrap_or_else(ExactRational::zero)
}

pub fn standard_atomic_weight(atomic_number: u8) -> Option<AtomicWeight> {
    let (_, entry) = STANDARD_ATOMIC_WEIGHTS_2024
        .iter()
        .find(|(element, _)| *element == atomic_number)?;
    Some(match *entry {
        AtomicWeightEntry::Interval { low, high, places } => AtomicWeight {
            kind: AtomicWeightKind::NaturalInterval,
            low: decimal(low, places).to_number(),
            high: decimal(high, places).to_number(),
        },
        AtomicWeightEntry::Expanded {
            value,
            uncertainty,
            places,
        } => {
            let value = decimal(value, places);
            let uncertainty = decimal(uncertainty, places);
            AtomicWeight {
                kind: AtomicWeightKind::ExpandedUncertainty,
                low: value.subtract(&uncertainty).to_number(),
                high: value.plus(&uncertainty).to_number(),
            }
        }
    })
}

fn molar_mass_constant() -> (ExactRational, ExactRational) {
    let value = decimal(MOLAR_MASS_CONSTANT_DIGITS, MOLAR_MASS_CONSTANT_PLACES);
    let half_width = decimal(
        MOLAR_MASS_CONSTANT_UNCERTAINTY_DIGITS * MOLAR_MASS_CONSTANT_COVERAGE,
        MOLAR_MASS_CONSTANT_PLACES,
    );
    (value.subtract(&half_width), value.plus(&half_width))
}

pub fn molar_mass(composition: &Composition) -> Result<MolarMassRange, MolarMassRefusal> {
    if composition.charge != 0 {
        return Err(MolarMassRefusal::Charged);
    }
    if composition.elements.is_empty() {
        return Err(MolarMassRefusal::NoElements);
    }
    let mut low = ExactRational::zero();
    let mut high = ExactRational::zero();
    let mut natural_interval = Vec::new();
    let mut expanded_uncertainty = Vec::new();
    for (element, count) in &composition.elements {
        let weight = standard_atomic_weight(*element)
            .ok_or(MolarMassRefusal::NoStandardAtomicWeight(*element))?;
        let count = ExactRational::from_integer(Integer::from(*count));
        let (Some(weight_low), Some(weight_high)) = (
            ExactRational::from_number(&weight.low),
            ExactRational::from_number(&weight.high),
        ) else {
            return Err(MolarMassRefusal::NoStandardAtomicWeight(*element));
        };
        low = low.plus(&weight_low.multiply(&count));
        high = high.plus(&weight_high.multiply(&count));
        let kinds = match weight.kind {
            AtomicWeightKind::NaturalInterval => &mut natural_interval,
            AtomicWeightKind::ExpandedUncertainty => &mut expanded_uncertainty,
        };
        if !kinds.contains(element) {
            kinds.push(*element);
        }
    }
    let (constant_low, constant_high) = molar_mass_constant();
    Ok(MolarMassRange {
        low: low.multiply(&constant_low).to_number(),
        high: high.multiply(&constant_high).to_number(),
        natural_interval,
        expanded_uncertainty,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConservedQuantity {
    Element(u8),
    Charge,
    NucleonNumber,
    ElectronLeptonNumber,
}

impl ConservedQuantity {
    pub fn amount_in(self, composition: &Composition) -> Integer {
        match self {
            ConservedQuantity::Element(element) => Integer::from(composition.count_of(element)),
            ConservedQuantity::Charge => Integer::from(composition.charge),
            ConservedQuantity::NucleonNumber => Integer::from(composition.nucleons),
            ConservedQuantity::ElectronLeptonNumber => Integer::from(composition.electron_leptons),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReactionSpecies {
    pub composition: Composition,
    pub is_product: bool,
    pub written: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideTotals {
    pub quantity: ConservedQuantity,
    pub reactants: Integer,
    pub products: Integer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Balancing {
    Balanced {
        coefficients: Vec<Integer>,
        totals: Vec<SideTotals>,
    },
    WrittenBalances {
        totals: Vec<SideTotals>,
    },
    WrittenFails {
        first: SideTotals,
    },
    NotUnique {
        independent: usize,
        reactions: Vec<Vec<Integer>>,
    },
    Impossible,
    TakesNoPart(usize),
    OtherSide(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BalanceRefusal {
    TooManySpeciesForRays { limit: usize, independent: usize },
    NoSpecies,
}

pub fn chemical_quantities(species: &[ReactionSpecies]) -> Vec<ConservedQuantity> {
    let mut quantities: Vec<ConservedQuantity> = Vec::new();
    for one in species {
        for (element, _) in &one.composition.elements {
            let quantity = ConservedQuantity::Element(*element);
            if !quantities.contains(&quantity) {
                quantities.push(quantity);
            }
        }
    }
    quantities.sort();
    if species.iter().any(|one| one.composition.charge != 0) {
        quantities.push(ConservedQuantity::Charge);
    }
    quantities
}

fn signed_amount(quantity: ConservedQuantity, species: &ReactionSpecies) -> ExactRational {
    let amount = ExactRational::from_integer(quantity.amount_in(&species.composition));
    if species.is_product {
        amount.negated()
    } else {
        amount
    }
}

fn kernel(rows: &[Vec<ExactRational>], columns: usize) -> Vec<Vec<ExactRational>> {
    let mut matrix: Vec<Vec<ExactRational>> = rows.to_vec();
    let mut pivots: Vec<usize> = Vec::new();
    let mut row = 0;
    for column in 0..columns {
        let Some(found) = (row..matrix.len()).find(|index| !matrix[*index][column].is_zero())
        else {
            continue;
        };
        matrix.swap(row, found);
        let Some(inverse) = matrix[row][column].reciprocal() else {
            continue;
        };
        for entry in &mut matrix[row] {
            *entry = entry.multiply(&inverse);
        }
        for other in 0..matrix.len() {
            if other != row && !matrix[other][column].is_zero() {
                let factor = matrix[other][column].clone();
                let pivot_row = matrix[row].clone();
                for (entry, pivot_entry) in matrix[other].iter_mut().zip(&pivot_row) {
                    *entry = entry.subtract(&factor.multiply(pivot_entry));
                }
            }
        }
        pivots.push(column);
        row += 1;
        if row == matrix.len() {
            break;
        }
    }
    (0..columns)
        .filter(|column| !pivots.contains(column))
        .map(|free| {
            let mut vector = vec![ExactRational::zero(); columns];
            vector[free] = ExactRational::one();
            for (pivot_row, pivot_column) in pivots.iter().enumerate() {
                vector[*pivot_column] = matrix[pivot_row][free].negated();
            }
            vector
        })
        .collect()
}

fn lcm(left: &Integer, right: &Integer) -> Integer {
    let divisor = left.gcd(right);
    let product = (left * right).absolute();
    product
        .div_rem_euclid(&divisor)
        .map(|(quotient, _)| quotient)
        .unwrap_or(product)
}

fn primitive(vector: &[ExactRational]) -> Vec<Integer> {
    let multiple = vector.iter().fold(Integer::one(), |multiple, entry| {
        lcm(&multiple, entry.denominator())
    });
    let scaled: Vec<Integer> = vector
        .iter()
        .map(|entry| {
            entry
                .multiply(&ExactRational::from_integer(multiple.clone()))
                .numerator()
                .clone()
        })
        .collect();
    let divisor = scaled
        .iter()
        .fold(Integer::zero(), |divisor, entry| divisor.gcd(entry));
    if divisor.is_zero() {
        return scaled;
    }
    scaled
        .iter()
        .map(|entry| {
            entry
                .div_rem_euclid(&divisor)
                .map(|(quotient, _)| quotient)
                .unwrap_or_else(|_| entry.clone())
        })
        .collect()
}

fn totals(
    quantities: &[ConservedQuantity],
    species: &[ReactionSpecies],
    coefficients: &[Integer],
) -> Vec<SideTotals> {
    quantities
        .iter()
        .map(|quantity| {
            let mut reactants = Integer::zero();
            let mut products = Integer::zero();
            for (one, coefficient) in species.iter().zip(coefficients) {
                let amount = coefficient * &quantity.amount_in(&one.composition);
                if one.is_product {
                    products = &products + &amount;
                } else {
                    reactants = &reactants + &amount;
                }
            }
            SideTotals {
                quantity: *quantity,
                reactants,
                products,
            }
        })
        .collect()
}

fn rows_for(
    quantities: &[ConservedQuantity],
    species: &[ReactionSpecies],
    columns: &[usize],
) -> Vec<Vec<ExactRational>> {
    quantities
        .iter()
        .map(|quantity| {
            columns
                .iter()
                .map(|column| signed_amount(*quantity, &species[*column]))
                .collect()
        })
        .collect()
}

fn extreme_reactions(
    quantities: &[ConservedQuantity],
    species: &[ReactionSpecies],
) -> Vec<Vec<Integer>> {
    let count = species.len();
    let mut found: Vec<(u32, Vec<Integer>)> = Vec::new();
    let mut supports: Vec<u32> = (1..(1_u32 << count)).collect();
    supports.sort_by_key(|support| support.count_ones());
    for support in supports {
        if found
            .iter()
            .any(|(smaller, _)| smaller & support == *smaller)
        {
            continue;
        }
        let columns: Vec<usize> = (0..count)
            .filter(|column| support & (1 << column) != 0)
            .collect();
        let basis = kernel(&rows_for(quantities, species, &columns), columns.len());
        let [vector] = basis.as_slice() else {
            continue;
        };
        let signs: Vec<Ordering> = vector.iter().map(ExactRational::sign).collect();
        let all_positive = signs.iter().all(|sign| *sign == Ordering::Greater);
        let all_negative = signs.iter().all(|sign| *sign == Ordering::Less);
        if !all_positive && !all_negative {
            continue;
        }
        let local = primitive(vector);
        let mut reaction = vec![Integer::zero(); count];
        for (column, value) in columns.iter().zip(local) {
            reaction[*column] = if all_negative { value.negated() } else { value };
        }
        found.push((support, reaction));
    }
    found.into_iter().map(|(_, reaction)| reaction).collect()
}

pub fn balance(
    species: &[ReactionSpecies],
    quantities: &[ConservedQuantity],
) -> Result<Balancing, BalanceRefusal> {
    if species.is_empty() {
        return Err(BalanceRefusal::NoSpecies);
    }
    let written: Vec<Option<u64>> = species.iter().map(|one| one.written).collect();
    if written.iter().any(Option::is_some) {
        let coefficients: Vec<Integer> = written
            .iter()
            .map(|coefficient| Integer::from(coefficient.unwrap_or(1)))
            .collect();
        let all = totals(quantities, species, &coefficients);
        return Ok(
            match all.iter().find(|side| side.reactants != side.products) {
                Some(first) => Balancing::WrittenFails {
                    first: first.clone(),
                },
                None => Balancing::WrittenBalances { totals: all },
            },
        );
    }
    let columns: Vec<usize> = (0..species.len()).collect();
    let basis = kernel(&rows_for(quantities, species, &columns), species.len());
    match basis.as_slice() {
        [] => Ok(Balancing::Impossible),
        [vector] => {
            let mut coefficients = primitive(vector);
            let positive = coefficients
                .iter()
                .filter(|value| !value.is_negative() && !value.is_zero())
                .count();
            let negative = coefficients
                .iter()
                .filter(|value| value.is_negative())
                .count();
            if negative > positive {
                coefficients = coefficients.iter().map(Integer::negated).collect();
            }
            if let Some(index) = coefficients.iter().position(Integer::is_zero) {
                return Ok(Balancing::TakesNoPart(index));
            }
            if let Some(index) = coefficients.iter().position(Integer::is_negative) {
                return Ok(Balancing::OtherSide(index));
            }
            Ok(Balancing::Balanced {
                totals: totals(quantities, species, &coefficients),
                coefficients,
            })
        }
        several => {
            if species.len() > EXTREME_RAY_SPECIES_LIMIT {
                return Err(BalanceRefusal::TooManySpeciesForRays {
                    limit: EXTREME_RAY_SPECIES_LIMIT,
                    independent: several.len(),
                });
            }
            Ok(Balancing::NotUnique {
                independent: several.len(),
                reactions: extreme_reactions(quantities, species),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: u8 = 1;
    const C: u8 = 6;
    const O: u8 = 8;
    const NA: u8 = 11;
    const S: u8 = 16;
    const CL: u8 = 17;
    const K: u8 = 19;
    const MN: u8 = 25;
    const FE: u8 = 26;

    fn species(elements: &[(u8, u64)], charge: i64, is_product: bool) -> ReactionSpecies {
        ReactionSpecies {
            composition: Composition {
                elements: elements.to_vec(),
                charge,
                ..Composition::default()
            },
            is_product,
            written: None,
        }
    }

    fn balanced(reaction: &[ReactionSpecies]) -> Balancing {
        balance(reaction, &chemical_quantities(reaction)).unwrap()
    }

    fn integers(values: &[i64]) -> Vec<Integer> {
        values.iter().map(|value| Integer::from(*value)).collect()
    }

    fn coefficients_of(balancing: Balancing) -> Vec<Integer> {
        match balancing {
            Balancing::Balanced { coefficients, .. } => coefficients,
            other => panic!("not balanced: {other:?}"),
        }
    }

    fn fraction(text: &str) -> ExactRational {
        let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
        let digits: u64 = format!("{whole}{fraction}").parse().unwrap();
        decimal(digits, u32::try_from(fraction.len()).unwrap())
    }

    #[test]
    fn propane_burns_as_one_five_three_four() {
        let reaction = [
            species(&[(C, 3), (H, 8)], 0, false),
            species(&[(O, 2)], 0, false),
            species(&[(C, 1), (O, 2)], 0, true),
            species(&[(H, 2), (O, 1)], 0, true),
        ];
        assert_eq!(
            coefficients_of(balanced(&reaction)),
            integers(&[1, 5, 3, 4])
        );
    }

    #[test]
    fn potassium_permanganate_and_hydrochloric_acid_balance_with_sixteen() {
        let reaction = [
            species(&[(K, 1), (MN, 1), (O, 4)], 0, false),
            species(&[(H, 1), (CL, 1)], 0, false),
            species(&[(K, 1), (CL, 1)], 0, true),
            species(&[(MN, 1), (CL, 2)], 0, true),
            species(&[(CL, 2)], 0, true),
            species(&[(H, 2), (O, 1)], 0, true),
        ];
        assert_eq!(
            coefficients_of(balanced(&reaction)),
            integers(&[2, 16, 2, 2, 5, 8])
        );
    }

    #[test]
    fn a_redox_between_ions_balances_through_the_charge_row() {
        let reaction = [
            species(&[(MN, 1), (O, 4)], -1, false),
            species(&[(FE, 1)], 2, false),
            species(&[(H, 1)], 1, false),
            species(&[(MN, 1)], 2, true),
            species(&[(FE, 1)], 3, true),
            species(&[(H, 2), (O, 1)], 0, true),
        ];
        assert_eq!(
            coefficients_of(balanced(&reaction)),
            integers(&[1, 5, 8, 1, 5, 4])
        );
    }

    #[test]
    fn the_same_ions_without_their_charge_balance_have_no_reaction() {
        let reaction = [
            species(&[(MN, 1), (O, 4)], -1, false),
            species(&[(FE, 1)], 2, false),
            species(&[(MN, 1)], 2, true),
            species(&[(FE, 1)], 3, true),
        ];
        assert_eq!(balanced(&reaction), Balancing::Impossible);
    }

    #[test]
    fn iron_sulfide_and_sulfuric_acid_cannot_be_balanced() {
        let reaction = [
            species(&[(FE, 1), (S, 1)], 0, false),
            species(&[(H, 2), (S, 1), (O, 4)], 0, false),
            species(&[(FE, 1), (S, 1), (O, 4)], 0, true),
            species(&[(H, 2), (O, 1)], 0, true),
        ];
        assert_eq!(balanced(&reaction), Balancing::Impossible);
    }

    #[test]
    fn a_species_every_balance_leaves_out_is_named() {
        let reaction = [
            species(&[(NA, 1), (CL, 1)], 0, false),
            species(&[(H, 2), (O, 1)], 0, false),
            species(&[(NA, 1), (O, 1), (H, 1)], 0, true),
            species(&[(H, 1), (CL, 1)], 0, true),
            species(&[(O, 2)], 0, true),
        ];
        assert_eq!(balanced(&reaction), Balancing::TakesNoPart(4));
    }

    #[test]
    fn a_species_on_the_wrong_side_is_named() {
        let reaction = [
            species(&[(C, 3), (H, 8)], 0, false),
            species(&[(O, 2)], 0, false),
            species(&[(H, 2), (O, 1)], 0, false),
            species(&[(C, 1), (O, 2)], 0, true),
        ];
        assert_eq!(balanced(&reaction), Balancing::OtherSide(2));
    }

    #[test]
    fn chlorate_and_chloride_allow_two_reactions_and_both_smallest_are_listed() {
        let reaction = [
            species(&[(CL, 1), (O, 3)], -1, false),
            species(&[(CL, 1)], -1, false),
            species(&[(H, 1)], 1, false),
            species(&[(CL, 1), (O, 2)], 0, true),
            species(&[(CL, 2)], 0, true),
            species(&[(H, 2), (O, 1)], 0, true),
        ];
        let Balancing::NotUnique {
            independent,
            mut reactions,
        } = balanced(&reaction)
        else {
            panic!("expected not unique");
        };
        reactions.sort();
        let mut expected = vec![integers(&[1, 5, 6, 0, 3, 3]), integers(&[5, 1, 6, 6, 0, 3])];
        expected.sort();
        assert_eq!(independent, 2);
        assert_eq!(reactions, expected);
    }

    #[test]
    fn written_coefficients_are_checked_and_the_first_failing_quantity_named() {
        let mut reaction = [
            species(&[(C, 1), (H, 4)], 0, false),
            species(&[(O, 2)], 0, false),
            species(&[(C, 1), (O, 2)], 0, true),
            species(&[(H, 2), (O, 1)], 0, true),
        ];
        for (one, written) in reaction.iter_mut().zip([1, 2, 1, 2]) {
            one.written = Some(written);
        }
        assert!(matches!(
            balanced(&reaction),
            Balancing::WrittenBalances { .. }
        ));
        reaction[3].written = Some(1);
        let Balancing::WrittenFails { first } = balanced(&reaction) else {
            panic!("expected a failing check");
        };
        assert_eq!(first.quantity, ConservedQuantity::Element(H));
    }

    #[test]
    fn a_coefficient_left_out_beside_written_ones_counts_as_one() {
        let mut reaction = [
            species(&[(C, 1), (H, 4)], 0, false),
            species(&[(O, 2)], 0, false),
            species(&[(C, 1), (O, 2)], 0, true),
            species(&[(H, 2), (O, 1)], 0, true),
        ];
        reaction[1].written = Some(2);
        reaction[3].written = Some(2);
        assert!(matches!(
            balanced(&reaction),
            Balancing::WrittenBalances { .. }
        ));
    }

    #[test]
    fn water_has_the_molar_mass_of_its_standard_intervals_times_the_constant() {
        let range = molar_mass(&Composition {
            elements: vec![(H, 2), (O, 1)],
            charge: 0,
            ..Composition::default()
        })
        .unwrap();
        let (constant_low, constant_high) = molar_mass_constant();
        assert_eq!(
            range.low,
            fraction("18.01471").multiply(&constant_low).to_number()
        );
        assert_eq!(
            range.high,
            fraction("18.01599").multiply(&constant_high).to_number()
        );
        assert_eq!(range.natural_interval, vec![H, O]);
        assert!(range.expanded_uncertainty.is_empty());
    }

    #[test]
    fn the_molar_mass_of_sodium_contains_its_atomic_weight_times_the_measured_constant() {
        let range = molar_mass(&Composition {
            elements: vec![(NA, 1)],
            charge: 0,
            ..Composition::default()
        })
        .unwrap();
        let true_value = fraction("22.98976928").multiply(&fraction("1.00000000105"));
        assert_eq!(
            ExactRational::from_number(&range.low)
                .unwrap()
                .compare(&true_value),
            Ordering::Less
        );
        assert_eq!(
            ExactRational::from_number(&range.high)
                .unwrap()
                .compare(&true_value),
            Ordering::Greater
        );
        assert_eq!(range.expanded_uncertainty, vec![NA]);
    }

    #[test]
    fn an_element_without_a_standard_atomic_weight_is_refused() {
        assert_eq!(
            molar_mass(&Composition {
                elements: vec![(43, 1)],
                charge: 0,
                ..Composition::default()
            }),
            Err(MolarMassRefusal::NoStandardAtomicWeight(43))
        );
    }

    #[test]
    fn a_charged_species_has_no_molar_mass_here() {
        assert_eq!(
            molar_mass(&Composition {
                elements: vec![(NA, 1)],
                charge: 1,
                ..Composition::default()
            }),
            Err(MolarMassRefusal::Charged)
        );
    }

    #[test]
    fn thallium_is_an_interval_from_its_standard_weight_not_its_conventional_value() {
        let weight = standard_atomic_weight(81).unwrap();
        assert_eq!(weight.kind, AtomicWeightKind::NaturalInterval);
        assert_eq!(weight.low, fraction("204.382").to_number());
        assert_eq!(weight.high, fraction("204.385").to_number());
    }

    #[test]
    fn the_table_holds_eighty_four_elements() {
        assert_eq!(STANDARD_ATOMIC_WEIGHTS_2024.len(), 84);
        assert_eq!(
            STANDARD_ATOMIC_WEIGHTS_2024
                .iter()
                .filter(|(_, entry)| matches!(entry, AtomicWeightEntry::Interval { .. }))
                .count(),
            14
        );
    }
}
