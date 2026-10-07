use std::arch::x86_64::{
    __m256, __m256d, _CMP_EQ_OQ, _CMP_LE_OQ, _CMP_LT_OQ, _CMP_NEQ_UQ, _CMP_UNORD_Q, _mm256_add_pd,
    _mm256_add_ps, _mm256_and_pd, _mm256_and_ps, _mm256_andnot_pd, _mm256_andnot_ps,
    _mm256_blendv_pd, _mm256_blendv_ps, _mm256_castpd_si256, _mm256_castps_si256,
    _mm256_castsi256_pd, _mm256_castsi256_ps, _mm256_cmp_pd, _mm256_cmp_ps, _mm256_div_pd,
    _mm256_div_ps, _mm256_loadu_pd, _mm256_loadu_ps, _mm256_movemask_pd, _mm256_movemask_ps,
    _mm256_mul_pd, _mm256_mul_ps, _mm256_or_pd, _mm256_or_ps, _mm256_set1_pd, _mm256_set1_ps,
    _mm256_setzero_pd, _mm256_setzero_ps, _mm256_shuffle_epi32, _mm256_sqrt_pd, _mm256_sqrt_ps,
    _mm256_srai_epi32, _mm256_storeu_pd, _mm256_storeu_ps, _mm256_sub_pd, _mm256_sub_ps,
    _mm256_xor_pd, _mm256_xor_ps,
};

use crate::lanes::Lanes;

#[derive(Clone, Copy)]
pub(crate) struct F32x8(__m256);

#[derive(Clone, Copy)]
pub(crate) struct F64x4(__m256d);

const SIGN_F32: f32 = -0.0;
const QUIET_F32: u32 = 0x0040_0000;
const SIGN_F64: f64 = -0.0;
const QUIET_F64: u64 = 0x0008_0000_0000_0000;

impl Lanes for F32x8 {
    type Element = f32;
    type Mask = F32x8;

    const WIDTH: usize = 8;

    fn splat(value: f32) -> F32x8 {
        F32x8(unsafe { _mm256_set1_ps(value) })
    }

    fn load(values: &[f32]) -> F32x8 {
        F32x8(unsafe { _mm256_loadu_ps(values.as_ptr()) })
    }

    fn store(self, values: &mut [f32]) {
        unsafe { _mm256_storeu_ps(values.as_mut_ptr(), self.0) }
    }

    fn add(self, other: F32x8) -> F32x8 {
        F32x8(unsafe { _mm256_add_ps(self.0, other.0) })
    }

    fn sub(self, other: F32x8) -> F32x8 {
        F32x8(unsafe { _mm256_sub_ps(self.0, other.0) })
    }

    fn mul(self, other: F32x8) -> F32x8 {
        F32x8(unsafe { _mm256_mul_ps(self.0, other.0) })
    }

    fn div(self, other: F32x8) -> F32x8 {
        F32x8(unsafe { _mm256_div_ps(self.0, other.0) })
    }

    fn sqrt(self) -> F32x8 {
        F32x8(unsafe { _mm256_sqrt_ps(self.0) })
    }

    fn neg(self) -> F32x8 {
        F32x8(unsafe { _mm256_xor_ps(self.0, _mm256_set1_ps(SIGN_F32)) })
    }

    fn abs(self) -> F32x8 {
        F32x8(unsafe { _mm256_andnot_ps(_mm256_set1_ps(SIGN_F32), self.0) })
    }

    fn copysign(self, sign: F32x8) -> F32x8 {
        F32x8(unsafe {
            let mask = _mm256_set1_ps(SIGN_F32);
            _mm256_or_ps(_mm256_andnot_ps(mask, self.0), _mm256_and_ps(mask, sign.0))
        })
    }

    fn less(self, other: F32x8) -> F32x8 {
        F32x8(unsafe { _mm256_cmp_ps::<_CMP_LT_OQ>(self.0, other.0) })
    }

    fn less_or_equal(self, other: F32x8) -> F32x8 {
        F32x8(unsafe { _mm256_cmp_ps::<_CMP_LE_OQ>(self.0, other.0) })
    }

    fn equal(self, other: F32x8) -> F32x8 {
        F32x8(unsafe { _mm256_cmp_ps::<_CMP_EQ_OQ>(self.0, other.0) })
    }

    fn not_equal(self, other: F32x8) -> F32x8 {
        F32x8(unsafe { _mm256_cmp_ps::<_CMP_NEQ_UQ>(self.0, other.0) })
    }

    fn unordered(self) -> F32x8 {
        F32x8(unsafe { _mm256_cmp_ps::<_CMP_UNORD_Q>(self.0, self.0) })
    }

    fn negative(self) -> F32x8 {
        F32x8(unsafe { _mm256_castsi256_ps(_mm256_srai_epi32(_mm256_castps_si256(self.0), 31)) })
    }

    fn select(mask: F32x8, when_true: F32x8, when_false: F32x8) -> F32x8 {
        F32x8(unsafe { _mm256_blendv_ps(when_false.0, when_true.0, mask.0) })
    }

    fn mask_and(left: F32x8, right: F32x8) -> F32x8 {
        F32x8(unsafe { _mm256_and_ps(left.0, right.0) })
    }

    fn mask_or(left: F32x8, right: F32x8) -> F32x8 {
        F32x8(unsafe { _mm256_or_ps(left.0, right.0) })
    }

    fn mask_not(mask: F32x8) -> F32x8 {
        F32x8(unsafe {
            let ones = _mm256_cmp_ps::<_CMP_EQ_OQ>(_mm256_setzero_ps(), _mm256_setzero_ps());
            _mm256_andnot_ps(mask.0, ones)
        })
    }

    fn mask_none(mask: F32x8) -> bool {
        unsafe { _mm256_movemask_ps(mask.0) == 0 }
    }

    fn mask_empty() -> F32x8 {
        F32x8(unsafe { _mm256_setzero_ps() })
    }

    fn quiet_nan(self) -> F32x8 {
        let mut values = [0.0_f32; 8];
        self.store(&mut values);
        for value in &mut values {
            *value = f32::from_bits(value.to_bits() | QUIET_F32);
        }
        F32x8::load(&values)
    }
}

impl Lanes for F64x4 {
    type Element = f64;
    type Mask = F64x4;

    const WIDTH: usize = 4;

    fn splat(value: f64) -> F64x4 {
        F64x4(unsafe { _mm256_set1_pd(value) })
    }

    fn load(values: &[f64]) -> F64x4 {
        F64x4(unsafe { _mm256_loadu_pd(values.as_ptr()) })
    }

    fn store(self, values: &mut [f64]) {
        unsafe { _mm256_storeu_pd(values.as_mut_ptr(), self.0) }
    }

    fn add(self, other: F64x4) -> F64x4 {
        F64x4(unsafe { _mm256_add_pd(self.0, other.0) })
    }

    fn sub(self, other: F64x4) -> F64x4 {
        F64x4(unsafe { _mm256_sub_pd(self.0, other.0) })
    }

    fn mul(self, other: F64x4) -> F64x4 {
        F64x4(unsafe { _mm256_mul_pd(self.0, other.0) })
    }

    fn div(self, other: F64x4) -> F64x4 {
        F64x4(unsafe { _mm256_div_pd(self.0, other.0) })
    }

    fn sqrt(self) -> F64x4 {
        F64x4(unsafe { _mm256_sqrt_pd(self.0) })
    }

    fn neg(self) -> F64x4 {
        F64x4(unsafe { _mm256_xor_pd(self.0, _mm256_set1_pd(SIGN_F64)) })
    }

    fn abs(self) -> F64x4 {
        F64x4(unsafe { _mm256_andnot_pd(_mm256_set1_pd(SIGN_F64), self.0) })
    }

    fn copysign(self, sign: F64x4) -> F64x4 {
        F64x4(unsafe {
            let mask = _mm256_set1_pd(SIGN_F64);
            _mm256_or_pd(_mm256_andnot_pd(mask, self.0), _mm256_and_pd(mask, sign.0))
        })
    }

    fn less(self, other: F64x4) -> F64x4 {
        F64x4(unsafe { _mm256_cmp_pd::<_CMP_LT_OQ>(self.0, other.0) })
    }

    fn less_or_equal(self, other: F64x4) -> F64x4 {
        F64x4(unsafe { _mm256_cmp_pd::<_CMP_LE_OQ>(self.0, other.0) })
    }

    fn equal(self, other: F64x4) -> F64x4 {
        F64x4(unsafe { _mm256_cmp_pd::<_CMP_EQ_OQ>(self.0, other.0) })
    }

    fn not_equal(self, other: F64x4) -> F64x4 {
        F64x4(unsafe { _mm256_cmp_pd::<_CMP_NEQ_UQ>(self.0, other.0) })
    }

    fn unordered(self) -> F64x4 {
        F64x4(unsafe { _mm256_cmp_pd::<_CMP_UNORD_Q>(self.0, self.0) })
    }

    fn negative(self) -> F64x4 {
        F64x4(unsafe {
            let bits = _mm256_castpd_si256(self.0);
            let high = _mm256_shuffle_epi32::<0b11_11_01_01>(bits);
            _mm256_castsi256_pd(_mm256_srai_epi32(high, 31))
        })
    }

    fn select(mask: F64x4, when_true: F64x4, when_false: F64x4) -> F64x4 {
        F64x4(unsafe { _mm256_blendv_pd(when_false.0, when_true.0, mask.0) })
    }

    fn mask_and(left: F64x4, right: F64x4) -> F64x4 {
        F64x4(unsafe { _mm256_and_pd(left.0, right.0) })
    }

    fn mask_or(left: F64x4, right: F64x4) -> F64x4 {
        F64x4(unsafe { _mm256_or_pd(left.0, right.0) })
    }

    fn mask_not(mask: F64x4) -> F64x4 {
        F64x4(unsafe {
            let ones = _mm256_cmp_pd::<_CMP_EQ_OQ>(_mm256_setzero_pd(), _mm256_setzero_pd());
            _mm256_andnot_pd(mask.0, ones)
        })
    }

    fn mask_none(mask: F64x4) -> bool {
        unsafe { _mm256_movemask_pd(mask.0) == 0 }
    }

    fn mask_empty() -> F64x4 {
        F64x4(unsafe { _mm256_setzero_pd() })
    }

    fn quiet_nan(self) -> F64x4 {
        let mut values = [0.0_f64; 4];
        self.store(&mut values);
        for value in &mut values {
            *value = f64::from_bits(value.to_bits() | QUIET_F64);
        }
        F64x4::load(&values)
    }
}
