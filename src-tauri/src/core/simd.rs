#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;
#[cfg(target_arch = "aarch64")]
use std::arch::is_aarch64_feature_detected;
#[cfg(target_arch = "x86_64")]
use std::arch::is_x86_feature_detected;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use itertools::Itertools;
use ndarray::{ArrayBase, ArrayRef, DataMut, Dimension};
use ndarray_stats::QuantileExt;

pub fn find_min_max(slice: &[f32]) -> (f32, f32) {
    // Use SIMD if available, otherwise fall back to scalar
    #[cfg(target_arch = "aarch64")]
    if is_aarch64_feature_detected!("neon") {
        find_min_max_neon(slice)
    } else {
        find_min_max_scalar(slice)
    }

    #[cfg(target_arch = "x86_64")]
    if is_x86_feature_detected!("avx2") {
        find_min_max_avx2(slice)
    } else if is_x86_feature_detected!("sse4.1") {
        find_min_max_sse4(slice)
    } else {
        find_min_max_scalar(slice)
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    find_min_max_scalar(slice)
}

pub fn find_min(slice: &[f32]) -> f32 {
    // Use SIMD if available, otherwise fall back to scalar
    #[cfg(target_arch = "aarch64")]
    if is_aarch64_feature_detected!("neon") {
        find_min_neon(slice)
    } else {
        find_min_scalar(slice)
    }

    #[cfg(target_arch = "x86_64")]
    if is_x86_feature_detected!("avx2") {
        find_min_avx2(slice)
    } else if is_x86_feature_detected!("sse4.1") {
        find_min_sse4(slice)
    } else {
        find_min_scalar(slice)
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    find_min_scalar(slice)
}

pub trait MinSIMD<A, D>
where
    D: Dimension,
{
    fn min_simd(&self) -> A;
}

impl<D> MinSIMD<f32, D> for ArrayRef<f32, D>
where
    D: Dimension,
{
    fn min_simd(&self) -> f32 {
        find_min(self.as_slice_memory_order().unwrap_or_default())
    }
}

impl<D> MinSIMD<f64, D> for ArrayRef<f64, D>
where
    D: Dimension,
{
    fn min_simd(&self) -> f64 {
        log::warn!("min_simd for f64 is not implemented");
        *self.min_skipnan()
    }
}

#[allow(unused)]
pub fn find_max(slice: &[f32]) -> f32 {
    // Use SIMD if available, otherwise fall back to scalar
    #[cfg(target_arch = "aarch64")]
    if is_aarch64_feature_detected!("neon") {
        find_max_neon(slice)
    } else {
        find_max_scalar(slice)
    }

    #[cfg(target_arch = "x86_64")]
    if is_x86_feature_detected!("avx2") {
        find_max_avx2(slice)
    } else if is_x86_feature_detected!("sse4.1") {
        find_max_sse4(slice)
    } else {
        find_max_scalar(slice)
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    find_max_scalar(slice)
}

pub fn sum_squares(slice: &[f32]) -> f32 {
    // Use SIMD if available, otherwise fall back to scalar
    #[cfg(target_arch = "aarch64")]
    if is_aarch64_feature_detected!("neon") {
        sum_squares_neon(slice)
    } else {
        sum_squares_scalar(slice)
    }

    #[cfg(target_arch = "x86_64")]
    if is_x86_feature_detected!("avx2") {
        sum_squares_avx2(slice)
    } else if is_x86_feature_detected!("sse4.1") {
        sum_squares_sse4(slice)
    } else {
        sum_squares_scalar(slice)
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    sum_squares_scalar(slice)
}

pub fn abs_max(slice: &[f32]) -> f32 {
    // Use SIMD if available, otherwise fall back to scalar
    #[cfg(target_arch = "aarch64")]
    if is_aarch64_feature_detected!("neon") {
        abs_max_neon(slice)
    } else {
        abs_max_scalar(slice)
    }

    #[cfg(target_arch = "x86_64")]
    if is_x86_feature_detected!("avx2") {
        abs_max_avx2(slice)
    } else if is_x86_feature_detected!("sse4.1") {
        abs_max_sse4(slice)
    } else {
        abs_max_scalar(slice)
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    abs_max_scalar(slice)
}

pub fn scalar_mul(slice: &mut [f32], scalar: f32) {
    // Use SIMD if available, otherwise fall back to scalar
    #[cfg(target_arch = "aarch64")]
    if is_aarch64_feature_detected!("neon") {
        scalar_mul_neon(slice, scalar)
    } else {
        scalar_mul_scalar(slice, scalar)
    }

    #[cfg(target_arch = "x86_64")]
    if is_x86_feature_detected!("avx2") {
        scalar_mul_avx2(slice, scalar)
    } else if is_x86_feature_detected!("sse4.1") {
        scalar_mul_sse4(slice, scalar)
    } else {
        scalar_mul_scalar(slice, scalar)
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    scalar_mul_scalar(slice, scalar)
}

pub trait ScalarMulSIMDInplace<A> {
    fn scalar_mul_simd_inplace(&mut self, scalar: A);
}

impl<S, D> ScalarMulSIMDInplace<f32> for ArrayBase<S, D>
where
    S: DataMut<Elem = f32>,
    D: Dimension,
{
    fn scalar_mul_simd_inplace(&mut self, scalar: f32) {
        scalar_mul(self.as_slice_memory_order_mut().unwrap(), scalar)
    }
}

impl<S, D> ScalarMulSIMDInplace<f64> for ArrayBase<S, D>
where
    S: DataMut<Elem = f64>,
    D: Dimension,
{
    fn scalar_mul_simd_inplace(&mut self, scalar: f64) {
        log::warn!("scalar_mul for f64 is not implemented");
        self.mapv_inplace(|x| x * scalar);
    }
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn find_min_max_neon(slice: &[f32]) -> (f32, f32) {
    let mut min_val = f32::INFINITY;
    let mut max_val = f32::NEG_INFINITY;

    if slice.is_empty() {
        return (min_val, max_val);
    }

    // SAFETY: align_to is unsafe because it relies on the caller to ensure that
    // creating references to `float32x4_t` from the slice's memory is valid.
    // f32 is trivially transmutable to float32x4_t if the alignment and length are correct.
    // float32x4_t requires 16-byte alignment.
    let (prefix, middle, suffix) = unsafe { slice.align_to::<float32x4_t>() };

    for &val in prefix {
        min_val = min_val.min(val);
        max_val = max_val.max(val);
    }

    for &v_chunk in middle {
        // v_chunk is float32x4_t
        unsafe {
            min_val = min_val.min(vminvq_f32(v_chunk));
            max_val = max_val.max(vmaxvq_f32(v_chunk));
        }
    }

    for &val in suffix {
        min_val = min_val.min(val);
        max_val = max_val.max(val);
    }

    (min_val, max_val)
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn find_min_max_avx2(slice: &[f32]) -> (f32, f32) {
    let mut min_val = f32::INFINITY;
    let mut max_val = f32::NEG_INFINITY;

    if slice.is_empty() {
        return (min_val, max_val);
    }

    // SAFETY: align_to is unsafe for the same reasons as in NEON.
    // __m256 requires 32-byte alignment.
    let (prefix, middle, suffix) = unsafe { slice.align_to::<__m256>() };

    for &val in prefix {
        min_val = min_val.min(val);
        max_val = max_val.max(val);
    }

    for &v_chunk in middle {
        // v_chunk is __m256
        unsafe {
            min_val = min_val.min(_mm256_reduce_min_ps(v_chunk));
            max_val = max_val.max(_mm256_reduce_max_ps(v_chunk));
        }
    }

    for &val in suffix {
        min_val = min_val.min(val);
        max_val = max_val.max(val);
    }

    (min_val, max_val)
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn find_min_max_sse4(slice: &[f32]) -> (f32, f32) {
    let mut min_val = f32::INFINITY;
    let mut max_val = f32::NEG_INFINITY;

    if slice.is_empty() {
        return (min_val, max_val);
    }

    // SAFETY: align_to is unsafe for the same reasons as in NEON.
    // __m128 requires 16-byte alignment.
    let (prefix, middle, suffix) = unsafe { slice.align_to::<__m128>() };

    for &val in prefix {
        min_val = min_val.min(val);
        max_val = max_val.max(val);
    }

    for &v_chunk in middle {
        // v_chunk is __m128
        unsafe {
            min_val = min_val.min(_mm_reduce_min_ps(v_chunk));
            max_val = max_val.max(_mm_reduce_max_ps(v_chunk));
        }
    }

    for &val in suffix {
        min_val = min_val.min(val);
        max_val = max_val.max(val);
    }

    (min_val, max_val)
}

// Helper functions for SSE4.1 and AVX2 reductions
#[cfg(target_arch = "x86_64")]
#[inline]
unsafe fn _mm256_reduce_min_ps(v: __m256) -> f32 {
    unsafe {
        let low = _mm256_extractf128_ps(v, 0);
        let high = _mm256_extractf128_ps(v, 1);
        let min1 = _mm_min_ps(low, high);
        _mm_reduce_min_ps(min1)
    }
}

#[cfg(target_arch = "x86_64")]
#[inline]
unsafe fn _mm256_reduce_max_ps(v: __m256) -> f32 {
    unsafe {
        let low = _mm256_extractf128_ps(v, 0);
        let high = _mm256_extractf128_ps(v, 1);
        let max1 = _mm_max_ps(low, high);
        _mm_reduce_max_ps(max1)
    }
}
#[cfg(target_arch = "x86_64")]
#[inline]
unsafe fn _mm_reduce_min_ps(v: __m128) -> f32 {
    unsafe {
        let shuf = _mm_movehdup_ps(v);
        let min1 = _mm_min_ps(v, shuf);
        let shuf2 = _mm_movehl_ps(min1, min1);
        let min2 = _mm_min_ps(min1, shuf2);
        _mm_cvtss_f32(min2)
    }
}

#[cfg(target_arch = "x86_64")]
#[inline]
unsafe fn _mm_reduce_max_ps(v: __m128) -> f32 {
    unsafe {
        let shuf = _mm_movehdup_ps(v);
        let max1 = _mm_max_ps(v, shuf);
        let shuf2 = _mm_movehl_ps(max1, max1);
        let max2 = _mm_max_ps(max1, shuf2);
        _mm_cvtss_f32(max2)
    }
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn find_min_neon(slice: &[f32]) -> f32 {
    let mut min_val = f32::INFINITY;

    if slice.is_empty() {
        return min_val;
    }

    let (prefix, middle, suffix) = unsafe { slice.align_to::<float32x4_t>() };

    for &val in prefix {
        min_val = min_val.min(val);
    }

    for &v_chunk in middle {
        unsafe {
            min_val = min_val.min(vminvq_f32(v_chunk));
        }
    }

    for &val in suffix {
        min_val = min_val.min(val);
    }

    min_val
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn find_max_neon(slice: &[f32]) -> f32 {
    let mut max_val = f32::NEG_INFINITY;

    if slice.is_empty() {
        return max_val;
    }

    let (prefix, middle, suffix) = unsafe { slice.align_to::<float32x4_t>() };

    for &val in prefix {
        max_val = max_val.max(val);
    }

    for &v_chunk in middle {
        unsafe {
            max_val = max_val.max(vmaxvq_f32(v_chunk));
        }
    }

    for &val in suffix {
        max_val = max_val.max(val);
    }

    max_val
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn find_min_avx2(slice: &[f32]) -> f32 {
    let mut min_val = f32::INFINITY;

    if slice.is_empty() {
        return min_val;
    }

    let (prefix, middle, suffix) = unsafe { slice.align_to::<__m256>() };

    for &val in prefix {
        min_val = min_val.min(val);
    }

    for &v_chunk in middle {
        unsafe {
            min_val = min_val.min(_mm256_reduce_min_ps(v_chunk));
        }
    }

    for &val in suffix {
        min_val = min_val.min(val);
    }

    min_val
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn find_max_avx2(slice: &[f32]) -> f32 {
    let mut max_val = f32::NEG_INFINITY;

    if slice.is_empty() {
        return max_val;
    }

    let (prefix, middle, suffix) = unsafe { slice.align_to::<__m256>() };

    for &val in prefix {
        max_val = max_val.max(val);
    }

    for &v_chunk in middle {
        unsafe {
            max_val = max_val.max(_mm256_reduce_max_ps(v_chunk));
        }
    }

    for &val in suffix {
        max_val = max_val.max(val);
    }

    max_val
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn find_min_sse4(slice: &[f32]) -> f32 {
    let mut min_val = f32::INFINITY;

    if slice.is_empty() {
        return min_val;
    }

    let (prefix, middle, suffix) = unsafe { slice.align_to::<__m128>() };

    for &val in prefix {
        min_val = min_val.min(val);
    }

    for &v_chunk in middle {
        unsafe {
            min_val = min_val.min(_mm_reduce_min_ps(v_chunk));
        }
    }

    for &val in suffix {
        min_val = min_val.min(val);
    }

    min_val
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn find_max_sse4(slice: &[f32]) -> f32 {
    let mut max_val = f32::NEG_INFINITY;

    if slice.is_empty() {
        return max_val;
    }

    let (prefix, middle, suffix) = unsafe { slice.align_to::<__m128>() };

    for &val in prefix {
        max_val = max_val.max(val);
    }

    for &v_chunk in middle {
        unsafe {
            max_val = max_val.max(_mm_reduce_max_ps(v_chunk));
        }
    }

    for &val in suffix {
        max_val = max_val.max(val);
    }

    max_val
}

#[inline]
fn find_min_max_scalar(slice: &[f32]) -> (f32, f32) {
    let (min, max) = slice.iter().minmax().into_option().unwrap();
    (*min, *max)
}

#[inline]
fn find_min_scalar(slice: &[f32]) -> f32 {
    slice.iter().fold(f32::INFINITY, |a, &b| a.min(b))
}

#[inline]
fn find_max_scalar(slice: &[f32]) -> f32 {
    slice.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b))
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn sum_squares_neon(slice: &[f32]) -> f32 {
    if slice.is_empty() {
        return 0.0;
    }

    let mut sum = 0.0f32;
    let mut c = 0.0f32; // Running compensation
    let (prefix, middle, suffix) = unsafe { slice.align_to::<float32x4_t>() };

    // Handle prefix elements with Kahan summation
    for &val in prefix {
        let y = val * val - c;
        let t = sum + y;
        c = (t - sum) - y;
        sum = t;
    }

    // Handle aligned elements using NEON
    let mut sum_vec = unsafe { vdupq_n_f32(0.0) };
    let mut comp_vec = unsafe { vdupq_n_f32(0.0) };

    for &v_chunk in middle {
        unsafe {
            let squared = vmulq_f32(v_chunk, v_chunk);
            let y = vsubq_f32(squared, comp_vec);
            let t = vaddq_f32(sum_vec, y);
            comp_vec = vsubq_f32(vsubq_f32(t, sum_vec), y);
            sum_vec = t;
        }
    }
    // Reduce the vector sum
    sum += unsafe { vaddvq_f32(sum_vec) };

    // Handle suffix elements with Kahan summation
    for &val in suffix {
        let y = val * val - c;
        let t = sum + y;
        c = (t - sum) - y;
        sum = t;
    }

    sum
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn sum_squares_avx2(slice: &[f32]) -> f32 {
    if slice.is_empty() {
        return 0.0;
    }

    let mut sum = 0.0f32;
    let mut c = 0.0f32; // Running compensation
    let (prefix, middle, suffix) = unsafe { slice.align_to::<__m256>() };

    // Handle prefix elements with Kahan summation
    for &val in prefix {
        let y = val * val - c;
        let t = sum + y;
        c = (t - sum) - y;
        sum = t;
    }

    // Handle aligned elements using AVX2
    let mut sum_vec = unsafe { _mm256_setzero_ps() };
    let mut comp_vec = unsafe { _mm256_setzero_ps() };

    for &v_chunk in middle {
        unsafe {
            let squared = _mm256_mul_ps(v_chunk, v_chunk);
            let y = _mm256_sub_ps(squared, comp_vec);
            let t = _mm256_add_ps(sum_vec, y);
            comp_vec = _mm256_sub_ps(_mm256_sub_ps(t, sum_vec), y);
            sum_vec = t;
        }
    }
    // Reduce the vector sum
    sum += unsafe { _mm256_reduce_add_ps(sum_vec) };

    // Handle suffix elements with Kahan summation
    for &val in suffix {
        let y = val * val - c;
        let t = sum + y;
        c = (t - sum) - y;
        sum = t;
    }

    sum
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn sum_squares_sse4(slice: &[f32]) -> f32 {
    if slice.is_empty() {
        return 0.0;
    }

    let mut sum = 0.0f32;
    let mut c = 0.0f32; // Running compensation
    let (prefix, middle, suffix) = unsafe { slice.align_to::<__m128>() };

    // Handle prefix elements with Kahan summation
    for &val in prefix {
        let y = val * val - c;
        let t = sum + y;
        c = (t - sum) - y;
        sum = t;
    }

    // Handle aligned elements using SSE4
    let mut sum_vec = unsafe { _mm_setzero_ps() };
    let mut comp_vec = unsafe { _mm_setzero_ps() };

    for &v_chunk in middle {
        unsafe {
            let squared = _mm_mul_ps(v_chunk, v_chunk);
            let y = _mm_sub_ps(squared, comp_vec);
            let t = _mm_add_ps(sum_vec, y);
            comp_vec = _mm_sub_ps(_mm_sub_ps(t, sum_vec), y);
            sum_vec = t;
        }
    }
    // Reduce the vector sum
    sum += unsafe { _mm_reduce_add_ps(sum_vec) };

    // Handle suffix elements with Kahan summation
    for &val in suffix {
        let y = val * val - c;
        let t = sum + y;
        c = (t - sum) - y;
        sum = t;
    }

    sum
}

// Helper function for SSE4.1 reduction
#[cfg(target_arch = "x86_64")]
#[inline]
unsafe fn _mm_reduce_add_ps(v: __m128) -> f32 {
    unsafe {
        let shuf = _mm_movehdup_ps(v);
        let sum1 = _mm_add_ps(v, shuf);
        let shuf2 = _mm_movehl_ps(sum1, sum1);
        let sum2 = _mm_add_ps(sum1, shuf2);
        _mm_cvtss_f32(sum2)
    }
}

// Helper function for AVX2 reduction
#[cfg(target_arch = "x86_64")]
#[inline]
unsafe fn _mm256_reduce_add_ps(v: __m256) -> f32 {
    unsafe {
        let low = _mm256_extractf128_ps(v, 0);
        let high = _mm256_extractf128_ps(v, 1);
        let sum1 = _mm_add_ps(low, high);
        _mm_reduce_add_ps(sum1)
    }
}

#[inline]
fn sum_squares_scalar(slice: &[f32]) -> f32 {
    // Use Kahan summation for better numerical stability
    let mut sum = 0.0f32;
    let mut c = 0.0f32; // Running compensation for lost low-order bits

    for &x in slice {
        let y = x * x - c; // Subtract the compensation
        let t = sum + y; // Add to sum
        c = (t - sum) - y; // New compensation
        sum = t;
    }
    sum
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn abs_max_neon(slice: &[f32]) -> f32 {
    if slice.is_empty() {
        return 0.0;
    }

    let mut max_val = 0.0f32;
    let (prefix, middle, suffix) = unsafe { slice.align_to::<float32x4_t>() };

    // Handle prefix elements
    for &val in prefix {
        max_val = max_val.max(val.abs());
    }

    // Handle aligned elements using NEON
    let mut max_vec = unsafe { vdupq_n_f32(0.0) };
    for &v_chunk in middle {
        unsafe {
            let abs = vabsq_f32(v_chunk);
            max_vec = vmaxq_f32(max_vec, abs);
        }
    }
    // Reduce the vector max
    max_val = max_val.max(unsafe { vmaxvq_f32(max_vec) });

    // Handle suffix elements
    for &val in suffix {
        max_val = max_val.max(val.abs());
    }

    max_val
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn abs_max_avx2(slice: &[f32]) -> f32 {
    if slice.is_empty() {
        return 0.0;
    }

    let mut max_val = 0.0f32;
    let (prefix, middle, suffix) = unsafe { slice.align_to::<__m256>() };

    // Handle prefix elements
    for &val in prefix {
        max_val = max_val.max(val.abs());
    }

    // Handle aligned elements using AVX2
    let mut max_vec = unsafe { _mm256_setzero_ps() };
    for &v_chunk in middle {
        unsafe {
            let abs = _mm256_andnot_ps(_mm256_set1_ps(-0.0), v_chunk); // Clear sign bit
            max_vec = _mm256_max_ps(max_vec, abs);
        }
    }
    // Reduce the vector max
    max_val = max_val.max(unsafe { _mm256_reduce_max_ps(max_vec) });

    // Handle suffix elements
    for &val in suffix {
        max_val = max_val.max(val.abs());
    }

    max_val
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn abs_max_sse4(slice: &[f32]) -> f32 {
    if slice.is_empty() {
        return 0.0;
    }

    let mut max_val = 0.0f32;
    let (prefix, middle, suffix) = unsafe { slice.align_to::<__m128>() };

    // Handle prefix elements
    for &val in prefix {
        max_val = max_val.max(val.abs());
    }

    // Handle aligned elements using SSE4
    let mut max_vec = unsafe { _mm_setzero_ps() };
    for &v_chunk in middle {
        unsafe {
            let abs = _mm_andnot_ps(_mm_set1_ps(-0.0), v_chunk); // Clear sign bit
            max_vec = _mm_max_ps(max_vec, abs);
        }
    }
    // Reduce the vector max
    max_val = max_val.max(unsafe { _mm_reduce_max_ps(max_vec) });

    // Handle suffix elements
    for &val in suffix {
        max_val = max_val.max(val.abs());
    }

    max_val
}

#[inline]
fn abs_max_scalar(slice: &[f32]) -> f32 {
    slice.iter().map(|x| x.abs()).fold(0.0, |a, b| a.max(b))
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn scalar_mul_neon(slice: &mut [f32], scalar: f32) {
    if slice.is_empty() {
        return;
    }

    let scalar_vec = unsafe { vdupq_n_f32(scalar) };
    let (prefix, middle, suffix) = unsafe { slice.align_to_mut::<float32x4_t>() };

    // Handle prefix elements
    for val in prefix {
        *val *= scalar;
    }

    // Handle aligned elements using NEON
    for v_chunk in middle {
        unsafe {
            *v_chunk = vmulq_f32(*v_chunk, scalar_vec);
        }
    }

    // Handle suffix elements
    for val in suffix {
        *val *= scalar;
    }
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn scalar_mul_avx2(slice: &mut [f32], scalar: f32) {
    if slice.is_empty() {
        return;
    }

    let scalar_vec = unsafe { _mm256_set1_ps(scalar) };
    let (prefix, middle, suffix) = unsafe { slice.align_to_mut::<__m256>() };

    // Handle prefix elements
    for val in prefix {
        *val *= scalar;
    }

    // Handle aligned elements using AVX2
    for v_chunk in middle {
        unsafe {
            *v_chunk = _mm256_mul_ps(*v_chunk, scalar_vec);
        }
    }

    // Handle suffix elements
    for val in suffix {
        *val *= scalar;
    }
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn scalar_mul_sse4(slice: &mut [f32], scalar: f32) {
    if slice.is_empty() {
        return;
    }

    let scalar_vec = unsafe { _mm_set1_ps(scalar) };
    let (prefix, middle, suffix) = unsafe { slice.align_to_mut::<__m128>() };

    // Handle prefix elements
    for val in prefix {
        *val *= scalar;
    }

    // Handle aligned elements using SSE4
    for v_chunk in middle {
        unsafe {
            *v_chunk = _mm_mul_ps(*v_chunk, scalar_vec);
        }
    }

    // Handle suffix elements
    for val in suffix {
        *val *= scalar;
    }
}

#[inline]
fn scalar_mul_scalar(slice: &mut [f32], scalar: f32) {
    for val in slice {
        *val *= scalar;
    }
}

#[cfg(test)]
mod tests {
    use ndarray::Array1;
    use ndarray_rand::{RandomExt, rand_distr::Uniform};

    use super::*;
    use std::time::Instant;

    // A simple pseudo-random number generator for test data
    fn generate_random_data(size: usize) -> Vec<f32> {
        let arr = Array1::random(size, Uniform::new(-100.0, 100.0).unwrap());
        let (vec, _) = arr.into_raw_vec_and_offset();
        vec
    }

    #[test]
    #[ignore]
    fn benchmark_find_min_max_simd() {
        let data_size = 1_000_000;
        let data = generate_random_data(data_size);

        if data_size > 1000 {
            let warm_up_data = generate_random_data(1000);
            let _ = find_min_max_scalar(&warm_up_data);
            let _ = find_min_max(&warm_up_data); // Dispatch to SIMD or scalar
        }

        println!("\n--- Performance Test: find_min_max ---");
        println!("Data size: {} f32 elements", data_size);

        let start_scalar = Instant::now();
        let (min_s, max_s) = find_min_max_scalar(&data);
        let duration_scalar = start_scalar.elapsed();
        println!(
            "Scalar: min={:.6}, max={:.6}, time={:?}",
            min_s, max_s, duration_scalar
        );

        let start_simd = Instant::now();
        let (min_simd, max_simd) = find_min_max(&data); // This will call the main function which dispatches
        let duration_simd = start_simd.elapsed();
        println!(
            "SIMD (auto-dispatch): min={:.6}, max={:.6}, time={:?}",
            min_simd, max_simd, duration_simd
        );

        let epsilon = 1e-5;
        if (min_s - min_simd).abs() > epsilon || (max_s - max_simd).abs() > epsilon {
            eprintln!("Warning: Scalar and SIMD results differ significantly!");
            eprintln!("Scalar: min={}, max={}", min_s, max_s);
            eprintln!("SIMD:   min={}, max={}", min_simd, max_simd);
        }

        if duration_simd < duration_scalar {
            let diff = duration_scalar - duration_simd;
            let percentage = (diff.as_secs_f64() / duration_scalar.as_secs_f64()) * 100.0;
            println!("SIMD version was faster by {:?} ({:.2}%)", diff, percentage);
        } else if duration_scalar < duration_simd {
            let diff = duration_simd - duration_scalar;
            let percentage = (diff.as_secs_f64() / duration_simd.as_secs_f64()) * 100.0;
            println!(
                "Scalar version was faster by {:?} ({:.2}%)",
                diff, percentage
            );
        } else {
            println!("Scalar and SIMD versions had similar performance.");
        }
        println!("---------------------------------------\n");

        assert!(
            (min_s - min_simd).abs() < epsilon,
            "Minima do not match: scalar {} vs SIMD {}",
            min_s,
            min_simd
        );
        assert!(
            (max_s - max_simd).abs() < epsilon,
            "Maxima do not match: scalar {} vs SIMD {}",
            max_s,
            max_simd
        );
    }

    #[test]
    fn test_find_min_max_separate() {
        let test_cases = vec![
            vec![1.0, 2.0, 3.0, 4.0, 5.0],
            vec![-1.0, -2.0, -3.0, -4.0, -5.0],
            vec![0.0, 0.0, 0.0],
            vec![f32::INFINITY, f32::NEG_INFINITY, 0.0],
            vec![1.0],
            vec![],
        ];

        for data in test_cases {
            let (min_combined, max_combined) = find_min_max(&data);
            let min_separate = find_min(&data);
            let max_separate = find_max(&data);

            assert_eq!(
                min_combined, min_separate,
                "Min values don't match for data: {:?}",
                data
            );
            assert_eq!(
                max_combined, max_separate,
                "Max values don't match for data: {:?}",
                data
            );
        }
    }

    #[test]
    #[ignore]
    fn benchmark_find_min_max_separate() {
        let data_size = 1_000_000;
        let data = generate_random_data(data_size);

        if data_size > 1000 {
            let warm_up_data = generate_random_data(1000);
            let _ = find_min_scalar(&warm_up_data);
            let _ = find_max_scalar(&warm_up_data);
            let _ = find_min(&warm_up_data);
            let _ = find_max(&warm_up_data);
        }

        println!("\n--- Performance Test: find_min and find_max ---");
        println!("Data size: {} f32 elements", data_size);

        // Test find_min
        let start_scalar_min = Instant::now();
        let min_s = find_min_scalar(&data);
        let duration_scalar_min = start_scalar_min.elapsed();
        println!(
            "Scalar find_min: min={:.6}, time={:?}",
            min_s, duration_scalar_min
        );

        let start_simd_min = Instant::now();
        let min_simd = find_min(&data);
        let duration_simd_min = start_simd_min.elapsed();
        println!(
            "SIMD find_min: min={:.6}, time={:?}",
            min_simd, duration_simd_min
        );

        // Test find_max
        let start_scalar_max = Instant::now();
        let max_s = find_max_scalar(&data);
        let duration_scalar_max = start_scalar_max.elapsed();
        println!(
            "Scalar find_max: max={:.6}, time={:?}",
            max_s, duration_scalar_max
        );

        let start_simd_max = Instant::now();
        let max_simd = find_max(&data);
        let duration_simd_max = start_simd_max.elapsed();
        println!(
            "SIMD find_max: max={:.6}, time={:?}",
            max_simd, duration_simd_max
        );

        // Compare with combined min_max
        let start_combined = Instant::now();
        let (min_combined, max_combined) = find_min_max(&data);
        let duration_combined = start_combined.elapsed();
        println!(
            "Combined find_min_max: min={:.6}, max={:.6}, time={:?}",
            min_combined, max_combined, duration_combined
        );

        // Verify results
        let epsilon = 1e-5;
        assert!((min_s - min_simd).abs() < epsilon, "Min values don't match");
        assert!((max_s - max_simd).abs() < epsilon, "Max values don't match");
        assert!(
            (min_combined - min_simd).abs() < epsilon,
            "Combined min doesn't match separate min"
        );
        assert!(
            (max_combined - max_simd).abs() < epsilon,
            "Combined max doesn't match separate max"
        );

        // Print performance comparisons
        if duration_simd_min < duration_scalar_min {
            let diff = duration_scalar_min - duration_simd_min;
            let percentage = (diff.as_secs_f64() / duration_scalar_min.as_secs_f64()) * 100.0;
            println!(
                "SIMD find_min was faster by {:?} ({:.2}%)",
                diff, percentage
            );
        }

        if duration_simd_max < duration_scalar_max {
            let diff = duration_scalar_max - duration_simd_max;
            let percentage = (diff.as_secs_f64() / duration_scalar_max.as_secs_f64()) * 100.0;
            println!(
                "SIMD find_max was faster by {:?} ({:.2}%)",
                diff, percentage
            );
        }

        let total_separate = duration_simd_min + duration_simd_max;
        if total_separate < duration_combined {
            let diff = duration_combined - total_separate;
            let percentage = (diff.as_secs_f64() / duration_combined.as_secs_f64()) * 100.0;
            println!(
                "Separate min/max was faster than combined by {:?} ({:.2}%)",
                diff, percentage
            );
        } else {
            let diff = total_separate - duration_combined;
            let percentage = (diff.as_secs_f64() / total_separate.as_secs_f64()) * 100.0;
            println!(
                "Combined min/max was faster than separate by {:?} ({:.2}%)",
                diff, percentage
            );
        }

        println!("---------------------------------------\n");
    }

    #[test]
    fn test_sum_squares() {
        let test_cases = vec![
            (vec![1.0, 2.0, 3.0, 4.0], 30.0), // 1 + 4 + 9 + 16
            (vec![-1.0, -2.0, -3.0], 14.0),   // 1 + 4 + 9
            (vec![0.0, 0.0, 0.0], 0.0),
            (vec![1.0], 1.0),
            (vec![], 0.0),
        ];

        for (data, expected) in test_cases {
            let result = sum_squares(&data);
            assert!(
                (result - expected).abs() < 1e-5,
                "Sum of squares doesn't match for data: {:?}, expected: {}, got: {}",
                data,
                expected,
                result
            );
        }
    }

    #[test]
    #[ignore]
    fn benchmark_sum_squares() {
        let data_size = 1_000_000;
        let data = generate_random_data(data_size);

        if data_size > 1000 {
            let warm_up_data = generate_random_data(1000);
            let _ = sum_squares_scalar(&warm_up_data);
            let _ = sum_squares(&warm_up_data);
        }

        println!("\n--- Performance Test: sum_squares ---");
        println!("Data size: {} f32 elements", data_size);

        let start_scalar = Instant::now();
        let sum_scalar = sum_squares_scalar(&data);
        let duration_scalar = start_scalar.elapsed();
        println!(
            "Scalar sum_squares: sum={:.6}, time={:?}",
            sum_scalar, duration_scalar
        );

        let start_simd = Instant::now();
        let sum_simd = sum_squares(&data);
        let duration_simd = start_simd.elapsed();
        println!(
            "SIMD sum_squares: sum={:.6}, time={:?}",
            sum_simd, duration_simd
        );

        // Verify results with a larger epsilon for floating-point comparison
        let epsilon = 1e-3; // Increased epsilon for floating-point comparison
        let relative_error = (sum_scalar - sum_simd).abs() / sum_scalar.abs();
        assert!(
            relative_error < epsilon,
            "Sum of squares doesn't match: scalar {} vs SIMD {}, relative error: {}",
            sum_scalar,
            sum_simd,
            relative_error
        );

        // Print performance comparison
        if duration_simd < duration_scalar {
            let diff = duration_scalar - duration_simd;
            let percentage = (diff.as_secs_f64() / duration_scalar.as_secs_f64()) * 100.0;
            println!("SIMD version was faster by {:?} ({:.2}%)", diff, percentage);
        } else if duration_scalar < duration_simd {
            let diff = duration_simd - duration_scalar;
            let percentage = (diff.as_secs_f64() / duration_simd.as_secs_f64()) * 100.0;
            println!(
                "Scalar version was faster by {:?} ({:.2}%)",
                diff, percentage
            );
        } else {
            println!("Scalar and SIMD versions had similar performance.");
        }
        println!("---------------------------------------\n");
    }

    #[test]
    fn test_abs_max() {
        let test_cases = vec![
            (vec![1.0, -2.0, 3.0, -4.0], 4.0),
            (vec![-1.0, -2.0, -3.0], 3.0),
            (vec![0.0, 0.0, 0.0], 0.0),
            (vec![1.0], 1.0),
            (vec![-1.0], 1.0),
            (vec![], 0.0),
        ];

        for (data, expected) in test_cases {
            let result = abs_max(&data);
            assert!(
                (result - expected).abs() < 1e-5,
                "Absolute maximum doesn't match for data: {:?}, expected: {}, got: {}",
                data,
                expected,
                result
            );
        }
    }

    #[test]
    #[ignore]
    fn benchmark_abs_max() {
        let data_size = 1_000_000;
        let data = generate_random_data(data_size);

        if data_size > 1000 {
            let warm_up_data = generate_random_data(1000);
            let _ = abs_max_scalar(&warm_up_data);
            let _ = abs_max(&warm_up_data);
        }

        println!("\n--- Performance Test: abs_max ---");
        println!("Data size: {} f32 elements", data_size);

        let start_scalar = Instant::now();
        let max_scalar = abs_max_scalar(&data);
        let duration_scalar = start_scalar.elapsed();
        println!(
            "Scalar abs_max: max={:.6}, time={:?}",
            max_scalar, duration_scalar
        );

        let start_simd = Instant::now();
        let max_simd = abs_max(&data);
        let duration_simd = start_simd.elapsed();
        println!(
            "SIMD abs_max: max={:.6}, time={:?}",
            max_simd, duration_simd
        );

        // Verify results
        let epsilon = 1e-5;
        assert!(
            (max_scalar - max_simd).abs() < epsilon,
            "Absolute maximum doesn't match: scalar {} vs SIMD {}",
            max_scalar,
            max_simd
        );

        // Print performance comparison
        if duration_simd < duration_scalar {
            let diff = duration_scalar - duration_simd;
            let percentage = (diff.as_secs_f64() / duration_scalar.as_secs_f64()) * 100.0;
            println!("SIMD version was faster by {:?} ({:.2}%)", diff, percentage);
        } else if duration_scalar < duration_simd {
            let diff = duration_simd - duration_scalar;
            let percentage = (diff.as_secs_f64() / duration_simd.as_secs_f64()) * 100.0;
            println!(
                "Scalar version was faster by {:?} ({:.2}%)",
                diff, percentage
            );
        } else {
            println!("Scalar and SIMD versions had similar performance.");
        }
        println!("---------------------------------------\n");
    }

    #[test]
    fn test_scalar_mul() {
        let test_cases = vec![
            (vec![1.0, 2.0, 3.0, 4.0], 2.0, vec![2.0, 4.0, 6.0, 8.0]),
            (vec![-1.0, -2.0, -3.0], 3.0, vec![-3.0, -6.0, -9.0]),
            (vec![0.0, 0.0, 0.0], 5.0, vec![0.0, 0.0, 0.0]),
            (vec![1.0], 0.0, vec![0.0]),
            (vec![], 2.0, vec![]),
        ];

        for (mut data, scalar, expected) in test_cases {
            scalar_mul(&mut data, scalar);
            assert_eq!(
                data, expected,
                "Scalar multiplication failed for data: {:?}, scalar: {}",
                data, scalar
            );
        }
    }

    #[test]
    #[ignore]
    fn benchmark_scalar_mul() {
        let data_size = 1_000_000;
        let data = generate_random_data(data_size);
        let scalar = 2.5f32;

        if data_size > 1000 {
            let mut warm_up_data = generate_random_data(1000);
            scalar_mul_scalar(&mut warm_up_data, scalar);
            scalar_mul(&mut warm_up_data, scalar);
        }

        println!("\n--- Performance Test: scalar_mul ---");
        println!("Data size: {} f32 elements", data_size);

        let mut data_scalar = data.clone();
        let start_scalar = Instant::now();
        scalar_mul_scalar(&mut data_scalar, scalar);
        let duration_scalar = start_scalar.elapsed();
        println!("Scalar multiplication: time={:?}", duration_scalar);

        let mut data_simd = data.clone();
        let start_simd = Instant::now();
        scalar_mul(&mut data_simd, scalar);
        let duration_simd = start_simd.elapsed();
        println!("SIMD multiplication: time={:?}", duration_simd);

        // Verify results
        let epsilon = 1e-5;
        for (i, (a, b)) in data_scalar.iter().zip(data_simd.iter()).enumerate() {
            assert!(
                (a - b).abs() < epsilon,
                "Results don't match at index {}: scalar {} vs SIMD {}",
                i,
                a,
                b
            );
        }

        // Print performance comparison
        if duration_simd < duration_scalar {
            let diff = duration_scalar - duration_simd;
            let percentage = (diff.as_secs_f64() / duration_scalar.as_secs_f64()) * 100.0;
            println!("SIMD version was faster by {:?} ({:.2}%)", diff, percentage);
        } else if duration_scalar < duration_simd {
            let diff = duration_simd - duration_scalar;
            let percentage = (diff.as_secs_f64() / duration_simd.as_secs_f64()) * 100.0;
            println!(
                "Scalar version was faster by {:?} ({:.2}%)",
                diff, percentage
            );
        } else {
            println!("Scalar and SIMD versions had similar performance.");
        }
        println!("---------------------------------------\n");
    }
}
