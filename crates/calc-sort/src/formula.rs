use calc_numbers::{ExactArithmeticError, Integer, Number};

use crate::method::Gaps;
use crate::shell::gaps_for;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Formula {
    Length,
    Whole(i64),
    Harmonic,
    Sum(Vec<Formula>),
    Difference(Box<Formula>, Box<Formula>),
    Product(Vec<Formula>),
    Quotient(Box<Formula>, Box<Formula>),
    Index,
    SumOver { from: u64, body: Box<Formula> },
    FloorLog2(Box<Formula>),
    CeilLog2(Box<Formula>),
    PowerOfTwo(Box<Formula>),
    BitCount(Box<Formula>),
    MergeAverage,
    Floor(Box<Formula>),
    GapSum(Gaps),
}

impl Formula {
    pub fn value_at(&self, length: u64) -> Result<Number, ExactArithmeticError> {
        self.evaluated(length, None)
    }

    fn evaluated(&self, length: u64, index: Option<u64>) -> Result<Number, ExactArithmeticError> {
        match self {
            Self::Length => Ok(Number::Integer(Integer::from(length))),
            Self::Whole(whole) => Ok(Number::Integer(Integer::from(*whole))),
            Self::Harmonic => harmonic(length),
            Self::Sum(terms) => terms.iter().try_fold(zero(), |total, term| {
                total.add_exact(&term.evaluated(length, index)?)
            }),
            Self::Difference(left, right) => left
                .evaluated(length, index)?
                .sub_exact(&right.evaluated(length, index)?),
            Self::Product(factors) => factors.iter().try_fold(one(), |total, factor| {
                total.mul_exact(&factor.evaluated(length, index)?)
            }),
            Self::Quotient(numerator, denominator) => numerator
                .evaluated(length, index)?
                .div_exact(&denominator.evaluated(length, index)?),
            Self::Index => Ok(Number::Integer(Integer::from(index.unwrap_or(0)))),
            Self::SumOver { from, body } => (*from..=length).try_fold(zero(), |total, k| {
                total.add_exact(&body.evaluated(length, Some(k))?)
            }),
            Self::FloorLog2(argument) => {
                let whole = whole_of(&argument.evaluated(length, index)?)?;
                Ok(Number::Integer(Integer::from(
                    whole.bit_length().saturating_sub(1),
                )))
            }
            Self::CeilLog2(argument) => {
                let whole = whole_of(&argument.evaluated(length, index)?)?;
                let below = &whole - &Integer::one();
                let bits = if below.is_negative() || below.is_zero() {
                    0
                } else {
                    below.bit_length()
                };
                Ok(Number::Integer(Integer::from(bits)))
            }
            Self::BitCount(argument) => {
                let whole = whole_of(&argument.evaluated(length, index)?)?;
                Ok(Number::Integer(Integer::from(bit_count(&whole))))
            }
            Self::MergeAverage => merge_average(length),
            Self::Floor(argument) => floor(&argument.evaluated(length, index)?),
            Self::GapSum(gaps) => {
                let length =
                    usize::try_from(length).map_err(|_| ExactArithmeticError::MachineOperand)?;
                let total: usize = gaps_for(*gaps, length).iter().map(|gap| length - gap).sum();
                Ok(Number::Integer(Integer::from(
                    u64::try_from(total).map_err(|_| ExactArithmeticError::MachineOperand)?,
                )))
            }
            Self::PowerOfTwo(argument) => {
                let whole = whole_of(&argument.evaluated(length, index)?)?;
                let exponent = whole
                    .to_i64()
                    .and_then(|exponent| usize::try_from(exponent).ok())
                    .ok_or(ExactArithmeticError::MachineOperand)?;
                Ok(Number::Integer(Integer::one().shifted_left(exponent)))
            }
        }
    }
}

fn bit_count(whole: &Integer) -> u64 {
    let mut remaining = whole.absolute();
    let mut count = 0;
    while !remaining.is_zero() {
        if remaining
            .bit_and(&Integer::one())
            .is_some_and(|bit| bit.is_one())
        {
            count += 1;
        }
        remaining = remaining.shifted_right(1).unwrap_or_else(Integer::zero);
    }
    count
}

fn merge_average(length: u64) -> Result<Number, ExactArithmeticError> {
    let mut known: std::collections::BTreeMap<u64, Number> = std::collections::BTreeMap::new();
    let mut pending = vec![length];
    while let Some(size) = pending.pop() {
        if known.contains_key(&size) {
            continue;
        }
        if size < 2 {
            known.insert(size, zero());
            continue;
        }
        let (larger, smaller) = (size.div_ceil(2), size / 2);
        let (Some(left), Some(right)) = (known.get(&larger), known.get(&smaller)) else {
            pending.push(size);
            pending.push(larger);
            pending.push(smaller);
            continue;
        };
        let merges = Number::Integer(Integer::from(size))
            .sub_exact(&Number::fraction(
                &Integer::from(larger),
                &Integer::from(smaller + 1),
            )?)?
            .sub_exact(&Number::fraction(
                &Integer::from(smaller),
                &Integer::from(larger + 1),
            )?)?;
        let value = left.add_exact(right)?.add_exact(&merges)?;
        known.insert(size, value);
    }
    Ok(known.remove(&length).unwrap_or_else(zero))
}

fn whole_of(number: &Number) -> Result<Integer, ExactArithmeticError> {
    match number {
        Number::Integer(integer) => Ok(integer.clone()),
        Number::Rational(_) | Number::F32(_) | Number::F64(_) => {
            Err(ExactArithmeticError::MachineOperand)
        }
    }
}

fn floor(number: &Number) -> Result<Number, ExactArithmeticError> {
    match number {
        Number::Integer(integer) => Ok(Number::Integer(integer.clone())),
        Number::Rational(rational) => {
            let (quotient, _) = rational
                .numerator()
                .div_rem_euclid(rational.denominator())
                .map_err(|_| ExactArithmeticError::MachineOperand)?;
            Ok(Number::Integer(quotient))
        }
        Number::F32(_) | Number::F64(_) => Err(ExactArithmeticError::MachineOperand),
    }
}

fn zero() -> Number {
    Number::Integer(Integer::zero())
}

fn one() -> Number {
    Number::Integer(Integer::one())
}

fn harmonic(length: u64) -> Result<Number, ExactArithmeticError> {
    (1..=length).try_fold(zero(), |total, denominator| {
        total.add_exact(&Number::fraction(
            &Integer::one(),
            &Integer::from(denominator),
        )?)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fraction(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    #[test]
    fn harmonic_number_of_three_is_eleven_sixths() {
        assert_eq!(Formula::Harmonic.value_at(3).unwrap(), fraction(11, 6));
    }

    #[test]
    fn harmonic_number_of_zero_is_zero() {
        assert_eq!(Formula::Harmonic.value_at(0).unwrap(), fraction(0, 1));
    }

    #[test]
    fn ceiling_of_log2_counts_the_bits_below() {
        let values: Vec<Number> = (1..=9)
            .map(|length| {
                Formula::CeilLog2(Box::new(Formula::Length))
                    .value_at(length)
                    .unwrap()
            })
            .collect();

        let expected: Vec<Number> = [0_i64, 1, 2, 2, 3, 3, 3, 3, 4]
            .into_iter()
            .map(|bits| Number::Integer(Integer::from(bits)))
            .collect();
        assert_eq!(values, expected);
    }

    #[test]
    fn floor_of_log2_of_one_is_zero() {
        assert_eq!(
            Formula::FloorLog2(Box::new(Formula::Whole(1)))
                .value_at(0)
                .unwrap(),
            fraction(0, 1)
        );
    }

    #[test]
    fn a_sum_over_the_index_adds_from_its_start_to_the_length() {
        let formula = Formula::SumOver {
            from: 2,
            body: Box::new(Formula::Index),
        };

        assert_eq!(formula.value_at(4).unwrap(), fraction(9, 1));
    }

    #[test]
    fn quotient_stays_an_exact_fraction() {
        let formula = Formula::Quotient(Box::new(Formula::Length), Box::new(Formula::Whole(4)));

        assert_eq!(formula.value_at(6).unwrap(), fraction(3, 2));
    }
}
