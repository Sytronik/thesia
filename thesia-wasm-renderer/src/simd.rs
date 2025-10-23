#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
use core::arch::wasm32::{
    f32x4_add, f32x4_max, f32x4_min, f32x4_mul, f32x4_splat, v128_load, v128_store,
};

#[allow(unused)]
#[inline]
fn min_max_f32_scalar(values: &[f32]) -> (f32, f32) {
    let mut min_v = f32::INFINITY;
    let mut max_v = f32::NEG_INFINITY;
    for &v in values {
        if v < min_v {
            min_v = v;
        }
        if v > max_v {
            max_v = v;
        }
    }
    (min_v, max_v)
}

#[allow(unused)]
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[inline]
unsafe fn min_max_f32_simd(values: &[f32]) -> (f32, f32) {
    let len = values.len();
    if len == 0 {
        return (0.0, 0.0);
    }

    let mut i = 0usize;
    let ptr = values.as_ptr();

    // Initialize vector mins/maxs
    let mut v_min = f32x4_splat(f32::INFINITY);
    let mut v_max = f32x4_splat(f32::NEG_INFINITY);

    while i + 4 <= len {
        unsafe {
            let v = v128_load(ptr.add(i) as *const _);
            v_min = f32x4_min(v_min, v);
            v_max = f32x4_max(v_max, v);
        }
        i += 4;
    }

    // Reduce lanes to scalars
    let mut tmp_min = [0.0f32; 4];
    let mut tmp_max = [0.0f32; 4];
    unsafe {
        v128_store(tmp_min.as_mut_ptr() as *mut _, v_min);
        v128_store(tmp_max.as_mut_ptr() as *mut _, v_max);
    }

    let mut min_v = tmp_min[0].min(tmp_min[1]).min(tmp_min[2]).min(tmp_min[3]);
    let mut max_v = tmp_max[0].max(tmp_max[1]).max(tmp_max[2]).max(tmp_max[3]);

    // Remainder
    while i < len {
        unsafe {
            let v = *ptr.add(i);
            if v < min_v {
                min_v = v;
            }
            if v > max_v {
                max_v = v;
            }
        }
        i += 1;
    }

    (min_v, max_v)
}

#[inline]
pub(crate) fn min_max_f32(values: &[f32]) -> (f32, f32) {
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    unsafe {
        min_max_f32_simd(values)
    }
    #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
    {
        min_max_f32_scalar(values)
    }
}

#[allow(unused)]
#[inline]
fn add_scalar_to_slice_scalar(values: &mut [f32], scalar: f32) {
    for v in values.iter_mut() {
        *v += scalar;
    }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[inline]
unsafe fn add_scalar_to_slice_simd(values: &mut [f32], scalar: f32) {
    let len = values.len();
    let mut i = 0;
    let ptr = values.as_mut_ptr();
    let splat_scalar = f32x4_splat(scalar);

    while i + 4 <= len {
        unsafe {
            let v = v128_load(ptr.add(i) as *const _);
            let result = f32x4_add(v, splat_scalar);
            v128_store(ptr.add(i) as *mut _, result);
        }
        i += 4;
    }

    // Remainder
    while i < len {
        unsafe {
            *ptr.add(i) += scalar;
        }
        i += 1;
    }
}

#[inline]
pub(crate) fn add_scalar_to_slice(values: &mut [f32], scalar: f32) {
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    unsafe {
        add_scalar_to_slice_simd(values, scalar);
    }
    #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
    {
        add_scalar_to_slice_scalar(values, scalar);
    }
}

/// Apply affine transformation: out[i] = values[i] * scale + offset
#[allow(unused)]
#[inline]
fn fused_mul_add_scalar(values: &[f32], scale: f32, offset: f32, out: &mut Vec<f32>) {
    for &v in values {
        out.push(v * scale + offset);
    }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[inline]
unsafe fn fused_mul_add_simd(values: &[f32], scale: f32, offset: f32, out: &mut Vec<f32>) {
    let len = values.len();
    let mut i = 0;
    let ptr = values.as_ptr();
    let splat_scale = f32x4_splat(scale);
    let splat_offset = f32x4_splat(offset);

    // Reserve capacity for the output
    out.reserve(len);
    let out_ptr = out.as_mut_ptr();
    let old_len = out.len();

    // Process 4 elements at a time
    while i + 4 <= len {
        unsafe {
            let v = v128_load(ptr.add(i) as *const _);
            let scaled = f32x4_mul(v, splat_scale);
            let result = f32x4_add(scaled, splat_offset);

            // Write directly to Vec's memory
            v128_store(out_ptr.add(old_len + i) as *mut _, result);
        }
        i += 4;
    }

    // Update length
    unsafe {
        out.set_len(old_len + i);
    }

    // Process remainder
    while i < len {
        unsafe {
            out.push(*ptr.add(i) * scale + offset);
        }
        i += 1;
    }
}

#[inline]
pub(crate) fn fused_mul_add(values: &[f32], scale: f32, offset: f32, out: &mut Vec<f32>) {
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    unsafe {
        fused_mul_add_simd(values, scale, offset, out);
    }
    #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
    {
        fused_mul_add_scalar(values, scale, offset, out);
    }
}

/// Clamp values in-place: values[i] = values[i].max(min).min(max)
#[allow(unused)]
#[inline]
fn clamp_f32_scalar(values: &mut [f32], min: f32, max: f32) {
    for v in values.iter_mut() {
        *v = v.max(min).min(max);
    }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[inline]
unsafe fn clamp_f32_simd(values: &mut [f32], min: f32, max: f32) {
    let len = values.len();
    let mut i = 0;
    let ptr = values.as_mut_ptr();
    let splat_min = f32x4_splat(min);
    let splat_max = f32x4_splat(max);

    // Process 4 elements at a time
    while i + 4 <= len {
        unsafe {
            let v = v128_load(ptr.add(i) as *const _);
            let clamped = f32x4_min(f32x4_max(v, splat_min), splat_max);
            v128_store(ptr.add(i) as *mut _, clamped);
        }
        i += 4;
    }

    // Process remainder
    while i < len {
        unsafe {
            let v = ptr.add(i);
            *v = (*v).max(min).min(max);
        }
        i += 1;
    }
}

#[inline]
pub(crate) fn clamp_f32(values: &mut [f32], min: f32, max: f32) {
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    unsafe {
        clamp_f32_simd(values, min, max);
    }
    #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
    {
        clamp_f32_scalar(values, min, max);
    }
}
