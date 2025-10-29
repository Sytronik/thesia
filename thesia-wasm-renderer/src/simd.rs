#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
use core::arch::wasm32::{
    f32x4_add, f32x4_max, f32x4_min, f32x4_mul, f32x4_splat, v128_load, v128_store,
};

#[allow(unused)]
#[inline]
fn find_min_max_scalar(values: &[f32]) -> (f32, f32) {
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
#[inline]
fn find_min_scalar(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut min_v = f32::INFINITY;
    for &v in values {
        if v < min_v {
            min_v = v;
        }
    }
    min_v
}

#[allow(unused)]
#[inline]
fn find_max_scalar(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut max_v = f32::NEG_INFINITY;
    for &v in values {
        if v > max_v {
            max_v = v;
        }
    }
    max_v
}

#[allow(unused)]
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[inline]
unsafe fn find_min_max_simd(values: &[f32]) -> (f32, f32) {
    let len = values.len();
    if len == 0 {
        return (0.0, 0.0);
    }

    let mut i = 0usize;
    let ptr = values.as_ptr();

    // Initialize vector mins/maxs
    let mut v_min = f32x4_splat(f32::INFINITY);
    let mut v_max = f32x4_splat(f32::NEG_INFINITY);

    // Loop unrolling: process 16 elements (4x4) at a time
    while i + 16 <= len {
        let v0 = unsafe { v128_load(ptr.add(i) as *const _) };
        let v1 = unsafe { v128_load(ptr.add(i + 4) as *const _) };
        let v2 = unsafe { v128_load(ptr.add(i + 8) as *const _) };
        let v3 = unsafe { v128_load(ptr.add(i + 12) as *const _) };

        v_min = f32x4_min(v_min, v0);
        v_max = f32x4_max(v_max, v0);
        v_min = f32x4_min(v_min, v1);
        v_max = f32x4_max(v_max, v1);
        v_min = f32x4_min(v_min, v2);
        v_max = f32x4_max(v_max, v2);
        v_min = f32x4_min(v_min, v3);
        v_max = f32x4_max(v_max, v3);
        i += 16;
    }

    // Handle remaining 4-element chunks
    while i + 4 <= len {
        let v = unsafe { v128_load(ptr.add(i) as *const _) };
        v_min = f32x4_min(v_min, v);
        v_max = f32x4_max(v_max, v);
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
        let v = unsafe { *ptr.add(i) };
        if v < min_v {
            min_v = v;
        }
        if v > max_v {
            max_v = v;
        }
        i += 1;
    }

    (min_v, max_v)
}

#[allow(unused)]
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[inline]
unsafe fn find_min_simd(values: &[f32]) -> f32 {
    let len = values.len();
    if len == 0 {
        return 0.0;
    }

    let mut i = 0usize;
    let ptr = values.as_ptr();

    let mut v_min = f32x4_splat(f32::INFINITY);

    // Loop unrolling: process 16 elements (4x4) at a time
    while i + 16 <= len {
        let v0 = unsafe { v128_load(ptr.add(i) as *const _) };
        let v1 = unsafe { v128_load(ptr.add(i + 4) as *const _) };
        let v2 = unsafe { v128_load(ptr.add(i + 8) as *const _) };
        let v3 = unsafe { v128_load(ptr.add(i + 12) as *const _) };

        v_min = f32x4_min(v_min, v0);
        v_min = f32x4_min(v_min, v1);
        v_min = f32x4_min(v_min, v2);
        v_min = f32x4_min(v_min, v3);
        i += 16;
    }

    // Handle remaining 4-element chunks
    while i + 4 <= len {
        let v = unsafe { v128_load(ptr.add(i) as *const _) };
        v_min = f32x4_min(v_min, v);
        i += 4;
    }

    let mut tmp_min = [0.0f32; 4];
    unsafe { v128_store(tmp_min.as_mut_ptr() as *mut _, v_min) };
    let mut min_v = tmp_min[0].min(tmp_min[1]).min(tmp_min[2]).min(tmp_min[3]);

    while i < len {
        let v = unsafe { *ptr.add(i) };
        if v < min_v {
            min_v = v;
        }
        i += 1;
    }

    min_v
}

#[allow(unused)]
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[inline]
unsafe fn find_max_simd(values: &[f32]) -> f32 {
    let len = values.len();
    if len == 0 {
        return 0.0;
    }

    let mut i = 0usize;
    let ptr = values.as_ptr();

    let mut v_max = f32x4_splat(f32::NEG_INFINITY);

    // Loop unrolling: process 16 elements (4x4) at a time
    while i + 16 <= len {
        let v0 = unsafe { v128_load(ptr.add(i) as *const _) };
        let v1 = unsafe { v128_load(ptr.add(i + 4) as *const _) };
        let v2 = unsafe { v128_load(ptr.add(i + 8) as *const _) };
        let v3 = unsafe { v128_load(ptr.add(i + 12) as *const _) };

        v_max = f32x4_max(v_max, v0);
        v_max = f32x4_max(v_max, v1);
        v_max = f32x4_max(v_max, v2);
        v_max = f32x4_max(v_max, v3);
        i += 16;
    }

    // Handle remaining 4-element chunks
    while i + 4 <= len {
        let v = unsafe { v128_load(ptr.add(i) as *const _) };
        v_max = f32x4_max(v_max, v);
        i += 4;
    }

    let mut tmp_max = [0.0f32; 4];
    unsafe { v128_store(tmp_max.as_mut_ptr() as *mut _, v_max) };
    let mut max_v = tmp_max[0].max(tmp_max[1]).max(tmp_max[2]).max(tmp_max[3]);

    while i < len {
        let v = unsafe { *ptr.add(i) };
        if v > max_v {
            max_v = v;
        }
        i += 1;
    }

    max_v
}

#[inline]
pub(crate) fn find_min_max(values: &[f32]) -> (f32, f32) {
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    unsafe {
        find_min_max_simd(values)
    }
    #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
    {
        find_min_max_scalar(values)
    }
}

#[inline]
pub(crate) fn find_min(values: &[f32]) -> f32 {
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    unsafe {
        find_min_simd(values)
    }
    #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
    {
        find_min_scalar(values)
    }
}

#[inline]
pub(crate) fn find_max(values: &[f32]) -> f32 {
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    unsafe {
        find_max_simd(values)
    }
    #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
    {
        find_max_scalar(values)
    }
}

#[allow(unused)]
#[inline]
fn add_scalar_inplace_scalar(values: &mut [f32], scalar: f32) {
    for v in values.iter_mut() {
        *v += scalar;
    }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[inline]
unsafe fn add_scalar_inplace_simd(values: &mut [f32], scalar: f32) {
    let len = values.len();
    let mut i = 0;
    let ptr = values.as_mut_ptr();
    let splat_scalar = f32x4_splat(scalar);

    // Loop unrolling: process 16 elements (4x4) at a time
    while i + 16 <= len {
        let v0 = unsafe { v128_load(ptr.add(i) as *const _) };
        let v1 = unsafe { v128_load(ptr.add(i + 4) as *const _) };
        let v2 = unsafe { v128_load(ptr.add(i + 8) as *const _) };
        let v3 = unsafe { v128_load(ptr.add(i + 12) as *const _) };

        let result0 = f32x4_add(v0, splat_scalar);
        let result1 = f32x4_add(v1, splat_scalar);
        let result2 = f32x4_add(v2, splat_scalar);
        let result3 = f32x4_add(v3, splat_scalar);

        unsafe {
            v128_store(ptr.add(i) as *mut _, result0);
            v128_store(ptr.add(i + 4) as *mut _, result1);
            v128_store(ptr.add(i + 8) as *mut _, result2);
            v128_store(ptr.add(i + 12) as *mut _, result3);
        }
        i += 16;
    }

    // Handle remaining 4-element chunks
    while i + 4 <= len {
        let v = unsafe { v128_load(ptr.add(i) as *const _) };
        let result = f32x4_add(v, splat_scalar);
        unsafe { v128_store(ptr.add(i) as *mut _, result) };
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
pub(crate) fn add_scalar_inplace(values: &mut [f32], scalar: f32) {
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    unsafe {
        add_scalar_inplace_simd(values, scalar);
    }
    #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
    {
        add_scalar_inplace_scalar(values, scalar);
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

    // Loop unrolling: process 16 elements (4x4) at a time
    while i + 16 <= len {
        let v0 = unsafe { v128_load(ptr.add(i) as *const _) };
        let v1 = unsafe { v128_load(ptr.add(i + 4) as *const _) };
        let v2 = unsafe { v128_load(ptr.add(i + 8) as *const _) };
        let v3 = unsafe { v128_load(ptr.add(i + 12) as *const _) };

        let scaled0 = f32x4_mul(v0, splat_scale);
        let scaled1 = f32x4_mul(v1, splat_scale);
        let scaled2 = f32x4_mul(v2, splat_scale);
        let scaled3 = f32x4_mul(v3, splat_scale);

        let result0 = f32x4_add(scaled0, splat_offset);
        let result1 = f32x4_add(scaled1, splat_offset);
        let result2 = f32x4_add(scaled2, splat_offset);
        let result3 = f32x4_add(scaled3, splat_offset);

        // Write directly to Vec's memory
        unsafe {
            v128_store(out_ptr.add(old_len + i) as *mut _, result0);
            v128_store(out_ptr.add(old_len + i + 4) as *mut _, result1);
            v128_store(out_ptr.add(old_len + i + 8) as *mut _, result2);
            v128_store(out_ptr.add(old_len + i + 12) as *mut _, result3);
        }

        i += 16;
    }

    // Handle remaining 4-element chunks
    while i + 4 <= len {
        let v = unsafe { v128_load(ptr.add(i) as *const _) };
        let scaled = f32x4_mul(v, splat_scale);
        let result = f32x4_add(scaled, splat_offset);

        // Write directly to Vec's memory
        unsafe { v128_store(out_ptr.add(old_len + i) as *mut _, result) };
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
fn clamp_inplace_scalar(values: &mut [f32], min: f32, max: f32) {
    for v in values.iter_mut() {
        *v = v.max(min).min(max);
    }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[inline]
unsafe fn clamp_inplace_simd(values: &mut [f32], min: f32, max: f32) {
    let len = values.len();
    let mut i = 0;
    let ptr = values.as_mut_ptr();
    let splat_min = f32x4_splat(min);
    let splat_max = f32x4_splat(max);

    // Loop unrolling: process 16 elements (4x4) at a time
    while i + 16 <= len {
        let v0 = unsafe { v128_load(ptr.add(i) as *const _) };
        let v1 = unsafe { v128_load(ptr.add(i + 4) as *const _) };
        let v2 = unsafe { v128_load(ptr.add(i + 8) as *const _) };
        let v3 = unsafe { v128_load(ptr.add(i + 12) as *const _) };

        let clamped0 = f32x4_min(f32x4_max(v0, splat_min), splat_max);
        let clamped1 = f32x4_min(f32x4_max(v1, splat_min), splat_max);
        let clamped2 = f32x4_min(f32x4_max(v2, splat_min), splat_max);
        let clamped3 = f32x4_min(f32x4_max(v3, splat_min), splat_max);

        unsafe {
            v128_store(ptr.add(i) as *mut _, clamped0);
            v128_store(ptr.add(i + 4) as *mut _, clamped1);
            v128_store(ptr.add(i + 8) as *mut _, clamped2);
            v128_store(ptr.add(i + 12) as *mut _, clamped3);
        }

        i += 16;
    }

    // Handle remaining 4-element chunks
    while i + 4 <= len {
        let v = unsafe { v128_load(ptr.add(i) as *const _) };
        let clamped = f32x4_min(f32x4_max(v, splat_min), splat_max);
        unsafe { v128_store(ptr.add(i) as *mut _, clamped) };
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
pub(crate) fn clamp_inplace(values: &mut [f32], min: f32, max: f32) {
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    unsafe {
        clamp_inplace_simd(values, min, max);
    }
    #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
    {
        clamp_inplace_scalar(values, min, max);
    }
}

/// Negate values: out[i] = -values[i]
#[allow(unused)]
#[inline]
fn negate_scalar(values: &[f32], out: &mut Vec<f32>) {
    for &v in values {
        out.push(-v);
    }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[inline]
unsafe fn negate_simd(values: &[f32], out: &mut Vec<f32>) {
    let len = values.len();
    let mut i = 0;
    let ptr = values.as_ptr();
    let neg_one = f32x4_splat(-1.0);

    // Reserve capacity for the output
    out.reserve(len);
    let out_ptr = out.as_mut_ptr();
    let old_len = out.len();

    // Loop unrolling: process 16 elements (4x4) at a time
    while i + 16 <= len {
        let v0 = unsafe { v128_load(ptr.add(i) as *const _) };
        let v1 = unsafe { v128_load(ptr.add(i + 4) as *const _) };
        let v2 = unsafe { v128_load(ptr.add(i + 8) as *const _) };
        let v3 = unsafe { v128_load(ptr.add(i + 12) as *const _) };

        let result0 = f32x4_mul(v0, neg_one);
        let result1 = f32x4_mul(v1, neg_one);
        let result2 = f32x4_mul(v2, neg_one);
        let result3 = f32x4_mul(v3, neg_one);

        // Write directly to Vec's memory
        unsafe {
            v128_store(out_ptr.add(old_len + i) as *mut _, result0);
            v128_store(out_ptr.add(old_len + i + 4) as *mut _, result1);
            v128_store(out_ptr.add(old_len + i + 8) as *mut _, result2);
            v128_store(out_ptr.add(old_len + i + 12) as *mut _, result3);
        }

        i += 16;
    }

    // Handle remaining 4-element chunks
    while i + 4 <= len {
        let v = unsafe { v128_load(ptr.add(i) as *const _) };
        let result = f32x4_mul(v, neg_one);

        // Write directly to Vec's memory
        unsafe { v128_store(out_ptr.add(old_len + i) as *mut _, result) };
        i += 4;
    }

    // Update length
    unsafe {
        out.set_len(old_len + i);
    }

    // Process remainder
    while i < len {
        unsafe {
            out.push(-*ptr.add(i));
        }
        i += 1;
    }
}

#[inline]
pub(crate) fn negate(values: &[f32], out: &mut Vec<f32>) {
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    unsafe {
        negate_simd(values, out);
    }
    #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
    {
        negate_scalar(values, out);
    }
}
