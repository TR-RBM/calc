use crate::binary64_words::{
    DoubleWord, decided_with_absolute, plus, plus_double, square, times, two_product, two_sum,
};
use crate::word_conversion::{f64_from_small_u64, i64_from_small_integral_f64};

const TABLE_STEPS: f64 = 64.0;
const ROUNDING_SHIFT: f64 = 4_503_599_627_370_496.0;
const TWO_OVER_PI: f64 = std::f64::consts::FRAC_2_PI;
const SMALLEST_MAGNITUDE: f64 = f64::from_bits(723 << 52);
const LARGEST_MAGNITUDE: f64 = f64::from_bits(1043 << 52);
const ERROR_EXPONENT: i64 = 74;
const SPLIT_ERROR_PER_QUARTER_TURN: f64 = f64::from_bits(905 << 52);
const HALF_PI_FIRST: f64 = f64::from_bits(0x3ff9_21fb_5440_0000);
const HALF_PI_SECOND: f64 = f64::from_bits(0x3dd0_b461_1a60_0000);
const HALF_PI_THIRD: f64 = f64::from_bits(0x3ba3_198a_2e03_7073);
const MINUS_ONE_SIXTH: DoubleWord = DoubleWord {
    high: f64::from_bits(0xbfc5_5555_5555_5555),
    low: f64::from_bits(0xbc65_5555_5555_5555),
};
const SINE_TAIL: [f64; 3] = [1.0 / 120.0, -1.0 / 5040.0, 1.0 / 362_880.0];
const COSINE_TAIL: [f64; 3] = [1.0 / 24.0, -1.0 / 720.0, 1.0 / 40320.0];

const SINE_COSINE_TABLE: [(u64, u64, u64, u64); 52] = [
    (
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3ff0_0000_0000_0000,
        0x0000_0000_0000_0000,
    ),
    (
        0x3f8f_ffaa_aaee_eed5,
        0xbc02_ab63_9a9f_0776,
        0x3fef_ff00_0155_549f,
        0x3c82_8a28_a03a_5ef3,
    ),
    (
        0x3f9f_feaa_aeee_e86f,
        0xbc3c_d406_fb22_4ae2,
        0x3fef_fc00_1555_27d3,
        0xbc83_b544_92d8_9b5b,
    ),
    (
        0x3fa7_fdc0_1032_fba9,
        0xbc45_99bd_f46e_997a,
        0x3fef_f700_6bfd_f99f,
        0xbc78_b3b5_6064_8d5f,
    ),
    (
        0x3faf_faaa_eeed_4edb,
        0xbc42_d16d_3268_4b69,
        0x3fef_f001_5549_f4d3,
        0x3c83_2838_7b99_426f,
    ),
    (
        0x3fb3_facb_12d1_755b,
        0xbc59_2191_5299_468b,
        0x3fef_e703_4129_ef6f,
        0xbc6c_bf43_37c9_6f97,
    ),
    (
        0x3fb7_f701_0325_50e4,
        0x3c3a_fc2d_1800_501a,
        0x3fef_dc06_bf7e_6b9b,
        0x3c83_1902_b535_f8db,
    ),
    (
        0x3fbb_f1b7_8568_391d,
        0x3c5e_9184_1dea_4cc8,
        0x3fef_cf0c_800e_99b1,
        0x3c6e_a3d7_86d1_86ac,
    ),
    (
        0x3fbf_eaae_ee86_ee36,
        0xbc4a_fcb2_bcc6_f03b,
        0x3fef_c015_527d_5bd3,
        0x3c8b_68f3_5094_efb8,
    ),
    (
        0x3fc1_f0d3_d7af_ceaf,
        0xbc66_ef95_0997_69a5,
        0x3fef_af22_263c_4bd3,
        0xbc55_2ace_133a_2769,
    ),
    (
        0x3fc3_eb31_2c5d_66cb,
        0x3c64_7d66_6b66_cb91,
        0x3fef_9c34_0a7c_c428,
        0x3c8c_5b6b_063b_7462,
    ),
    (
        0x3fc5_e44f_cfa1_26f3,
        0xbc66_f443_063f_89b6,
        0x3fef_874c_2e1e_ecf6,
        0xbc8c_6514_e133_2b16,
    ),
    (
        0x3fc7_dc10_2fba_f2b5,
        0x3c45_ab50_e23c_97c3,
        0x3fef_706b_df9e_ce1c,
        0xbc86_98c8_0c36_dcb4,
    ),
    (
        0x3fc9_d252_d0ce_c312,
        0x3c59_c43d_80b1_137d,
        0x3fef_5794_8cff_6797,
        0x3c6e_3a0d_3e03_b1d4,
    ),
    (
        0x3fcb_c6f8_4edc_6199,
        0x3c69_c1a5_6a7b_0cab,
        0x3fef_3cc7_c3b3_d16e,
        0xbc62_1a3a_d28a_3494,
    ),
    (
        0x3fcd_b9e1_5fb5_a5d0,
        0xbc63_2e20_d6cc_6fc2,
        0x3fef_2007_3086_649f,
        0x3c7b_9404_16c1_984b,
    ),
    (
        0x3fcf_aaee_d4f3_1577,
        0xbc61_5d88_508e_32b8,
        0x3fef_0154_9f7d_eea1,
        0x3c8d_3c1e_99e5_cafd,
    ),
    (
        0x3fd0_cd00_cef3_6436,
        0xbc79_fb0a_0c93_e2b4,
        0x3fee_e0b1_fbc0_f11c,
        0xbc4b_fd23_80bb_c3b1,
    ),
    (
        0x3fd1_c37d_64c6_b876,
        0x3c74_6076_fe0d_cff4,
        0x3fee_be21_4f76_efa8,
        0xbc80_2f9f_12ba_543e,
    ),
    (
        0x3fd2_b8dd_c43e_b49f,
        0x3c61_5538_99f2_d807,
        0x3fee_99a4_c3a7_cd83,
        0xbc82_264b_1bc5_3ce8,
    ),
    (
        0x3fd3_ad12_9769_d3d8,
        0x3c00_3d55_0487_839a,
        0x3fee_733e_a019_3d40,
        0xbc86_428b_3546_ce13,
    ),
    (
        0x3fd4_a00c_9b0f_3d20,
        0x3c78_23ba_6bb0_8ead,
        0x3fee_4af1_4b2a_449c,
        0xbc86_8ca0_2e8a_6833,
    ),
    (
        0x3fd5_91bc_9fa2_f597,
        0x3c67_c74b_ac3f_e0cb,
        0x3fee_20bf_49ac_d6c1,
        0xbc56_60ae_c7ef_636b,
    ),
    (
        0x3fd6_8213_8a38_d7f7,
        0xbc7d_8892_0244_4aad,
        0x3fed_f4ab_3ebd_875e,
        0xbc8e_2d8a_7e67_36c4,
    ),
    (
        0x3fd7_7102_5576_4214,
        0xbc66_ead7_314b_b6ce,
        0x3fed_c6b7_eb99_5912,
        0x3c54_b364_776d_cd35,
    ),
    (
        0x3fd8_5e7a_1282_6949,
        0x3c78_a40e_9b5f_ace0,
        0x3fed_96e8_2f71_a9dc,
        0x3c8f_f61b_d5d2_039d,
    ),
    (
        0x3fd9_4a6b_e9f5_46c5,
        0xbc76_9ce1_3e68_3f58,
        0x3fed_653f_073e_4040,
        0xbc87_6236_434b_ec37,
    ),
    (
        0x3fda_34c9_1cc5_0cca,
        0xbc5a_310e_3b50_cecd,
        0x3fed_31bf_8d8d_7c06,
        0x3c7e_60dd_3089_cbdd,
    ),
    (
        0x3fdb_1d83_0532_1617,
        0xbc7a_e242_cb99_f519,
        0x3fec_fc6c_fa52_ad9f,
        0x3c88_b5b5_508f_2a0d,
    ),
    (
        0x3fdc_048b_17b1_40a3,
        0x3c61_9fe6_757e_9fa7,
        0x3fec_c54a_a2b2_972e,
        0x3c64_ee16_2ba8_3a98,
    ),
    (
        0x3fdc_e9d2_e3d4_a51f,
        0xbc62_fc8a_12da_e298,
        0x3fec_8c5b_f8ce_1a84,
        0x3c7a_b3d1_a159_0123,
    ),
    (
        0x3fdd_cd4c_1532_9c9a,
        0x3c70_d4c6_e171_fd9a,
        0x3fec_51a4_8b8b_175e,
        0xbc61_bbb4_3b9a_a880,
    ),
    (
        0x3fde_aee8_744b_05f0,
        0xbc57_89b4_3c9b_027d,
        0x3fec_1528_065b_7d50,
        0xbc88_9211_1312_e828,
    ),
    (
        0x3fdf_8e99_e76a_bc97,
        0x3c59_d950_af2d_00a3,
        0x3feb_d6ea_3102_94f5,
        0x3c73_1bbc_c88c_109d,
    ),
    (
        0x3fe0_3629_39c6_9955,
        0xbc82_d8cd_7839_7b01,
        0x3feb_96ee_ef58_840e,
        0x3c54_5a3c_c78f_ade0,
    ),
    (
        0x3fe0_a402_1e9e_1001,
        0xbc86_f643_a139_14f6,
        0x3feb_553a_410c_104e,
        0x3c58_ff79_4702_7a15,
    ),
    (
        0x3fe1_10d0_c4b6_9c3b,
        0x3c8d_9189_9880_9981,
        0x3feb_11d0_4162_a4c6,
        0x3c71_dd56_1efb_c0c2,
    ),
    (
        0x3fe1_7c8e_5f2e_edb0,
        0x3c63_5e57_102e_2488,
        0x3fea_ccb5_26f6_9de5,
        0x3c88_fb6a_8dd6_b6cc,
    ),
    (
        0x3fe1_e734_3236_574c,
        0x3c72_2a3f_a4f4_1d5a,
        0x3fea_85ed_4373_e02d,
        0x3c69_be06_385e_c792,
    ),
    (
        0x3fe2_50bb_9378_8bbb,
        0x3c7e_a3d0_2457_bcce,
        0x3fea_3d7d_0352_bdcf,
        0xbc86_8dba_eca1_9669,
    ),
    (
        0x3fe2_b91d_ea88_421e,
        0xbc8f_a371_db21_6ab0,
        0x3fe9_f368_ed91_2f85,
        0xbc81_d200_c579_1606,
    ),
    (
        0x3fe3_2054_b148_bc4f,
        0x3c8f_6b42_095a_135b,
        0x3fe9_a7b5_a36a_6514,
        0x3c87_22cf_cc9f_a7a9,
    ),
    (
        0x3fe3_8659_7456_282b,
        0xbc71_0fad_a93b_07a8,
        0x3fe9_5a67_e00c_b1fd,
        0xbc80_befd_a21f_862d,
    ),
    (
        0x3fe3_eb25_d36c_d53a,
        0xbc5b_e570_e157_0fc0,
        0x3fe9_0b84_784d_daf7,
        0xbc70_feb1_0ab9_3b87,
    ),
    (
        0x3fe4_4eb3_81cf_386b,
        0xbc83_ed6c_1e6a_5505,
        0x3fe8_bb10_5a5d_c900,
        0x3c88_63e0_3e94_74c1,
    ),
    (
        0x3fe4_b0fc_46aa_b761,
        0x3c20_da05_738c_c59c,
        0x3fe8_6910_8d77_a6c6,
        0x3c73_38ff_e2bf_e9dd,
    ),
    (
        0x3fe5_11f9_fd7b_351c,
        0xbc85_c0e8_61c4_8831,
        0x3fe8_158a_3191_6d5d,
        0xbc6d_e8b9_0b82_28de,
    ),
    (
        0x3fe5_71a6_966d_59b3,
        0x3c5c_843b_4d0f_b197,
        0x3fe7_c082_7f09_e54f,
        0xbc6c_73d6_d72a_ee68,
    ),
    (
        0x3fe5_cffc_16bf_8f0d,
        0x3c89_6cb3_70eb_578a,
        0x3fe7_69fe_c655_211f,
        0xbc68_27d5_cf8c_68c5,
    ),
    (
        0x3fe6_2cf4_9921_ac79,
        0xbc8e_dd98_55b6_241a,
        0x3fe7_1204_6fa7_7678,
        0x3c84_25b0_a502_9c81,
    ),
    (
        0x3fe6_888a_4e13_4b2f,
        0xbc86_b7d3_7644_d5e6,
        0x3fe6_b898_fa9e_fb5d,
        0x3c71_5ac7_86cc_f4b2,
    ),
    (
        0x3fe6_e2b7_7c40_bde1,
        0xbc70_e729_857f_ad53,
        0x3fe6_5dc1_fdeb_8cba,
        0xbc59_7c1b_4733_7c77,
    ),
];
struct SineCosine {
    sine: DoubleWord,
    cosine: DoubleWord,
}

fn tail(square: f64, coefficients: [f64; 3]) -> f64 {
    square * (coefficients[0] + square * (coefficients[1] + square * coefficients[2]))
}

fn sine_cosine_of_small(value: DoubleWord) -> SineCosine {
    let squared = square(value);
    let sine_series = plus_double(MINUS_ONE_SIXTH, tail(squared.high, SINE_TAIL));
    let sine = plus(value, times(times(value, squared), sine_series));
    let cosine_series = plus_double(DoubleWord::exact(-0.5), tail(squared.high, COSINE_TAIL));
    let cosine = plus_double(times(squared, cosine_series), 1.0);
    SineCosine { sine, cosine }
}

fn nearest_index(value: f64) -> Option<usize> {
    let rounded = (value * TABLE_STEPS + ROUNDING_SHIFT) - ROUNDING_SHIFT;
    usize::try_from(i64_from_small_integral_f64(rounded)).ok()
}

fn sine_cosine_of_reduced(reduced: DoubleWord) -> Option<SineCosine> {
    let magnitude = if reduced.high < 0.0 {
        reduced.negated()
    } else {
        reduced
    };
    let index = nearest_index(magnitude.high)?;
    let center = f64_from_small_u64(u64::try_from(index).ok()?) / TABLE_STEPS;
    let small = sine_cosine_of_small(two_sum(magnitude.high - center, magnitude.low));
    let (sine_high, sine_low, cosine_high, cosine_low) = *SINE_COSINE_TABLE.get(index)?;
    let base_sine = DoubleWord {
        high: f64::from_bits(sine_high),
        low: f64::from_bits(sine_low),
    };
    let base_cosine = DoubleWord {
        high: f64::from_bits(cosine_high),
        low: f64::from_bits(cosine_low),
    };
    let sine = plus(
        times(base_sine, small.cosine),
        times(base_cosine, small.sine),
    );
    let cosine = plus(
        times(base_cosine, small.cosine),
        times(base_sine, small.sine).negated(),
    );
    Some(SineCosine {
        sine: if reduced.high < 0.0 {
            sine.negated()
        } else {
            sine
        },
        cosine,
    })
}

fn reduced(magnitude: f64) -> Option<(DoubleWord, i64)> {
    let quarter_turns = (magnitude * TWO_OVER_PI + ROUNDING_SHIFT) - ROUNDING_SHIFT;
    let first = magnitude - quarter_turns * HALF_PI_FIRST;
    let second = two_sum(first, -(quarter_turns * HALF_PI_SECOND));
    let third = two_product(quarter_turns, HALF_PI_THIRD);
    Some((
        plus(second, third.negated()),
        i64_from_small_integral_f64(quarter_turns),
    ))
}

fn in_range(magnitude: f64) -> bool {
    (SMALLEST_MAGNITUDE..=LARGEST_MAGNITUDE).contains(&magnitude)
}

fn evaluated(value: f64) -> Option<(SineCosine, i64)> {
    let magnitude = value.abs();
    if !in_range(magnitude) {
        return None;
    }
    let (reduced, quarter_turns) = reduced(magnitude)?;
    Some((sine_cosine_of_reduced(reduced)?, quarter_turns))
}

fn reduction_error(quarter_turns: i64) -> Option<f64> {
    let turns = f64_from_small_u64(quarter_turns.unsigned_abs());
    Some(turns * SPLIT_ERROR_PER_QUARTER_TURN)
}

pub(crate) fn sine(value: f64) -> Option<f64> {
    let (pair, quarter_turns) = evaluated(value)?;
    let result = match quarter_turns.rem_euclid(4) {
        0 => pair.sine,
        1 => pair.cosine,
        2 => pair.sine.negated(),
        _ => pair.cosine.negated(),
    };
    let signed = if value < 0.0 {
        result.negated()
    } else {
        result
    };
    decided_with_absolute(signed, ERROR_EXPONENT, reduction_error(quarter_turns)?)
}

pub(crate) fn cosine(value: f64) -> Option<f64> {
    let (pair, quarter_turns) = evaluated(value)?;
    let result = match quarter_turns.rem_euclid(4) {
        0 => pair.cosine,
        1 => pair.sine.negated(),
        2 => pair.cosine.negated(),
        _ => pair.sine,
    };
    decided_with_absolute(result, ERROR_EXPONENT, reduction_error(quarter_turns)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ball::Ball;
    use crate::elementary::{accurate_cos_f64, accurate_sin_f64};

    const PRECISION: usize = 400;

    fn words(ball: &Ball) -> (u64, u64) {
        let high = ball.rounded_f64().unwrap();
        let rest = ball.sub(&Ball::from_f64(high, PRECISION).unwrap());
        (high.to_bits(), rest.rounded_f64().unwrap().to_bits())
    }

    #[test]
    fn every_table_row_is_the_double_words_nearest_the_sine_and_cosine_of_its_centre() {
        assert_eq!(SINE_COSINE_TABLE[0], (0, 0, 1.0_f64.to_bits(), 0));
        for (index, row) in SINE_COSINE_TABLE.iter().enumerate().skip(1) {
            let center = f64_from_small_u64(u64::try_from(index).unwrap()) / TABLE_STEPS;
            let pair = crate::series::sine_cosine(center, PRECISION - 53).unwrap();
            let (sine_high, sine_low) = words(&pair.sine);
            let (cosine_high, cosine_low) = words(&pair.cosine);
            assert_eq!(
                (sine_high, sine_low, cosine_high, cosine_low),
                *row,
                "row {index}"
            );
        }
    }

    #[test]
    fn the_split_of_half_pi_leaves_less_than_its_stated_error() {
        let half_pi = crate::series::pi(PRECISION).mul_power_of_two(-1);
        let parts = [HALF_PI_FIRST, HALF_PI_SECOND, HALF_PI_THIRD]
            .iter()
            .fold(Ball::from_f64(0.0, PRECISION).unwrap(), |total, part| {
                total.add(&Ball::from_f64(*part, PRECISION).unwrap())
            });
        let (lower, upper) = half_pi.sub(&parts).bounds();
        let limit = crate::number::Number::F64(SPLIT_ERROR_PER_QUARTER_TURN)
            .to_exact()
            .unwrap();
        let negative_limit = crate::number::Number::from(0_i64)
            .sub_exact(&limit)
            .unwrap();
        assert!(crate::test_support::is_at_most(&upper, &limit));
        assert!(crate::test_support::is_at_most(&negative_limit, &lower));
        assert_eq!(HALF_PI_FIRST.to_bits() & ((1 << 20) - 1), 0);
        assert_eq!(HALF_PI_SECOND.to_bits() & ((1 << 20) - 1), 0);
    }

    #[test]
    fn the_minus_one_sixth_constant_is_its_nearest_double_word() {
        let sixth = crate::number::Number::fraction(
            &crate::integer::Integer::from(-1_i64),
            &crate::integer::Integer::from(6_i64),
        )
        .unwrap();
        let ball = Ball::from_exact(&sixth, PRECISION).unwrap();
        assert_eq!(
            words(&ball),
            (
                MINUS_ONE_SIXTH.high.to_bits(),
                MINUS_ONE_SIXTH.low.to_bits()
            )
        );
    }

    fn samples(count: usize, seed: u64, lowest: i64, highest: i64) -> Vec<f64> {
        let mut state = seed;
        let span = u64::try_from(highest - lowest).unwrap();
        (0..count)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let exponent = lowest + i64::try_from((state >> 40) % span).unwrap();
                let fraction = f64::from_bits((state >> 12) | 0x3ff0_0000_0000_0000);
                let sign = if state & 1 == 0 { 1.0 } else { -1.0 };
                sign * fraction * f64::from_bits(u64::try_from(1023 + exponent).unwrap() << 52)
            })
            .collect()
    }

    fn agrees(values: &[f64]) -> usize {
        let mut decided = 0;
        for value in values {
            if let Some(fast) = sine(*value) {
                decided += 1;
                assert_eq!(
                    fast.to_bits(),
                    accurate_sin_f64(*value).to_bits(),
                    "sin({value:e})"
                );
            }
            if let Some(fast) = cosine(*value) {
                decided += 1;
                assert_eq!(
                    fast.to_bits(),
                    accurate_cos_f64(*value).to_bits(),
                    "cos({value:e})"
                );
            }
        }
        decided
    }

    #[test]
    fn the_fast_sine_and_cosine_match_the_accurate_path_bit_for_bit() {
        let values = samples(2000, 0x9e37_79b9_7f4a_7c15, -30, 20);
        assert!(agrees(&values) > 3900);
    }

    #[test]
    fn arguments_near_multiples_of_half_pi_are_answered_as_the_accurate_path_answers() {
        let mut values = Vec::new();
        for turns in [
            1_u64, 2, 3, 4, 5, 7, 11, 100, 355, 1000, 65_536, 400_000, 667_000,
        ] {
            let nearest = f64_from_small_u64(turns) * std::f64::consts::FRAC_PI_2;
            values.extend([nearest.next_down(), nearest, nearest.next_up()]);
        }
        agrees(&values);
    }

    #[test]
    fn the_table_centres_and_their_neighbours_agree() {
        let mut values = Vec::new();
        for index in 0..52_u64 {
            let center = f64_from_small_u64(index) / TABLE_STEPS;
            values.extend([center.next_down(), center, center.next_up()]);
        }
        agrees(&values);
    }

    #[test]
    #[ignore = "an evidence run: how often the fast path declines"]
    fn count_declines() {
        for (lowest, highest) in [(-30, 2), (2, 7), (7, 20)] {
            let values = samples(200_000, 0x0bad_5eed_0000_0001, lowest, highest);
            let sine_declined = values
                .iter()
                .filter(|value| sine(**value).is_none())
                .count();
            let cosine_declined = values
                .iter()
                .filter(|value| cosine(**value).is_none())
                .count();
            println!(
                "DECLINES 2^{lowest}..2^{highest}: sine {sine_declined}, cosine {cosine_declined} of 200000"
            );
        }
    }

    const CLOSEST_TO_A_MIDPOINT: [(bool, u64); 40] = [
        (true, 0x3e98_235a_0cb1_0add),
        (false, 0xc07f_d169_4a11_fbf4),
        (true, 0xbf39_da0c_e575_22df),
        (false, 0xc102_8697_fd7e_f531),
        (true, 0xbf75_ae53_af1b_54b0),
        (true, 0x403c_e56c_a1a1_0d19),
        (true, 0xbed0_32d8_6b73_a72b),
        (false, 0xc098_808f_6750_25c6),
        (false, 0x3e9f_43d6_cde0_8660),
        (false, 0xc0f7_f4a1_ee06_ae5f),
        (false, 0xbf55_c605_2f77_5caa),
        (false, 0x3e89_5674_bf33_cd3f),
        (false, 0xbe8d_a18d_762a_4e09),
        (true, 0xbe8f_a44f_993f_0f7b),
        (false, 0xc100_556c_9a86_9a62),
        (false, 0xbf92_1002_4faf_019f),
        (true, 0x3ea9_7224_5f0b_6cba),
        (false, 0xc022_1115_db58_a0c8),
        (false, 0xbfca_c105_9167_a6e1),
        (true, 0x4117_6079_eb99_5bf2),
        (true, 0x408a_627e_5307_3d06),
        (false, 0xbf61_baca_7167_c77e),
        (false, 0x40e4_2585_bdc6_91a7),
        (false, 0x3f8d_3288_9734_e518),
        (true, 0xbe64_f747_3e42_7e8e),
        (true, 0x3fb3_f540_4200_6de4),
        (true, 0x3fd0_4153_a9cd_275e),
        (true, 0x3f01_fa6c_b18a_1b5c),
        (false, 0x40b5_230c_ae9a_2199),
        (false, 0xc015_555d_4853_98ae),
        (true, 0xc04f_ce3c_979b_caaa),
        (true, 0x3f98_6d22_8c59_d8db),
        (true, 0xc037_0358_6831_0ec0),
        (false, 0x3f2b_07d1_f4cf_f11e),
        (false, 0xbe92_a42f_a31a_8ee5),
        (true, 0x3e64_f747_3c3b_3ef1),
        (true, 0x3f27_c9a8_11ec_b2e4),
        (true, 0x4014_aa2c_4c00_e63a),
        (false, 0x3ebf_9264_481b_2d26),
        (false, 0x3f77_d8f0_ed64_fd68),
    ];

    #[test]
    fn the_arguments_closest_to_a_midpoint_are_answered_as_the_previous_path_answers() {
        for (is_sine, bits) in CLOSEST_TO_A_MIDPOINT {
            let value = f64::from_bits(bits);
            if is_sine {
                assert_eq!(
                    crate::elementary::sin_f64(value).to_bits(),
                    accurate_sin_f64(value).to_bits(),
                    "sin({value:e})"
                );
            } else {
                assert_eq!(
                    crate::elementary::cos_f64(value).to_bits(),
                    accurate_cos_f64(value).to_bits(),
                    "cos({value:e})"
                );
            }
        }
    }

    fn midpoint_distance(value: DoubleWord) -> f64 {
        let magnitude = value.high.abs();
        let low = if value.high < 0.0 {
            -value.low
        } else {
            value.low
        };
        let half = if low >= 0.0 {
            (magnitude.next_up() - magnitude) / 2.0
        } else {
            (magnitude - magnitude.next_down()) / 2.0
        };
        (half - low.abs()).abs() / half
    }

    #[test]
    #[ignore = "an evidence run: a million arguments against the previous path"]
    fn sweep_a_million_arguments() {
        let values = samples(1_000_000, 0x5eed_5eed_5eed_5eed, -30, 20);
        let mut declined = 0;
        for value in &values {
            match (sine(*value), cosine(*value)) {
                (Some(sine_value), Some(cosine_value)) => {
                    assert_eq!(
                        sine_value.to_bits(),
                        accurate_sin_f64(*value).to_bits(),
                        "sin({value:e})"
                    );
                    assert_eq!(
                        cosine_value.to_bits(),
                        accurate_cos_f64(*value).to_bits(),
                        "cos({value:e})"
                    );
                }
                _ => declined += 1,
            }
        }
        println!("SWEEP declined {declined} of 1000000 pairs");
    }

    #[test]
    #[ignore = "an evidence run: the arguments whose sine or cosine lies closest to a midpoint"]
    fn search_arguments_closest_to_a_midpoint() {
        let mut closest: Vec<(f64, u64, bool)> = Vec::new();
        for chunk in 0..100_u64 {
            for value in samples(1_000_000, 0xfeed_0000 + chunk, -30, 20) {
                let Some((pair, turns)) = evaluated(value) else {
                    continue;
                };
                let sine_value = match turns.rem_euclid(4) {
                    0 | 2 => pair.sine,
                    _ => pair.cosine,
                };
                let cosine_value = match turns.rem_euclid(4) {
                    0 | 2 => pair.cosine,
                    _ => pair.sine,
                };
                for (result, is_sine) in [(sine_value, true), (cosine_value, false)] {
                    let distance = midpoint_distance(result);
                    if closest.len() < 40 || distance < closest.last().unwrap().0 {
                        closest.push((distance, value.to_bits(), is_sine));
                        closest.sort_by(|one, two| one.0.total_cmp(&two.0));
                        closest.truncate(40);
                    }
                }
            }
        }
        for (distance, bits, is_sine) in closest {
            let value = f64::from_bits(bits);
            let (fast, slow) = if is_sine {
                (sine(value), accurate_sin_f64(value))
            } else {
                (cosine(value), accurate_cos_f64(value))
            };
            if let Some(fast) = fast {
                assert_eq!(fast.to_bits(), slow.to_bits());
            }
            let name = if is_sine { "sin" } else { "cos" };
            println!(
                "HARD {name} 0x{bits:016x} distance {distance:e} fast {}",
                fast.is_some()
            );
        }
    }
}
