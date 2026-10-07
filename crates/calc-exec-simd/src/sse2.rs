use std::arch::x86_64::{
    __m128, __m128d, _mm_add_pd, _mm_add_ps, _mm_and_pd, _mm_and_ps, _mm_andnot_pd, _mm_andnot_ps,
    _mm_castpd_si128, _mm_castps_si128, _mm_castsi128_pd, _mm_castsi128_ps, _mm_cmpeq_pd,
    _mm_cmpeq_ps, _mm_cmple_pd, _mm_cmple_ps, _mm_cmplt_pd, _mm_cmplt_ps, _mm_cmpneq_pd,
    _mm_cmpneq_ps, _mm_cmpunord_pd, _mm_cmpunord_ps, _mm_div_pd, _mm_div_ps, _mm_loadu_pd,
    _mm_loadu_ps, _mm_movemask_pd, _mm_movemask_ps, _mm_mul_pd, _mm_mul_ps, _mm_or_pd, _mm_or_ps,
    _mm_set1_pd, _mm_set1_ps, _mm_setzero_pd, _mm_setzero_ps, _mm_shuffle_epi32, _mm_sqrt_pd,
    _mm_sqrt_ps, _mm_srai_epi32, _mm_storeu_pd, _mm_storeu_ps, _mm_sub_pd, _mm_sub_ps, _mm_xor_pd,
    _mm_xor_ps,
};

use crate::lanes::Lanes;

#[derive(Clone, Copy)]
pub(crate) struct F32x4(__m128);

#[derive(Clone, Copy)]
pub(crate) struct F64x2(__m128d);

const SIGN_F32: f32 = -0.0;
const QUIET_F32: u32 = 0x0040_0000;
const SIGN_F64: f64 = -0.0;
const QUIET_F64: u64 = 0x0008_0000_0000_0000;

impl Lanes for F32x4 {
    type Element = f32;
    type Mask = F32x4;

    const WIDTH: usize = 4;

    fn splat(value: f32) -> F32x4 {
        F32x4(unsafe { _mm_set1_ps(value) })
    }

    fn load(values: &[f32]) -> F32x4 {
        F32x4(unsafe { _mm_loadu_ps(values.as_ptr()) })
    }

    fn store(self, values: &mut [f32]) {
        unsafe { _mm_storeu_ps(values.as_mut_ptr(), self.0) }
    }

    fn add(self, other: F32x4) -> F32x4 {
        F32x4(unsafe { _mm_add_ps(self.0, other.0) })
    }

    fn sub(self, other: F32x4) -> F32x4 {
        F32x4(unsafe { _mm_sub_ps(self.0, other.0) })
    }

    fn mul(self, other: F32x4) -> F32x4 {
        F32x4(unsafe { _mm_mul_ps(self.0, other.0) })
    }

    fn div(self, other: F32x4) -> F32x4 {
        F32x4(unsafe { _mm_div_ps(self.0, other.0) })
    }

    fn sqrt(self) -> F32x4 {
        F32x4(unsafe { _mm_sqrt_ps(self.0) })
    }

    fn neg(self) -> F32x4 {
        F32x4(unsafe { _mm_xor_ps(self.0, _mm_set1_ps(SIGN_F32)) })
    }

    fn abs(self) -> F32x4 {
        F32x4(unsafe { _mm_andnot_ps(_mm_set1_ps(SIGN_F32), self.0) })
    }

    fn copysign(self, sign: F32x4) -> F32x4 {
        F32x4(unsafe {
            let mask = _mm_set1_ps(SIGN_F32);
            _mm_or_ps(_mm_andnot_ps(mask, self.0), _mm_and_ps(mask, sign.0))
        })
    }

    fn less(self, other: F32x4) -> F32x4 {
        F32x4(unsafe { _mm_cmplt_ps(self.0, other.0) })
    }

    fn less_or_equal(self, other: F32x4) -> F32x4 {
        F32x4(unsafe { _mm_cmple_ps(self.0, other.0) })
    }

    fn equal(self, other: F32x4) -> F32x4 {
        F32x4(unsafe { _mm_cmpeq_ps(self.0, other.0) })
    }

    fn not_equal(self, other: F32x4) -> F32x4 {
        F32x4(unsafe { _mm_cmpneq_ps(self.0, other.0) })
    }

    fn unordered(self) -> F32x4 {
        F32x4(unsafe { _mm_cmpunord_ps(self.0, self.0) })
    }

    fn negative(self) -> F32x4 {
        F32x4(unsafe { _mm_castsi128_ps(_mm_srai_epi32(_mm_castps_si128(self.0), 31)) })
    }

    fn select(mask: F32x4, when_true: F32x4, when_false: F32x4) -> F32x4 {
        F32x4(unsafe {
            _mm_or_ps(
                _mm_and_ps(mask.0, when_true.0),
                _mm_andnot_ps(mask.0, when_false.0),
            )
        })
    }

    fn mask_and(left: F32x4, right: F32x4) -> F32x4 {
        F32x4(unsafe { _mm_and_ps(left.0, right.0) })
    }

    fn mask_or(left: F32x4, right: F32x4) -> F32x4 {
        F32x4(unsafe { _mm_or_ps(left.0, right.0) })
    }

    fn mask_not(mask: F32x4) -> F32x4 {
        F32x4(unsafe { _mm_andnot_ps(mask.0, _mm_cmpeq_ps(_mm_setzero_ps(), _mm_setzero_ps())) })
    }

    fn mask_none(mask: F32x4) -> bool {
        unsafe { _mm_movemask_ps(mask.0) == 0 }
    }

    fn mask_empty() -> F32x4 {
        F32x4(unsafe { _mm_setzero_ps() })
    }

    fn quiet_nan(self) -> F32x4 {
        let mut values = [0.0_f32; 4];
        self.store(&mut values);
        for value in &mut values {
            *value = f32::from_bits(value.to_bits() | QUIET_F32);
        }
        F32x4::load(&values)
    }
}

impl Lanes for F64x2 {
    type Element = f64;
    type Mask = F64x2;

    const WIDTH: usize = 2;

    fn splat(value: f64) -> F64x2 {
        F64x2(unsafe { _mm_set1_pd(value) })
    }

    fn load(values: &[f64]) -> F64x2 {
        F64x2(unsafe { _mm_loadu_pd(values.as_ptr()) })
    }

    fn store(self, values: &mut [f64]) {
        unsafe { _mm_storeu_pd(values.as_mut_ptr(), self.0) }
    }

    fn add(self, other: F64x2) -> F64x2 {
        F64x2(unsafe { _mm_add_pd(self.0, other.0) })
    }

    fn sub(self, other: F64x2) -> F64x2 {
        F64x2(unsafe { _mm_sub_pd(self.0, other.0) })
    }

    fn mul(self, other: F64x2) -> F64x2 {
        F64x2(unsafe { _mm_mul_pd(self.0, other.0) })
    }

    fn div(self, other: F64x2) -> F64x2 {
        F64x2(unsafe { _mm_div_pd(self.0, other.0) })
    }

    fn sqrt(self) -> F64x2 {
        F64x2(unsafe { _mm_sqrt_pd(self.0) })
    }

    fn neg(self) -> F64x2 {
        F64x2(unsafe { _mm_xor_pd(self.0, _mm_set1_pd(SIGN_F64)) })
    }

    fn abs(self) -> F64x2 {
        F64x2(unsafe { _mm_andnot_pd(_mm_set1_pd(SIGN_F64), self.0) })
    }

    fn copysign(self, sign: F64x2) -> F64x2 {
        F64x2(unsafe {
            let mask = _mm_set1_pd(SIGN_F64);
            _mm_or_pd(_mm_andnot_pd(mask, self.0), _mm_and_pd(mask, sign.0))
        })
    }

    fn less(self, other: F64x2) -> F64x2 {
        F64x2(unsafe { _mm_cmplt_pd(self.0, other.0) })
    }

    fn less_or_equal(self, other: F64x2) -> F64x2 {
        F64x2(unsafe { _mm_cmple_pd(self.0, other.0) })
    }

    fn equal(self, other: F64x2) -> F64x2 {
        F64x2(unsafe { _mm_cmpeq_pd(self.0, other.0) })
    }

    fn not_equal(self, other: F64x2) -> F64x2 {
        F64x2(unsafe { _mm_cmpneq_pd(self.0, other.0) })
    }

    fn unordered(self) -> F64x2 {
        F64x2(unsafe { _mm_cmpunord_pd(self.0, self.0) })
    }

    fn negative(self) -> F64x2 {
        F64x2(unsafe {
            let bits = _mm_castpd_si128(self.0);
            let high = _mm_shuffle_epi32::<0b11_11_01_01>(bits);
            _mm_castsi128_pd(_mm_srai_epi32(high, 31))
        })
    }

    fn select(mask: F64x2, when_true: F64x2, when_false: F64x2) -> F64x2 {
        F64x2(unsafe {
            _mm_or_pd(
                _mm_and_pd(mask.0, when_true.0),
                _mm_andnot_pd(mask.0, when_false.0),
            )
        })
    }

    fn mask_and(left: F64x2, right: F64x2) -> F64x2 {
        F64x2(unsafe { _mm_and_pd(left.0, right.0) })
    }

    fn mask_or(left: F64x2, right: F64x2) -> F64x2 {
        F64x2(unsafe { _mm_or_pd(left.0, right.0) })
    }

    fn mask_not(mask: F64x2) -> F64x2 {
        F64x2(unsafe { _mm_andnot_pd(mask.0, _mm_cmpeq_pd(_mm_setzero_pd(), _mm_setzero_pd())) })
    }

    fn mask_none(mask: F64x2) -> bool {
        unsafe { _mm_movemask_pd(mask.0) == 0 }
    }

    fn mask_empty() -> F64x2 {
        F64x2(unsafe { _mm_setzero_pd() })
    }

    fn quiet_nan(self) -> F64x2 {
        let mut values = [0.0_f64; 2];
        self.store(&mut values);
        for value in &mut values {
            *value = f64::from_bits(value.to_bits() | QUIET_F64);
        }
        F64x2::load(&values)
    }
}
