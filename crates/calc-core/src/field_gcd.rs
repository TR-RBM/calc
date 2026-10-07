use std::collections::BTreeMap;

use calc_numbers::Integer;

use crate::exact_rational::ExactRational;
use crate::polynomial_gcd::{
    Exponents, Residues, added, inverse, is_constant, leading, modular_gcd, multiplied, powered,
    primes, residue, scaled, subtracted,
};
use crate::square_root_sum::SquareRootSum;

pub(crate) type FieldPolynomial = BTreeMap<Exponents, SquareRootSum>;

type Accumulated = BTreeMap<(Exponents, usize), Integer>;

fn square_root_modulo(value: u64, prime: u64) -> Option<u64> {
    let value = value % prime;
    if value == 0 {
        return Some(0);
    }
    if powered(value, (prime - 1) / 2, prime) != 1 {
        return None;
    }
    let mut odd = prime - 1;
    let mut twos = 0_u32;
    while odd.is_multiple_of(2) {
        odd /= 2;
        twos += 1;
    }
    let mut non_residue = 2;
    while powered(non_residue, (prime - 1) / 2, prime) != prime - 1 {
        non_residue += 1;
    }
    let mut order = twos;
    let mut factor = powered(non_residue, odd, prime);
    let mut remaining = powered(value, odd, prime);
    let mut root = powered(value, odd.div_ceil(2), prime);
    loop {
        if remaining == 1 {
            return Some(root);
        }
        let mut least = 0;
        let mut probe = remaining;
        while probe != 1 {
            probe = multiplied(probe, probe, prime);
            least += 1;
            if least == order {
                return None;
            }
        }
        let step = powered(factor, 1_u64 << (order - least - 1), prime);
        order = least;
        factor = multiplied(step, step, prime);
        remaining = multiplied(remaining, factor, prime);
        root = multiplied(root, step, prime);
    }
}

fn rational_residue(value: &ExactRational, prime: u64) -> Option<u64> {
    let denominator = inverse(residue(value.denominator(), prime)?, prime)?;
    Some(multiplied(
        residue(value.numerator(), prime)?,
        denominator,
        prime,
    ))
}

fn subset_of(primes: &[Integer], generators: &[Integer]) -> Option<usize> {
    primes.iter().try_fold(0_usize, |subset, prime| {
        let position = generators.iter().position(|generator| generator == prime)?;
        Some(subset | (1 << position))
    })
}

fn image(
    polynomial: &FieldPolynomial,
    generators: &[Integer],
    roots: &[u64],
    prime: u64,
) -> Option<Residues> {
    let mut residues = Residues::new();
    for (exponents, coefficient) in polynomial {
        let mut total = 0;
        for (primes, rational) in coefficient.prime_terms() {
            let subset = subset_of(primes, generators)?;
            let root = roots
                .iter()
                .enumerate()
                .filter(|(position, _)| subset & (1 << position) != 0)
                .fold(1, |product, (_, root)| multiplied(product, *root, prime));
            total = added(
                total,
                multiplied(rational_residue(rational, prime)?, root, prime),
                prime,
            );
        }
        if total != 0 {
            residues.insert(exponents.clone(), total);
        }
    }
    Some(residues)
}

fn combined(
    known: &Accumulated,
    modulus: &Integer,
    image: &Accumulated,
    prime: u64,
) -> Option<Accumulated> {
    let modulus_inverse = inverse(residue(modulus, prime)?, prime)?;
    let mut keys: Vec<&(Exponents, usize)> = known.keys().chain(image.keys()).collect();
    keys.sort();
    keys.dedup();
    let mut result = Accumulated::new();
    for key in keys {
        let old = known.get(key).cloned().unwrap_or_else(Integer::zero);
        let new = image
            .get(key)
            .and_then(|value| residue(value, prime))
            .unwrap_or(0);
        let step = multiplied(
            subtracted(new, residue(&old, prime)?, prime),
            modulus_inverse,
            prime,
        );
        let value = &old + &(modulus * &Integer::from(step));
        if !value.is_zero() {
            result.insert(key.clone(), value);
        }
    }
    Some(result)
}

fn reconstructed(value: &Integer, modulus: &Integer) -> Option<ExactRational> {
    let half = modulus.bit_length().saturating_sub(1) / 2;
    let (mut previous, mut current) = (modulus.clone(), value.clone());
    let (mut previous_factor, mut current_factor) = (Integer::zero(), Integer::one());
    while current.bit_length() > half {
        let (quotient, remainder) = previous.div_rem_euclid(&current).ok()?;
        let next_factor = &previous_factor - &(&quotient * &current_factor);
        previous = current;
        current = remainder;
        previous_factor = current_factor;
        current_factor = next_factor;
    }
    if current_factor.is_zero() || current_factor.bit_length() > half {
        return None;
    }
    if !current.gcd(&current_factor).is_one() {
        return None;
    }
    ExactRational::fraction(&current, &current_factor)
}

fn candidate(
    known: &Accumulated,
    modulus: &Integer,
    generators: &[Integer],
) -> Option<FieldPolynomial> {
    let mut polynomial = FieldPolynomial::new();
    for ((exponents, subset), value) in known {
        let rational = reconstructed(value, modulus)?;
        let primes: Vec<Integer> = generators
            .iter()
            .enumerate()
            .filter(|(position, _)| subset & (1 << position) != 0)
            .map(|(_, prime)| prime.clone())
            .collect();
        let term = SquareRootSum::from_prime_term(primes, rational);
        let entry = polynomial
            .entry(exponents.clone())
            .or_insert_with(SquareRootSum::zero);
        *entry = entry.plus(&term);
    }
    polynomial.retain(|_, coefficient| !coefficient.is_zero());
    Some(polynomial)
}

fn images_for(
    left: &FieldPolynomial,
    right: &FieldPolynomial,
    generators: &[Integer],
    prime: u64,
) -> Option<Option<Vec<Residues>>> {
    let mut roots = Vec::with_capacity(generators.len());
    for generator in generators {
        match square_root_modulo(residue(generator, prime)?, prime) {
            Some(root) if root != 0 => roots.push(root),
            _ => return Some(None),
        }
    }
    let count = 1_usize.checked_shl(u32::try_from(generators.len()).ok()?)?;
    let mut images = Vec::with_capacity(count);
    for signs in 0..count {
        let signed: Vec<u64> = roots
            .iter()
            .enumerate()
            .map(|(position, root)| {
                if signs & (1 << position) == 0 {
                    *root
                } else {
                    prime - root
                }
            })
            .collect();
        let (Some(one), Some(two)) = (
            image(left, generators, &signed, prime),
            image(right, generators, &signed, prime),
        ) else {
            return Some(None);
        };
        let keeps_leading = |polynomial: &FieldPolynomial, image: &Residues| {
            leading(image).map(|(exponents, _)| exponents) == polynomial.keys().next_back()
        };
        if !keeps_leading(left, &one) || !keeps_leading(right, &two) {
            return Some(None);
        }
        let width = left.keys().chain(right.keys()).next()?.len();
        let gcd = modular_gcd(&one, &two, width, prime)?;
        let (_, gcd_leading) = leading(&gcd)?;
        images.push(scaled(&gcd, inverse(gcd_leading, prime)?, prime));
    }
    Some(Some(images))
}

fn transformed(images: &[Residues], generators: &[Integer], prime: u64) -> Option<Accumulated> {
    let count = images.len();
    let scale = inverse(
        residue(&Integer::from(u64::try_from(count).ok()?), prime)?,
        prime,
    )?;
    let roots: Vec<u64> = generators
        .iter()
        .map(|generator| square_root_modulo(residue(generator, prime)?, prime))
        .collect::<Option<Vec<u64>>>()?;
    let mut keys: Vec<&Exponents> = images.iter().flat_map(BTreeMap::keys).collect();
    keys.sort();
    keys.dedup();
    let mut result = Accumulated::new();
    for exponents in keys {
        for subset in 0..count {
            let mut total = 0;
            for (signs, image) in images.iter().enumerate() {
                let value = image.get(exponents).copied().unwrap_or(0);
                let is_negative = (signs & subset).count_ones() % 2 == 1;
                total = if is_negative {
                    subtracted(total, value, prime)
                } else {
                    added(total, value, prime)
                };
            }
            let root = roots
                .iter()
                .enumerate()
                .filter(|(position, _)| subset & (1 << position) != 0)
                .fold(1, |product, (_, root)| multiplied(product, *root, prime));
            let value = multiplied(
                multiplied(total, scale, prime),
                inverse(root, prime)?,
                prime,
            );
            if value != 0 {
                result.insert((exponents.clone(), subset), Integer::from(value));
            }
        }
    }
    Some(result)
}

fn written_bits(polynomial: &FieldPolynomial) -> u64 {
    polynomial
        .values()
        .flat_map(SquareRootSum::prime_terms)
        .map(|(_, rational)| {
            rational
                .numerator()
                .bit_length()
                .saturating_add(rational.denominator().bit_length())
        })
        .fold(0, u64::saturating_add)
}

fn kronecker_degree(polynomial: &FieldPolynomial) -> u64 {
    let width = polynomial.keys().next().map_or(0, Vec::len);
    (0..width)
        .map(|variable| {
            u64::from(
                polynomial
                    .keys()
                    .map(|exponents| exponents[variable])
                    .max()
                    .unwrap_or(0),
            ) + 1
        })
        .fold(1_u64, u64::saturating_mul)
}

pub(crate) fn field_gcd(
    left: &FieldPolynomial,
    right: &FieldPolynomial,
    generators: &[Integer],
    accepts: impl Fn(&FieldPolynomial) -> bool,
) -> Option<FieldPolynomial> {
    let width = left.keys().chain(right.keys()).next()?.len();
    let one = || -> FieldPolynomial {
        [(
            vec![0; width],
            SquareRootSum::from_rational(ExactRational::one()),
        )]
        .into_iter()
        .collect()
    };
    let mut known = Accumulated::new();
    let mut modulus = Integer::one();
    let mut shape: Option<Exponents> = None;
    let mut last: Option<FieldPolynomial> = None;
    let bound_bits = written_bits(left)
        .saturating_add(written_bits(right))
        .saturating_add(kronecker_degree(left).min(kronecker_degree(right)))
        .saturating_mul(2);
    for prime in primes() {
        let Some(images) = images_for(left, right, generators, prime)? else {
            continue;
        };
        if images.iter().any(is_constant) {
            return Some(one());
        }
        let shapes: Vec<&Exponents> = images
            .iter()
            .filter_map(|image| leading(image).map(|(exponents, _)| exponents))
            .collect();
        let image_shape = shapes.iter().copied().min()?.clone();
        if shapes.iter().any(|candidate| **candidate != image_shape) {
            continue;
        }
        match &shape {
            Some(current) if image_shape > *current => continue,
            Some(current) if image_shape == *current => {}
            _ => {
                shape = Some(image_shape);
                known = Accumulated::new();
                modulus = Integer::one();
                last = None;
            }
        }
        let image = transformed(&images, generators, prime)?;
        known = combined(&known, &modulus, &image, prime)?;
        modulus = &modulus * &Integer::from(prime);
        let Some(next) = candidate(&known, &modulus, generators) else {
            continue;
        };
        let is_past_bound = modulus.bit_length() > bound_bits;
        if (is_past_bound || last.as_ref() == Some(&next)) && accepts(&next) {
            return Some(next);
        }
        if is_past_bound {
            return None;
        }
        last = Some(next);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_root_modulo_a_prime_squares_back() {
        let prime = 1_000_000_007;
        let root = square_root_modulo(2, prime).unwrap();

        assert_eq!(multiplied(root, root, prime), 2);
    }

    #[test]
    fn a_non_residue_has_no_square_root() {
        assert_eq!(square_root_modulo(3, 7), None);
    }

    #[test]
    fn a_fraction_is_reconstructed_from_its_residue() {
        let modulus = Integer::from(1_000_000_007_i64);
        let residue_of_two_thirds = Integer::from(666_666_672_i64);

        assert_eq!(
            reconstructed(&residue_of_two_thirds, &modulus),
            ExactRational::fraction(&Integer::from(2_i64), &Integer::from(3_i64))
        );
    }
}
