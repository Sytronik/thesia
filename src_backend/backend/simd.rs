#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;
#[cfg(target_arch = "aarch64")]
use std::arch::is_aarch64_feature_detected;
#[cfg(target_arch = "x86_64")]
use std::arch::is_x86_feature_detected;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use itertools::Itertools;

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

#[inline]
fn find_min_max_scalar(slice: &[f32]) -> (f32, f32) {
    let (min, max) = slice.iter().minmax().into_option().unwrap();
    (*min, *max)
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn find_min_max_neon(slice: &[f32]) -> (f32, f32) {
    let mut min_val = f32::INFINITY;
    let mut max_val = f32::NEG_INFINITY;
    const SIMD_WIDTH: usize = 4;

    // Process full NEON chunks
    for chunk in slice.chunks_exact(SIMD_WIDTH) {
        unsafe {
            let v = vld1q_f32(chunk.as_ptr());
            min_val = min_val.min(vminvq_f32(v));
            max_val = max_val.max(vmaxvq_f32(v));
        }
    }

    // Handle remaining elements
    for &val in slice.chunks_exact(SIMD_WIDTH).remainder() {
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
    const SIMD_WIDTH: usize = 8; // AVX2 processes 8 f32 values at once

    // Process full AVX2 chunks
    for chunk in slice.chunks_exact(SIMD_WIDTH) {
        unsafe {
            let v = _mm256_loadu_ps(chunk.as_ptr());
            min_val = min_val.min(_mm256_reduce_min_ps(v));
            max_val = max_val.max(_mm256_reduce_max_ps(v));
        }
    }

    // Handle remaining elements
    for &val in slice.chunks_exact(SIMD_WIDTH).remainder() {
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
    const SIMD_WIDTH: usize = 4; // SSE4 processes 4 f32 values at once

    // Process full SSE4 chunks
    for chunk in slice.chunks_exact(SIMD_WIDTH) {
        unsafe {
            let v = _mm_loadu_ps(chunk.as_ptr());
            min_val = min_val.min(_mm_reduce_min_ps(v));
            max_val = max_val.max(_mm_reduce_max_ps(v));
        }
    }

    // Handle remaining elements
    for &val in slice.chunks_exact(SIMD_WIDTH).remainder() {
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
