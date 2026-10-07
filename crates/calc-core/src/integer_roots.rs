use calc_numbers::Integer;

const TRIAL_DIVISION_LIMIT: u64 = 1 << 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnsplitFactor(pub Integer);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SquareFreeDecomposition {
    pub square_root_of_square_part: Integer,
    pub radicand: Integer,
    pub radicand_primes: Vec<Integer>,
}

fn two() -> Integer {
    Integer::from(2_u64)
}

fn halved(value: &Integer) -> Integer {
    match value.div_rem_euclid(&two()) {
        Ok((quotient, _)) => quotient,
        Err(_) => unreachable!(),
    }
}

fn bit_length(value: &Integer) -> u64 {
    let base = two();
    let mut power = Integer::one();
    let mut bits = 0;
    while power <= *value {
        power = &power * &base;
        bits += 1;
    }
    bits
}

pub(crate) fn integer_root(value: &Integer, degree: u32) -> (Integer, bool) {
    if value.is_zero() || degree == 1 {
        return (value.clone(), true);
    }
    let root_bits = bit_length(value).div_ceil(u64::from(degree));
    let mut high = Integer::one();
    for _ in 0..root_bits {
        high = &high * &two();
    }
    let mut low = Integer::zero();
    while &high - &low > Integer::one() {
        let middle = halved(&(&low + &high));
        if middle.pow(degree) <= *value {
            low = middle;
        } else {
            high = middle;
        }
    }
    let is_exact = low.pow(degree) == *value;
    (low, is_exact)
}

pub(crate) fn prime_factors(value: &Integer) -> (Vec<(Integer, u32)>, Integer) {
    let mut rest = value.absolute();
    let mut factors = Vec::new();
    let mut candidate: u64 = 2;
    while candidate <= TRIAL_DIVISION_LIMIT {
        let prime = Integer::from(candidate);
        if &prime * &prime > rest {
            break;
        }
        let mut exponent: u32 = 0;
        while let Ok((quotient, remainder)) = rest.div_rem_euclid(&prime) {
            if !remainder.is_zero() {
                break;
            }
            rest = quotient;
            exponent += 1;
        }
        if exponent > 0 {
            factors.push((prime, exponent));
        }
        candidate += if candidate == 2 { 1 } else { 2 };
    }
    let whole = Integer::from(candidate);
    if !rest.is_one() && &whole * &whole > rest {
        factors.push((rest, 1));
        rest = Integer::one();
    }
    (factors, rest)
}

pub(crate) fn square_free_decomposition(
    value: &Integer,
) -> Result<SquareFreeDecomposition, UnsplitFactor> {
    let mut rest = value.clone();
    let mut square_root_of_square_part = Integer::one();
    let mut radicand = Integer::one();
    let mut radicand_primes = Vec::new();
    let mut candidate: u64 = 2;
    while candidate <= TRIAL_DIVISION_LIMIT {
        let prime = Integer::from(candidate);
        if &prime * &prime > rest {
            break;
        }
        let mut exponent: u32 = 0;
        while let Ok((quotient, remainder)) = rest.div_rem_euclid(&prime) {
            if !remainder.is_zero() {
                break;
            }
            rest = quotient;
            exponent += 1;
        }
        square_root_of_square_part = &square_root_of_square_part * &prime.pow(exponent / 2);
        if exponent % 2 == 1 {
            radicand = &radicand * &prime;
            radicand_primes.push(prime);
        }
        candidate += if candidate == 2 { 1 } else { 2 };
    }
    if !rest.is_one() {
        let candidate_square = Integer::from(candidate);
        if &candidate_square * &candidate_square > rest {
            radicand = &radicand * &rest;
            radicand_primes.push(rest);
        } else {
            let (root, is_square) = integer_root(&rest, 2);
            if is_square {
                square_root_of_square_part = &square_root_of_square_part * &root;
            } else {
                let mut split =
                    crate::factorization::proven_prime_factors(&rest).map_err(UnsplitFactor)?;
                split.sort();
                for (prime, exponent) in split {
                    square_root_of_square_part =
                        &square_root_of_square_part * &prime.pow(exponent / 2);
                    if exponent % 2 == 1 {
                        radicand = &radicand * &prime;
                        radicand_primes.push(prime);
                    }
                }
            }
        }
    }
    Ok(SquareFreeDecomposition {
        square_root_of_square_part,
        radicand,
        radicand_primes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integer(value: i64) -> Integer {
        Integer::from(value)
    }

    #[test]
    fn square_root_of_perfect_square_is_exact() {
        assert_eq!(integer_root(&integer(144), 2), (integer(12), true));
    }

    #[test]
    fn square_root_of_non_square_is_floor_and_not_exact() {
        assert_eq!(integer_root(&integer(143), 2), (integer(11), false));
    }

    #[test]
    fn cube_root_of_perfect_cube_is_exact() {
        assert_eq!(integer_root(&integer(4913), 3), (integer(17), true));
    }

    #[test]
    fn root_of_high_degree_of_small_value_is_one() {
        assert_eq!(integer_root(&integer(5), 1000), (integer(1), false));
    }

    #[test]
    fn square_root_of_large_square_is_exact() {
        let root = Integer::from(10_u64).pow(40);

        assert_eq!(integer_root(&root.pow(2), 2), (root, true));
    }

    #[test]
    fn eight_is_two_squared_times_two() {
        let decomposition = square_free_decomposition(&integer(8)).unwrap();

        assert_eq!(
            (
                decomposition.square_root_of_square_part,
                decomposition.radicand,
                decomposition.radicand_primes
            ),
            (integer(2), integer(2), vec![integer(2)])
        );
    }

    #[test]
    fn perfect_square_has_radicand_one() {
        let decomposition = square_free_decomposition(&integer(3600)).unwrap();

        assert_eq!(
            (
                decomposition.square_root_of_square_part,
                decomposition.radicand
            ),
            (integer(60), integer(1))
        );
    }

    #[test]
    fn square_free_value_keeps_all_primes() {
        let decomposition = square_free_decomposition(&integer(2 * 3 * 7)).unwrap();

        assert_eq!(
            decomposition.radicand_primes,
            vec![integer(2), integer(3), integer(7)]
        );
    }

    #[test]
    fn one_has_empty_decomposition() {
        let decomposition = square_free_decomposition(&integer(1)).unwrap();

        assert_eq!(
            (decomposition.radicand, decomposition.radicand_primes.len()),
            (integer(1), 0)
        );
    }

    #[test]
    fn large_prime_factor_below_square_of_limit_is_certified() {
        let large_prime = integer(1_000_003);
        let value = &integer(2) * &large_prime;

        let decomposition = square_free_decomposition(&value).unwrap();

        assert_eq!(decomposition.radicand_primes, vec![integer(2), large_prime]);
    }

    #[test]
    fn square_of_prime_above_limit_is_detected() {
        let large_prime = integer(4_294_967_311);
        let value = &(&large_prime * &large_prime) * &integer(3);

        let decomposition = square_free_decomposition(&value).unwrap();

        assert_eq!(
            (
                decomposition.square_root_of_square_part,
                decomposition.radicand
            ),
            (large_prime, integer(3))
        );
    }

    #[test]
    fn product_of_two_primes_above_limit_is_split_into_both() {
        let value = &integer(4_294_967_311) * &integer(4_294_967_357);

        let decomposition = square_free_decomposition(&value).unwrap();

        assert_eq!(
            decomposition.radicand_primes,
            vec![integer(4_294_967_311), integer(4_294_967_357)]
        );
    }

    #[test]
    fn square_of_a_split_factor_above_limit_leaves_the_radicand() {
        let value = &(&integer(4_294_967_311) * &integer(4_294_967_311)) * &integer(4_294_967_357);
        let value = &value * &integer(1_000_003);

        let decomposition = square_free_decomposition(&value).unwrap();

        assert_eq!(
            (
                decomposition.square_root_of_square_part,
                decomposition.radicand_primes
            ),
            (
                integer(4_294_967_311),
                vec![integer(1_000_003), integer(4_294_967_357)]
            )
        );
    }

    #[test]
    fn a_factor_proven_only_probably_prime_is_named() {
        let probable = &integer(2).pow(89) - &integer(1);

        assert_eq!(
            square_free_decomposition(&probable),
            Err(UnsplitFactor(probable.clone()))
        );
    }

    #[test]
    fn decomposition_reconstructs_the_value() {
        let value = integer(2 * 2 * 2 * 3 * 3 * 5 * 11 * 11 * 11);

        let decomposition = square_free_decomposition(&value).unwrap();

        let square = decomposition.square_root_of_square_part.pow(2);
        assert_eq!(&square * &decomposition.radicand, value);
    }
}
