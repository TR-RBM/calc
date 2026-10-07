use calc_core::ResultValue;
use calc_expr::ExprPool;
use calc_numbers::Number;
use calc_syntax::parse_expression;

fn numbers_equal(left: &Number, right: &Number) -> bool {
    match (left, right) {
        (Number::F64(left), Number::F64(right)) => {
            left.to_bits() == right.to_bits() || (left.is_nan() && right.is_nan())
        }
        (Number::F32(left), Number::F32(right)) => {
            left.to_bits() == right.to_bits() || (left.is_nan() && right.is_nan())
        }
        (Number::Integer(_), Number::Integer(_)) | (Number::Rational(_), Number::Rational(_)) => {
            left == right
        }
        _ => false,
    }
}

fn expressions_equal(left: &str, right: &str) -> bool {
    let mut pool = ExprPool::new();
    match (
        parse_expression(&mut pool, left),
        parse_expression(&mut pool, right),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

pub fn values_equal(left: &ResultValue, right: &ResultValue) -> bool {
    match (left, right) {
        (ResultValue::Number(left), ResultValue::Number(right)) => numbers_equal(left, right),
        (
            ResultValue::Complex {
                real: left_real,
                imaginary: left_imaginary,
            },
            ResultValue::Complex {
                real: right_real,
                imaginary: right_imaginary,
            },
        ) => numbers_equal(left_real, right_real) && numbers_equal(left_imaginary, right_imaginary),
        (
            ResultValue::Array {
                shape: left_shape,
                elements: left_elements,
            },
            ResultValue::Array {
                shape: right_shape,
                elements: right_elements,
            },
        ) => {
            left_shape == right_shape
                && left_elements.len() == right_elements.len()
                && left_elements
                    .iter()
                    .zip(right_elements)
                    .all(|(left, right)| values_equal(left, right))
        }
        (ResultValue::Expression(left), ResultValue::Expression(right)) => {
            expressions_equal(left, right)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expression(text: &str) -> ResultValue {
        ResultValue::Expression(text.to_owned())
    }

    #[test]
    fn expressions_with_different_spacing_are_equal() {
        assert!(values_equal(
            &expression("10^(100^10)"),
            &expression("10 ^ (100 ^ 10)")
        ));
    }

    #[test]
    fn expressions_with_different_grouping_differ() {
        assert!(!values_equal(
            &expression("(10^100)^10"),
            &expression("10^(100^10)")
        ));
    }

    #[test]
    fn any_nan_equals_any_nan() {
        assert!(values_equal(
            &ResultValue::Number(Number::F64(f64::NAN)),
            &ResultValue::Number(Number::F64(-f64::NAN))
        ));
    }

    #[test]
    fn zeros_of_different_sign_differ() {
        assert!(!values_equal(
            &ResultValue::Number(Number::F64(0.0)),
            &ResultValue::Number(Number::F64(-0.0))
        ));
    }
}
