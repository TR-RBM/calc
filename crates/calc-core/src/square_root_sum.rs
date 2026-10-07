use std::cmp::Ordering;
use std::collections::BTreeMap;

use calc_numbers::Integer;

use crate::exact_rational::ExactRational;
use crate::integer_roots::{UnsplitFactor, integer_root, square_free_decomposition};

const INITIAL_FLOOR_PRECISION_BITS: u32 = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SquareRootRefusal {
    Negative,
    UnsplitFactor(Integer),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Term {
    primes: Vec<Integer>,
    coefficient: ExactRational,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SquareRootSum {
    terms: BTreeMap<Integer, Term>,
}

fn product(primes: &[Integer]) -> Integer {
    primes
        .iter()
        .fold(Integer::one(), |accumulated, prime| &accumulated * prime)
}

fn merge_primes(left: &[Integer], right: &[Integer]) -> (Vec<Integer>, Vec<Integer>) {
    let mut shared = Vec::new();
    let mut unshared = Vec::new();
    let mut left_position = 0;
    let mut right_position = 0;
    while let (Some(left_prime), Some(right_prime)) =
        (left.get(left_position), right.get(right_position))
    {
        match left_prime.cmp(right_prime) {
            Ordering::Equal => {
                shared.push(left_prime.clone());
                left_position += 1;
                right_position += 1;
            }
            Ordering::Less => {
                unshared.push(left_prime.clone());
                left_position += 1;
            }
            Ordering::Greater => {
                unshared.push(right_prime.clone());
                right_position += 1;
            }
        }
    }
    unshared.extend(left.iter().skip(left_position).cloned());
    unshared.extend(right.iter().skip(right_position).cloned());
    unshared.sort();
    (shared, unshared)
}

impl SquareRootSum {
    pub fn zero() -> Self {
        Self {
            terms: BTreeMap::new(),
        }
    }

    pub fn from_rational(value: ExactRational) -> Self {
        let mut sum = Self::zero();
        sum.insert(Vec::new(), value);
        sum
    }

    pub fn square_root_of_rational(value: &ExactRational) -> Option<Self> {
        Self::real_square_root_of_rational(value).ok()
    }

    pub(crate) fn real_square_root_of_rational(
        value: &ExactRational,
    ) -> Result<Self, SquareRootRefusal> {
        match value.sign() {
            Ordering::Less => return Err(SquareRootRefusal::Negative),
            Ordering::Equal => return Ok(Self::zero()),
            Ordering::Greater => {}
        }
        let scaled = value.numerator() * value.denominator();
        let decomposition = square_free_decomposition(&scaled)
            .map_err(|UnsplitFactor(number)| SquareRootRefusal::UnsplitFactor(number))?;
        let Some(coefficient) = ExactRational::fraction(
            &decomposition.square_root_of_square_part,
            value.denominator(),
        ) else {
            unreachable!()
        };
        let mut sum = Self::zero();
        sum.insert(decomposition.radicand_primes, coefficient);
        Ok(sum)
    }

    pub fn rational_part(&self) -> ExactRational {
        self.terms
            .get(&Integer::one())
            .map_or_else(ExactRational::zero, |term| term.coefficient.clone())
    }

    pub fn coefficient_of(&self, radicand: &Integer) -> ExactRational {
        self.terms
            .get(radicand)
            .map_or_else(ExactRational::zero, |term| term.coefficient.clone())
    }

    pub fn single_radicand(&self) -> Option<Integer> {
        let mut terms = self.terms.keys();
        let radicand = terms.next()?;
        (terms.next().is_none() && !radicand.is_one()).then(|| radicand.clone())
    }

    pub fn as_single_root(&self) -> Option<Integer> {
        let mut terms = self.terms.iter();
        let (radicand, term) = terms.next()?;
        (terms.next().is_none() && !radicand.is_one() && term.coefficient.is_one())
            .then(|| radicand.clone())
    }

    pub(crate) fn denested_square_root(&self) -> Option<Self> {
        let mut radical = self.terms.iter().filter(|(radicand, _)| !radicand.is_one());
        let (radicand, term) = radical.next()?;
        if radical.next().is_some() {
            return None;
        }
        let rational = self.rational_part();
        let coefficient = &term.coefficient;
        let square = coefficient
            .multiply(coefficient)
            .multiply(&ExactRational::from_integer(radicand.clone()));
        let difference =
            Self::square_root_of_rational(&rational.multiply(&rational).subtract(&square))?
                .as_rational()?;
        let half = ExactRational::one_half();
        let larger = Self::square_root_of_rational(&rational.plus(&difference).multiply(&half))?;
        let smaller =
            Self::square_root_of_rational(&rational.subtract(&difference).multiply(&half))?;
        let root = match coefficient.sign() {
            Ordering::Less => larger.plus(&smaller.negated()),
            _ => larger.plus(&smaller),
        };
        (root.sign() != Ordering::Less && root.times(&root) == *self).then_some(root)
    }

    pub(crate) fn separated_terms(&self) -> Vec<Self> {
        self.terms
            .values()
            .map(|term| {
                let mut single = Self::zero();
                single.insert(term.primes.clone(), term.coefficient.clone());
                single
            })
            .collect()
    }

    fn insert(&mut self, primes: Vec<Integer>, coefficient: ExactRational) {
        let radicand = product(&primes);
        let combined = match self.terms.remove(&radicand) {
            Some(existing) => existing.coefficient.plus(&coefficient),
            None => coefficient,
        };
        if !combined.is_zero() {
            self.terms.insert(
                radicand,
                Term {
                    primes,
                    coefficient: combined,
                },
            );
        }
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn as_rational(&self) -> Option<ExactRational> {
        match self.terms.len() {
            0 => Some(ExactRational::zero()),
            1 => self
                .terms
                .get(&Integer::one())
                .map(|term| term.coefficient.clone()),
            _ => None,
        }
    }

    pub(crate) fn prime_terms(&self) -> impl Iterator<Item = (&[Integer], &ExactRational)> {
        self.terms
            .values()
            .map(|term| (term.primes.as_slice(), &term.coefficient))
    }

    pub(crate) fn from_prime_term(primes: Vec<Integer>, coefficient: ExactRational) -> Self {
        let mut sum = Self::zero();
        sum.insert(primes, coefficient);
        sum
    }

    pub fn terms(&self) -> impl Iterator<Item = (&Integer, &ExactRational)> {
        self.terms
            .iter()
            .map(|(radicand, term)| (radicand, &term.coefficient))
    }

    pub fn plus(&self, other: &Self) -> Self {
        let mut sum = self.clone();
        for term in other.terms.values() {
            sum.insert(term.primes.clone(), term.coefficient.clone());
        }
        sum
    }

    pub fn negated(&self) -> Self {
        let mut negated = Self::zero();
        for term in self.terms.values() {
            negated.insert(term.primes.clone(), term.coefficient.negated());
        }
        negated
    }

    pub fn minus(&self, other: &Self) -> Self {
        self.plus(&other.negated())
    }

    pub fn times(&self, other: &Self) -> Self {
        let mut result = Self::zero();
        for left in self.terms.values() {
            for right in other.terms.values() {
                let (shared, unshared) = merge_primes(&left.primes, &right.primes);
                let coefficient = left
                    .coefficient
                    .multiply(&right.coefficient)
                    .multiply(&ExactRational::from_integer(product(&shared)));
                result.insert(unshared, coefficient);
            }
        }
        result
    }

    fn scaled(&self, factor: &ExactRational) -> Self {
        let mut result = Self::zero();
        for term in self.terms.values() {
            result.insert(term.primes.clone(), term.coefficient.multiply(factor));
        }
        result
    }

    fn largest_prime(&self) -> Option<Integer> {
        self.terms
            .values()
            .filter_map(|term| term.primes.last())
            .max()
            .cloned()
    }

    fn split_by_prime(&self, prime: &Integer) -> (Self, Self) {
        let mut without = Self::zero();
        let mut with = Self::zero();
        for term in self.terms.values() {
            if term.primes.contains(prime) {
                let remaining = term
                    .primes
                    .iter()
                    .filter(|factor| *factor != prime)
                    .cloned()
                    .collect();
                with.insert(remaining, term.coefficient.clone());
            } else {
                without.insert(term.primes.clone(), term.coefficient.clone());
            }
        }
        (without, with)
    }

    fn times_square_root_of_prime(&self, prime: &Integer) -> Self {
        let mut result = Self::zero();
        for term in self.terms.values() {
            let mut primes = term.primes.clone();
            primes.push(prime.clone());
            primes.sort();
            result.insert(primes, term.coefficient.clone());
        }
        result
    }

    fn rational_part_sign(&self) -> Ordering {
        self.terms
            .get(&Integer::one())
            .map_or(Ordering::Equal, |term| term.coefficient.sign())
    }

    pub fn sign(&self) -> Ordering {
        let Some(prime) = self.largest_prime() else {
            return self.rational_part_sign();
        };
        let (rational_side, radical_side) = self.split_by_prime(&prime);
        let rational_sign = rational_side.sign();
        let radical_sign = radical_side.sign();
        if radical_sign == Ordering::Equal || rational_sign == radical_sign {
            return rational_sign.then(radical_sign);
        }
        if rational_sign == Ordering::Equal {
            return radical_sign;
        }
        let prime_factor = ExactRational::from_integer(prime);
        let difference_of_squares = rational_side
            .times(&rational_side)
            .minus(&radical_side.times(&radical_side).scaled(&prime_factor));
        match difference_of_squares.sign() {
            Ordering::Greater => rational_sign,
            Ordering::Less => radical_sign,
            Ordering::Equal => Ordering::Equal,
        }
    }

    pub fn compare(&self, other: &Self) -> Ordering {
        self.minus(other).sign()
    }

    pub fn absolute(&self) -> Self {
        if self.sign() == Ordering::Less {
            self.negated()
        } else {
            self.clone()
        }
    }

    pub fn reciprocal(&self) -> Option<Self> {
        let Some(prime) = self.largest_prime() else {
            let value = self.as_rational()?.reciprocal()?;
            return Some(Self::from_rational(value));
        };
        let (rational_side, radical_side) = self.split_by_prime(&prime);
        let conjugate = rational_side.minus(&radical_side.times_square_root_of_prime(&prime));
        let norm = self.times(&conjugate);
        Some(conjugate.times(&norm.reciprocal()?))
    }

    pub fn divided_by(&self, other: &Self) -> Option<Self> {
        Some(self.times(&other.reciprocal()?))
    }

    pub fn power(&self, exponent: u32) -> Self {
        let mut result = Self::from_rational(ExactRational::one());
        let mut base = self.clone();
        let mut remaining = exponent;
        while remaining > 0 {
            if remaining % 2 == 1 {
                result = result.times(&base);
            }
            remaining /= 2;
            if remaining > 0 {
                base = base.times(&base);
            }
        }
        result
    }

    pub fn floor(&self) -> Integer {
        if let Some(rational) = self.as_rational() {
            return rational.floor();
        }
        let mut precision_bits = INITIAL_FLOOR_PRECISION_BITS;
        loop {
            let (lower, upper) = self.enclosure(precision_bits);
            let lower_floor = lower.floor();
            if lower_floor == upper.floor() {
                return lower_floor;
            }
            precision_bits = precision_bits.saturating_mul(2);
        }
    }

    fn enclosure(&self, precision_bits: u32) -> (ExactRational, ExactRational) {
        let scale = Integer::from(2_u64).pow(precision_bits);
        let scale_squared = &scale * &scale;
        let mut lower = ExactRational::zero();
        let mut upper = ExactRational::zero();
        for (radicand, term) in &self.terms {
            if radicand.is_one() {
                lower = lower.plus(&term.coefficient);
                upper = upper.plus(&term.coefficient);
                continue;
            }
            let (root_floor, _) = integer_root(&(radicand * &scale_squared), 2);
            let root_ceiling = &root_floor + &Integer::one();
            let (Some(below), Some(above)) = (
                ExactRational::fraction(&root_floor, &scale),
                ExactRational::fraction(&root_ceiling, &scale),
            ) else {
                unreachable!()
            };
            let (low_root, high_root) = if term.coefficient.sign() == Ordering::Less {
                (above, below)
            } else {
                (below, above)
            };
            lower = lower.plus(&term.coefficient.multiply(&low_root));
            upper = upper.plus(&term.coefficient.multiply(&high_root));
        }
        (lower, upper)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rational(numerator: i64, denominator: i64) -> ExactRational {
        ExactRational::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    fn whole(value: i64) -> SquareRootSum {
        SquareRootSum::from_rational(ExactRational::from_i64(value))
    }

    fn root(value: i64) -> SquareRootSum {
        SquareRootSum::square_root_of_rational(&ExactRational::from_i64(value)).unwrap()
    }

    fn scaled_root(coefficient: i64, value: i64) -> SquareRootSum {
        root(value).times(&whole(coefficient))
    }

    #[test]
    fn a_square_of_a_sum_of_roots_denests_to_that_sum() {
        let radicand = whole(17).plus(&scaled_root(12, 2));

        assert_eq!(
            radicand.denested_square_root(),
            Some(whole(3).plus(&scaled_root(2, 2)))
        );
    }

    #[test]
    fn a_root_that_denests_into_a_bigger_field_is_found() {
        let radicand = whole(5).plus(&scaled_root(2, 6));

        assert_eq!(
            radicand.denested_square_root(),
            Some(root(2).plus(&root(3)))
        );
    }

    #[test]
    fn a_negative_coefficient_denests_to_the_positive_difference() {
        let radicand = whole(3).plus(&scaled_root(-2, 2));

        assert_eq!(
            radicand.denested_square_root(),
            Some(root(2).plus(&whole(-1)))
        );
    }

    #[test]
    fn a_root_whose_norm_is_no_square_does_not_denest() {
        let radicand = whole(1).plus(&root(2));

        assert_eq!(radicand.denested_square_root(), None);
    }

    #[test]
    fn a_radicand_with_two_roots_is_left_as_it_is() {
        let radicand = whole(10)
            .plus(&scaled_root(2, 6))
            .plus(&scaled_root(2, 10))
            .plus(&scaled_root(2, 15));

        assert_eq!(radicand.denested_square_root(), None);
    }

    #[test]
    fn a_value_is_taken_apart_into_its_rational_part_and_coefficients() {
        let value = whole(17).plus(&scaled_root(12, 2));

        assert_eq!(value.rational_part(), rational(17, 1));
        assert_eq!(value.coefficient_of(&Integer::from(2_i64)), rational(12, 1));
        assert_eq!(value.coefficient_of(&Integer::from(3_i64)), rational(0, 1));
    }

    #[test]
    fn only_a_bare_square_root_names_a_term() {
        assert_eq!(root(2).as_single_root(), Some(Integer::from(2_i64)));
        assert_eq!(root(8).as_single_root(), None);
        assert_eq!(root(8).single_radicand(), Some(Integer::from(2_i64)));
        assert_eq!(whole(2).as_single_root(), None);
    }

    fn approximate(sum: &SquareRootSum) -> f64 {
        sum.terms()
            .map(|(radicand, coefficient)| {
                let radicand = f64::from(i32::try_from(radicand.to_i64().unwrap()).unwrap());
                let numerator = coefficient.numerator().to_i64().unwrap();
                let denominator = coefficient.denominator().to_i64().unwrap();
                let numerator = f64::from(i32::try_from(numerator).unwrap());
                let denominator = f64::from(i32::try_from(denominator).unwrap());
                numerator / denominator * radicand.sqrt()
            })
            .fold(0.0, |accumulated, term| accumulated + term)
    }

    #[test]
    fn square_root_of_eight_is_two_square_root_two() {
        let expected = root(2).times(&whole(2));

        assert_eq!(root(8), expected);
    }

    #[test]
    fn square_root_of_fraction_is_rationalized() {
        let value = SquareRootSum::square_root_of_rational(&rational(1, 2)).unwrap();

        assert_eq!(value, root(2).scaled(&rational(1, 2)));
    }

    #[test]
    fn square_root_of_negative_rational_is_none() {
        assert_eq!(
            SquareRootSum::square_root_of_rational(&ExactRational::from_i64(-4)),
            None
        );
    }

    #[test]
    fn square_root_of_square_is_rational() {
        assert_eq!(root(49).as_rational(), Some(ExactRational::from_i64(7)));
    }

    #[test]
    fn product_of_equal_roots_is_rational() {
        assert_eq!(root(2).times(&root(2)), whole(2));
    }

    #[test]
    fn product_of_coprime_roots_is_root_of_product() {
        assert_eq!(root(2).times(&root(3)), root(6));
    }

    #[test]
    fn product_of_roots_with_shared_prime_moves_it_out() {
        assert_eq!(root(6).times(&root(10)), root(15).times(&whole(2)));
    }

    #[test]
    fn sum_cancels_equal_terms() {
        let sum = root(2).plus(&whole(1)).minus(&root(2));

        assert_eq!(sum, whole(1));
    }

    #[test]
    fn square_of_binomial_expands() {
        let binomial = root(2).plus(&root(3));

        let square = binomial.power(2);

        assert_eq!(square, whole(5).plus(&root(6).times(&whole(2))));
    }

    #[test]
    fn sign_of_root_two_minus_one_is_positive() {
        assert_eq!(root(2).minus(&whole(1)).sign(), Ordering::Greater);
    }

    #[test]
    fn sign_of_root_two_minus_root_three_is_negative() {
        assert_eq!(root(2).minus(&root(3)).sign(), Ordering::Less);
    }

    #[test]
    fn sign_of_close_approximation_is_exact() {
        let above = root(2).times(&whole(99)).minus(&whole(140));
        let below = whole(140).minus(&root(2).times(&whole(99)));

        assert_eq!(
            (above.sign(), below.sign()),
            (Ordering::Greater, Ordering::Less)
        );
    }

    #[test]
    fn sign_matches_floating_point_on_mixed_sums() {
        let cases = [
            root(2).plus(&root(3)).minus(&root(10)),
            root(5).plus(&root(7)).minus(&root(2).times(&whole(3))),
            root(6).minus(&root(2)).minus(&root(3)).plus(&whole(1)),
            root(30).minus(&root(2).times(&root(3)).times(&root(5))),
            whole(3).minus(&root(2)).minus(&root(3)),
            whole(2).minus(&root(2)),
            root(2).minus(&whole(2)),
            whole(4).minus(&root(2)).minus(&root(3)),
            root(3).plus(&root(5)).minus(&root(2)).minus(&whole(2)),
        ];

        let signs: Vec<Ordering> = cases.iter().map(SquareRootSum::sign).collect();

        let expected: Vec<Ordering> = cases
            .iter()
            .map(|case| approximate(case).total_cmp(&0.0))
            .collect();
        assert_eq!(signs, expected);
    }

    #[test]
    fn reciprocal_times_value_is_one() {
        let value = whole(1).plus(&root(2)).plus(&root(3));

        let product = value.times(&value.reciprocal().unwrap());

        assert_eq!(product, whole(1));
    }

    #[test]
    fn reciprocal_of_zero_is_none() {
        assert_eq!(SquareRootSum::zero().reciprocal(), None);
    }

    #[test]
    fn product_of_conjugates_is_rational() {
        let left = root(3).plus(&root(2));
        let right = root(3).minus(&root(2));

        assert_eq!(left.times(&right), whole(1));
    }

    #[test]
    fn floor_of_scaled_root_two() {
        assert_eq!(root(2).times(&whole(1000)).floor(), Integer::from(1414_i64));
    }

    #[test]
    fn floor_of_negative_root_two_is_minus_two() {
        assert_eq!(root(2).negated().floor(), Integer::from(-2_i64));
    }

    #[test]
    fn floor_just_above_integer() {
        assert_eq!(root(2).times(&whole(99)).floor(), Integer::from(140_i64));
    }

    #[test]
    fn floor_of_rational_is_rational_floor() {
        let value = SquareRootSum::from_rational(rational(-1, 3));

        assert_eq!(value.floor(), Integer::from(-1_i64));
    }

    #[test]
    fn absolute_value_of_negative_sum_is_negated() {
        let value = whole(1).minus(&root(2));

        assert_eq!(value.absolute(), root(2).minus(&whole(1)));
    }

    #[test]
    fn compare_orders_roots() {
        assert_eq!(root(7).compare(&root(5)), Ordering::Greater);
    }

    #[test]
    fn the_square_root_of_zero_is_zero_and_carries_no_radical() {
        let root = SquareRootSum::square_root_of_rational(&ExactRational::zero()).unwrap();
        assert!(root.is_zero());
    }
}
