use calc_numbers::Integer;

use crate::exact_rational::ExactRational;

const SMALL_PRIMES: [i64; 24] = [
    3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RationalFactoring {
    pub(crate) constant: ExactRational,
    pub(crate) factors: Vec<(Vec<Integer>, u32)>,
}

fn int(value: i64) -> Integer {
    Integer::from(value)
}

fn trimmed_integers(mut coefficients: Vec<Integer>) -> Vec<Integer> {
    while coefficients.last().is_some_and(Integer::is_zero) {
        coefficients.pop();
    }
    coefficients
}

fn content(coefficients: &[Integer]) -> Integer {
    coefficients
        .iter()
        .fold(Integer::zero(), |common, coefficient| {
            common.gcd(coefficient)
        })
}

fn divided_exactly(coefficients: &[Integer], divisor: &Integer) -> Vec<Integer> {
    coefficients
        .iter()
        .map(|coefficient| {
            coefficient
                .div_rem_euclid(divisor)
                .map(|(quotient, _)| quotient)
                .unwrap_or_else(|_| coefficient.clone())
        })
        .collect()
}

fn primitive(coefficients: &[Integer]) -> Vec<Integer> {
    let common = content(coefficients);
    let mut part = if common.is_zero() || common.is_one() {
        coefficients.to_vec()
    } else {
        divided_exactly(coefficients, &common)
    };
    if part.last().is_some_and(Integer::is_negative) {
        part = part.iter().map(Integer::negated).collect();
    }
    part
}

fn product(left: &[Integer], right: &[Integer]) -> Vec<Integer> {
    if left.is_empty() || right.is_empty() {
        return Vec::new();
    }
    let mut result = vec![Integer::zero(); left.len() + right.len() - 1];
    for (i, a) in left.iter().enumerate() {
        for (j, b) in right.iter().enumerate() {
            result[i + j] = &result[i + j] + &(a * b);
        }
    }
    result
}

fn exact_quotient(dividend: &[Integer], divisor: &[Integer]) -> Option<Vec<Integer>> {
    let divisor = trimmed_integers(divisor.to_vec());
    let leading = divisor.last()?.clone();
    let mut rest = trimmed_integers(dividend.to_vec());
    if rest.len() < divisor.len() {
        return rest.is_empty().then(Vec::new);
    }
    let mut quotient = vec![Integer::zero(); rest.len() - divisor.len() + 1];
    while rest.len() >= divisor.len() {
        let shift = rest.len() - divisor.len();
        let top = rest.last()?.clone();
        let (factor, remainder) = top.div_rem_euclid(&leading).ok()?;
        if !remainder.is_zero() {
            return None;
        }
        for (position, coefficient) in divisor.iter().enumerate() {
            rest[shift + position] = &rest[shift + position] - &(&factor * coefficient);
        }
        quotient[shift] = factor;
        rest.pop();
        rest = trimmed_integers(rest);
        if rest.is_empty() {
            break;
        }
    }
    rest.is_empty().then_some(quotient)
}

fn rational_polynomial(coefficients: &[Integer]) -> Vec<ExactRational> {
    coefficients
        .iter()
        .map(|coefficient| ExactRational::from_integer(coefficient.clone()))
        .collect()
}

fn integer_polynomial(coefficients: &[ExactRational]) -> Vec<Integer> {
    let mut multiple = Integer::one();
    for coefficient in coefficients {
        let denominator = coefficient.denominator();
        let common = multiple.gcd(denominator);
        let quotient = denominator
            .div_rem_euclid(&common)
            .map(|(quotient, _)| quotient)
            .unwrap_or_else(|_| Integer::one());
        multiple = &multiple * &quotient;
    }
    primitive(
        &coefficients
            .iter()
            .map(|coefficient| {
                coefficient
                    .multiply(&ExactRational::from_integer(multiple.clone()))
                    .numerator()
                    .clone()
            })
            .collect::<Vec<_>>(),
    )
}

pub(crate) fn square_free_parts(polynomial: &[Integer]) -> Option<Vec<(Vec<Integer>, u32)>> {
    let rational = rational_polynomial(polynomial);
    let derivative = crate::real_roots::derivative(&rational);
    let mut common = crate::real_roots::greatest_common_divisor(&rational, &derivative)?;
    let (mut remaining, _) = crate::real_roots::divided(&rational, &common)?;
    let mut parts = Vec::new();
    let mut multiplicity = 1;
    while remaining.len() > 1 {
        let shared = crate::real_roots::greatest_common_divisor(&remaining, &common)?;
        let (part, _) = crate::real_roots::divided(&remaining, &shared)?;
        if part.len() > 1 {
            parts.push((integer_polynomial(&part), multiplicity));
        }
        let (next_common, _) = crate::real_roots::divided(&common, &shared)?;
        remaining = shared;
        common = next_common;
        multiplicity += 1;
    }
    Some(parts)
}

fn modulo(value: i64, prime: i64) -> i64 {
    value.rem_euclid(prime)
}

fn reduced_small(coefficients: &[Integer], prime: i64) -> Vec<i64> {
    let modulus = int(prime);
    let mut reduced: Vec<i64> = coefficients
        .iter()
        .map(|coefficient| {
            coefficient
                .div_rem_euclid(&modulus)
                .ok()
                .and_then(|(_, rest)| rest.to_i64())
                .unwrap_or(0)
        })
        .collect();
    while reduced.last() == Some(&0) {
        reduced.pop();
    }
    reduced
}

fn inverse_small(value: i64, prime: i64) -> i64 {
    let mut result = 1;
    let mut base = modulo(value, prime);
    let mut exponent = prime - 2;
    while exponent > 0 {
        if exponent % 2 == 1 {
            result = modulo(result * base, prime);
        }
        base = modulo(base * base, prime);
        exponent /= 2;
    }
    result
}

fn trimmed_small(mut coefficients: Vec<i64>) -> Vec<i64> {
    while coefficients.last() == Some(&0) {
        coefficients.pop();
    }
    coefficients
}

fn monic_small(coefficients: &[i64], prime: i64) -> Vec<i64> {
    let Some(leading) = coefficients.last() else {
        return Vec::new();
    };
    let inverse = inverse_small(*leading, prime);
    coefficients
        .iter()
        .map(|coefficient| modulo(coefficient * inverse, prime))
        .collect()
}

fn remainder_small(dividend: &[i64], divisor: &[i64], prime: i64) -> Vec<i64> {
    let divisor = trimmed_small(divisor.to_vec());
    let Some(leading) = divisor.last() else {
        return dividend.to_vec();
    };
    let inverse = inverse_small(*leading, prime);
    let mut rest = trimmed_small(dividend.to_vec());
    while rest.len() >= divisor.len() && !rest.is_empty() {
        let shift = rest.len() - divisor.len();
        let factor = modulo(rest[rest.len() - 1] * inverse, prime);
        for (position, coefficient) in divisor.iter().enumerate() {
            rest[shift + position] = modulo(rest[shift + position] - factor * coefficient, prime);
        }
        rest = trimmed_small(rest);
    }
    rest
}

fn product_small(left: &[i64], right: &[i64], prime: i64) -> Vec<i64> {
    if left.is_empty() || right.is_empty() {
        return Vec::new();
    }
    let mut result = vec![0; left.len() + right.len() - 1];
    for (i, a) in left.iter().enumerate() {
        for (j, b) in right.iter().enumerate() {
            result[i + j] = modulo(result[i + j] + a * b, prime);
        }
    }
    trimmed_small(result)
}

fn gcd_small(left: &[i64], right: &[i64], prime: i64) -> Vec<i64> {
    let mut first = trimmed_small(left.to_vec());
    let mut second = trimmed_small(right.to_vec());
    while !second.is_empty() {
        let rest = remainder_small(&first, &second, prime);
        first = second;
        second = rest;
    }
    monic_small(&first, prime)
}

fn derivative_small(coefficients: &[i64], prime: i64) -> Vec<i64> {
    trimmed_small(
        coefficients
            .iter()
            .enumerate()
            .skip(1)
            .map(|(power, coefficient)| {
                modulo(coefficient * i64::try_from(power).unwrap_or(0), prime)
            })
            .collect(),
    )
}

fn null_space_small(matrix: &mut [Vec<i64>], prime: i64) -> Vec<Vec<i64>> {
    let size = matrix.len();
    let mut pivots: Vec<(usize, usize)> = Vec::new();
    let mut row = 0;
    for column in 0..size {
        let Some(found) = (row..size).find(|candidate| matrix[*candidate][column] != 0) else {
            continue;
        };
        matrix.swap(row, found);
        let inverse = inverse_small(matrix[row][column], prime);
        for entry in matrix[row].iter_mut() {
            *entry = modulo(*entry * inverse, prime);
        }
        let pivot_row = matrix[row].clone();
        for (other, line) in matrix.iter_mut().enumerate() {
            if other == row || line[column] == 0 {
                continue;
            }
            let factor = line[column];
            for (entry, pivot) in line.iter_mut().zip(&pivot_row) {
                *entry = modulo(*entry - factor * pivot, prime);
            }
        }
        pivots.push((row, column));
        row += 1;
    }
    let free: Vec<usize> = (0..size)
        .filter(|column| !pivots.iter().any(|(_, pivot)| pivot == column))
        .collect();
    free.iter()
        .map(|chosen| {
            let mut vector = vec![0; size];
            vector[*chosen] = 1;
            for (row, column) in &pivots {
                vector[*column] = modulo(-matrix[*row][*chosen], prime);
            }
            vector
        })
        .collect()
}

fn berlekamp(polynomial: &[i64], prime: i64) -> Vec<Vec<i64>> {
    let degree = polynomial.len() - 1;
    let mut rows = Vec::with_capacity(degree);
    let mut power = vec![1_i64];
    let step = {
        let mut monomial = vec![0; usize::try_from(prime).unwrap_or(0) + 1];
        if let Some(last) = monomial.last_mut() {
            *last = 1;
        }
        remainder_small(&monomial, polynomial, prime)
    };
    for _ in 0..degree {
        let mut row = power.clone();
        row.resize(degree, 0);
        rows.push(row);
        power = remainder_small(&product_small(&power, &step, prime), polynomial, prime);
    }
    let mut transposed: Vec<Vec<i64>> = (0..degree)
        .map(|column| {
            (0..degree)
                .map(|row| {
                    let identity = i64::from(row == column);
                    modulo(rows[row][column] - identity, prime)
                })
                .collect()
        })
        .collect();
    let basis = null_space_small(&mut transposed, prime);
    let count = basis.len();
    let mut factors = vec![monic_small(polynomial, prime)];
    for vector in basis.iter().skip(1) {
        if factors.len() == count {
            break;
        }
        let mut next = Vec::new();
        for factor in factors {
            if factor.len() <= 2 {
                next.push(factor);
                continue;
            }
            let mut pending = vec![factor];
            for shift in 0..prime {
                let mut shifted = vector.clone();
                if let Some(first) = shifted.first_mut() {
                    *first = modulo(*first - shift, prime);
                }
                let mut split = Vec::new();
                for piece in pending {
                    let common = gcd_small(&piece, &shifted, prime);
                    if common.len() > 1 && common.len() < piece.len() {
                        let other = quotient_small(&piece, &common, prime);
                        split.push(common);
                        split.push(other);
                    } else {
                        split.push(piece);
                    }
                }
                pending = split;
            }
            next.extend(pending);
        }
        factors = next;
    }
    factors
}

fn quotient_small(dividend: &[i64], divisor: &[i64], prime: i64) -> Vec<i64> {
    let inverse = inverse_small(*divisor.last().unwrap_or(&1), prime);
    let mut rest = dividend.to_vec();
    let mut quotient = vec![0; dividend.len().saturating_sub(divisor.len()) + 1];
    while rest.len() >= divisor.len() && !rest.is_empty() {
        let shift = rest.len() - divisor.len();
        let factor = modulo(rest[rest.len() - 1] * inverse, prime);
        quotient[shift] = factor;
        for (position, coefficient) in divisor.iter().enumerate() {
            rest[shift + position] = modulo(rest[shift + position] - factor * coefficient, prime);
        }
        rest.pop();
        rest = trimmed_small(rest);
    }
    monic_small(&trimmed_small(quotient), prime)
}

fn symmetric(value: &Integer, modulus: &Integer) -> Integer {
    let rest = value
        .div_rem_euclid(modulus)
        .map(|(_, rest)| rest)
        .unwrap_or_else(|_| value.clone());
    if &rest + &rest > *modulus {
        &rest - modulus
    } else {
        rest
    }
}

fn reduced_big(coefficients: &[Integer], modulus: &Integer) -> Vec<Integer> {
    trimmed_integers(
        coefficients
            .iter()
            .map(|coefficient| symmetric(coefficient, modulus))
            .collect(),
    )
}

fn small_to_big(coefficients: &[i64]) -> Vec<Integer> {
    coefficients.iter().map(|value| int(*value)).collect()
}

fn subtract(left: &[Integer], right: &[Integer]) -> Vec<Integer> {
    let length = left.len().max(right.len());
    trimmed_integers(
        (0..length)
            .map(|position| {
                let a = left.get(position).cloned().unwrap_or_else(Integer::zero);
                let b = right.get(position).cloned().unwrap_or_else(Integer::zero);
                &a - &b
            })
            .collect(),
    )
}

fn bezout_small(first: &[i64], second: &[i64], prime: i64) -> Option<(Vec<i64>, Vec<i64>)> {
    let (mut old_r, mut r) = (
        trimmed_small(first.to_vec()),
        trimmed_small(second.to_vec()),
    );
    let (mut old_s, mut s) = (vec![1_i64], Vec::new());
    let (mut old_t, mut t) = (Vec::new(), vec![1_i64]);
    while !r.is_empty() {
        let quotient = quotient_raw_small(&old_r, &r, prime);
        let next_r = subtract_small(&old_r, &product_small(&quotient, &r, prime), prime);
        let next_s = subtract_small(&old_s, &product_small(&quotient, &s, prime), prime);
        let next_t = subtract_small(&old_t, &product_small(&quotient, &t, prime), prime);
        old_r = std::mem::replace(&mut r, next_r);
        old_s = std::mem::replace(&mut s, next_s);
        old_t = std::mem::replace(&mut t, next_t);
    }
    if old_r.len() != 1 {
        return None;
    }
    let inverse = inverse_small(old_r[0], prime);
    let scale = |values: &[i64]| -> Vec<i64> {
        trimmed_small(
            values
                .iter()
                .map(|value| modulo(value * inverse, prime))
                .collect(),
        )
    };
    Some((scale(&old_s), scale(&old_t)))
}

fn quotient_raw_small(dividend: &[i64], divisor: &[i64], prime: i64) -> Vec<i64> {
    let divisor = trimmed_small(divisor.to_vec());
    let inverse = inverse_small(*divisor.last().unwrap_or(&1), prime);
    let mut rest = trimmed_small(dividend.to_vec());
    if rest.len() < divisor.len() {
        return Vec::new();
    }
    let mut quotient = vec![0; rest.len() - divisor.len() + 1];
    while rest.len() >= divisor.len() && !rest.is_empty() {
        let shift = rest.len() - divisor.len();
        let factor = modulo(rest[rest.len() - 1] * inverse, prime);
        quotient[shift] = factor;
        for (position, coefficient) in divisor.iter().enumerate() {
            rest[shift + position] = modulo(rest[shift + position] - factor * coefficient, prime);
        }
        rest = trimmed_small(rest);
    }
    trimmed_small(quotient)
}

fn add_small(left: &[i64], right: &[i64], prime: i64) -> Vec<i64> {
    let length = left.len().max(right.len());
    trimmed_small(
        (0..length)
            .map(|position| {
                modulo(
                    left.get(position).copied().unwrap_or(0)
                        + right.get(position).copied().unwrap_or(0),
                    prime,
                )
            })
            .collect(),
    )
}

fn subtract_small(left: &[i64], right: &[i64], prime: i64) -> Vec<i64> {
    let length = left.len().max(right.len());
    trimmed_small(
        (0..length)
            .map(|position| {
                modulo(
                    left.get(position).copied().unwrap_or(0)
                        - right.get(position).copied().unwrap_or(0),
                    prime,
                )
            })
            .collect(),
    )
}

fn hensel_lift(
    whole: &[Integer],
    monic: &[i64],
    cofactor: &[i64],
    prime: i64,
    steps: u32,
) -> Option<(Vec<Integer>, Vec<Integer>)> {
    let (s, t) = bezout_small(monic, cofactor, prime)?;
    let mut g = small_to_big(monic);
    let mut h = small_to_big(cofactor);
    let p = int(prime);
    let mut modulus = p.clone();
    for _ in 1..steps {
        let error = subtract(whole, &product(&g, &h));
        let error: Vec<Integer> = error
            .iter()
            .map(|coefficient| {
                coefficient
                    .div_rem_euclid(&modulus)
                    .map(|(quotient, _)| quotient)
                    .unwrap_or_else(|_| Integer::zero())
            })
            .collect();
        let error = reduced_small(&error, prime);
        let et = product_small(&error, &t, prime);
        let quotient = quotient_raw_small(&et, monic, prime);
        let a = remainder_small(&et, monic, prime);
        let b = add_small(
            &product_small(&error, &s, prime),
            &product_small(&quotient, cofactor, prime),
            prime,
        );
        let lifted_g: Vec<Integer> = small_to_big(&a)
            .iter()
            .map(|value| value * &modulus)
            .collect();
        let lifted_h: Vec<Integer> = small_to_big(&b)
            .iter()
            .map(|value| value * &modulus)
            .collect();
        g = trimmed_integers(add(&g, &lifted_g));
        h = trimmed_integers(add(&h, &lifted_h));
        modulus = &modulus * &p;
        g = reduced_big(&g, &modulus);
        h = reduced_big(&h, &modulus);
    }
    Some((g, h))
}

fn add(left: &[Integer], right: &[Integer]) -> Vec<Integer> {
    let length = left.len().max(right.len());
    (0..length)
        .map(|position| {
            let a = left.get(position).cloned().unwrap_or_else(Integer::zero);
            let b = right.get(position).cloned().unwrap_or_else(Integer::zero);
            &a + &b
        })
        .collect()
}

fn coefficient_bound(polynomial: &[Integer]) -> Integer {
    let sum = polynomial
        .iter()
        .fold(Integer::zero(), |total, coefficient| {
            &total + &coefficient.absolute()
        });
    let leading = polynomial
        .last()
        .map_or_else(Integer::one, Integer::absolute);
    let degree = u32::try_from(polynomial.len().saturating_sub(1)).unwrap_or(u32::MAX);
    &(&(&sum * &leading) * &int(2).pow(degree)) * &int(2)
}

fn factor_square_free(polynomial: &[Integer]) -> Option<Vec<Vec<Integer>>> {
    if polynomial.len() <= 2 {
        return Some(vec![primitive(polynomial)]);
    }
    let leading = polynomial.last()?.clone();
    let prime = SMALL_PRIMES.iter().copied().find(|prime| {
        let reduced = reduced_small(polynomial, *prime);
        reduced.len() == polynomial.len()
            && gcd_small(&reduced, &derivative_small(&reduced, *prime), *prime).len() == 1
    })?;
    let local = berlekamp(&reduced_small(polynomial, prime), prime);
    if local.len() <= 1 {
        return Some(vec![primitive(polynomial)]);
    }
    let bound = coefficient_bound(polynomial);
    let p = int(prime);
    let mut steps = 1_u32;
    let mut modulus = p.clone();
    while modulus <= bound {
        modulus = &modulus * &p;
        steps += 1;
    }
    let mut lifted: Vec<Vec<Integer>> = Vec::new();
    let mut remaining_whole = polynomial.to_vec();
    let mut remaining_local = local.clone();
    while remaining_local.len() > 1 {
        let first = remaining_local.remove(0);
        let mut cofactor = reduced_small(&[remaining_whole.last()?.clone()], prime);
        for piece in &remaining_local {
            cofactor = product_small(&cofactor, piece, prime);
        }
        let (g, h) = hensel_lift(&remaining_whole, &first, &cofactor, prime, steps)?;
        lifted.push(g);
        remaining_whole = h;
    }
    lifted.push(monic_lifted(&remaining_whole, &modulus)?);
    recombined(polynomial, &lifted, &modulus, &leading)
}

fn monic_lifted(polynomial: &[Integer], modulus: &Integer) -> Option<Vec<Integer>> {
    let leading = polynomial.last()?.clone();
    let inverse = modular_inverse(&leading, modulus)?;
    Some(reduced_big(
        &polynomial
            .iter()
            .map(|coefficient| coefficient * &inverse)
            .collect::<Vec<_>>(),
        modulus,
    ))
}

fn modular_inverse(value: &Integer, modulus: &Integer) -> Option<Integer> {
    let (mut old_r, mut r) = (symmetric(value, modulus), modulus.clone());
    let (mut old_s, mut s) = (Integer::one(), Integer::zero());
    while !r.is_zero() {
        let (quotient, rest) = old_r.div_rem_euclid(&r).ok()?;
        old_r = std::mem::replace(&mut r, rest);
        let next = &old_s - &(&quotient * &s);
        old_s = std::mem::replace(&mut s, next);
    }
    if old_r.is_one() {
        Some(symmetric(&old_s, modulus))
    } else if old_r == Integer::one().negated() {
        Some(symmetric(&old_s.negated(), modulus))
    } else {
        None
    }
}

fn recombined(
    polynomial: &[Integer],
    lifted: &[Vec<Integer>],
    modulus: &Integer,
    leading: &Integer,
) -> Option<Vec<Vec<Integer>>> {
    let mut remaining = polynomial.to_vec();
    let mut pieces: Vec<Vec<Integer>> = lifted.to_vec();
    let mut found = Vec::new();
    let mut size = 1;
    while 2 * size <= pieces.len() {
        let mut matched = None;
        for subset in subsets(pieces.len(), size) {
            let leading_now = remaining.last()?.clone();
            let mut candidate = vec![leading_now.clone()];
            for index in &subset {
                candidate = reduced_big(&product(&candidate, &pieces[*index]), modulus);
            }
            let candidate = primitive(&candidate);
            if exact_quotient(&remaining, &candidate).is_some() {
                matched = Some((subset, candidate));
                break;
            }
        }
        match matched {
            Some((subset, candidate)) => {
                remaining = primitive(&exact_quotient(&remaining, &candidate)?);
                found.push(candidate);
                pieces = pieces
                    .into_iter()
                    .enumerate()
                    .filter(|(index, _)| !subset.contains(index))
                    .map(|(_, piece)| piece)
                    .collect();
            }
            None => size += 1,
        }
    }
    let _ = leading;
    if remaining.len() > 1 {
        found.push(primitive(&remaining));
    }
    Some(found)
}

fn subsets(count: usize, size: usize) -> Vec<Vec<usize>> {
    let mut all = Vec::new();
    let mut current: Vec<usize> = (0..size).collect();
    if size > count {
        return all;
    }
    loop {
        all.push(current.clone());
        let mut position = size;
        while position > 0 {
            position -= 1;
            if current[position] < count - size + position {
                current[position] += 1;
                for next in position + 1..size {
                    current[next] = current[next - 1] + 1;
                }
                break;
            }
            if position == 0 {
                return all;
            }
        }
        if size == 0 {
            return all;
        }
    }
}

fn is_prime(candidate: i64) -> bool {
    candidate >= 2
        && (2..)
            .take_while(|divisor: &i64| divisor * divisor <= candidate)
            .all(|divisor| candidate % divisor != 0)
}

fn value_small(coefficients: &[i64], point: i64, prime: i64) -> i64 {
    coefficients.iter().rev().fold(0, |total, coefficient| {
        modulo(total * point + coefficient, prime)
    })
}

fn value_big(coefficients: &[Integer], point: &Integer) -> Integer {
    coefficients
        .iter()
        .rev()
        .fold(Integer::zero(), |total, coefficient| {
            &(&total * point) + coefficient
        })
}

fn derivative_big(coefficients: &[Integer]) -> Vec<Integer> {
    coefficients
        .iter()
        .enumerate()
        .skip(1)
        .map(|(power, coefficient)| coefficient * &int(i64::try_from(power).unwrap_or(i64::MAX)))
        .collect()
}

fn simple_roots_modulo(polynomial: &[Integer], prime: i64) -> Option<Vec<i64>> {
    let reduced = reduced_small(polynomial, prime);
    if reduced.len() != polynomial.len() {
        return None;
    }
    let slope = derivative_small(&reduced, prime);
    let mut roots = Vec::new();
    for point in 0..prime {
        if value_small(&reduced, point, prime) == 0 {
            if value_small(&slope, point, prime) == 0 {
                return None;
            }
            roots.push(point);
        }
    }
    Some(roots)
}

fn lifted_root(
    polynomial: &[Integer],
    root: i64,
    prime: i64,
    bound: &Integer,
) -> Option<(Integer, Integer)> {
    let slope = derivative_big(polynomial);
    let mut modulus = int(prime);
    let mut lifted = int(root);
    while modulus <= *bound {
        modulus = &modulus * &modulus;
        let inverse = modular_inverse(&value_big(&slope, &lifted), &modulus)?;
        let step = &value_big(polynomial, &lifted) * &inverse;
        lifted = symmetric(&(&lifted - &step), &modulus);
    }
    Some((lifted, modulus))
}

pub(crate) fn rational_roots_of_square_free(
    polynomial: &crate::real_roots::SquareFree,
) -> Option<Vec<ExactRational>> {
    let mut whole = integer_polynomial(polynomial.coefficients());
    let mut roots = Vec::new();
    if whole.len() <= 1 {
        return Some(roots);
    }
    if whole.first()?.is_zero() {
        roots.push(ExactRational::zero());
        whole.remove(0);
    }
    if whole.len() <= 1 {
        return Some(roots);
    }
    let leading = whole.last()?.clone();
    let bound = &(&leading.absolute() * &whole.first()?.absolute()) * &int(2);
    let (prime, local) = (2..)
        .filter(|candidate| is_prime(*candidate))
        .find_map(|prime| simple_roots_modulo(&whole, prime).map(|local| (prime, local)))?;
    let rational = rational_polynomial(&whole);
    for root in local {
        let (lifted, modulus) = lifted_root(&whole, root, prime, &bound)?;
        let scaled = symmetric(&(&lifted * &leading), &modulus);
        let candidate = ExactRational::fraction(&scaled, &leading)?;
        let value = rational
            .iter()
            .rev()
            .fold(ExactRational::zero(), |total, coefficient| {
                total.multiply(&candidate).plus(coefficient)
            });
        if value.is_zero() {
            roots.push(candidate);
        }
    }
    Some(roots)
}

pub(crate) fn factor_over_rationals(coefficients: &[ExactRational]) -> Option<RationalFactoring> {
    let trimmed: Vec<ExactRational> = {
        let mut values = coefficients.to_vec();
        while values.last().is_some_and(ExactRational::is_zero) {
            values.pop();
        }
        values
    };
    if trimmed.len() <= 1 {
        return Some(RationalFactoring {
            constant: trimmed.first().cloned().unwrap_or_else(ExactRational::zero),
            factors: Vec::new(),
        });
    }
    let whole = integer_polynomial(&trimmed);
    let mut factors = Vec::new();
    for (part, multiplicity) in square_free_parts(&whole)? {
        for factor in factor_square_free(&part)? {
            factors.push((factor, multiplicity));
        }
    }
    let mut expanded = vec![Integer::one()];
    for (factor, multiplicity) in &factors {
        for _ in 0..*multiplicity {
            expanded = product(&expanded, factor);
        }
    }
    let constant = trimmed
        .last()?
        .divide(&ExactRational::from_integer(expanded.last()?.clone()))?;
    factors.sort_by(|(left, _), (right, _)| {
        left.len().cmp(&right.len()).then_with(|| left.cmp(right))
    });
    Some(RationalFactoring { constant, factors })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn whole(values: &[i64]) -> Vec<ExactRational> {
        values
            .iter()
            .map(|value| ExactRational::from_i64(*value))
            .collect()
    }

    fn rational_roots_of(values: &[i64]) -> Vec<ExactRational> {
        let square_free = crate::real_roots::square_free_part(&whole(values)).unwrap();
        let mut roots = rational_roots_of_square_free(&square_free).unwrap();
        roots.sort_by(ExactRational::compare);
        roots
    }

    fn fraction(numerator: i64, denominator: i64) -> ExactRational {
        ExactRational::from_i64(numerator)
            .divide(&ExactRational::from_i64(denominator))
            .unwrap()
    }

    #[test]
    fn every_rational_root_of_a_square_free_polynomial_is_found() {
        assert_eq!(
            rational_roots_of(&[0, 2, 2, -13, -1, 6]),
            vec![fraction(-1, 3), ExactRational::zero(), fraction(1, 2)]
        );
    }

    #[test]
    fn a_root_is_found_when_the_smallest_primes_divide_the_leading_coefficient() {
        assert_eq!(
            rational_roots_of(&[-1, 210, -1, 210]),
            vec![fraction(1, 210)]
        );
    }

    #[test]
    fn roots_that_meet_modulo_the_smallest_primes_are_all_found() {
        assert_eq!(
            rational_roots_of(&[-21, 31, -11, 1]),
            vec![fraction(1, 1), fraction(3, 1), fraction(7, 1)]
        );
    }

    #[test]
    fn a_root_as_large_as_the_constant_is_lifted_past_twice_its_size() {
        assert_eq!(
            rational_roots_of(&[-200, -199, -199, 1]),
            vec![fraction(200, 1)]
        );
    }

    #[test]
    fn an_irreducible_polynomial_has_no_rational_root() {
        assert_eq!(rational_roots_of(&[-100001, 0, 0, 1]), Vec::new());
    }

    fn factors_of(values: &[i64]) -> Vec<(Vec<i64>, u32)> {
        factor_over_rationals(&whole(values))
            .unwrap()
            .factors
            .into_iter()
            .map(|(factor, multiplicity)| {
                (
                    factor.iter().map(|value| value.to_i64().unwrap()).collect(),
                    multiplicity,
                )
            })
            .collect()
    }

    #[test]
    fn a_product_of_two_quadratics_is_split() {
        let mut found = factors_of(&[6, 0, -5, 0, 1]);
        found.sort();
        assert_eq!(found, vec![(vec![-3, 0, 1], 1), (vec![-2, 0, 1], 1)]);
    }

    #[test]
    fn a_product_of_linear_factors_is_split() {
        let mut found = factors_of(&[6, -5, 1]);
        found.sort();
        assert_eq!(found, vec![(vec![-3, 1], 1), (vec![-2, 1], 1)]);
    }

    #[test]
    fn x_to_the_fourth_plus_one_is_irreducible() {
        assert_eq!(factors_of(&[1, 0, 0, 0, 1]), vec![(vec![1, 0, 0, 0, 1], 1)]);
    }

    #[test]
    fn a_repeated_factor_keeps_its_multiplicity() {
        assert_eq!(factors_of(&[1, -2, 1]), vec![(vec![-1, 1], 2)]);
    }

    #[test]
    fn a_cyclotomic_product_is_split() {
        let mut found = factors_of(&[-1, 0, 0, 0, 0, 0, 1]);
        found.sort();
        assert_eq!(
            found,
            vec![
                (vec![-1, 1], 1),
                (vec![1, -1, 1], 1),
                (vec![1, 1], 1),
                (vec![1, 1, 1], 1)
            ]
        );
    }

    #[test]
    fn a_polynomial_that_splits_modulo_every_prime_is_still_irreducible() {
        assert_eq!(
            factors_of(&[1, 0, -10, 0, 1]),
            vec![(vec![1, 0, -10, 0, 1], 1)]
        );
    }

    #[test]
    fn a_product_with_a_repeated_linear_factor_and_a_cubic_is_split() {
        let product_of = |left: &[i64], right: &[i64]| -> Vec<i64> {
            let mut result = vec![0; left.len() + right.len() - 1];
            for (i, a) in left.iter().enumerate() {
                for (j, b) in right.iter().enumerate() {
                    result[i + j] += a * b;
                }
            }
            result
        };
        let square = product_of(&[3, 2], &[3, 2]);
        let whole = product_of(&product_of(&[1, 0, 1], &[-2, 0, 0, 1]), &square);
        let mut found = factors_of(&whole);
        found.sort();
        assert_eq!(
            found,
            vec![(vec![-2, 0, 0, 1], 1), (vec![1, 0, 1], 1), (vec![3, 2], 2)]
        );
    }
}
