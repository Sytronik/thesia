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

#[cfg(test)]
mod tests {
    use ndarray::Array1;
    use ndarray_rand::{RandomExt, rand_distr::Uniform};

    use super::*;
    use std::time::Instant;

    // A simple pseudo-random number generator for test data
    fn generate_random_data(size: usize) -> Vec<f32> {
        let arr = Array1::random(size, Uniform::new(-100.0, 100.0));
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

        println!(
            "
--- Performance Test: find_min_max ---"
        );
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
}
