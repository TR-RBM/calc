use std::cmp::Ordering;

use calc_numbers::{Integer, Number};

const PROVEN_BELOW: i128 = 3_317_044_064_679_887_385_961_981;

const WITNESSES: [i64; 13] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41];

const TRIAL_DIVISORS_BELOW: i64 = 1_000;

const RHO_STEPS: u32 = 1 << 18;

const RHO_CONSTANTS: [i64; 4] = [1, 3, 5, 7];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primality {
    Proven,
    Probable,
    NotSplit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrimeFactor {
    pub base: Integer,
    pub exponent: i64,
    pub primality: Primality,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Factorization {
    pub is_negative: bool,
    pub is_zero: bool,
    pub factors: Vec<PrimeFactor>,
}

fn remainder(value: &Integer, modulus: &Integer) -> Integer {
    value
        .div_rem_euclid(modulus)
        .map(|(_, rest)| rest)
        .unwrap_or_else(|_| Integer::zero())
}

fn power_modulo(base: &Integer, exponent: &Integer, modulus: &Integer) -> Integer {
    let two = Integer::from(2_i64);
    let mut result = Integer::one();
    let mut square = remainder(base, modulus);
    let mut rest = exponent.clone();
    while !rest.is_zero() {
        let Ok((half, bit)) = rest.div_rem_euclid(&two) else {
            break;
        };
        if bit.is_one() {
            result = remainder(&(&result * &square), modulus);
        }
        square = remainder(&(&square * &square), modulus);
        rest = half;
    }
    result
}

fn passes_miller_rabin(candidate: &Integer) -> bool {
    let one = Integer::one();
    let two = Integer::from(2_i64);
    let less = candidate - &one;
    let mut odd = less.clone();
    let mut doublings = 0_u32;
    while let Ok((half, bit)) = odd.div_rem_euclid(&two) {
        if !bit.is_zero() {
            break;
        }
        odd = half;
        doublings += 1;
    }
    'witness: for witness in WITNESSES {
        let witness = Integer::from(witness);
        if remainder(&witness, candidate).is_zero() {
            continue;
        }
        let mut value = power_modulo(&witness, &odd, candidate);
        if value == one || value == less {
            continue;
        }
        for _ in 1..doublings {
            value = remainder(&(&value * &value), candidate);
            if value == less {
                continue 'witness;
            }
        }
        return false;
    }
    true
}

fn primality_of(candidate: &Integer) -> Option<Primality> {
    if !passes_miller_rabin(candidate) {
        return None;
    }
    Some(if *candidate < Integer::from(PROVEN_BELOW) {
        Primality::Proven
    } else {
        Primality::Probable
    })
}

const RHO_BATCH: u32 = 128;

fn rho_divisor(composite: &Integer) -> Option<Integer> {
    for constant in RHO_CONSTANTS {
        let constant = Integer::from(constant);
        let step = |value: &Integer| remainder(&(&(value * value) + &constant), composite);
        let mut slow = Integer::from(2_i64);
        let mut fast = Integer::from(2_i64);
        let mut taken = 0;
        while taken < RHO_STEPS {
            let (batch_slow, batch_fast) = (slow.clone(), fast.clone());
            let mut product = Integer::one();
            for _ in 0..RHO_BATCH {
                slow = step(&slow);
                fast = step(&step(&fast));
                product = remainder(&(&product * &(&slow - &fast).absolute()), composite);
            }
            taken += RHO_BATCH;
            let divisor = product.gcd(composite);
            if divisor.is_one() {
                continue;
            }
            if divisor != *composite {
                return Some(divisor);
            }
            let (mut slow, mut fast) = (batch_slow, batch_fast);
            for _ in 0..RHO_BATCH {
                slow = step(&slow);
                fast = step(&step(&fast));
                let divisor = (&slow - &fast).absolute().gcd(composite);
                if divisor.is_one() {
                    continue;
                }
                if divisor != *composite {
                    return Some(divisor);
                }
                break;
            }
            break;
        }
    }
    None
}

fn add_factor(factors: &mut Vec<PrimeFactor>, base: Integer, exponent: i64, primality: Primality) {
    match factors
        .iter_mut()
        .find(|factor| factor.base == base && factor.primality == primality)
    {
        Some(factor) => factor.exponent += exponent,
        None => factors.push(PrimeFactor {
            base,
            exponent,
            primality,
        }),
    }
}

fn factor_magnitude(value: &Integer, sign: i64, factors: &mut Vec<PrimeFactor>) {
    let mut rest = value.absolute();
    let mut divisor = 2_i64;
    while divisor < TRIAL_DIVISORS_BELOW {
        let prime = Integer::from(divisor);
        while let Ok((quotient, left)) = rest.div_rem_euclid(&prime) {
            if !left.is_zero() {
                break;
            }
            add_factor(factors, prime.clone(), sign, Primality::Proven);
            rest = quotient;
        }
        divisor += if divisor == 2 { 1 } else { 2 };
    }
    let mut pending = vec![rest];
    while let Some(current) = pending.pop() {
        if current.is_one() {
            continue;
        }
        if &Integer::from(TRIAL_DIVISORS_BELOW) * &Integer::from(TRIAL_DIVISORS_BELOW) > current {
            add_factor(factors, current, sign, Primality::Proven);
            continue;
        }
        if let Some(primality) = primality_of(&current) {
            add_factor(factors, current, sign, primality);
            continue;
        }
        match rho_divisor(&current) {
            Some(divisor) => {
                let other = current
                    .div_rem_euclid(&divisor)
                    .map(|(quotient, _)| quotient)
                    .unwrap_or_else(|_| Integer::one());
                pending.push(divisor);
                pending.push(other);
            }
            None => add_factor(factors, current, sign, Primality::NotSplit),
        }
    }
}

pub(crate) fn proven_prime_factors(value: &Integer) -> Result<Vec<(Integer, u32)>, Integer> {
    let mut factors = Vec::new();
    factor_magnitude(value, 1, &mut factors);
    factors
        .into_iter()
        .map(|factor| match factor.primality {
            Primality::Proven => Ok((
                factor.base,
                u32::try_from(factor.exponent).unwrap_or(u32::MAX),
            )),
            Primality::Probable | Primality::NotSplit => Err(factor.base),
        })
        .collect()
}

pub fn factorization(value: &Number) -> Option<Factorization> {
    let exact = value.to_exact().ok()?;
    let (numerator, denominator) = match &exact {
        Number::Integer(integer) => (integer.clone(), Integer::one()),
        Number::Rational(rational) => {
            (rational.numerator().clone(), rational.denominator().clone())
        }
        Number::F32(_) | Number::F64(_) => return None,
    };
    let mut factors = Vec::new();
    if !numerator.is_zero() {
        factor_magnitude(&numerator, 1, &mut factors);
        factor_magnitude(&denominator, -1, &mut factors);
    }
    factors.sort_by(
        |left, right| match (left.exponent > 0).cmp(&(right.exponent > 0)) {
            Ordering::Equal => left.base.cmp(&right.base),
            other => other.reverse(),
        },
    );
    Some(Factorization {
        is_negative: numerator.is_negative(),
        is_zero: numerator.is_zero(),
        factors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn factored(value: i128) -> Vec<(i128, i64, Primality)> {
        factorization(&Number::Integer(Integer::from(value)))
            .unwrap()
            .factors
            .into_iter()
            .map(|factor| {
                (
                    factor.base.to_i128().unwrap(),
                    factor.exponent,
                    factor.primality,
                )
            })
            .collect()
    }

    #[test]
    fn a_small_number_is_split_into_proven_primes() {
        assert_eq!(
            factored(360),
            vec![
                (2, 3, Primality::Proven),
                (3, 2, Primality::Proven),
                (5, 1, Primality::Proven)
            ]
        );
    }

    #[test]
    fn a_product_of_two_large_primes_is_split_by_rho() {
        assert_eq!(
            factored(1_000_003 * 1_000_033),
            vec![
                (1_000_003, 1, Primality::Proven),
                (1_000_033, 1, Primality::Proven)
            ]
        );
    }

    #[test]
    fn a_strong_pseudoprime_to_small_bases_is_not_taken_for_a_prime() {
        assert_eq!(
            factored(3_215_031_751),
            vec![
                (151, 1, Primality::Proven),
                (751, 1, Primality::Proven),
                (28_351, 1, Primality::Proven)
            ]
        );
    }

    #[test]
    fn a_prime_above_the_proven_bound_is_probable() {
        let mersenne = (1_i128 << 89) - 1;
        assert_eq!(factored(mersenne), vec![(mersenne, 1, Primality::Probable)]);
    }

    #[test]
    fn the_strong_pseudoprime_to_the_first_twelve_bases_is_split() {
        let factors = factored(318_665_857_834_031_151_167_461);
        assert!(factors.len() > 1);
        assert!(
            factors
                .iter()
                .all(|(_, _, primality)| *primality == Primality::Proven)
        );
    }
}
