use std::cmp::Ordering;

use calc_numbers::{Integer, Number};

fn sign(number: &Number) -> Option<Ordering> {
    match number {
        Number::Integer(integer) => Some(integer_sign(integer)),
        Number::Rational(rational) => Some(integer_sign(rational.numerator())),
        Number::F32(_) | Number::F64(_) => None,
    }
}

fn integer_sign(integer: &Integer) -> Ordering {
    if integer.is_zero() {
        Ordering::Equal
    } else if integer.is_negative() {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

pub(crate) fn compare_exact(left: &Number, right: &Number) -> Option<Ordering> {
    left.sub_exact(right).ok().as_ref().and_then(sign)
}

pub(crate) fn is_positive_exact(number: &Number) -> bool {
    sign(number) == Some(Ordering::Greater)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smaller_rational_compares_less() {
        let third = Number::fraction(&Integer::from(1_i64), &Integer::from(3_i64)).unwrap();
        let half = Number::fraction(&Integer::from(1_i64), &Integer::from(2_i64)).unwrap();

        let order = compare_exact(&third, &half);

        assert_eq!(order, Some(Ordering::Less));
    }

    #[test]
    fn machine_number_has_no_exact_order() {
        let order = compare_exact(&Number::F64(1.0), &Number::from(0_i64));

        assert_eq!(order, None);
    }

    #[test]
    fn negative_rational_is_not_positive() {
        let value = Number::fraction(&Integer::from(-1_i64), &Integer::from(2_i64)).unwrap();

        assert!(!is_positive_exact(&value));
    }
}
