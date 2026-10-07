use crate::integer::Integer;
use crate::number::Number;

pub(crate) struct DecimalReference {
    pub(crate) value: Number,
    pub(crate) last_digit_unit: Number,
}

fn power_of_ten(exponent: i64) -> Number {
    let magnitude = Integer::from(10i64).pow(u32::try_from(exponent.unsigned_abs()).unwrap());
    if exponent >= 0 {
        Number::Integer(magnitude)
    } else {
        Number::fraction(&Integer::one(), &magnitude).unwrap()
    }
}

pub(crate) fn decimal_reference(text: &str) -> DecimalReference {
    let (mantissa, exponent) = match text.split_once(['E', 'e']) {
        Some((mantissa, exponent)) => (mantissa, exponent.parse::<i64>().unwrap()),
        None => (text, 0),
    };
    let is_negative = mantissa.starts_with('-');
    let unsigned = mantissa.trim_start_matches(['-', '+']);
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let digits = format!("{whole}{fraction}");
    let magnitude = digits.bytes().fold(Integer::zero(), |value, digit| {
        &(&value * &Integer::from(10i64)) + &Integer::from(i64::from(digit - b'0'))
    });
    let signed = if is_negative {
        magnitude.negated()
    } else {
        magnitude
    };
    let scale_exponent = exponent - i64::try_from(fraction.len()).unwrap();
    let last_digit_unit = power_of_ten(scale_exponent);
    DecimalReference {
        value: Number::Integer(signed).mul_exact(&last_digit_unit).unwrap(),
        last_digit_unit,
    }
}

pub(crate) fn is_at_most(left: &Number, right: &Number) -> bool {
    !right.sub_exact(left).unwrap().is_negative_exact()
}

pub(crate) fn absolute_difference(left: &Number, right: &Number) -> Number {
    let difference = left.sub_exact(right).unwrap();
    if difference.is_negative_exact() {
        difference.negate_exact().unwrap()
    } else {
        difference
    }
}
