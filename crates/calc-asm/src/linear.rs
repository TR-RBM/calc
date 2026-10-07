use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Linear {
    constant: i128,
    terms: BTreeMap<String, i128>,
    denominator: i128,
}

impl Default for Linear {
    fn default() -> Self {
        Self::constant(0)
    }
}

impl Linear {
    pub fn constant(value: i128) -> Self {
        Self {
            constant: value,
            terms: BTreeMap::new(),
            denominator: 1,
        }
    }

    pub fn symbol(name: &str) -> Self {
        Self {
            constant: 0,
            terms: BTreeMap::from([(name.to_owned(), 1)]),
            denominator: 1,
        }
    }

    fn normalized(mut self) -> Self {
        self.terms.retain(|_, coefficient| *coefficient != 0);
        if self.denominator < 0 {
            self.denominator = -self.denominator;
            self.constant = -self.constant;
            for coefficient in self.terms.values_mut() {
                *coefficient = -*coefficient;
            }
        }
        let divisor = self
            .terms
            .values()
            .fold(
                greatest_common_divisor(self.constant, self.denominator),
                |divisor, coefficient| greatest_common_divisor(divisor, *coefficient),
            )
            .max(1);
        self.constant /= divisor;
        self.denominator /= divisor;
        for coefficient in self.terms.values_mut() {
            *coefficient /= divisor;
        }
        self
    }

    pub fn as_constant(&self) -> Option<i128> {
        (self.terms.is_empty() && self.constant % self.denominator == 0)
            .then(|| self.constant / self.denominator)
    }

    pub fn constant_part(&self) -> i128 {
        self.constant
    }

    pub fn terms(&self) -> &BTreeMap<String, i128> {
        &self.terms
    }

    pub fn denominator(&self) -> i128 {
        self.denominator
    }

    pub fn numerator(&self) -> Self {
        Self {
            constant: self.constant,
            terms: self.terms.clone(),
            denominator: 1,
        }
    }

    pub fn plus(&self, other: &Self) -> Self {
        let mut sum = Self {
            constant: self.constant * other.denominator + other.constant * self.denominator,
            terms: self
                .terms
                .iter()
                .map(|(name, coefficient)| (name.clone(), coefficient * other.denominator))
                .collect(),
            denominator: self.denominator * other.denominator,
        };
        for (name, coefficient) in &other.terms {
            *sum.terms.entry(name.clone()).or_insert(0) += coefficient * self.denominator;
        }
        sum.normalized()
    }

    pub fn minus(&self, other: &Self) -> Self {
        self.plus(&other.scaled(-1))
    }

    pub fn plus_constant(&self, value: i128) -> Self {
        self.plus(&Self::constant(value))
    }

    pub fn scaled(&self, factor: i128) -> Self {
        Self {
            constant: self.constant * factor,
            terms: self
                .terms
                .iter()
                .map(|(name, coefficient)| (name.clone(), coefficient * factor))
                .collect(),
            denominator: self.denominator,
        }
        .normalized()
    }

    pub fn divided(&self, divisor: i128) -> Self {
        Self {
            constant: self.constant,
            terms: self.terms.clone(),
            denominator: self.denominator * divisor,
        }
        .normalized()
    }

    pub fn substituted(&self, values: &BTreeMap<String, Linear>) -> Option<Self> {
        let mut result = Self::constant(self.constant);
        for (name, coefficient) in &self.terms {
            let value = values.get(name)?;
            result = result.plus(&value.scaled(*coefficient));
        }
        Some(result.divided(self.denominator))
    }
}

fn greatest_common_divisor(left: i128, right: i128) -> i128 {
    let (mut left, mut right) = (left.abs(), right.abs());
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fraction {
    numerator: i128,
    denominator: i128,
}

impl Fraction {
    pub fn whole(value: i128) -> Self {
        Self {
            numerator: value,
            denominator: 1,
        }
    }

    fn reduced(numerator: i128, denominator: i128) -> Self {
        let divisor = greatest_common_divisor(numerator, denominator).max(1);
        let sign = if denominator < 0 { -1 } else { 1 };
        Self {
            numerator: sign * numerator / divisor,
            denominator: sign * denominator / divisor,
        }
    }

    fn plus(self, other: Self) -> Self {
        Self::reduced(
            self.numerator * other.denominator + other.numerator * self.denominator,
            self.denominator * other.denominator,
        )
    }

    fn times(self, other: Self) -> Self {
        Self::reduced(
            self.numerator * other.numerator,
            self.denominator * other.denominator,
        )
    }

    fn is_zero(self) -> bool {
        self.numerator == 0
    }

    fn is_negative(self) -> bool {
        self.numerator < 0
    }

    fn magnitude_text(self) -> String {
        let numerator = self.numerator.unsigned_abs();
        if self.denominator == 1 {
            numerator.to_string()
        } else {
            format!("{numerator}/{}", self.denominator)
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Poly {
    terms: BTreeMap<Vec<String>, Fraction>,
}

impl Poly {
    pub fn zero() -> Self {
        Self::default()
    }

    pub fn constant(value: i128) -> Self {
        let mut poly = Self::zero();
        if value != 0 {
            poly.terms.insert(Vec::new(), Fraction::whole(value));
        }
        poly
    }

    pub fn divided(&self, divisor: i128) -> Self {
        let factor = Fraction::reduced(1, divisor);
        let mut poly = Self::zero();
        for (monomial, coefficient) in &self.terms {
            poly.terms
                .insert(monomial.clone(), coefficient.times(factor));
        }
        poly
    }

    pub fn from_linear(linear: &Linear) -> Self {
        let mut poly = Self::zero();
        if linear.constant != 0 {
            poly.terms.insert(
                Vec::new(),
                Fraction::reduced(linear.constant, linear.denominator),
            );
        }
        for (name, coefficient) in &linear.terms {
            poly.terms.insert(
                vec![name.clone()],
                Fraction::reduced(*coefficient, linear.denominator),
            );
        }
        poly
    }

    pub fn symbols(&self) -> std::collections::BTreeSet<String> {
        self.terms.keys().flatten().cloned().collect()
    }

    pub fn as_linear(&self) -> Option<Linear> {
        let mut linear = Linear::constant(0);
        for (monomial, coefficient) in &self.terms {
            let part = match monomial.as_slice() {
                [] => Linear::constant(coefficient.numerator),
                [name] => Linear::symbol(name).scaled(coefficient.numerator),
                _ => return None,
            };
            linear = linear.plus(&part.divided(coefficient.denominator));
        }
        Some(linear)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.terms
            .keys()
            .any(|monomial| monomial.iter().any(|symbol| symbol == name))
    }

    pub fn plus(&self, other: &Self) -> Self {
        let mut sum = self.clone();
        for (monomial, coefficient) in &other.terms {
            let entry = sum
                .terms
                .entry(monomial.clone())
                .or_insert(Fraction::whole(0));
            *entry = entry.plus(*coefficient);
        }
        sum.terms.retain(|_, coefficient| !coefficient.is_zero());
        sum
    }

    pub fn minus(&self, other: &Self) -> Self {
        self.plus(&other.times(&Self::constant(-1)))
    }

    pub fn times(&self, other: &Self) -> Self {
        let mut product = Self::zero();
        for (left, left_coefficient) in &self.terms {
            for (right, right_coefficient) in &other.terms {
                let mut monomial: Vec<String> = left.iter().chain(right).cloned().collect();
                monomial.sort();
                let entry = product.terms.entry(monomial).or_insert(Fraction::whole(0));
                *entry = entry.plus(left_coefficient.times(*right_coefficient));
            }
        }
        product
            .terms
            .retain(|_, coefficient| !coefficient.is_zero());
        product
    }

    fn is_nonnegative(&self, bounds: &Bounds) -> bool {
        self.terms.iter().all(|(monomial, coefficient)| {
            !coefficient.is_negative()
                && monomial
                    .iter()
                    .all(|symbol| bounds.lower(symbol).is_some_and(|lower| lower >= 0))
        })
    }

    pub fn is_at_most(&self, other: &Self, bounds: &Bounds) -> bool {
        other.minus(self).is_nonnegative(bounds)
    }

    pub fn text(&self) -> String {
        let mut ordered: Vec<(&Vec<String>, &Fraction)> = self.terms.iter().collect();
        ordered.sort_by(|(left, _), (right, _)| right.len().cmp(&left.len()).then(left.cmp(right)));
        let mut text = String::new();
        for (monomial, coefficient) in ordered {
            let magnitude = coefficient.magnitude_text();
            let factors = monomial_text(monomial);
            let body = match (magnitude.as_str(), factors.is_empty()) {
                (_, true) => magnitude,
                ("1", false) => factors,
                (_, false) => format!("{magnitude} * {factors}"),
            };
            match (text.is_empty(), coefficient.is_negative()) {
                (true, true) => text = format!("-{body}"),
                (true, false) => text = body,
                (false, true) => text = format!("{text} - {body}"),
                (false, false) => text = format!("{text} + {body}"),
            }
        }
        if text.is_empty() {
            text.push('0');
        }
        text
    }
}

fn monomial_text(monomial: &[String]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut index = 0;
    while let Some(symbol) = monomial.get(index) {
        let power = monomial[index..]
            .iter()
            .take_while(|other| *other == symbol)
            .count();
        parts.push(match power {
            1 => symbol.clone(),
            _ => format!("{symbol}^{power}"),
        });
        index += power;
    }
    parts.join(" * ")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Relation {
    AtLeastZero,
    Zero,
    NotZero,
    Congruent(i128),
    NotCongruent(i128),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Condition {
    pub expression: Linear,
    pub relation: Relation,
    pub is_unsigned: bool,
}

impl Condition {
    pub fn new(expression: Linear, relation: Relation, is_unsigned: bool) -> Self {
        let denominator = expression.denominator();
        let relation = match relation {
            Relation::Congruent(modulus) => Relation::Congruent(modulus * denominator),
            Relation::NotCongruent(modulus) => Relation::NotCongruent(modulus * denominator),
            other => other,
        };
        Self {
            expression: expression.numerator(),
            relation,
            is_unsigned,
        }
    }

    pub fn at_least_zero(expression: Linear) -> Self {
        Self::new(expression, Relation::AtLeastZero, false)
    }

    pub fn negated(&self) -> Self {
        let (expression, relation) = match self.relation {
            Relation::AtLeastZero => (
                self.expression.scaled(-1).plus_constant(-1),
                Relation::AtLeastZero,
            ),
            Relation::Zero => (self.expression.clone(), Relation::NotZero),
            Relation::NotZero => (self.expression.clone(), Relation::Zero),
            Relation::Congruent(2) => (self.expression.plus_constant(1), Relation::Congruent(2)),
            Relation::Congruent(modulus) => {
                (self.expression.clone(), Relation::NotCongruent(modulus))
            }
            Relation::NotCongruent(modulus) => {
                (self.expression.clone(), Relation::Congruent(modulus))
            }
        };
        Self {
            expression,
            relation,
            is_unsigned: self.is_unsigned,
        }
    }

    pub fn substituted(&self, values: &BTreeMap<String, Linear>) -> Option<Self> {
        Some(Self::new(
            self.expression.substituted(values)?,
            self.relation,
            self.is_unsigned,
        ))
    }

    pub fn congruent(expression: Linear, modulus: i128) -> Self {
        Self::new(expression, Relation::Congruent(modulus), false)
    }

    pub fn text(&self) -> String {
        if let Relation::Congruent(modulus) | Relation::NotCongruent(modulus) = self.relation {
            let operator = if matches!(self.relation, Relation::Congruent(_)) {
                "="
            } else {
                "!="
            };
            if let Some(Residue::Class(name, residue, reduced)) =
                residue_of(&self.expression, modulus)
            {
                return format!("{name} mod {reduced} {operator} {residue}");
            }
            return format!(
                "{} mod {modulus} {operator} 0",
                Poly::from_linear(&self.expression).text()
            );
        }
        let operator = match self.relation {
            Relation::AtLeastZero => ">=",
            Relation::Zero => "=",
            _ => "!=",
        };
        let suffix = if self.is_unsigned { " (unsigned)" } else { "" };
        let terms = self.expression.terms();
        if let (Relation::AtLeastZero, Some((name, coefficient))) =
            (self.relation, single_term(terms))
        {
            let constant = self.expression.constant_part();
            if coefficient > 0 {
                let bound = ceiling_division(-constant, coefficient);
                return format!("{name} >= {bound}{suffix}");
            }
            let bound = floor_division(constant, -coefficient);
            return format!("{name} <= {bound}{suffix}");
        }
        let left = Linear {
            constant: 0,
            terms: terms.clone(),
            denominator: 1,
        };
        let right = -self.expression.constant_part();
        format!(
            "{} {operator} {right}{suffix}",
            Poly::from_linear(&left).text()
        )
    }
}

fn inverse_modulo(value: i128, modulus: i128) -> Option<i128> {
    let value = value.rem_euclid(modulus);
    (1..modulus).find(|candidate| (value * candidate).rem_euclid(modulus) == 1)
}

enum Residue {
    Unsatisfiable,
    Class(String, i128, i128),
}

fn residue_of(expression: &Linear, modulus: i128) -> Option<Residue> {
    let (name, coefficient) = single_term(expression.terms())?;
    let common = greatest_common_divisor(coefficient, modulus);
    let constant = expression.constant_part();
    if constant % common != 0 {
        return Some(Residue::Unsatisfiable);
    }
    let reduced = modulus / common;
    if reduced == 1 {
        return None;
    }
    let inverse = inverse_modulo(coefficient / common, reduced)?;
    let residue = (-(constant / common) * inverse).rem_euclid(reduced);
    Some(Residue::Class(name.clone(), residue, reduced))
}

fn single_term(terms: &BTreeMap<String, i128>) -> Option<(&String, i128)> {
    let mut iterator = terms.iter();
    let (name, coefficient) = iterator.next()?;
    iterator.next().is_none().then_some((name, *coefficient))
}

fn floor_division(numerator: i128, denominator: i128) -> i128 {
    numerator.div_euclid(denominator)
}

fn ceiling_division(numerator: i128, denominator: i128) -> i128 {
    -((-numerator).div_euclid(denominator))
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bounds {
    ranges: BTreeMap<String, (Option<i128>, Option<i128>)>,
    residues: BTreeMap<String, (i128, i128)>,
    excluded: BTreeMap<String, std::collections::BTreeSet<i128>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Holds,
    Fails,
    Open,
}

impl Bounds {
    pub fn lower(&self, name: &str) -> Option<i128> {
        self.ranges.get(name).and_then(|(lower, _)| *lower)
    }

    pub fn upper(&self, name: &str) -> Option<i128> {
        self.ranges.get(name).and_then(|(_, upper)| *upper)
    }

    pub fn with(&self, condition: &Condition) -> Option<Self> {
        let mut bounds = self.clone();
        if condition.is_unsigned {
            return Some(bounds);
        }
        if let Relation::Congruent(modulus) = condition.relation {
            let (name, residue, modulus) = match residue_of(&condition.expression, modulus) {
                Some(Residue::Unsatisfiable) => return None,
                Some(Residue::Class(name, residue, reduced)) => (name, residue, reduced),
                None => return Some(bounds),
            };
            if let Some((known_modulus, known_residue)) = bounds.residues.get(&name).copied() {
                let common = greatest_common_divisor(modulus, known_modulus);
                if (residue - known_residue).rem_euclid(common) != 0 {
                    return None;
                }
                if known_modulus % modulus == 0 {
                    return Some(bounds);
                }
                if modulus % known_modulus != 0 {
                    return Some(bounds);
                }
            }
            bounds.residues.insert(name.clone(), (modulus, residue));
            return bounds.tightened(&name);
        }
        if let Relation::NotCongruent(_) = condition.relation {
            return match bounds.decide(condition) {
                Decision::Fails => None,
                _ => Some(bounds),
            };
        }
        let Some((name, coefficient)) = single_term(condition.expression.terms()) else {
            return Some(bounds);
        };
        let constant = condition.expression.constant_part();
        let (lower, upper) = match condition.relation {
            Relation::AtLeastZero if coefficient > 0 => {
                (Some(ceiling_division(-constant, coefficient)), None)
            }
            Relation::AtLeastZero => (None, Some(floor_division(constant, -coefficient))),
            Relation::Zero if constant % coefficient == 0 => {
                let value = -constant / coefficient;
                (Some(value), Some(value))
            }
            Relation::Zero => return None,
            Relation::NotZero if constant % coefficient == 0 => {
                bounds
                    .excluded
                    .entry(name.clone())
                    .or_default()
                    .insert(-constant / coefficient);
                return bounds.tightened(name);
            }
            Relation::NotZero | Relation::Congruent(_) | Relation::NotCongruent(_) => (None, None),
        };
        let entry = bounds.ranges.entry(name.clone()).or_insert((None, None));
        if let Some(value) = lower {
            entry.0 = Some(entry.0.map_or(value, |current| current.max(value)));
        }
        if let Some(value) = upper {
            entry.1 = Some(entry.1.map_or(value, |current| current.min(value)));
        }
        match *entry {
            (Some(lower), Some(upper)) if lower > upper => None,
            _ => bounds.tightened(name),
        }
    }

    fn allowed(&self, name: &str, value: i128) -> bool {
        let excluded = self
            .excluded
            .get(name)
            .is_some_and(|values| values.contains(&value));
        let residue = self
            .residues
            .get(name)
            .is_none_or(|(modulus, residue)| value.rem_euclid(*modulus) == *residue);
        !excluded && residue
    }

    fn tightened(mut self, name: &str) -> Option<Self> {
        let limit = self
            .excluded
            .get(name)
            .map_or(0, std::collections::BTreeSet::len)
            + 1;
        let Some(entry) = self.ranges.get(name).copied() else {
            return Some(self);
        };
        let modulus = self.residues.get(name).map_or(1, |(modulus, _)| *modulus);
        let mut lower = entry.0;
        let mut upper = entry.1;
        if let Some(mut value) = lower {
            let mut steps = 0;
            while !self.allowed(name, value)
                && steps <= limit * usize::try_from(modulus).unwrap_or(1)
            {
                value += 1;
                steps += 1;
            }
            lower = Some(value);
        }
        if let Some(mut value) = upper {
            let mut steps = 0;
            while !self.allowed(name, value)
                && steps <= limit * usize::try_from(modulus).unwrap_or(1)
            {
                value -= 1;
                steps += 1;
            }
            upper = Some(value);
        }
        if let (Some(lower), Some(upper)) = (lower, upper)
            && lower > upper
        {
            return None;
        }
        self.ranges.insert(name.to_owned(), (lower, upper));
        Some(self)
    }

    pub fn texts(&self) -> Vec<String> {
        let mut texts = self.range_texts();
        for (name, values) in &self.excluded {
            let (lower, upper) = self.ranges.get(name).copied().unwrap_or((None, None));
            if lower.is_some() && lower == upper {
                continue;
            }
            for value in values {
                let inside = lower.is_none_or(|lower| lower < *value)
                    && upper.is_none_or(|upper| *value < upper);
                if inside {
                    texts.push(format!("{name} != {value}"));
                }
            }
        }
        texts
    }

    fn range_texts(&self) -> Vec<String> {
        let mut texts = Vec::new();
        for (name, (lower, upper)) in &self.ranges {
            match (lower, upper) {
                (Some(lower), Some(upper)) if lower == upper => {
                    texts.push(format!("{name} = {lower}"))
                }
                _ => {
                    if let Some(lower) = lower {
                        texts.push(format!("{name} >= {lower}"));
                    }
                    if let Some(upper) = upper {
                        texts.push(format!("{name} <= {upper}"));
                    }
                }
            }
        }
        for (name, (modulus, residue)) in &self.residues {
            let fixed = self
                .ranges
                .get(name)
                .is_some_and(|(lower, upper)| lower.is_some() && lower == upper);
            if !fixed {
                texts.push(format!("{name} mod {modulus} = {residue}"));
            }
        }
        texts
    }

    fn range(&self, expression: &Linear) -> (Option<i128>, Option<i128>) {
        let mut lower = Some(expression.constant_part());
        let mut upper = Some(expression.constant_part());
        for (name, coefficient) in expression.terms() {
            let (low, high) = if *coefficient > 0 {
                (self.lower(name), self.upper(name))
            } else {
                (self.upper(name), self.lower(name))
            };
            lower = lower.zip(low).map(|(sum, value)| sum + coefficient * value);
            upper = upper
                .zip(high)
                .map(|(sum, value)| sum + coefficient * value);
        }
        (lower, upper)
    }

    fn decide_congruence(&self, expression: &Linear, modulus: i128) -> Decision {
        let (lower, upper) = self.range(expression);
        if let (Some(lower), Some(upper)) = (lower, upper)
            && lower == upper
        {
            return decide_constant(lower.rem_euclid(modulus), Relation::Zero);
        }
        let Some((name, coefficient)) = single_term(expression.terms()) else {
            return Decision::Open;
        };
        let common = greatest_common_divisor(coefficient, modulus);
        if expression.constant_part() % common != 0 {
            return Decision::Fails;
        }
        let needed = modulus / common;
        match self.residues.get(name) {
            Some((known_modulus, known_residue)) if known_modulus % needed == 0 => {
                let value = coefficient * known_residue + expression.constant_part();
                decide_constant(value.rem_euclid(modulus), Relation::Zero)
            }
            _ => Decision::Open,
        }
    }

    pub fn decide(&self, condition: &Condition) -> Decision {
        match condition.relation {
            Relation::Congruent(modulus) => {
                return self.decide_congruence(&condition.expression, modulus);
            }
            Relation::NotCongruent(modulus) => {
                return match self.decide_congruence(&condition.expression, modulus) {
                    Decision::Holds => Decision::Fails,
                    Decision::Fails => Decision::Holds,
                    Decision::Open => Decision::Open,
                };
            }
            _ => {}
        }
        if condition.is_unsigned {
            return match condition.expression.as_constant() {
                Some(value) => decide_constant(value, condition.relation),
                None => Decision::Open,
            };
        }
        let (lower, upper) = self.range(&condition.expression);
        match condition.relation {
            Relation::AtLeastZero => match (lower, upper) {
                (Some(lower), _) if lower >= 0 => Decision::Holds,
                (_, Some(upper)) if upper < 0 => Decision::Fails,
                _ => Decision::Open,
            },
            _ => {
                let excluded_zero =
                    single_term(condition.expression.terms()).is_some_and(|(name, coefficient)| {
                        let constant = condition.expression.constant_part();
                        constant % coefficient == 0 && !self.allowed(name, -constant / coefficient)
                    });
                let zero = match (lower, upper) {
                    _ if excluded_zero => Decision::Fails,
                    (Some(lower), Some(upper)) if lower == 0 && upper == 0 => Decision::Holds,
                    (Some(lower), _) if lower > 0 => Decision::Fails,
                    (_, Some(upper)) if upper < 0 => Decision::Fails,
                    _ => Decision::Open,
                };
                match (condition.relation, zero) {
                    (Relation::NotZero, Decision::Holds) => Decision::Fails,
                    (Relation::NotZero, Decision::Fails) => Decision::Holds,
                    (_, decision) => decision,
                }
            }
        }
    }
}

fn decide_constant(value: i128, relation: Relation) -> Decision {
    let holds = match relation {
        Relation::AtLeastZero => value >= 0,
        Relation::Zero => value == 0,
        Relation::NotZero => value != 0,
        Relation::Congruent(modulus) => value.rem_euclid(modulus) == 0,
        Relation::NotCongruent(modulus) => value.rem_euclid(modulus) != 0,
    };
    if holds {
        Decision::Holds
    } else {
        Decision::Fails
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_polynomial_is_written_highest_degree_first() {
        let n = Poly::from_linear(&Linear::symbol("n"));
        let m = Poly::from_linear(&Linear::symbol("m"));
        let poly = n
            .times(&m)
            .times(&Poly::constant(5))
            .plus(&m.times(&Poly::constant(4)))
            .plus(&Poly::constant(3));
        assert_eq!(poly.text(), "5 * m * n + 4 * m + 3");
    }

    #[test]
    fn a_single_symbol_condition_is_written_as_a_bound() {
        let condition = Condition::at_least_zero(Linear::symbol("edx").plus_constant(-1));
        assert_eq!(condition.text(), "edx >= 1");
        assert_eq!(condition.negated().text(), "edx <= 0");
    }

    #[test]
    fn a_bound_decides_a_condition() {
        let bounds = Bounds::default()
            .with(&Condition::at_least_zero(
                Linear::symbol("edx").plus_constant(-1),
            ))
            .unwrap();
        let later = Condition::at_least_zero(Linear::symbol("edx"));
        assert_eq!(bounds.decide(&later), Decision::Holds);
    }

    #[test]
    fn contradictory_bounds_are_refused() {
        let bounds = Bounds::default()
            .with(&Condition::at_least_zero(
                Linear::symbol("x").plus_constant(-1),
            ))
            .unwrap();
        let contrary = Condition::at_least_zero(Linear::symbol("x").scaled(-1));
        assert!(bounds.with(&contrary).is_none());
    }
}
