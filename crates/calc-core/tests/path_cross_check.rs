use calc_core::{ResultValue, RoundingError, enclose_decimal, evaluate_exact, evaluate_f64};
use calc_exec::Preference;
use calc_exec_cpu::CpuBackend;
use calc_expr::{ExprId, ExprPool};
use calc_numbers::{Integer, Number};
use calc_syntax::parse_expression;
use std::time::{Duration, Instant};

const SEED: u64 = 0x2026_0916_0001;
const EXPRESSIONS: usize = 400;
const SIGNIFICANT_DIGITS: u32 = 15;
const LEAST_COMPARED: usize = EXPRESSIONS / 2;
const DEPTH: u32 = 3;
const SIGNIFICANCE_FACTOR: i64 = 10;
const ENCLOSURE_TIME: Duration = Duration::from_millis(200);

struct Random {
    state: u64,
}

impl Random {
    fn new(seed: u64) -> Random {
        Random { state: seed | 1 }
    }

    fn next(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    fn below(&mut self, limit: u64) -> u64 {
        self.next() % limit
    }
}

fn leaf_text(random: &mut Random) -> String {
    match random.below(6) {
        0 => (random.below(9) + 1).to_string(),
        1 => "pi".to_owned(),
        2 => "e".to_owned(),
        3 => format!("0.{}", random.below(9) + 1),
        4 => format!("{}.{}", random.below(9) + 1, random.below(9) + 1),
        _ => format!("{}/{}", random.below(9) + 1, random.below(9) + 1),
    }
}

fn expression_text(random: &mut Random, depth: u32) -> String {
    if depth == 0 {
        return leaf_text(random);
    }
    let inner = |random: &mut Random| expression_text(random, depth - 1);
    match random.below(12) {
        0 => format!("-({})", inner(random)),
        1 => format!("sqrt(abs({}))", inner(random)),
        2 => format!("exp(({}) / 8)", inner(random)),
        3 => format!("ln(abs({}) + 1)", inner(random)),
        4 => format!("sin({})", inner(random)),
        5 => format!("cos({})", inner(random)),
        6 => format!("atan({})", inner(random)),
        7 => format!("({}) + ({})", inner(random), inner(random)),
        8 => format!("({}) - ({})", inner(random), inner(random)),
        9 => format!("({}) * ({})", inner(random), inner(random)),
        10 => format!("({}) / {}", inner(random), random.below(8) + 2),
        _ => format!("({})^{}", inner(random), random.below(2) + 2),
    }
}

fn exact_value(value: &ResultValue) -> Option<Number> {
    match value {
        ResultValue::Number(number) => number.to_exact().ok(),
        _ => None,
    }
}

fn is_at_most(left: &Number, right: &Number) -> bool {
    match right.sub_exact(left) {
        Ok(Number::Integer(difference)) => !difference.is_negative(),
        Ok(Number::Rational(difference)) => !difference.numerator().is_negative(),
        _ => false,
    }
}

fn readable(value: &Number) -> String {
    format!("{:e}", value.round_to_f64_ties_even())
}

fn is_equal(left: &Number, right: &Number) -> bool {
    matches!(left.sub_exact(right), Ok(Number::Integer(difference)) if difference.is_zero())
}

struct Compared {
    machine: usize,
    points: usize,
}

fn has_significance(machine: f64, value: &Number, bound: &Number) -> bool {
    if machine == 0.0 || !machine.is_finite() {
        return false;
    }
    let Ok(magnitude) = value.mul_exact(&Number::Integer(Integer::from(
        if machine.is_sign_negative() {
            -1_i64
        } else {
            1
        },
    ))) else {
        return false;
    };
    let Ok(widened) = bound.mul_exact(&Number::Integer(Integer::from(SIGNIFICANCE_FACTOR))) else {
        return false;
    };
    is_at_most(&widened, &magnitude)
}

fn machine_scalar(pool: &mut ExprPool, expression: ExprId) -> Option<(f64, Number, Number)> {
    let backend = CpuBackend::new();
    let machine = evaluate_f64(pool, expression, &[&backend], Preference::Automatic).ok()?;
    let ResultValue::Number(Number::F64(value)) = machine.result().value() else {
        return None;
    };
    let RoundingError::Bound(bound) = machine.result().rounding_error() else {
        return None;
    };
    Some((
        *value,
        exact_value(machine.result().value())?,
        exact_value(bound)?,
    ))
}

fn compare_paths(pool: &mut ExprPool, text: &str, compared: &mut Compared) {
    let Ok(expression) = parse_expression(pool, text) else {
        return;
    };
    let scalar = machine_scalar(pool, expression)
        .filter(|(machine, value, bound)| has_significance(*machine, value, bound));
    let deadline = Instant::now() + ENCLOSURE_TIME;
    let Ok(enclosure) = enclose_decimal(pool, expression, SIGNIFICANT_DIGITS, &|| {
        Instant::now() >= deadline
    }) else {
        return;
    };
    if let Some((_, value, bound)) = scalar
        && let (Ok(lowest), Ok(highest)) = (value.sub_exact(&bound), value.add_exact(&bound))
    {
        assert!(
            is_at_most(&lowest, &enclosure.upper) && is_at_most(&enclosure.lower, &highest),
            "seed {SEED:#x}: the machine value of {text} and its enclosure do not meet: \
             machine {} with bound {}, enclosure {} to {}",
            readable(&value),
            readable(&bound),
            readable(&enclosure.lower),
            readable(&enclosure.upper)
        );
        compared.machine += 1;
    }
    if is_equal(&enclosure.lower, &enclosure.upper)
        && let Ok(exact) = evaluate_exact(pool, expression)
        && let Some(value) = exact
            .rational_value()
            .and_then(|number| number.to_exact().ok())
    {
        assert!(
            is_equal(&value, &enclosure.lower),
            "seed {SEED:#x}: the exact value of {text} is not the point its enclosure gives: \
             exact {}, enclosure {}",
            readable(&value),
            readable(&enclosure.lower)
        );
        compared.points += 1;
    }
}

#[test]
fn every_path_agrees_on_generated_expressions() {
    let mut random = Random::new(SEED);
    let mut pool = ExprPool::new();
    let mut compared = Compared {
        machine: 0,
        points: 0,
    };
    for _ in 0..EXPRESSIONS {
        let text = expression_text(&mut random, DEPTH);
        compare_paths(&mut pool, &text, &mut compared);
    }
    assert!(
        compared.machine >= LEAST_COMPARED,
        "seed {SEED:#x}: only {} of {EXPRESSIONS} expressions reached the machine comparison; \
         on a loaded machine the {ENCLOSURE_TIME:?} deadline of each enclosure is the likely cause",
        compared.machine
    );
    assert!(
        compared.points > 0,
        "seed {SEED:#x}: no expression gave a point enclosure to compare an exact value with"
    );
}
