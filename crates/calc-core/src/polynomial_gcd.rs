use std::collections::BTreeMap;

use calc_numbers::Integer;

pub(crate) type Exponents = Vec<u32>;

pub(crate) type IntegerPolynomial = BTreeMap<Exponents, Integer>;

pub(crate) type Residues = BTreeMap<Exponents, u64>;

const LARGEST_PRIME_BELOW: u64 = 1 << 63;

const WITNESSES: [u64; 12] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];

pub(crate) fn multiplied(left: u64, right: u64, prime: u64) -> u64 {
    let product = u128::from(left) * u128::from(right) % u128::from(prime);
    u64::try_from(product).unwrap_or(0)
}

pub(crate) fn added(left: u64, right: u64, prime: u64) -> u64 {
    let sum = u128::from(left) + u128::from(right);
    u64::try_from(sum % u128::from(prime)).unwrap_or(0)
}

pub(crate) fn subtracted(left: u64, right: u64, prime: u64) -> u64 {
    added(left, prime - right % prime, prime)
}

pub(crate) fn powered(base: u64, exponent: u64, prime: u64) -> u64 {
    let mut result = 1 % prime;
    let mut square = base % prime;
    let mut remaining = exponent;
    while remaining > 0 {
        if remaining & 1 == 1 {
            result = multiplied(result, square, prime);
        }
        square = multiplied(square, square, prime);
        remaining >>= 1;
    }
    result
}

pub(crate) fn inverse(value: u64, prime: u64) -> Option<u64> {
    (!value.is_multiple_of(prime)).then(|| powered(value, prime - 2, prime))
}

fn is_prime(candidate: u64) -> bool {
    if candidate < 2 {
        return false;
    }
    for witness in WITNESSES {
        if candidate == witness {
            return true;
        }
        if candidate.is_multiple_of(witness) {
            return false;
        }
    }
    let mut odd = candidate - 1;
    let mut twos = 0;
    while odd.is_multiple_of(2) {
        odd /= 2;
        twos += 1;
    }
    WITNESSES.iter().all(|witness| {
        let mut value = powered(*witness, odd, candidate);
        if value == 1 || value == candidate - 1 {
            return true;
        }
        for _ in 1..twos {
            value = multiplied(value, value, candidate);
            if value == candidate - 1 {
                return true;
            }
        }
        false
    })
}

pub(crate) struct Primes {
    next: u64,
}

impl Iterator for Primes {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        while self.next > 3 {
            self.next -= 2;
            if is_prime(self.next) {
                return Some(self.next);
            }
        }
        None
    }
}

pub(crate) fn primes() -> Primes {
    Primes {
        next: LARGEST_PRIME_BELOW + 1,
    }
}

pub(crate) fn residue(value: &Integer, prime: u64) -> Option<u64> {
    let (_, remainder) = value.div_rem_euclid(&Integer::from(prime)).ok()?;
    u64::try_from(remainder.to_i128()?).ok()
}

fn reduced(polynomial: &IntegerPolynomial, prime: u64) -> Option<Residues> {
    let mut residues = Residues::new();
    for (exponents, coefficient) in polynomial {
        let value = residue(coefficient, prime)?;
        if value != 0 {
            residues.insert(exponents.clone(), value);
        }
    }
    Some(residues)
}

pub(crate) fn leading(polynomial: &Residues) -> Option<(&Exponents, u64)> {
    polynomial
        .iter()
        .next_back()
        .map(|(exponents, value)| (exponents, *value))
}

pub(crate) fn is_constant(polynomial: &Residues) -> bool {
    polynomial
        .keys()
        .all(|exponents| exponents.iter().all(|exponent| *exponent == 0))
}

pub(crate) fn scaled(polynomial: &Residues, factor: u64, prime: u64) -> Residues {
    polynomial
        .iter()
        .filter_map(|(exponents, value)| {
            let product = multiplied(*value, factor, prime);
            (product != 0).then(|| (exponents.clone(), product))
        })
        .collect()
}

fn accumulate(total: &mut Residues, exponents: Exponents, value: u64, prime: u64) {
    let sum = added(total.get(&exponents).copied().unwrap_or(0), value, prime);
    if sum == 0 {
        total.remove(&exponents);
    } else {
        total.insert(exponents, sum);
    }
}

fn sum(left: &Residues, right: &Residues, prime: u64) -> Residues {
    let mut total = left.clone();
    for (exponents, value) in right {
        accumulate(&mut total, exponents.clone(), *value, prime);
    }
    total
}

fn difference(left: &Residues, right: &Residues, prime: u64) -> Residues {
    sum(left, &scaled(right, prime - 1, prime), prime)
}

fn exponents_product(left: &[u32], right: &[u32]) -> Exponents {
    left.iter().zip(right).map(|(one, two)| one + two).collect()
}

fn product(left: &Residues, right: &Residues, prime: u64) -> Residues {
    let mut total = Residues::new();
    for (one, first) in left {
        for (two, second) in right {
            accumulate(
                &mut total,
                exponents_product(one, two),
                multiplied(*first, *second, prime),
                prime,
            );
        }
    }
    total
}

fn quotient_of_monomials(dividend: &[u32], divisor: &[u32]) -> Option<Exponents> {
    dividend
        .iter()
        .zip(divisor)
        .map(|(one, two)| one.checked_sub(*two))
        .collect()
}

fn divided_exactly(dividend: &Residues, divisor: &Residues, prime: u64) -> Option<Residues> {
    let (divisor_leading, divisor_value) = leading(divisor)?;
    let scale = inverse(divisor_value, prime)?;
    let mut remainder = dividend.clone();
    let mut quotient = Residues::new();
    while let Some((exponents, value)) = leading(&remainder) {
        let shift = quotient_of_monomials(exponents, divisor_leading)?;
        let factor = multiplied(value, scale, prime);
        let term: Residues = [(shift.clone(), factor)].into_iter().collect();
        remainder = difference(&remainder, &product(&term, divisor, prime), prime);
        accumulate(&mut quotient, shift, factor, prime);
    }
    Some(quotient)
}

fn dense_trimmed(mut coefficients: Vec<u64>) -> Vec<u64> {
    while coefficients.last() == Some(&0) {
        coefficients.pop();
    }
    coefficients
}

fn dense_remainder(dividend: &[u64], divisor: &[u64], prime: u64) -> Option<Vec<u64>> {
    let scale = inverse(*divisor.last()?, prime)?;
    let mut remainder = dense_trimmed(dividend.to_vec());
    while remainder.len() >= divisor.len() {
        let shift = remainder.len() - divisor.len();
        let factor = multiplied(*remainder.last()?, scale, prime);
        for (index, value) in divisor.iter().enumerate() {
            remainder[shift + index] = subtracted(
                remainder[shift + index],
                multiplied(factor, *value, prime),
                prime,
            );
        }
        remainder = dense_trimmed(remainder);
    }
    Some(remainder)
}

fn dense_monic(coefficients: &[u64], prime: u64) -> Option<Vec<u64>> {
    let scale = inverse(*coefficients.last()?, prime)?;
    Some(
        coefficients
            .iter()
            .map(|value| multiplied(*value, scale, prime))
            .collect(),
    )
}

fn dense_gcd(left: &[u64], right: &[u64], prime: u64) -> Option<Vec<u64>> {
    let mut first = dense_trimmed(left.to_vec());
    let mut second = dense_trimmed(right.to_vec());
    while !second.is_empty() {
        let remainder = dense_remainder(&first, &second, prime)?;
        first = second;
        second = remainder;
    }
    if first.is_empty() {
        return Some(first);
    }
    dense_monic(&first, prime)
}

fn dense_quotient(dividend: &[u64], divisor: &[u64], prime: u64) -> Option<Vec<u64>> {
    let scale = inverse(*divisor.last()?, prime)?;
    let mut remainder = dense_trimmed(dividend.to_vec());
    if remainder.len() < divisor.len() {
        return remainder.is_empty().then(Vec::new);
    }
    let mut quotient = vec![0; remainder.len() - divisor.len() + 1];
    while remainder.len() >= divisor.len() {
        let shift = remainder.len() - divisor.len();
        let factor = multiplied(*remainder.last()?, scale, prime);
        quotient[shift] = factor;
        for (index, value) in divisor.iter().enumerate() {
            remainder[shift + index] = subtracted(
                remainder[shift + index],
                multiplied(factor, *value, prime),
                prime,
            );
        }
        remainder = dense_trimmed(remainder);
    }
    remainder.is_empty().then_some(quotient)
}

fn dense_value(coefficients: &[u64], at: u64, prime: u64) -> u64 {
    coefficients.iter().rev().fold(0, |total, coefficient| {
        added(multiplied(total, at, prime), *coefficient, prime)
    })
}

fn slot(exponent: u32) -> Option<usize> {
    usize::try_from(exponent).ok()
}

fn in_variable(polynomial: &Residues, variable: usize) -> Option<BTreeMap<Exponents, Vec<u64>>> {
    let mut groups: BTreeMap<Exponents, Vec<u64>> = BTreeMap::new();
    for (exponents, value) in polynomial {
        let mut rest = exponents.clone();
        let degree = slot(*rest.get(variable)?)?;
        rest[variable] = 0;
        let coefficients = groups.entry(rest).or_default();
        if coefficients.len() <= degree {
            coefficients.resize(degree + 1, 0);
        }
        coefficients[degree] = *value;
    }
    Some(groups)
}

fn from_groups(groups: &BTreeMap<Exponents, Vec<u64>>, variable: usize) -> Option<Residues> {
    let mut polynomial = Residues::new();
    for (rest, coefficients) in groups {
        for (degree, value) in coefficients.iter().enumerate() {
            if *value != 0 {
                let mut exponents = rest.clone();
                exponents[variable] = u32::try_from(degree).ok()?;
                polynomial.insert(exponents, *value);
            }
        }
    }
    Some(polynomial)
}

fn content_in(polynomial: &Residues, variable: usize, prime: u64) -> Option<Vec<u64>> {
    let mut content: Vec<u64> = Vec::new();
    for coefficients in in_variable(polynomial, variable)?.values() {
        content = dense_gcd(&content, coefficients, prime)?;
    }
    Some(content)
}

fn without_content(
    polynomial: &Residues,
    content: &[u64],
    variable: usize,
    prime: u64,
) -> Option<Residues> {
    let groups = in_variable(polynomial, variable)?
        .into_iter()
        .map(|(rest, coefficients)| {
            dense_quotient(&coefficients, content, prime).map(|quotient| (rest, quotient))
        })
        .collect::<Option<BTreeMap<Exponents, Vec<u64>>>>()?;
    from_groups(&groups, variable)
}

fn leading_in(polynomial: &Residues, variable: usize) -> Option<Vec<u64>> {
    in_variable(polynomial, variable)?
        .into_iter()
        .next_back()
        .map(|(_, coefficients)| coefficients)
}

fn degree_in(polynomial: &Residues, variable: usize) -> u32 {
    polynomial
        .keys()
        .filter_map(|exponents| exponents.get(variable).copied())
        .max()
        .unwrap_or(0)
}

fn evaluated(polynomial: &Residues, variable: usize, at: u64, prime: u64) -> Residues {
    let mut image = Residues::new();
    for (exponents, value) in polynomial {
        let mut rest = exponents.clone();
        let power = powered(at, u64::from(rest[variable]), prime);
        rest[variable] = 0;
        accumulate(&mut image, rest, multiplied(*value, power, prime), prime);
    }
    image
}

fn times_dense(
    polynomial: &Residues,
    dense: &[u64],
    variable: usize,
    prime: u64,
) -> Option<Residues> {
    let mut total = Residues::new();
    for (degree, factor) in dense.iter().enumerate() {
        if *factor == 0 {
            continue;
        }
        let shift = u32::try_from(degree).ok()?;
        for (exponents, value) in polynomial {
            let mut moved = exponents.clone();
            moved[variable] = moved[variable].checked_add(shift)?;
            accumulate(&mut total, moved, multiplied(*value, *factor, prime), prime);
        }
    }
    Some(total)
}

fn dense_times_linear(dense: &[u64], root: u64, prime: u64) -> Vec<u64> {
    let mut result = vec![0; dense.len() + 1];
    for (degree, value) in dense.iter().enumerate() {
        result[degree + 1] = added(result[degree + 1], *value, prime);
        result[degree] = subtracted(result[degree], multiplied(*value, root, prime), prime);
    }
    result
}

fn interpolated(points: &[(u64, Residues)], variable: usize, prime: u64) -> Option<Residues> {
    let ((first_point, first_image), rest) = points.split_first()?;
    let mut interpolant = first_image.clone();
    let mut basis = dense_times_linear(&[1], *first_point, prime);
    for (point, image) in rest {
        let current = evaluated(&interpolant, variable, *point, prime);
        let correction = difference(image, &current, prime);
        let scale = inverse(dense_value(&basis, *point, prime), prime)?;
        let step = times_dense(&scaled(&correction, scale, prime), &basis, variable, prime)?;
        interpolant = sum(&interpolant, &step, prime);
        basis = dense_times_linear(&basis, *point, prime);
    }
    Some(interpolant)
}

fn with_dense(dense: &[u64], width: usize, variable: usize) -> Option<Residues> {
    let mut polynomial = Residues::new();
    for (degree, value) in dense.iter().enumerate() {
        if *value != 0 {
            let mut exponents = vec![0; width];
            exponents[variable] = u32::try_from(degree).ok()?;
            polynomial.insert(exponents, *value);
        }
    }
    Some(polynomial)
}

fn prefix_leading(polynomial: &Residues, variable: usize) -> Option<Exponents> {
    polynomial.keys().next_back().map(|exponents| {
        let mut prefix = exponents.clone();
        for exponent in prefix.iter_mut().skip(variable) {
            *exponent = 0;
        }
        prefix
    })
}

pub(crate) fn modular_gcd(
    left: &Residues,
    right: &Residues,
    count: usize,
    prime: u64,
) -> Option<Residues> {
    let width = left.keys().chain(right.keys()).next()?.len();
    if count <= 1 {
        let to_dense = |polynomial: &Residues| -> Option<Vec<u64>> {
            let groups = in_variable(polynomial, 0)?;
            Some(groups.into_values().next().unwrap_or_default())
        };
        let gcd = dense_gcd(&to_dense(left)?, &to_dense(right)?, prime)?;
        return with_dense(&gcd, width, 0);
    }
    let variable = count - 1;
    let left_content = content_in(left, variable, prime)?;
    let right_content = content_in(right, variable, prime)?;
    let content = dense_gcd(&left_content, &right_content, prime)?;
    let left = without_content(left, &left_content, variable, prime)?;
    let right = without_content(right, &right_content, variable, prime)?;
    let left_leading = leading_in(&left, variable)?;
    let right_leading = leading_in(&right, variable)?;
    let scale = dense_gcd(&left_leading, &right_leading, prime)?;
    let needed = degree_in(&left, variable).min(degree_in(&right, variable))
        + u32::try_from(scale.len().saturating_sub(1)).ok()?;
    let with_content = |polynomial: &Residues| times_dense(polynomial, &content, variable, prime);
    let mut points: Vec<(u64, Residues)> = Vec::new();
    let mut shape: Option<Exponents> = None;
    for point in 1..prime {
        if dense_value(&left_leading, point, prime) == 0
            || dense_value(&right_leading, point, prime) == 0
        {
            continue;
        }
        let left_image = evaluated(&left, variable, point, prime);
        let right_image = evaluated(&right, variable, point, prime);
        let image = modular_gcd(&left_image, &right_image, variable, prime)?;
        if is_constant(&image) {
            return with_content(&with_dense(&[1], width, 0)?);
        }
        let (_, image_leading) = leading(&image)?;
        let factor = multiplied(
            dense_value(&scale, point, prime),
            inverse(image_leading, prime)?,
            prime,
        );
        let image = scaled(&image, factor, prime);
        let image_shape = prefix_leading(&image, variable)?;
        match &shape {
            Some(current) if image_shape > *current => continue,
            Some(current) if image_shape == *current => points.push((point, image)),
            _ => {
                shape = Some(image_shape);
                points = vec![(point, image)];
            }
        }
        if points.len() > slot(needed)? {
            let candidate = interpolated(&points, variable, prime)?;
            let candidate_content = content_in(&candidate, variable, prime)?;
            let candidate = without_content(&candidate, &candidate_content, variable, prime)?;
            let divides = |polynomial: &Residues| {
                divided_exactly(polynomial, &candidate, prime)
                    .is_some_and(|quotient| product(&quotient, &candidate, prime) == *polynomial)
            };
            if divides(&left) && divides(&right) {
                return with_content(&candidate);
            }
            points.clear();
            shape = None;
        }
    }
    None
}

fn content_of(polynomial: &IntegerPolynomial) -> Integer {
    polynomial
        .values()
        .fold(Integer::zero(), |content, coefficient| {
            content.gcd(coefficient)
        })
}

fn exact_quotient(dividend: &Integer, divisor: &Integer) -> Option<Integer> {
    let (quotient, remainder) = dividend.div_rem_euclid(divisor).ok()?;
    remainder.is_zero().then_some(quotient)
}

fn divided_by_integer(
    polynomial: &IntegerPolynomial,
    divisor: &Integer,
) -> Option<IntegerPolynomial> {
    polynomial
        .iter()
        .map(|(exponents, coefficient)| {
            exact_quotient(coefficient, divisor).map(|quotient| (exponents.clone(), quotient))
        })
        .collect()
}

fn integer_product(left: &IntegerPolynomial, right: &IntegerPolynomial) -> IntegerPolynomial {
    let mut total = IntegerPolynomial::new();
    for (one, first) in left {
        for (two, second) in right {
            let exponents = exponents_product(one, two);
            let value = total
                .get(&exponents)
                .map_or_else(|| first * second, |existing| existing + &(first * second));
            if value.is_zero() {
                total.remove(&exponents);
            } else {
                total.insert(exponents, value);
            }
        }
    }
    total
}

pub(crate) fn integer_quotient(
    dividend: &IntegerPolynomial,
    divisor: &IntegerPolynomial,
) -> Option<IntegerPolynomial> {
    let (divisor_leading, divisor_value) = divisor.iter().next_back()?;
    let mut remainder = dividend.clone();
    let mut quotient = IntegerPolynomial::new();
    while let Some((exponents, value)) = remainder.iter().next_back() {
        let shift = quotient_of_monomials(exponents, divisor_leading)?;
        let factor = exact_quotient(value, divisor_value)?;
        let term: IntegerPolynomial = [(shift.clone(), factor.clone())].into_iter().collect();
        let subtrahend = integer_product(&term, divisor);
        for (exponents, value) in subtrahend {
            let rest = remainder
                .get(&exponents)
                .map_or_else(|| value.negated(), |existing| existing - &value);
            if rest.is_zero() {
                remainder.remove(&exponents);
            } else {
                remainder.insert(exponents, rest);
            }
        }
        quotient.insert(shift, factor);
    }
    Some(quotient)
}

fn combined(
    known: &IntegerPolynomial,
    modulus: &Integer,
    image: &Residues,
    prime: u64,
) -> Option<IntegerPolynomial> {
    let prime_integer = Integer::from(prime);
    let modulus_inverse = inverse(residue(modulus, prime)?, prime)?;
    let product = modulus * &prime_integer;
    let (half, _) = product.div_rem_euclid(&Integer::from(2_i64)).ok()?;
    let mut keys: Vec<&Exponents> = known.keys().chain(image.keys()).collect();
    keys.sort();
    keys.dedup();
    let mut result = IntegerPolynomial::new();
    for exponents in keys {
        let old = known.get(exponents).cloned().unwrap_or_else(Integer::zero);
        let new = image.get(exponents).copied().unwrap_or(0);
        let step = multiplied(
            subtracted(new, residue(&old, prime)?, prime),
            modulus_inverse,
            prime,
        );
        let mut value = &old + &(modulus * &Integer::from(step));
        let (_, positive) = value.div_rem_euclid(&product).ok()?;
        value = if positive > half {
            &positive - &product
        } else {
            positive
        };
        if !value.is_zero() {
            result.insert(exponents.clone(), value);
        }
    }
    Some(result)
}

fn norm_bits(polynomial: &IntegerPolynomial) -> u64 {
    let total = polynomial
        .values()
        .fold(Integer::zero(), |total, coefficient| {
            &total + &coefficient.absolute()
        });
    total.bit_length()
}

fn kronecker_degree(polynomial: &IntegerPolynomial) -> u64 {
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

fn is_constant_integer(polynomial: &IntegerPolynomial) -> bool {
    polynomial
        .keys()
        .all(|exponents| exponents.iter().all(|exponent| *exponent == 0))
}

fn constant_polynomial(value: Integer, width: usize) -> IntegerPolynomial {
    [(vec![0; width], value)].into_iter().collect()
}

pub(crate) fn integer_gcd(
    left: &IntegerPolynomial,
    right: &IntegerPolynomial,
) -> Option<IntegerPolynomial> {
    let width = left.keys().chain(right.keys()).next()?.len();
    if left.is_empty() || right.is_empty() {
        let other = if left.is_empty() { right } else { left };
        let content = content_of(other);
        return divided_by_integer(other, &content);
    }
    let left_content = content_of(left);
    let right_content = content_of(right);
    let content = left_content.gcd(&right_content);
    let left = divided_by_integer(left, &left_content)?;
    let right = divided_by_integer(right, &right_content)?;
    if is_constant_integer(&left) || is_constant_integer(&right) {
        return Some(constant_polynomial(content, width));
    }
    let (_, left_leading) = left.iter().next_back()?;
    let (_, right_leading) = right.iter().next_back()?;
    let scale = left_leading.gcd(right_leading);
    let smaller = if norm_bits(&left) <= norm_bits(&right) {
        &left
    } else {
        &right
    };
    let bound_bits = scale
        .bit_length()
        .saturating_add(kronecker_degree(smaller))
        .saturating_add(norm_bits(smaller))
        .saturating_add(1);
    let mut known = IntegerPolynomial::new();
    let mut modulus = Integer::one();
    let mut shape: Option<Exponents> = None;
    for prime in primes() {
        if residue(left_leading, prime)? == 0 || residue(right_leading, prime)? == 0 {
            continue;
        }
        let image = modular_gcd(
            &reduced(&left, prime)?,
            &reduced(&right, prime)?,
            width,
            prime,
        )?;
        if is_constant(&image) {
            return Some(constant_polynomial(content, width));
        }
        let (image_shape, image_leading) = leading(&image)?;
        let image_shape = image_shape.clone();
        let factor = multiplied(
            residue(&scale, prime)?,
            inverse(image_leading, prime)?,
            prime,
        );
        let image = scaled(&image, factor, prime);
        match &shape {
            Some(current) if image_shape > *current => continue,
            Some(current) if image_shape == *current => {}
            _ => {
                shape = Some(image_shape);
                known = IntegerPolynomial::new();
                modulus = Integer::one();
            }
        }
        let next = combined(&known, &modulus, &image, prime)?;
        let is_stable = next == known;
        known = next;
        modulus = &modulus * &Integer::from(prime);
        let is_past_bound = modulus.bit_length() > bound_bits;
        if is_stable || is_past_bound {
            let candidate = divided_by_integer(&known, &content_of(&known))?;
            let divides = |polynomial: &IntegerPolynomial| {
                integer_quotient(polynomial, &candidate)
                    .is_some_and(|quotient| integer_product(&quotient, &candidate) == *polynomial)
            };
            if divides(&left) && divides(&right) {
                let scaled = integer_product(&candidate, &constant_polynomial(content, width));
                return Some(scaled);
            }
            if is_past_bound {
                return None;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn polynomial(terms: &[(&[u32], i64)]) -> IntegerPolynomial {
        terms
            .iter()
            .map(|(exponents, value)| (exponents.to_vec(), Integer::from(*value)))
            .collect()
    }

    fn power(base: &IntegerPolynomial, exponent: u32) -> IntegerPolynomial {
        let width = base.keys().next().map_or(0, Vec::len);
        (0..exponent).fold(constant_polynomial(Integer::one(), width), |total, _| {
            integer_product(&total, base)
        })
    }

    fn up_to_sign(left: &IntegerPolynomial, right: &IntegerPolynomial) -> bool {
        let negated: IntegerPolynomial = right
            .iter()
            .map(|(exponents, value)| (exponents.clone(), value.negated()))
            .collect();
        left == right || *left == negated
    }

    #[test]
    fn the_prime_search_starts_below_two_to_the_sixty_third() {
        assert_eq!(primes().next(), Some(9_223_372_036_854_775_783));
    }

    #[test]
    fn a_strong_pseudoprime_to_small_bases_is_not_prime() {
        assert!(!is_prime(3_215_031_751));
        assert!(is_prime(1_000_000_007));
    }

    #[test]
    fn the_gcd_of_x_squared_minus_one_and_x_minus_one_is_x_minus_one() {
        let left = polynomial(&[(&[2], 1), (&[0], -1)]);
        let right = polynomial(&[(&[1], 1), (&[0], -1)]);

        assert!(up_to_sign(&integer_gcd(&left, &right).unwrap(), &right));
    }

    #[test]
    fn coprime_polynomials_have_the_gcd_one() {
        let left = polynomial(&[(&[1], 1), (&[0], -1)]);
        let right = polynomial(&[(&[1], 1), (&[0], 1)]);

        assert_eq!(integer_gcd(&left, &right), Some(polynomial(&[(&[0], 1)])));
    }

    #[test]
    fn the_integer_content_is_kept_in_the_gcd() {
        let left = polynomial(&[(&[1], 6), (&[0], 6)]);
        let right = polynomial(&[(&[1], 4), (&[0], 4)]);

        assert_eq!(
            integer_gcd(&left, &right),
            Some(polynomial(&[(&[1], 2), (&[0], 2)]))
        );
    }

    #[test]
    fn the_gcd_in_two_names_is_the_common_factor() {
        let left = polynomial(&[(&[2, 0], 1), (&[0, 2], -1)]);
        let right = polynomial(&[(&[1, 0], 1), (&[0, 1], -1)]);

        assert!(up_to_sign(&integer_gcd(&left, &right).unwrap(), &right));
    }

    #[test]
    fn a_common_factor_in_three_names_is_found_at_degree_eight() {
        let common = polynomial(&[
            (&[1, 0, 0], 1),
            (&[0, 1, 0], 1),
            (&[0, 0, 1], 1),
            (&[0, 0, 0], 1),
        ]);
        let one = polynomial(&[(&[1, 0, 0], 1), (&[0, 1, 0], -1), (&[0, 0, 0], 2)]);
        let two = polynomial(&[(&[1, 0, 0], 1), (&[0, 0, 1], 1), (&[0, 0, 0], -3)]);
        let shared = power(&common, 4);
        let left = integer_product(&shared, &power(&one, 4));
        let right = integer_product(&shared, &power(&two, 4));

        assert!(up_to_sign(&integer_gcd(&left, &right).unwrap(), &shared));
    }

    #[test]
    fn a_common_factor_that_leads_in_the_second_name_is_found() {
        let common = polynomial(&[(&[0, 2], 1), (&[1, 0], 1), (&[0, 0], 1)]);
        let one = polynomial(&[(&[1, 1], 1), (&[0, 0], 3)]);
        let two = polynomial(&[(&[1, 0], 1), (&[0, 1], -2)]);
        let left = integer_product(&common, &one);
        let right = integer_product(&common, &two);

        assert!(up_to_sign(&integer_gcd(&left, &right).unwrap(), &common));
    }

    #[test]
    fn a_polynomial_is_divided_exactly_by_its_factor() {
        let left = polynomial(&[(&[2], 1), (&[0], -1)]);
        let right = polynomial(&[(&[1], 1), (&[0], -1)]);

        assert_eq!(
            integer_quotient(&left, &right),
            Some(polynomial(&[(&[1], 1), (&[0], 1)]))
        );
    }

    #[test]
    fn a_polynomial_that_is_not_a_multiple_has_no_exact_quotient() {
        let left = polynomial(&[(&[2], 1), (&[0], 1)]);
        let right = polynomial(&[(&[1], 1), (&[0], -1)]);

        assert_eq!(integer_quotient(&left, &right), None);
    }
}
