use crate::binary64_words::{
    DoubleWord, decided_within, divided, fast_two_sum, plus, plus_double, square, times,
    two_product, two_sum,
};
use crate::word_conversion::{f64_from_small_u64, i64_from_small_integral_f64};

const TABLE_STEPS: f64 = 64.0;
const ROUNDING_SHIFT: f64 = 4_503_599_627_370_496.0;
const SMALLEST_MAGNITUDE: f64 = f64::from_bits(723 << 52);
const LARGEST_MAGNITUDE: f64 = f64::from_bits(1323 << 52);
const ERROR_EXPONENT: i64 = 74;
const MINUS_ONE_THIRD: DoubleWord = DoubleWord {
    high: f64::from_bits(0xbfd5_5555_5555_5555),
    low: f64::from_bits(0xbc75_5555_5555_5555),
};
const ONE_FIFTH: f64 = 0.2;
const MINUS_ONE_SEVENTH: f64 = -1.0 / 7.0;
const ONE_NINTH: f64 = 1.0 / 9.0;
const MINUS_ONE_ELEVENTH: f64 = -1.0 / 11.0;

const ARCTANGENT_TABLE: [(u64, u64); 64] = [
    (0x3f8f_ff55_5bbb_729b, 0xbc22_20c3_9d4d_ff50),
    (0x3f9f_fd55_bba9_7625, 0xbc35_ec43_1444_912c),
    (0x3fa7_fb81_8430_da2a, 0xbc08_6ef8_f794_f105),
    (0x3faf_f55b_b72c_fdea, 0xbc3c_934d_86d2_3f1d),
    (0x3fb3_f59f_0e7c_559d, 0x3c5a_c4ce_285d_f847),
    (0x3fb7_ee18_2602_f10f, 0xbc5c_fb65_4c0c_3d98),
    (0x3fbb_e39e_be6f_07c3, 0x3c5f_7b8f_29a0_5987),
    (0x3fbf_d5ba_9aac_2f6e, 0xbc4c_d376_8676_0c17),
    (0x3fc1_e1fa_fb04_3727, 0xbc4b_4859_14da_cf8c),
    (0x3fc3_d6ee_e8c6_626c, 0x3c66_1a3b_0ce9_281b),
    (0x3fc5_c981_1e3e_c26a, 0xbc50_54ab_2c01_0f3d),
    (0x3fc7_b97b_4bce_5b02, 0x3c53_47b0_b4f8_81ca),
    (0x3fc9_a6a8_e96c_8626, 0x3c4c_f601_e7b4_348e),
    (0x3fcb_90d7_5292_60a2, 0x3c21_7b10_d2e0_e5ab),
    (0x3fcd_77d5_df20_5736, 0x3c6c_648d_1534_597e),
    (0x3fcf_5b75_f92c_80dd, 0x3c68_ab6e_3cf7_afbd),
    (0x3fd0_9dc5_97d8_6362, 0x3c76_2e47_390c_b865),
    (0x3fd1_8bf5_a30b_f178, 0x3c63_0ca4_748b_1bf9),
    (0x3fd2_7837_2057_ef46, 0xbc70_77cd_d36d_fc81),
    (0x3fd3_6277_3707_ebcc, 0xbc69_63a5_44b6_72d8),
    (0x3fd4_4aa4_36c2_af0a, 0xbc75_d5e4_3c55_b3ba),
    (0x3fd5_30ad_9951_cd4a, 0xbc62_5664_8088_4082),
    (0x3fd6_1484_0309_cfe2, 0xbc7a_7257_1571_1f00),
    (0x3fd6_f619_41e4_def1, 0xbc7c_63aa_e6f6_e918),
    (0x3fd7_d560_4b63_b3f7, 0x3c76_9c88_5c2b_249a),
    (0x3fd8_b24d_394a_1b25, 0x3c7b_6d0b_a374_8fa8),
    (0x3fd9_8cd5_454d_6b18, 0x3c79_e6c9_88fd_0a77),
    (0x3fda_64ee_c3cc_23fd, 0xbc72_4dec_1b50_b7ff),
    (0x3fdb_3a91_1da6_5c6c, 0x3c7a_e187_b1ca_5040),
    (0x3fdc_0db4_c94e_c9f0, 0xbc7c_c1ce_7093_4c34),
    (0x3fdc_de53_432c_1351, 0xbc7a_2cfa_4418_f1ad),
    (0x3fdd_ac67_0561_bb4f, 0x3c7a_2b7f_222f_65e2),
    (0x3fde_77eb_7f17_5a34, 0x3c70_e53d_c1bf_3435),
    (0x3fdf_40dd_0b54_1418, 0xbc6a_3992_dc38_2a23),
    (0x3fe0_039c_73c1_a40c, 0xbc8b_32c9_49c9_d593),
    (0x3fe0_657e_94db_30d0, 0xbc7d_5b49_5f63_49e6),
    (0x3fe0_c614_5b5b_43da, 0x3c59_74fa_13b5_404f),
    (0x3fe1_255d_9bfb_d2a9, 0xbc52_bdae_e1c0_ee35),
    (0x3fe1_835a_88be_7c13, 0x3c8c_621c_ec00_c301),
    (0x3fe1_e00b_abde_feb4, 0xbc59_28df_287a_668f),
    (0x3fe2_3b71_e2cc_9e6a, 0x3c6c_421c_9f38_224e),
    (0x3fe2_958e_5930_8e31, 0xbc70_9e73_b0c6_c087),
    (0x3fe2_ee62_8406_cbca, 0x3c8c_5d5e_9ff0_cf8d),
    (0x3fe3_45f0_1cce_37bb, 0x3c81_0211_37c7_1102),
    (0x3fe3_9c39_1cd4_171a, 0xbc82_3043_31d8_bf46),
    (0x3fe3_f13f_b89e_96f4, 0x3c7e_cf8b_4926_44f0),
    (0x3fe4_4506_5b79_5b56, 0xbc7f_76d0_163f_79c8),
    (0x3fe4_978f_a326_9ee1, 0x3c72_419a_87f2_a458),
    (0x3fe4_e8de_5bb6_ec04, 0x3c84_a33d_beb3_796c),
    (0x3fe5_38f5_7b89_061f, 0xbc81_bb74_abda_520c),
    (0x3fe5_87d8_1f73_2fbb, 0xbc75_e5c9_d8c5_a950),
    (0x3fe5_d589_8716_9b18, 0x3c60_028e_4bc5_e7ca),
    (0x3fe6_220d_115d_7b8e, 0xbc62_b785_350e_e8c1),
    (0x3fe6_6d66_3923_e087, 0xbc76_ea6f_ebe8_bbba),
    (0x3fe6_b798_920b_3d99, 0xbc8a_8038_6188_c50e),
    (0x3fe7_00a7_c578_4634, 0xbc78_c34d_25aa_def6),
    (0x3fe7_4897_8fba_8e0f, 0x3c47_b2a6_1658_84a1),
    (0x3fe7_8f6b_bd5d_315e, 0x3c84_06a0_8980_3740),
    (0x3fe7_d528_289f_a093, 0x3c85_6082_1e2f_3aa9),
    (0x3fe8_19d0_b715_8a4d, 0xbc7b_f762_29d3_b917),
    (0x3fe8_5d69_576c_c2c5, 0x3c66_b66e_7fc8_b8c3),
    (0x3fe8_9ff5_ff57_f1f8, 0xbc85_5b9a_5e17_7a1b),
    (0x3fe8_e17a_a99c_c05e, 0xbc7e_c182_ab04_2f61),
    (0x3fe9_21fb_5444_2d18, 0x3c81_a626_3314_5c07),
];
const HALF_PI_WORDS: (u64, u64) = (0x3ff9_21fb_5444_2d18, 0x3c91_a626_3314_5c07);
const PI_WORDS: (u64, u64) = (0x4009_21fb_5444_2d18, 0x3ca1_a626_3314_5c07);

fn nearest_index(value: f64) -> Option<usize> {
    let rounded = (value * TABLE_STEPS + ROUNDING_SHIFT) - ROUNDING_SHIFT;
    usize::try_from(i64_from_small_integral_f64(rounded)).ok()
}

fn arctangent_of_small(value: DoubleWord) -> DoubleWord {
    let squared = square(value);
    let small = squared.high;
    let tail = small
        * (ONE_FIFTH
            + small * (MINUS_ONE_SEVENTH + small * (ONE_NINTH + small * MINUS_ONE_ELEVENTH)));
    let series = plus_double(MINUS_ONE_THIRD, tail);
    let cube = times(value, squared);
    plus(value, times(cube, series))
}

fn arctangent_up_to_one(value: DoubleWord) -> Option<DoubleWord> {
    let index = nearest_index(value.high)?;
    if index == 0 {
        return Some(arctangent_of_small(value));
    }
    let (high_bits, low_bits) = *ARCTANGENT_TABLE.get(index - 1)?;
    let base = DoubleWord {
        high: f64::from_bits(high_bits),
        low: f64::from_bits(low_bits),
    };
    let center = f64_from_small_u64(u64::try_from(index).ok()?) / TABLE_STEPS;
    let difference = two_sum(value.high - center, value.low);
    let product = two_product(center, value.high);
    let product = fast_two_sum(product.high, product.low + center * value.low);
    let denominator = plus_double(product, 1.0);
    let reduced = divided(difference, denominator);
    Some(plus(base, arctangent_of_small(reduced)))
}

fn half_pi() -> DoubleWord {
    DoubleWord {
        high: f64::from_bits(HALF_PI_WORDS.0),
        low: f64::from_bits(HALF_PI_WORDS.1),
    }
}

fn pi() -> DoubleWord {
    DoubleWord {
        high: f64::from_bits(PI_WORDS.0),
        low: f64::from_bits(PI_WORDS.1),
    }
}

fn in_range(magnitude: f64) -> bool {
    (SMALLEST_MAGNITUDE..=LARGEST_MAGNITUDE).contains(&magnitude)
}

fn arctangent_of_magnitude(magnitude: DoubleWord) -> Option<DoubleWord> {
    if magnitude.high <= 1.0 {
        arctangent_up_to_one(magnitude)
    } else {
        let reciprocal = divided(DoubleWord::exact(1.0), magnitude);
        Some(plus(half_pi(), arctangent_up_to_one(reciprocal)?.negated()))
    }
}

pub(crate) fn arctangent(value: f64) -> Option<f64> {
    let magnitude = value.abs();
    if !in_range(magnitude) {
        return None;
    }
    let result = arctangent_of_magnitude(DoubleWord::exact(magnitude))?;
    decided_within(
        if value < 0.0 {
            result.negated()
        } else {
            result
        },
        ERROR_EXPONENT,
    )
}

pub(crate) fn arctangent_of_quotient(numerator: f64, denominator: f64) -> Option<f64> {
    if !in_range(numerator.abs()) || !in_range(denominator.abs()) {
        return None;
    }
    let quotient = divided(
        DoubleWord::exact(numerator.abs()),
        DoubleWord::exact(denominator.abs()),
    );
    if !in_range(quotient.high) {
        return None;
    }
    let angle = if denominator > 0.0 {
        arctangent_of_magnitude(quotient)?
    } else if quotient.high <= 1.0 {
        plus(pi(), arctangent_up_to_one(quotient)?.negated())
    } else {
        let reciprocal = divided(DoubleWord::exact(1.0), quotient);
        plus(half_pi(), arctangent_up_to_one(reciprocal)?)
    };
    decided_within(
        if numerator < 0.0 {
            angle.negated()
        } else {
            angle
        },
        ERROR_EXPONENT,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ball::Ball;
    use crate::integer::Integer;
    use crate::number::Number;

    const PRECISION: usize = 400;

    fn words(ball: &Ball) -> (u64, u64) {
        let high = ball.rounded_f64().unwrap();
        let rest = ball.sub(&Ball::from_f64(high, PRECISION).unwrap());
        (high.to_bits(), rest.rounded_f64().unwrap().to_bits())
    }

    #[test]
    fn every_table_entry_is_the_double_word_nearest_its_arctangent() {
        for (offset, entry) in ARCTANGENT_TABLE.iter().enumerate() {
            let index = i64::try_from(offset + 1).unwrap();
            let center = Number::fraction(&Integer::from(index), &Integer::from(64_i64)).unwrap();
            let value =
                crate::series::arctangent(&Ball::from_exact(&center, PRECISION).unwrap()).unwrap();
            assert_eq!(words(&value), *entry, "atan({index}/64)");
        }
    }

    #[test]
    fn the_pi_constants_are_the_double_words_nearest_pi_and_half_pi() {
        assert_eq!(words(&crate::series::pi(PRECISION)), PI_WORDS);
        assert_eq!(
            words(&crate::series::pi(PRECISION).mul_power_of_two(-1)),
            HALF_PI_WORDS
        );
    }

    #[test]
    fn the_minus_one_third_constant_is_its_nearest_double_word() {
        let third = Number::fraction(&Integer::from(-1_i64), &Integer::from(3_i64)).unwrap();
        let ball = Ball::from_exact(&third, PRECISION).unwrap();
        assert_eq!(
            words(&ball),
            (
                MINUS_ONE_THIRD.high.to_bits(),
                MINUS_ONE_THIRD.low.to_bits()
            )
        );
    }

    fn slow_arctangent(value: f64) -> f64 {
        crate::elementary::accurate_atan_f64(value)
    }

    fn slow_arctangent_of_quotient(numerator: f64, denominator: f64) -> f64 {
        crate::elementary::accurate_atan2_f64(numerator, denominator)
    }

    fn samples(count: usize, seed: u64) -> Vec<f64> {
        let mut state = seed;
        (0..count)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let exponent = i64::try_from(state >> 58).unwrap() - 32;
                let fraction = f64::from_bits((state >> 12) | 0x3ff0_0000_0000_0000) - 1.0;
                let sign = if state & 1 == 0 { 1.0 } else { -1.0 };
                sign * (1.0 + fraction)
                    * f64::from_bits(u64::try_from(1023 + exponent).unwrap() << 52)
            })
            .collect()
    }

    #[test]
    fn the_fast_arctangent_matches_the_accurate_path_bit_for_bit() {
        let mut decided_count = 0;
        for value in samples(3000, 0x9e37_79b9_7f4a_7c15) {
            if let Some(fast) = arctangent(value) {
                decided_count += 1;
                assert_eq!(
                    fast.to_bits(),
                    slow_arctangent(value).to_bits(),
                    "atan({value:e})"
                );
            }
        }
        assert!(decided_count > 2900);
    }

    #[test]
    fn the_fast_arctangent_of_a_quotient_matches_the_accurate_path_bit_for_bit() {
        let numerators = samples(1500, 0x1234_5678_9abc_def1);
        let denominators = samples(1500, 0x0fed_cba9_8765_4321);
        let mut decided_count = 0;
        for (numerator, denominator) in numerators.iter().zip(&denominators) {
            if let Some(fast) = arctangent_of_quotient(*numerator, *denominator) {
                decided_count += 1;
                assert_eq!(
                    fast.to_bits(),
                    slow_arctangent_of_quotient(*numerator, *denominator).to_bits(),
                    "atan2({numerator:e}, {denominator:e})"
                );
            }
        }
        assert!(decided_count > 1400);
    }

    #[test]
    fn the_table_centres_and_one_are_decided_exactly() {
        for index in 0..=64_u64 {
            let value = f64_from_small_u64(index) / TABLE_STEPS;
            if let Some(fast) = arctangent(value) {
                assert_eq!(
                    fast.to_bits(),
                    slow_arctangent(value).to_bits(),
                    "atan({value})"
                );
            }
        }
    }

    #[test]
    fn a_quotient_by_one_is_the_arctangent_of_the_numerator() {
        for value in [0.3, -0.25, 4.9, -1.0e-5] {
            assert_eq!(arctangent_of_quotient(value, 1.0), arctangent(value));
        }
    }

    const CLOSEST_TO_A_MIDPOINT: [u64; 40] = [
        0x412c_febb_8322_3c7f,
        0x408a_5dbc_51ed_474e,
        0x3e92_8710_6ff0_7e52,
        0x3e92_8710_6ff7_edc5,
        0x3f55_b922_ce91_86ea,
        0x3ff8_1e7b_79b3_6e2c,
        0x3ec3_4e5c_86fd_422e,
        0x3ec3_612d_777d_eb47,
        0x408a_5d96_8233_a6d9,
        0x3fe7_e276_ce45_a8ae,
        0x412c_d3fc_1676_da5e,
        0x41ae_c310_9ac1_13a4,
        0x3f76_0f32_c17d_f17b,
        0x411c_9e6a_8798_67cd,
        0x3f35_09ce_d044_153c,
        0x40aa_e8e7_9e9d_5a0a,
        0x3fb7_38b4_8678_f30e,
        0x3f86_4572_d963_222f,
        0x417e_162a_5aa5_9784,
        0x3ed3_9ace_75ca_85f1,
        0x40eb_e05c_27cf_64d3,
        0x416d_f66e_82c8_4a76,
        0x3f45_7b04_245e_354e,
        0x40fc_0df3_3499_a30a,
        0x414d_7665_6d4f_79b9,
        0x412c_cf75_8962_a55b,
        0x3f35_3684_e7e6_5d64,
        0x3f35_1295_40f8_64ee,
        0x3ee3_d86e_f063_e5d7,
        0x413d_0bff_ac1c_313c,
        0x3e82_690b_44d8_e080,
        0x40db_b640_3644_50f3,
        0x3ff8_02ab_fa7a_68a4,
        0x4069_ef1c_9fe8_4456,
        0x3fb7_34c7_18eb_556a,
        0x40db_9b1d_102b_b147,
        0x4008_7f9d_8a18_0e9e,
        0x417e_1cb6_f286_9a3b,
        0x3ff8_2265_02b7_63bf,
        0x415d_88e8_337b_014a,
    ];

    const GLIBC_QUOTIENTS: [(f64, f64); 12] = [
        (0.75, 1.0),
        (-0.75, 1.0),
        (0.75, -1.0),
        (-0.75, -1.0),
        (0.390_625, 0.000_29),
        (1.390_625, 0.929_687_5),
        (
            -f64::from_bits(0x3f7e_ffe8_1f85_2717),
            -f64::from_bits(0x3f5d_5f47_7839_4492),
        ),
        (
            f64::from_bits(0x3c9b_cab2_9da0_e947),
            f64::from_bits(0x3c9b_c41f_4d22_94b8),
        ),
        (
            f64::from_bits(0x2a3a_1189_1ec0_04d4),
            f64::from_bits(0x2a38_1483_0510_be26),
        ),
        (2.5, 1.0),
        (10.0, 1.0),
        (1.0e6, 1.0),
    ];

    #[test]
    fn the_arguments_closest_to_a_midpoint_are_answered_as_the_accurate_path_answers() {
        for bits in CLOSEST_TO_A_MIDPOINT {
            let value = f64::from_bits(bits);
            for signed in [value, -value] {
                assert_eq!(
                    crate::elementary::atan_f64(signed).to_bits(),
                    slow_arctangent(signed).to_bits(),
                    "atan({signed:e})"
                );
            }
        }
    }

    #[test]
    fn glibc_published_quotients_are_answered_as_the_accurate_path_answers() {
        for (numerator, denominator) in GLIBC_QUOTIENTS {
            assert_eq!(
                crate::elementary::atan2_f64(numerator, denominator).to_bits(),
                slow_arctangent_of_quotient(numerator, denominator).to_bits(),
                "atan2({numerator:e}, {denominator:e})"
            );
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
    #[ignore = "an evidence run: a million arguments against the accurate path"]
    fn sweep_a_million_arguments() {
        let mut declined = 0;
        for value in samples(1_000_000, 0x005e_ed0f_a7a7_a7a7) {
            match arctangent(value) {
                Some(fast) => {
                    assert_eq!(
                        fast.to_bits(),
                        slow_arctangent(value).to_bits(),
                        "atan({value:e})"
                    )
                }
                None => declined += 1,
            }
        }
        println!("SWEEP declined {declined} of 1000000");
    }

    #[test]
    #[ignore = "an evidence run: the arguments whose arctangent lies closest to a midpoint"]
    fn search_arguments_closest_to_a_midpoint() {
        let mut closest: Vec<(f64, u64)> = Vec::new();
        for chunk in 0..100_u64 {
            for value in samples(1_000_000, 0xabcd_0000 + chunk) {
                let magnitude = value.abs();
                if !in_range(magnitude) {
                    continue;
                }
                let Some(result) = arctangent_of_magnitude(DoubleWord::exact(magnitude)) else {
                    continue;
                };
                let distance = midpoint_distance(result);
                if closest.len() < 40 || distance < closest.last().unwrap().0 {
                    closest.push((distance, magnitude.to_bits()));
                    closest.sort_by(|one, two| one.0.total_cmp(&two.0));
                    closest.truncate(40);
                }
            }
        }
        for (distance, bits) in closest {
            let value = f64::from_bits(bits);
            let fast = arctangent(value);
            let slow = slow_arctangent(value);
            if let Some(fast) = fast {
                assert_eq!(fast.to_bits(), slow.to_bits());
            }
            println!(
                "HARD 0x{bits:016x} distance {distance:e} fast {}",
                fast.is_some()
            );
        }
    }
}
