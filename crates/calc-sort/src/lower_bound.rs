use calc_numbers::Integer;

pub fn comparison_lower_bound(length: u64) -> Integer {
    let orders = (2..=length).fold(Integer::one(), |product, factor| {
        &product * &Integer::from(factor)
    });
    if orders.is_one() {
        return Integer::zero();
    }
    Integer::from((&orders - &Integer::one()).bit_length())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bound_matches_the_ceiling_of_log2_of_the_factorial_up_to_twelve() {
        let expected: Vec<Integer> = [0_i64, 0, 1, 3, 5, 7, 10, 13, 16, 19, 22, 26, 29]
            .into_iter()
            .map(Integer::from)
            .collect();

        let bounds: Vec<Integer> = (0..=12).map(comparison_lower_bound).collect();

        assert_eq!(bounds, expected);
    }

    #[test]
    fn bound_for_a_thousand_entries_is_8530() {
        assert_eq!(comparison_lower_bound(1000), Integer::from(8530_i64));
    }
}
