use crate::integer::Integer;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Rational {
    numerator: Integer,
    denominator: Integer,
}

pub(crate) enum ReducedFraction {
    Integer(Integer),
    Rational(Rational),
}

impl Rational {
    pub fn numerator(&self) -> &Integer {
        &self.numerator
    }

    pub fn denominator(&self) -> &Integer {
        &self.denominator
    }

    pub(crate) fn reduce(numerator: &Integer, denominator: &Integer) -> Option<ReducedFraction> {
        if denominator.is_zero() {
            return None;
        }
        let common = numerator.gcd(denominator);
        let (numerator_quotient, _) = numerator.div_rem_euclid(&common).ok()?;
        let (denominator_quotient, _) = denominator.div_rem_euclid(&common).ok()?;
        let (numerator, denominator) = if denominator_quotient.is_negative() {
            (numerator_quotient.negated(), denominator_quotient.negated())
        } else {
            (numerator_quotient, denominator_quotient)
        };
        if denominator.is_one() {
            return Some(ReducedFraction::Integer(numerator));
        }
        Some(ReducedFraction::Rational(Self {
            numerator,
            denominator,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reduce(numerator: i64, denominator: i64) -> Option<ReducedFraction> {
        Rational::reduce(&Integer::from(numerator), &Integer::from(denominator))
    }

    #[test]
    fn fraction_is_reduced_to_lowest_terms() {
        let Some(ReducedFraction::Rational(rational)) = reduce(6, 8) else {
            panic!();
        };

        assert_eq!(rational.numerator(), &Integer::from(3i64));
        assert_eq!(rational.denominator(), &Integer::from(4i64));
    }

    #[test]
    fn negative_denominator_moves_sign_to_numerator() {
        let Some(ReducedFraction::Rational(rational)) = reduce(3, -4) else {
            panic!();
        };

        assert_eq!(rational.numerator(), &Integer::from(-3i64));
        assert_eq!(rational.denominator(), &Integer::from(4i64));
    }

    #[test]
    fn denominator_one_gives_integer() {
        assert!(matches!(
            reduce(-12, 4),
            Some(ReducedFraction::Integer(value)) if value == Integer::from(-3i64)
        ));
    }

    #[test]
    fn zero_numerator_gives_integer_zero() {
        assert!(matches!(
            reduce(0, -5),
            Some(ReducedFraction::Integer(value)) if value.is_zero()
        ));
    }

    #[test]
    fn zero_denominator_gives_none() {
        assert!(reduce(1, 0).is_none());
    }
}
