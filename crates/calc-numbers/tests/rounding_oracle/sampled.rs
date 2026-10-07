use crate::knowledge::ElementaryFunction;
use crate::oracle::Oracle;
use crate::sampling::{SplitMix, special_f32_values, special_f64_values};
use crate::verdict::Verdict;

const LISTED_FINDINGS_LIMIT: usize = 64;
const F64_SAMPLE_SIZE: usize = 1 << 14;
const F32_BIVARIATE_SAMPLE_SIZE: usize = 1 << 16;
const SAMPLE_SEED: u64 = 0x5452_4f55_4e44_3532;

#[derive(Clone, Debug, PartialEq)]
pub struct SampleFinding {
    pub function: ElementaryFunction,
    pub argument_bits: Vec<u64>,
    pub result_bits: u64,
    pub verdict: Verdict,
}

#[derive(Debug, Default)]
pub struct SampleReport {
    pub listed: Vec<SampleFinding>,
    pub unlisted_count: u64,
    pub checked_count: u64,
}

impl SampleReport {
    pub fn is_clean(&self) -> bool {
        self.listed.is_empty() && self.unlisted_count == 0
    }

    fn record(&mut self, finding: SampleFinding) {
        if self.listed.len() < LISTED_FINDINGS_LIMIT {
            self.listed.push(finding);
        } else {
            self.unlisted_count += 1;
        }
    }
}

pub fn arity(function: ElementaryFunction) -> usize {
    match function {
        ElementaryFunction::Power | ElementaryFunction::Arctangent2 => 2,
        _ => 1,
    }
}

fn special_f64_arguments(function: ElementaryFunction) -> Vec<Vec<f64>> {
    let specials = special_f64_values();
    if arity(function) == 1 {
        return specials.into_iter().map(|value| vec![value]).collect();
    }
    let mut pairs: Vec<Vec<f64>> = specials
        .iter()
        .flat_map(|first| specials.iter().map(move |second| vec![*first, *second]))
        .collect();
    if function == ElementaryFunction::Power {
        pairs.push(vec![3.0, 34.0]);
        pairs.push(vec![25.0, 11.5]);
        pairs.push(vec![2.0, -1075.0]);
    }
    pairs
}

fn special_f32_arguments(function: ElementaryFunction) -> Vec<Vec<f32>> {
    let specials = special_f32_values();
    if arity(function) == 1 {
        return specials.into_iter().map(|value| vec![value]).collect();
    }
    let mut pairs: Vec<Vec<f32>> = specials
        .iter()
        .flat_map(|first| specials.iter().map(move |second| vec![*first, *second]))
        .collect();
    if function == ElementaryFunction::Power {
        pairs.push(vec![3.0, 15.0]);
        pairs.push(vec![25.0, 5.5]);
        pairs.push(vec![2.0, -150.0]);
    }
    pairs
}

pub fn check_f64_arguments(
    oracle: &mut Oracle,
    function: ElementaryFunction,
    arguments: impl Iterator<Item = Vec<f64>>,
    report: &mut SampleReport,
) {
    for argument_list in arguments {
        let (result, verdict) = oracle.check_f64(function, &argument_list);
        report.checked_count += 1;
        if verdict != Verdict::WithinBound {
            report.record(SampleFinding {
                function,
                argument_bits: argument_list.iter().map(|value| value.to_bits()).collect(),
                result_bits: result.to_bits(),
                verdict,
            });
        }
    }
}

pub fn check_f32_arguments(
    oracle: &mut Oracle,
    function: ElementaryFunction,
    arguments: impl Iterator<Item = Vec<f32>>,
    report: &mut SampleReport,
) {
    for argument_list in arguments {
        let (result, verdict) = oracle.check_f32(function, &argument_list);
        report.checked_count += 1;
        if verdict != Verdict::WithinBound {
            report.record(SampleFinding {
                function,
                argument_bits: argument_list
                    .iter()
                    .map(|value| u64::from(value.to_bits()))
                    .collect(),
                result_bits: u64::from(result.to_bits()),
                verdict,
            });
        }
    }
}

fn seeded_f64_arguments(function: ElementaryFunction, count: usize) -> Vec<Vec<f64>> {
    let mut generator = SplitMix::new(SAMPLE_SEED);
    (0..count)
        .map(|_| (0..arity(function)).map(|_| generator.next_f64()).collect())
        .collect()
}

fn seeded_f32_arguments(function: ElementaryFunction, count: usize) -> Vec<Vec<f32>> {
    let mut generator = SplitMix::new(SAMPLE_SEED);
    (0..count)
        .map(|_| (0..arity(function)).map(|_| generator.next_f32()).collect())
        .collect()
}

fn f64_sample_report(function: ElementaryFunction) -> SampleReport {
    let mut oracle = Oracle::new();
    let mut report = SampleReport::default();
    check_f64_arguments(
        &mut oracle,
        function,
        special_f64_arguments(function).into_iter(),
        &mut report,
    );
    check_f64_arguments(
        &mut oracle,
        function,
        seeded_f64_arguments(function, F64_SAMPLE_SIZE).into_iter(),
        &mut report,
    );
    report
}

fn f32_bivariate_sample_report(function: ElementaryFunction) -> SampleReport {
    let mut oracle = Oracle::new();
    let mut report = SampleReport::default();
    check_f32_arguments(
        &mut oracle,
        function,
        special_f32_arguments(function).into_iter(),
        &mut report,
    );
    check_f32_arguments(
        &mut oracle,
        function,
        seeded_f32_arguments(function, F32_BIVARIATE_SAMPLE_SIZE).into_iter(),
        &mut report,
    );
    report
}

macro_rules! f64_samples {
    ($($name:ident = $function:ident),* $(,)?) => {
        $(
            #[test]
            #[ignore = "seeded f64 sample with the exact tier, run in release with --ignored"]
            fn $name() {
                let report = f64_sample_report(ElementaryFunction::$function);

                assert!(report.is_clean(), "{report:#x?}");
            }
        )*
    };
}

f64_samples!(
    sampled_f64_exponential = Exponential,
    sampled_f64_logarithm = Logarithm,
    sampled_f64_square_root = SquareRoot,
    sampled_f64_sine = Sine,
    sampled_f64_cosine = Cosine,
    sampled_f64_tangent = Tangent,
    sampled_f64_arcsine = Arcsine,
    sampled_f64_arccosine = Arccosine,
    sampled_f64_arctangent = Arctangent,
    sampled_f64_power = Power,
    sampled_f64_arctangent2 = Arctangent2,
);

#[test]
#[ignore = "seeded f32 sample of pow_f32, run in release with --ignored"]
fn sampled_f32_power() {
    let report = f32_bivariate_sample_report(ElementaryFunction::Power);

    assert!(report.is_clean(), "{report:#x?}");
}

#[test]
#[ignore = "seeded f32 sample of atan2_f32, run in release with --ignored"]
fn sampled_f32_arctangent2() {
    let report = f32_bivariate_sample_report(ElementaryFunction::Arctangent2);

    assert!(report.is_clean(), "{report:#x?}");
}
