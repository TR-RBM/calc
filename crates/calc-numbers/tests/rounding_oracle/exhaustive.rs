use crate::knowledge::{ElementaryFunction, UNIVARIATE_FUNCTIONS};
use crate::oracle::{Oracle, evaluate_f32};
use crate::verdict::Verdict;

const SHARD_INPUT_BITS: u32 = 26;
const LISTED_FINDINGS_LIMIT: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Finding {
    pub function: ElementaryFunction,
    pub input_bits: u32,
    pub result_bits: u32,
    pub verdict: Verdict,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct RangeReport {
    pub listed: Vec<Finding>,
    pub unlisted_count: u64,
}

impl RangeReport {
    pub fn is_clean(&self) -> bool {
        self.listed.is_empty() && self.unlisted_count == 0
    }

    fn record(&mut self, finding: Finding) {
        if self.listed.len() < LISTED_FINDINGS_LIMIT {
            self.listed.push(finding);
        } else {
            self.unlisted_count += 1;
        }
    }
}

pub type UnivariateEvaluator = fn(ElementaryFunction, f32) -> f32;

pub fn reference_evaluator(function: ElementaryFunction, argument: f32) -> f32 {
    evaluate_f32(function, &[argument])
}

pub fn check_f32_inputs(
    oracle: &mut Oracle,
    evaluator: UnivariateEvaluator,
    inputs: impl Iterator<Item = u32>,
) -> RangeReport {
    let mut report = RangeReport::default();
    for input_bits in inputs {
        let argument = f32::from_bits(input_bits);
        for function in UNIVARIATE_FUNCTIONS {
            let result = evaluator(function, argument);
            let verdict = oracle.check_f32_result(function, &[argument], result);
            if verdict != Verdict::WithinBound {
                report.record(Finding {
                    function,
                    input_bits,
                    result_bits: result.to_bits(),
                    verdict,
                });
            }
        }
    }
    report
}

fn check_shard(shard: u32) -> RangeReport {
    let mut oracle = Oracle::new();
    let first = shard << SHARD_INPUT_BITS;
    let last = first | ((1u32 << SHARD_INPUT_BITS) - 1);
    check_f32_inputs(&mut oracle, reference_evaluator, first..=last)
}

macro_rules! exhaustive_shards {
    ($($name:ident = $shard:literal),* $(,)?) => {
        $(
            #[test]
            #[ignore = "exhaustive run over 2^26 f32 inputs, run in release with --ignored"]
            fn $name() {
                let report = check_shard($shard);

                assert!(report.is_clean(), "{report:#x?}");
            }
        )*
    };
}

exhaustive_shards!(
    exhaustive_f32_shard_00 = 0,
    exhaustive_f32_shard_01 = 1,
    exhaustive_f32_shard_02 = 2,
    exhaustive_f32_shard_03 = 3,
    exhaustive_f32_shard_04 = 4,
    exhaustive_f32_shard_05 = 5,
    exhaustive_f32_shard_06 = 6,
    exhaustive_f32_shard_07 = 7,
    exhaustive_f32_shard_08 = 8,
    exhaustive_f32_shard_09 = 9,
    exhaustive_f32_shard_10 = 10,
    exhaustive_f32_shard_11 = 11,
    exhaustive_f32_shard_12 = 12,
    exhaustive_f32_shard_13 = 13,
    exhaustive_f32_shard_14 = 14,
    exhaustive_f32_shard_15 = 15,
    exhaustive_f32_shard_16 = 16,
    exhaustive_f32_shard_17 = 17,
    exhaustive_f32_shard_18 = 18,
    exhaustive_f32_shard_19 = 19,
    exhaustive_f32_shard_20 = 20,
    exhaustive_f32_shard_21 = 21,
    exhaustive_f32_shard_22 = 22,
    exhaustive_f32_shard_23 = 23,
    exhaustive_f32_shard_24 = 24,
    exhaustive_f32_shard_25 = 25,
    exhaustive_f32_shard_26 = 26,
    exhaustive_f32_shard_27 = 27,
    exhaustive_f32_shard_28 = 28,
    exhaustive_f32_shard_29 = 29,
    exhaustive_f32_shard_30 = 30,
    exhaustive_f32_shard_31 = 31,
    exhaustive_f32_shard_32 = 32,
    exhaustive_f32_shard_33 = 33,
    exhaustive_f32_shard_34 = 34,
    exhaustive_f32_shard_35 = 35,
    exhaustive_f32_shard_36 = 36,
    exhaustive_f32_shard_37 = 37,
    exhaustive_f32_shard_38 = 38,
    exhaustive_f32_shard_39 = 39,
    exhaustive_f32_shard_40 = 40,
    exhaustive_f32_shard_41 = 41,
    exhaustive_f32_shard_42 = 42,
    exhaustive_f32_shard_43 = 43,
    exhaustive_f32_shard_44 = 44,
    exhaustive_f32_shard_45 = 45,
    exhaustive_f32_shard_46 = 46,
    exhaustive_f32_shard_47 = 47,
    exhaustive_f32_shard_48 = 48,
    exhaustive_f32_shard_49 = 49,
    exhaustive_f32_shard_50 = 50,
    exhaustive_f32_shard_51 = 51,
    exhaustive_f32_shard_52 = 52,
    exhaustive_f32_shard_53 = 53,
    exhaustive_f32_shard_54 = 54,
    exhaustive_f32_shard_55 = 55,
    exhaustive_f32_shard_56 = 56,
    exhaustive_f32_shard_57 = 57,
    exhaustive_f32_shard_58 = 58,
    exhaustive_f32_shard_59 = 59,
    exhaustive_f32_shard_60 = 60,
    exhaustive_f32_shard_61 = 61,
    exhaustive_f32_shard_62 = 62,
    exhaustive_f32_shard_63 = 63,
);

#[cfg(test)]
mod tests {
    use super::*;

    const PLANTED_INPUT_BITS: u32 = 0x3f80_0010;

    fn evaluator_with_one_misrounded_sine(function: ElementaryFunction, argument: f32) -> f32 {
        let correct = reference_evaluator(function, argument);
        if function == ElementaryFunction::Sine && argument.to_bits() == PLANTED_INPUT_BITS {
            f32::from_bits(correct.to_bits() + 2)
        } else {
            correct
        }
    }

    #[test]
    fn range_check_lists_the_planted_misrounded_sine() {
        let mut oracle = Oracle::new();
        let inputs = PLANTED_INPUT_BITS - 16..PLANTED_INPUT_BITS + 16;

        let report = check_f32_inputs(&mut oracle, evaluator_with_one_misrounded_sine, inputs);

        assert_eq!(
            report.listed,
            vec![Finding {
                function: ElementaryFunction::Sine,
                input_bits: PLANTED_INPUT_BITS,
                result_bits: reference_evaluator(
                    ElementaryFunction::Sine,
                    f32::from_bits(PLANTED_INPUT_BITS)
                )
                .to_bits()
                    + 2,
                verdict: Verdict::ExceedsBound,
            }]
        );
    }
}
