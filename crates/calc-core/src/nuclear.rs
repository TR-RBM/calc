use std::cmp::Ordering;
use std::collections::BTreeMap;

use calc_expr::{
    COEFFICIENT_NOT_WRITTEN, ExprId, ExprPool, Head, NodeView, NuclearParticle, Operator,
};
use calc_numbers::{Integer, Number};
use calc_units::UnitId;

use crate::atomic_masses_2020::{ATOMIC_MASSES_2020, ESTIMATED_MASSES_2020};
use crate::chemistry::{
    BalanceRefusal, Balancing, Composition, ConservedQuantity, ReactionSpecies, SideTotals, balance,
};
use crate::exact_rational::ExactRational;

pub const ATOMIC_MASS_TABLE: &str = "AME2020";

pub const NUCLEAR_CONSTANT_TABLE: &str = "CODATA 2022";

pub const COVERAGE_FACTOR: u64 = 2;

const KILO_ELECTRON_VOLT: &str = "keV";
const ROUNDING_PADDING: u64 = 3;
const CARBON_TWELVE: (u16, u8) = (12, 6);
const MICRO_PLACES: u32 = 6;
const ATOMIC_MASS_ENERGY_DIGITS: u64 = 93_149_410_372;
const ATOMIC_MASS_ENERGY_UNCERTAINTY_DIGITS: u64 = 29;
const ATOMIC_MASS_ENERGY_PLACES: u32 = 5;
const ELECTRON_ENERGY_DIGITS: u64 = 51_099_895_069;
const ELECTRON_ENERGY_UNCERTAINTY_DIGITS: u64 = 16;
const ELECTRON_ENERGY_PLACES: u32 = 8;
const ELECTRONS_PER_POSITRON: u64 = 2;
const DECIMAL_BASE: u64 = 10;
const PRODUCT_SIDE: i64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtomicMassEntry {
    mass_number: u16,
    atomic_number: u8,
    value: u64,
    uncertainty: u64,
    places: u32,
}

impl AtomicMassEntry {
    pub(crate) const fn new(
        mass_number: u16,
        atomic_number: u8,
        value: u64,
        uncertainty: u64,
        places: u32,
    ) -> Self {
        Self {
            mass_number,
            atomic_number,
            value,
            uncertainty,
            places,
        }
    }

    fn key(&self) -> (u16, u8) {
        (self.mass_number, self.atomic_number)
    }

    fn rounding_unit(&self) -> ExactRational {
        if self.places > 0 {
            return decimal(1, self.places);
        }
        let mut unit = 1_u64;
        let mut rest = self.uncertainty;
        while rest != 0 && rest.is_multiple_of(DECIMAL_BASE) {
            rest /= DECIMAL_BASE;
            unit *= DECIMAL_BASE;
        }
        ExactRational::from_integer(Integer::from(unit))
    }

    fn range(&self) -> (ExactRational, ExactRational) {
        let value = decimal(self.value, self.places);
        if self.key() == CARBON_TWELVE {
            return (value.clone(), value);
        }
        let covered = decimal(self.uncertainty * COVERAGE_FACTOR, self.places);
        let padding = self
            .rounding_unit()
            .multiply(&ExactRational::from_integer(Integer::from(
                ROUNDING_PADDING,
            )));
        let half_width = covered.plus(&padding);
        (value.subtract(&half_width), value.plus(&half_width))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MassStatus {
    Measured,
    Estimated,
    NotInTable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NuclearSpecies {
    pub particle: NuclearParticle,
    pub is_product: bool,
    pub written: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NuclearBalancing {
    Checked {
        coefficients: Vec<Integer>,
        totals: Vec<SideTotals>,
    },
    Fails {
        first: SideTotals,
        only_balance: Option<Vec<Integer>>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QValueRefusal {
    NotANuclearReaction,
    DoesNotBalance,
    EstimatedMass(NuclearParticle),
    NoMass(NuclearParticle),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QValueRange {
    pub low: Number,
    pub high: Number,
    pub is_decay: bool,
    pub sign: Option<Ordering>,
}

fn decimal(digits: u64, places: u32) -> ExactRational {
    let mut denominator = Integer::one();
    for _ in 0..places {
        denominator = &denominator * &Integer::from(DECIMAL_BASE);
    }
    ExactRational::fraction(&Integer::from(digits), &denominator)
        .unwrap_or_else(ExactRational::zero)
}

fn measured(value: u64, uncertainty: u64, places: u32) -> (ExactRational, ExactRational) {
    let value = decimal(value, places);
    let half_width = decimal(uncertainty * COVERAGE_FACTOR, places);
    (value.subtract(&half_width), value.plus(&half_width))
}

fn table_key(particle: NuclearParticle) -> Option<(u16, u8)> {
    match particle {
        NuclearParticle::Nuclide {
            mass_number,
            atomic_number,
        } => Some((
            u16::try_from(mass_number).ok()?,
            u8::try_from(atomic_number).ok()?,
        )),
        _ => None,
    }
}

fn entry(particle: NuclearParticle) -> Option<&'static AtomicMassEntry> {
    let key = table_key(particle)?;
    ATOMIC_MASSES_2020
        .binary_search_by(|entry| entry.key().cmp(&key))
        .ok()
        .map(|index| &ATOMIC_MASSES_2020[index])
}

pub fn mass_status(particle: NuclearParticle) -> MassStatus {
    if entry(particle).is_some() {
        return MassStatus::Measured;
    }
    match table_key(particle) {
        Some(key) if ESTIMATED_MASSES_2020.binary_search(&key).is_ok() => MassStatus::Estimated,
        _ => MassStatus::NotInTable,
    }
}

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

pub fn nuclide_of(pool: &ExprPool, expression: ExprId) -> Option<NuclearParticle> {
    let NodeView::Apply {
        head: Head::Operator(Operator::Nuclide),
        arguments: [_, codes],
    } = pool.node(expression).ok()?
    else {
        return None;
    };
    NuclearParticle::from_codes(&integers(pool, *codes)?)
}

pub fn nuclear_reaction_of(pool: &ExprPool, expression: ExprId) -> Option<Vec<NuclearSpecies>> {
    let NodeView::Apply {
        head: Head::Operator(Operator::NuclearReaction),
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
            Some(NuclearSpecies {
                particle: nuclide_of(pool, *node)?,
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

pub fn nuclear_quantities() -> Vec<ConservedQuantity> {
    vec![
        ConservedQuantity::NucleonNumber,
        ConservedQuantity::Charge,
        ConservedQuantity::ElectronLeptonNumber,
    ]
}

fn composition(particle: NuclearParticle) -> Composition {
    Composition {
        charge: particle.charge(),
        nucleons: particle.nucleons(),
        electron_leptons: particle.electron_leptons(),
        ..Composition::default()
    }
}

fn expanded(
    kept: &[usize],
    local: &[Integer],
    photons: &[(usize, Integer)],
    count: usize,
) -> Vec<Integer> {
    let mut full = vec![Integer::zero(); count];
    for (index, coefficient) in kept.iter().zip(local) {
        full[*index] = coefficient.clone();
    }
    for (index, coefficient) in photons {
        full[*index] = coefficient.clone();
    }
    full
}

pub fn balance_nuclear(species: &[NuclearSpecies]) -> Result<NuclearBalancing, BalanceRefusal> {
    let any_written = species.iter().any(|one| one.written.is_some());
    let mut kept = Vec::new();
    let mut photons = Vec::new();
    let mut matter = Vec::new();
    for (index, one) in species.iter().enumerate() {
        if one.particle == NuclearParticle::Photon {
            photons.push((index, Integer::from(one.written.unwrap_or(1))));
            continue;
        }
        kept.push(index);
        matter.push(ReactionSpecies {
            composition: composition(one.particle),
            is_product: one.is_product,
            written: Some(one.written.unwrap_or(1)),
        });
    }
    let count = species.len();
    let full = |local: &[Integer]| expanded(&kept, local, &photons, count);
    let quantities = nuclear_quantities();
    let as_written: Vec<Integer> = matter
        .iter()
        .map(|one| Integer::from(one.written.unwrap_or(1)))
        .collect();
    let first = match balance(&matter, &quantities)? {
        Balancing::WrittenFails { first } => first,
        Balancing::WrittenBalances { totals } => {
            return Ok(NuclearBalancing::Checked {
                coefficients: full(&as_written),
                totals,
            });
        }
        _ => return Err(BalanceRefusal::NoSpecies),
    };
    let mut only_balance = None;
    if !any_written {
        let unknown: Vec<ReactionSpecies> = matter
            .iter()
            .map(|one| ReactionSpecies {
                written: None,
                ..one.clone()
            })
            .collect();
        if let Ok(Balancing::Balanced { coefficients, .. }) = balance(&unknown, &quantities) {
            only_balance = Some(full(&coefficients));
        }
    }
    Ok(NuclearBalancing::Fails {
        first,
        only_balance,
    })
}

fn scaled(value: &ExactRational, by: &Integer) -> ExactRational {
    value.multiply(&ExactRational::from_integer(by.clone()))
}

fn add_scaled(
    total: &mut (ExactRational, ExactRational),
    range: &(ExactRational, ExactRational),
    count: &Integer,
) {
    let (low, high) = if count.is_negative() {
        (scaled(&range.1, count), scaled(&range.0, count))
    } else {
        (scaled(&range.0, count), scaled(&range.1, count))
    };
    total.0 = total.0.plus(&low);
    total.1 = total.1.plus(&high);
}

fn product_range(
    left: &(ExactRational, ExactRational),
    right: &(ExactRational, ExactRational),
) -> (ExactRational, ExactRational) {
    let candidates = [
        left.0.multiply(&right.0),
        left.0.multiply(&right.1),
        left.1.multiply(&right.0),
        left.1.multiply(&right.1),
    ];
    let mut low = candidates[0].clone();
    let mut high = candidates[0].clone();
    for candidate in &candidates[1..] {
        if candidate.compare(&low) == Ordering::Less {
            low = candidate.clone();
        }
        if candidate.compare(&high) == Ordering::Greater {
            high = candidate.clone();
        }
    }
    (low, high)
}

fn is_decay(species: &[NuclearSpecies], coefficients: &[Integer]) -> bool {
    let reactants: Vec<(NuclearParticle, &Integer)> = species
        .iter()
        .zip(coefficients)
        .filter(|(one, coefficient)| !one.is_product && !coefficient.is_zero())
        .map(|(one, coefficient)| (one.particle, coefficient))
        .collect();
    let nuclides = reactants
        .iter()
        .filter(|(particle, _)| matches!(particle, NuclearParticle::Nuclide { .. }))
        .count();
    let electrons = reactants
        .iter()
        .filter(|(particle, _)| *particle == NuclearParticle::Electron)
        .count();
    let all_single = reactants
        .iter()
        .all(|(_, coefficient)| **coefficient == Integer::one());
    nuclides == 1 && electrons <= 1 && reactants.len() == nuclides + electrons && all_single
}

pub fn q_value(
    species: &[NuclearSpecies],
    coefficients: &[Integer],
) -> Result<QValueRange, QValueRefusal> {
    let mut net: BTreeMap<NuclearParticle, Integer> = BTreeMap::new();
    for (one, coefficient) in species.iter().zip(coefficients) {
        let signed = if one.is_product {
            coefficient.negated()
        } else {
            coefficient.clone()
        };
        let total = net.entry(one.particle).or_insert_with(Integer::zero);
        *total = &*total + &signed;
    }
    let zero = || (ExactRational::zero(), ExactRational::zero());
    let mut mass = zero();
    let mut positron_energy = zero();
    for (particle, count) in &net {
        if count.is_zero() {
            continue;
        }
        match particle {
            NuclearParticle::Nuclide { .. } => match entry(*particle) {
                Some(found) => add_scaled(&mut mass, &found.range(), count),
                None => {
                    return Err(match mass_status(*particle) {
                        MassStatus::Estimated => QValueRefusal::EstimatedMass(*particle),
                        _ => QValueRefusal::NoMass(*particle),
                    });
                }
            },
            NuclearParticle::Positron => {
                let electron = measured(
                    ELECTRON_ENERGY_DIGITS * ELECTRONS_PER_POSITRON,
                    ELECTRON_ENERGY_UNCERTAINTY_DIGITS * ELECTRONS_PER_POSITRON,
                    ELECTRON_ENERGY_PLACES,
                );
                add_scaled(&mut positron_energy, &electron, count);
            }
            _ => {}
        }
    }
    let per_micro_unit = measured(
        ATOMIC_MASS_ENERGY_DIGITS,
        ATOMIC_MASS_ENERGY_UNCERTAINTY_DIGITS,
        ATOMIC_MASS_ENERGY_PLACES + MICRO_PLACES,
    );
    let from_mass = product_range(&mass, &per_micro_unit);
    let low = from_mass.0.plus(&positron_energy.0);
    let high = from_mass.1.plus(&positron_energy.1);
    let sign = if high.sign() == Ordering::Less {
        Some(Ordering::Less)
    } else if low.sign() == Ordering::Greater {
        Some(Ordering::Greater)
    } else {
        None
    };
    Ok(QValueRange {
        low: low.to_number(),
        high: high.to_number(),
        is_decay: is_decay(species, coefficients),
        sign,
    })
}

pub fn q_value_unit(pool: &mut ExprPool) -> Option<UnitId> {
    pool.units_mut().lookup(KILO_ELECTRON_VOLT).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nuclide(mass_number: u32, atomic_number: u32) -> NuclearParticle {
        NuclearParticle::Nuclide {
            mass_number,
            atomic_number,
        }
    }

    fn species(particle: NuclearParticle, is_product: bool) -> NuclearSpecies {
        NuclearSpecies {
            particle,
            is_product,
            written: None,
        }
    }

    fn rational(value: &Number) -> ExactRational {
        ExactRational::from_number(value).unwrap()
    }

    fn exact(text: &str) -> ExactRational {
        let (is_negative, digits) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
        let places = u32::try_from(fraction.len()).unwrap();
        let value = decimal(format!("{whole}{fraction}").parse().unwrap(), places);
        if is_negative { value.negated() } else { value }
    }

    fn contains(range: &QValueRange, value: &str) -> bool {
        let value = exact(value);
        rational(&range.low).compare(&value) != Ordering::Greater
            && rational(&range.high).compare(&value) != Ordering::Less
    }

    fn solved(reaction: &[NuclearSpecies]) -> Vec<Integer> {
        match balance_nuclear(reaction) {
            Ok(NuclearBalancing::Checked { coefficients, .. }) => coefficients,
            other => panic!("not balanced: {other:?}"),
        }
    }

    #[test]
    fn the_table_holds_every_measured_mass_in_order() {
        assert_eq!(ATOMIC_MASSES_2020.len(), 2550);
        assert_eq!(ESTIMATED_MASSES_2020.len(), 1008);
        assert!(
            ATOMIC_MASSES_2020
                .windows(2)
                .all(|pair| pair[0].key() < pair[1].key())
        );
        assert!(
            ESTIMATED_MASSES_2020
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
    }

    #[test]
    fn carbon_twelve_is_exactly_twelve_units() {
        let (low, high) = entry(nuclide(12, 6)).unwrap().range();
        assert_eq!(
            low,
            ExactRational::from_integer(Integer::from(12_000_000_u64))
        );
        assert_eq!(high, low);
    }

    #[test]
    fn a_mass_range_is_two_uncertainties_and_three_rounding_units_wide() {
        let (low, high) = entry(nuclide(1, 1)).unwrap().range();
        assert_eq!(low, exact("1007825.03185"));
        assert_eq!(high, exact("1007825.03195"));
    }

    #[test]
    fn a_mass_known_only_by_estimate_is_told_apart() {
        assert_eq!(mass_status(nuclide(3, 3)), MassStatus::Estimated);
        assert_eq!(mass_status(nuclide(14, 6)), MassStatus::Measured);
        assert_eq!(mass_status(nuclide(400, 6)), MassStatus::NotInTable);
    }

    #[test]
    fn deuterium_and_tritium_fuse_to_about_seventeen_thousand_five_hundred_kev() {
        let reaction = [
            species(nuclide(2, 1), false),
            species(nuclide(3, 1), false),
            species(nuclide(4, 2), true),
            species(nuclide(1, 0), true),
        ];
        let found = q_value(&reaction, &solved(&reaction)).unwrap();
        assert!(contains(&found, "17589.2999"));
        assert!(!found.is_decay);
    }

    #[test]
    fn a_positron_costs_two_electron_masses() {
        let reaction = [
            species(nuclide(1, 1), false),
            NuclearSpecies {
                particle: nuclide(1, 1),
                is_product: false,
                written: None,
            },
            species(nuclide(2, 1), true),
            species(NuclearParticle::Positron, true),
            species(NuclearParticle::ElectronNeutrino, true),
        ];
        let coefficients = vec![Integer::one(); 5];
        let found = q_value(&reaction, &coefficients).unwrap();
        assert!(contains(&found, "420.22133"));
    }

    #[test]
    fn a_decay_below_zero_is_a_decay() {
        let reaction = [
            species(nuclide(4, 2), false),
            species(nuclide(3, 1), true),
            species(nuclide(1, 1), true),
        ];
        let found = q_value(&reaction, &solved(&reaction)).unwrap();
        assert!(found.is_decay);
        assert!(contains(&found, "-19813.866"));
        assert_eq!(rational(&found.high).sign(), Ordering::Less);
        assert_eq!(found.sign, Some(Ordering::Less));
    }

    #[test]
    fn a_decay_without_its_antineutrino_names_the_lepton_row() {
        let reaction = [
            species(nuclide(14, 6), false),
            species(nuclide(14, 7), true),
            species(NuclearParticle::Electron, true),
        ];
        match balance_nuclear(&reaction) {
            Ok(NuclearBalancing::Fails { first, .. }) => {
                assert_eq!(first.quantity, ConservedQuantity::ElectronLeptonNumber);
                assert_eq!(first.reactants, Integer::zero());
                assert_eq!(first.products, Integer::one());
            }
            other => panic!("not a lepton failure: {other:?}"),
        }
    }

    #[test]
    fn photons_are_counted_as_written_and_not_balanced() {
        let reaction = [
            species(NuclearParticle::Positron, false),
            species(NuclearParticle::Electron, false),
            NuclearSpecies {
                particle: NuclearParticle::Photon,
                is_product: true,
                written: Some(2),
            },
        ];
        match balance_nuclear(&reaction) {
            Ok(NuclearBalancing::Checked { .. }) => {}
            other => panic!("not checked: {other:?}"),
        }
        let coefficients = vec![Integer::one(), Integer::one(), Integer::from(2_i64)];
        let found = q_value(&reaction, &coefficients).unwrap();
        assert!(contains(&found, "1021.99790138"));
    }

    #[test]
    fn protons_to_helium_fail_as_written_and_name_their_only_balance() {
        let reaction = [
            species(nuclide(1, 1), false),
            species(nuclide(4, 2), true),
            species(NuclearParticle::Positron, true),
            species(NuclearParticle::ElectronNeutrino, true),
        ];
        match balance_nuclear(&reaction) {
            Ok(NuclearBalancing::Fails {
                first,
                only_balance: Some(coefficients),
            }) => {
                let expected: Vec<Integer> =
                    [4_i64, 1, 2, 2].into_iter().map(Integer::from).collect();
                assert_eq!(coefficients, expected);
                assert_eq!(first.quantity, ConservedQuantity::NucleonNumber);
            }
            other => panic!("not a failure with its balance: {other:?}"),
        }
    }

    #[test]
    fn a_line_that_balances_as_written_is_checked_even_where_the_kernel_is_wide() {
        let reaction = [
            species(nuclide(2, 1), false),
            species(nuclide(3, 1), false),
            species(nuclide(4, 2), true),
            species(nuclide(1, 0), true),
        ];
        assert!(matches!(
            balance_nuclear(&reaction),
            Ok(NuclearBalancing::Checked { .. })
        ));
    }

    #[test]
    fn a_neutron_on_both_sides_enters_once_with_its_net_coefficient() {
        let reaction = [
            species(nuclide(235, 92), false),
            species(nuclide(1, 0), false),
            species(nuclide(141, 56), true),
            species(nuclide(92, 36), true),
            NuclearSpecies {
                particle: nuclide(1, 0),
                is_product: true,
                written: Some(3),
            },
        ];
        let coefficients = vec![
            Integer::one(),
            Integer::one(),
            Integer::one(),
            Integer::one(),
            Integer::from(3_i64),
        ];
        let found = q_value(&reaction, &coefficients).unwrap();
        assert!(contains(&found, "173277.96688"));
    }
}
