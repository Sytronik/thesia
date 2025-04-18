#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{__m128, __m128i, __m256, __m256i};

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::{float32x4_t, int32x4_t};

#[allow(unused_imports)]
use aligned::{A16, A32, Aligned};
use itertools::{Itertools, multizip};

const BLACK: [u8; 3] = [000; 3];
const WHITE: [u8; 3] = [255; 3];
// const BLACK_F32: [f32; 3] = [0.; 3];
const WHITE_F32: [f32; 3] = [255.; 3];

const COLORMAP_R: [f32; 256] = [
    0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 3.0, 4.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
    13.0, 14.0, 16.0, 17.0, 18.0, 20.0, 21.0, 22.0, 24.0, 25.0, 27.0, 28.0, 30.0, 31.0, 33.0, 35.0,
    36.0, 38.0, 40.0, 42.0, 43.0, 45.0, 47.0, 49.0, 51.0, 52.0, 54.0, 56.0, 58.0, 59.0, 61.0, 63.0,
    64.0, 66.0, 68.0, 69.0, 71.0, 73.0, 74.0, 76.0, 78.0, 79.0, 81.0, 83.0, 84.0, 86.0, 87.0, 89.0,
    91.0, 92.0, 94.0, 95.0, 97.0, 99.0, 100.0, 102.0, 103.0, 105.0, 107.0, 108.0, 110.0, 111.0,
    113.0, 115.0, 116.0, 118.0, 119.0, 121.0, 123.0, 124.0, 126.0, 127.0, 129.0, 130.0, 132.0,
    134.0, 135.0, 137.0, 138.0, 140.0, 142.0, 143.0, 145.0, 146.0, 148.0, 150.0, 151.0, 153.0,
    154.0, 156.0, 158.0, 159.0, 161.0, 162.0, 164.0, 165.0, 167.0, 169.0, 170.0, 172.0, 173.0,
    175.0, 176.0, 178.0, 179.0, 181.0, 182.0, 184.0, 185.0, 187.0, 188.0, 190.0, 191.0, 193.0,
    194.0, 196.0, 197.0, 198.0, 200.0, 201.0, 203.0, 204.0, 205.0, 207.0, 208.0, 209.0, 211.0,
    212.0, 213.0, 214.0, 216.0, 217.0, 218.0, 219.0, 220.0, 221.0, 223.0, 224.0, 225.0, 226.0,
    227.0, 228.0, 229.0, 230.0, 231.0, 232.0, 233.0, 234.0, 235.0, 235.0, 236.0, 237.0, 238.0,
    239.0, 240.0, 240.0, 241.0, 242.0, 242.0, 243.0, 244.0, 244.0, 245.0, 246.0, 246.0, 247.0,
    247.0, 248.0, 248.0, 249.0, 249.0, 249.0, 250.0, 250.0, 250.0, 251.0, 251.0, 251.0, 252.0,
    252.0, 252.0, 252.0, 252.0, 253.0, 253.0, 253.0, 253.0, 253.0, 253.0, 253.0, 253.0, 253.0,
    253.0, 253.0, 253.0, 252.0, 252.0, 252.0, 252.0, 252.0, 251.0, 251.0, 251.0, 251.0, 250.0,
    250.0, 250.0, 249.0, 249.0, 248.0, 248.0, 247.0, 247.0, 246.0, 246.0, 245.0, 245.0, 244.0,
    244.0, 244.0, 243.0, 243.0, 243.0, 242.0, 242.0, 242.0, 242.0, 243.0, 243.0, 244.0, 244.0,
    245.0, 246.0, 247.0, 249.0, 250.0, 251.0, 253.0,
];
const COLORMAP_G: [f32; 256] = [
    0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 5.0, 6.0, 6.0, 7.0, 7.0, 8.0, 8.0,
    9.0, 9.0, 10.0, 10.0, 11.0, 11.0, 11.0, 12.0, 12.0, 12.0, 12.0, 12.0, 12.0, 12.0, 12.0, 12.0,
    12.0, 11.0, 11.0, 11.0, 11.0, 10.0, 10.0, 10.0, 10.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 10.0, 10.0,
    10.0, 10.0, 11.0, 11.0, 12.0, 12.0, 13.0, 13.0, 14.0, 14.0, 15.0, 15.0, 16.0, 17.0, 17.0, 18.0,
    18.0, 19.0, 20.0, 20.0, 21.0, 21.0, 22.0, 23.0, 23.0, 24.0, 24.0, 25.0, 25.0, 26.0, 27.0, 27.0,
    28.0, 28.0, 29.0, 29.0, 30.0, 31.0, 31.0, 32.0, 32.0, 33.0, 33.0, 34.0, 34.0, 35.0, 36.0, 36.0,
    37.0, 37.0, 38.0, 38.0, 39.0, 40.0, 40.0, 41.0, 41.0, 42.0, 43.0, 43.0, 44.0, 45.0, 45.0, 46.0,
    46.0, 47.0, 48.0, 49.0, 49.0, 50.0, 51.0, 51.0, 52.0, 53.0, 54.0, 54.0, 55.0, 56.0, 57.0, 58.0,
    59.0, 60.0, 60.0, 61.0, 62.0, 63.0, 64.0, 65.0, 66.0, 67.0, 68.0, 69.0, 70.0, 72.0, 73.0, 74.0,
    75.0, 76.0, 77.0, 79.0, 80.0, 81.0, 82.0, 84.0, 85.0, 86.0, 88.0, 89.0, 90.0, 92.0, 93.0, 95.0,
    96.0, 98.0, 99.0, 101.0, 102.0, 104.0, 105.0, 107.0, 109.0, 110.0, 112.0, 113.0, 115.0, 117.0,
    118.0, 120.0, 122.0, 123.0, 125.0, 127.0, 129.0, 130.0, 132.0, 134.0, 136.0, 137.0, 139.0,
    141.0, 143.0, 145.0, 146.0, 148.0, 150.0, 152.0, 154.0, 156.0, 158.0, 160.0, 161.0, 163.0,
    165.0, 167.0, 169.0, 171.0, 173.0, 175.0, 177.0, 179.0, 181.0, 183.0, 185.0, 186.0, 188.0,
    190.0, 192.0, 194.0, 196.0, 198.0, 200.0, 202.0, 204.0, 206.0, 208.0, 210.0, 212.0, 214.0,
    216.0, 218.0, 220.0, 222.0, 224.0, 226.0, 228.0, 229.0, 231.0, 233.0, 235.0, 237.0, 238.0,
    240.0, 241.0, 243.0, 244.0, 246.0, 247.0, 249.0, 250.0, 251.0, 252.0, 253.0, 254.0, 255.0,
];
const COLORMAP_B: [f32; 256] = [
    4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 21.0, 23.0, 25.0, 27.0, 29.0, 32.0, 34.0,
    36.0, 38.0, 41.0, 43.0, 45.0, 48.0, 50.0, 53.0, 55.0, 58.0, 60.0, 62.0, 65.0, 67.0, 70.0, 72.0,
    74.0, 77.0, 79.0, 81.0, 83.0, 85.0, 87.0, 89.0, 91.0, 93.0, 94.0, 96.0, 97.0, 98.0, 99.0,
    100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 105.0, 106.0, 107.0, 107.0, 108.0, 108.0, 108.0,
    109.0, 109.0, 109.0, 110.0, 110.0, 110.0, 110.0, 110.0, 111.0, 111.0, 111.0, 111.0, 111.0,
    111.0, 111.0, 111.0, 111.0, 111.0, 111.0, 111.0, 110.0, 110.0, 110.0, 110.0, 110.0, 110.0,
    109.0, 109.0, 109.0, 109.0, 108.0, 108.0, 108.0, 107.0, 107.0, 107.0, 106.0, 106.0, 105.0,
    105.0, 105.0, 104.0, 104.0, 103.0, 102.0, 102.0, 101.0, 101.0, 100.0, 100.0, 99.0, 98.0, 98.0,
    97.0, 96.0, 95.0, 95.0, 94.0, 93.0, 92.0, 92.0, 91.0, 90.0, 89.0, 88.0, 87.0, 86.0, 85.0, 85.0,
    84.0, 83.0, 82.0, 81.0, 80.0, 79.0, 78.0, 77.0, 76.0, 75.0, 74.0, 72.0, 71.0, 70.0, 69.0, 68.0,
    67.0, 66.0, 65.0, 64.0, 62.0, 61.0, 60.0, 59.0, 58.0, 57.0, 56.0, 54.0, 53.0, 52.0, 51.0, 50.0,
    48.0, 47.0, 46.0, 45.0, 43.0, 42.0, 41.0, 40.0, 38.0, 37.0, 36.0, 35.0, 33.0, 32.0, 31.0, 30.0,
    28.0, 27.0, 26.0, 24.0, 23.0, 22.0, 20.0, 19.0, 18.0, 16.0, 15.0, 14.0, 12.0, 11.0, 10.0, 9.0,
    8.0, 7.0, 7.0, 6.0, 6.0, 6.0, 6.0, 7.0, 7.0, 8.0, 9.0, 10.0, 12.0, 13.0, 15.0, 17.0, 19.0,
    20.0, 22.0, 24.0, 27.0, 29.0, 31.0, 33.0, 35.0, 38.0, 40.0, 43.0, 45.0, 48.0, 50.0, 53.0, 56.0,
    58.0, 61.0, 64.0, 67.0, 70.0, 73.0, 76.0, 80.0, 83.0, 86.0, 90.0, 94.0, 97.0, 101.0, 105.0,
    109.0, 113.0, 117.0, 122.0, 126.0, 130.0, 134.0, 138.0, 142.0, 146.0, 150.0, 154.0, 158.0,
    162.0, 165.0,
];
const GREY_TO_POS: f32 = COLORMAP_R.len() as f32 / (u16::MAX - 1) as f32;

#[inline]
pub fn get_colormap_rgb() -> Vec<u8> {
    multizip((COLORMAP_R.iter(), COLORMAP_G.iter(), COLORMAP_B.iter()))
        .flat_map(|(&r, &g, &b)| [r as u8, g as u8, b as u8].into_iter())
        .chain(WHITE.iter().copied())
        .collect()
}

#[inline]
fn interpolate<const L: usize>(color1: &[f32; L], color2: &[f32; L], ratio: f32) -> [u8; L] {
    let mut iter = color1.iter().zip(color2).map(|(&a, &b)| {
        let out_f32 = ratio.mul_add(a, b.mul_add(-ratio, b));
        #[cfg(target_arch = "x86_64")]
        {
            out_f32.round_ties_even() as u8 // to match with AVX2 rounding
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            out_f32.round() as u8
        }
    });
    [(); L].map(|_| iter.next().unwrap())
}

/// Map u16 GRAY to u8x4 RGBA color
/// 0 -> COLORMAP[0]
/// u16::MAX -> WHITE
fn map_grey_to_color(x: u16) -> [u8; 3] {
    if x == 0 {
        return BLACK;
    }
    if x == u16::MAX {
        return WHITE;
    }
    let position = (x as f32).mul_add(GREY_TO_POS, -GREY_TO_POS);
    let idx2 = position.floor() as usize;
    let idx1 = idx2 + 1;
    let ratio = position.fract();
    // dbg!(idx2, idx1, ratio);
    let rgb1 = if idx2 >= COLORMAP_R.len() - 1 {
        &WHITE_F32
    } else {
        &[COLORMAP_R[idx1], COLORMAP_G[idx1], COLORMAP_B[idx1]]
    };
    let rgb2 = &[COLORMAP_R[idx2], COLORMAP_G[idx2], COLORMAP_B[idx2]];
    interpolate(rgb1, rgb2, ratio)
}

fn map_grey_to_color_iter_fallback(grey: &[u16]) -> impl Iterator<Item = u8> + use<'_> {
    grey.iter()
        .flat_map(|&x| map_grey_to_color(x).into_iter().chain(Some(u8::MAX)))
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn map_grey_to_color_sse41(
    chunk_f32: Aligned<A16, [f32; 4]>,
    grey_to_pos: __m128,
    colormap_len: __m128i,
) -> impl Iterator<Item = u8> {
    use std::arch::x86_64::*;
    use std::mem::{self, MaybeUninit};

    // Load chunk_f32 into a SIMD register
    let chunk_simd = _mm_load_ps(chunk_f32.as_ptr());

    // Compute position = chunk_simd * grey_to_pos - grey_to_pos
    let position = if is_x86_feature_detected!("fma") {
        _mm_fmsub_ps(chunk_simd, grey_to_pos, grey_to_pos)
    } else {
        _mm_sub_ps(_mm_mul_ps(chunk_simd, grey_to_pos), grey_to_pos)
    };

    // Compute floor of position
    let position_floor = _mm_floor_ps(position);

    // Convert position_floor to integer indices
    let idx2 = _mm_cvtps_epi32(position_floor);

    // idx1 = idx2 + 1
    let idx1 = _mm_add_epi32(idx2, _mm_set1_epi32(1));

    // Clamp idx1 and idx2 to [0, colormap_len]
    let idx1 = _mm_min_epi32(idx1, colormap_len);
    let idx2 = _mm_max_epi32(idx2, _mm_setzero_si128());

    // Compute ratio = position - position_floor
    let ratio = _mm_sub_ps(position, position_floor);

    // Store idx1, idx2, and ratio into arrays
    let mut idx1_arr = Aligned::<A16, _>([MaybeUninit::<i32>::uninit(); 4]);
    let mut idx2_arr = Aligned::<A16, _>([MaybeUninit::<i32>::uninit(); 4]);
    let mut ratio_arr = Aligned::<A16, _>([MaybeUninit::<f32>::uninit(); 4]);
    _mm_store_si128(idx1_arr.as_mut_ptr() as _, idx1);
    _mm_store_si128(idx2_arr.as_mut_ptr() as _, idx2);
    _mm_store_ps(ratio_arr.as_mut_ptr() as _, ratio);
    let idx1_arr = mem::transmute::<_, Aligned<A16, [i32; 4]>>(idx1_arr);
    let idx2_arr = mem::transmute::<_, Aligned<A16, [i32; 4]>>(idx2_arr);
    let ratio_arr = mem::transmute::<_, Aligned<A16, [f32; 4]>>(ratio_arr);

    // Prepare output array
    let mut out = Aligned::<A16, _>([u8::MAX; 16]);

    // Process each of the 4 pixels
    for (chunk_value, idx1_scalar, idx2_scalar, ratio_scalar, out_chunk) in multizip((
        chunk_f32.into_iter(),
        idx1_arr.into_iter(),
        idx2_arr.into_iter(),
        ratio_arr.into_iter(),
        out.chunks_exact_mut(4),
    )) {
        if chunk_value == u16::MAX as f32 {
            continue;
        }
        if chunk_value == 0.0 {
            out_chunk[0] = 0;
            out_chunk[1] = 0;
            out_chunk[2] = 0;
            continue;
        }

        let idx1_scalar = idx1_scalar as usize;
        let idx2_scalar = idx2_scalar as usize;

        // Load colormap values
        let (color1_r, color1_g, color1_b) = if idx1_scalar <= 255 {
            (
                COLORMAP_R[idx1_scalar] as f32,
                COLORMAP_G[idx1_scalar] as f32,
                COLORMAP_B[idx1_scalar] as f32,
            )
        } else {
            (u8::MAX as f32, u8::MAX as f32, u8::MAX as f32)
        };

        let color2_r = COLORMAP_R[idx2_scalar] as f32;
        let color2_g = COLORMAP_G[idx2_scalar] as f32;
        let color2_b = COLORMAP_B[idx2_scalar] as f32;

        // Retrieve the ratio for this pixel
        let one_minus_ratio = 1.0 - ratio_scalar;

        // Interpolate colors
        let out_r = color1_r * ratio_scalar + color2_r * one_minus_ratio;
        let out_g = color1_g * ratio_scalar + color2_g * one_minus_ratio;
        let out_b = color1_b * ratio_scalar + color2_b * one_minus_ratio;

        // Store interpolated colors
        out_chunk[0] = out_r.round_ties_even() as u8;
        out_chunk[1] = out_g.round_ties_even() as u8;
        out_chunk[2] = out_b.round_ties_even() as u8;
    }

    out.into_iter()
}

/// slower than scalar version
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
pub unsafe fn map_grey_to_color_iter_sse41(grey: &[u16]) -> impl Iterator<Item = u8> + use<'_> {
    use std::arch::x86_64::*;

    use aligned::{A16, Aligned};

    let grey_to_pos_sse41 = _mm_set1_ps(GREY_TO_POS);
    let colormap_len_sse41 = _mm_set1_epi32(COLORMAP_R.len() as i32);

    let grey_sse41 = grey.chunks_exact(4);
    let grey_fallback = grey_sse41.remainder();
    grey_sse41
        .flat_map(move |chunk| {
            let mut chunk_iter = chunk.iter().map(|&x| x as f32);
            let chunk_f32 = Aligned::<A16, _>([(); 4].map(|_| chunk_iter.next().unwrap()));
            map_grey_to_color_sse41(chunk_f32, grey_to_pos_sse41, colormap_len_sse41)
        })
        .chain(map_grey_to_color_iter_fallback(grey_fallback))
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn map_grey_to_color_avx2(
    chunk_f32: Aligned<A32, [f32; 8]>,
    grey_to_pos: __m256,
    colormap_len: __m256i,
) -> impl Iterator<Item = u8> {
    use std::arch::x86_64::*;

    let chunk_simd = _mm256_load_ps(chunk_f32.as_ptr());
    let position = _mm256_fmsub_ps(chunk_simd, grey_to_pos, grey_to_pos);
    let position_floor = _mm256_floor_ps(position);
    let idx2 = _mm256_cvtps_epi32(position_floor);
    let idx1 = _mm256_add_epi32(idx2, _mm256_set1_epi32(1));
    let idx2 = _mm256_min_epi32(idx2, colormap_len);
    let idx1 = _mm256_max_epi32(idx1, _mm256_setzero_si256());
    let ratio = _mm256_sub_ps(position, position_floor);

    // dbg!(position_floor);
    // let mut tmp = [0i32; 8];
    // _mm256_storeu_si256(tmp.as_mut_ptr() as _, idx2);
    // println!("idx2: {:?}", tmp);
    // _mm256_storeu_si256(tmp.as_mut_ptr() as _, idx1);
    // println!("idx1: {:?}", tmp);
    // dbg!(ratio);

    let mask1 = _mm256_castsi256_ps(_mm256_cmpgt_epi32(colormap_len, idx1));
    let white = _mm256_set1_ps(u8::MAX as f32);
    let rgb1 = [
        _mm256_mask_i32gather_ps::<4>(white, COLORMAP_R.as_ptr(), idx1, mask1),
        _mm256_mask_i32gather_ps::<4>(white, COLORMAP_G.as_ptr(), idx1, mask1),
        _mm256_mask_i32gather_ps::<4>(white, COLORMAP_B.as_ptr(), idx1, mask1),
    ];

    let mask2 = _mm256_castsi256_ps(_mm256_cmpgt_epi32(idx2, _mm256_set1_epi32(-1)));
    let black = _mm256_setzero_ps();
    let rgb2 = [
        _mm256_mask_i32gather_ps::<4>(black, COLORMAP_R.as_ptr(), idx2, mask2),
        _mm256_mask_i32gather_ps::<4>(black, COLORMAP_G.as_ptr(), idx2, mask2),
        _mm256_mask_i32gather_ps::<4>(black, COLORMAP_B.as_ptr(), idx2, mask2),
    ];

    let mask = _mm256_castps_si256(_mm256_cmp_ps::<_CMP_NEQ_UQ>(
        chunk_simd,
        _mm256_setzero_ps(),
    ));
    let mask_white = _mm256_castps_si256(_mm256_cmp_ps::<_CMP_EQ_UQ>(
        chunk_simd,
        _mm256_set1_ps(u16::MAX as f32),
    ));
    let white = _mm256_set1_epi32(u8::MAX as i32);
    let mut out_r8g8b8 = Aligned::<A32, _>([0; 24]);
    for (out_chunk, color1, color2) in multizip((out_r8g8b8.chunks_exact_mut(8), rgb1, rgb2)) {
        let x = _mm256_fnmadd_ps(color2, ratio, color2);
        let out_f32 = _mm256_fmadd_ps(ratio, color1, x);
        // dbg!(out_f32);
        let out = _mm256_cvtps_epi32(_mm256_round_ps::<0>(out_f32));
        _mm256_maskstore_epi32(out_chunk.as_mut_ptr() as _, mask, out);
        _mm256_maskstore_epi32(out_chunk.as_mut_ptr() as _, mask_white, white);
    }
    (0..8).cartesian_product(0..4).map(move |(i, j)| {
        if j < 3 {
            out_r8g8b8[j * 8 + i] as u8
        } else {
            u8::MAX
        }
    })
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn map_grey_to_color_iter_avx2(grey: &[u16]) -> impl Iterator<Item = u8> + use<'_> {
    use std::arch::x86_64::*;

    let grey_to_pos_avx2 = _mm256_set1_ps(GREY_TO_POS);
    let colormap_len_avx2 = _mm256_set1_epi32(COLORMAP_R.len() as i32);
    let grey_to_pos_sse41 = _mm_set1_ps(GREY_TO_POS);
    let colormap_len_sse41 = _mm_set1_epi32(COLORMAP_R.len() as i32);

    let grey_avx2 = grey.chunks_exact(8);
    let grey_remainder = grey_avx2.remainder();
    let grey_sse41 = grey_remainder.chunks_exact(4);
    let grey_fallback = grey_sse41.remainder();
    grey_avx2
        .flat_map(move |chunk| {
            let mut chunk_iter = chunk.iter().map(|&x| x as f32);
            let chunk_f32 = Aligned::<A32, _>([(); 8].map(|_| chunk_iter.next().unwrap()));
            map_grey_to_color_avx2(chunk_f32, grey_to_pos_avx2, colormap_len_avx2)
        })
        .chain(grey_sse41.flat_map(move |chunk| {
            let mut chunk_iter = chunk.iter().map(|&x| x as f32);
            let chunk_f32 = Aligned::<A16, _>([(); 4].map(|_| chunk_iter.next().unwrap()));
            map_grey_to_color_sse41(chunk_f32, grey_to_pos_sse41, colormap_len_sse41)
        }))
        .chain(map_grey_to_color_iter_fallback(grey_fallback))
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn map_grey_to_color_neon(
    chunk_f32: Aligned<A16, [f32; 4]>,
    grey_to_pos: float32x4_t,
    colormap_len: int32x4_t,
) -> impl Iterator<Item = u8> {
    unsafe {
        use std::arch::aarch64::*;

        // Load the chunk into a NEON vector
        let chunk_simd = vld1q_f32(chunk_f32.as_ptr());

        // Correct computation of position
        let position = vsubq_f32(vmulq_f32(chunk_simd, grey_to_pos), grey_to_pos);

        // Floor the position to get idx2 and add 1 to get idx1
        let position_floor = vrndmq_f32(position);
        let idx2 = vcvtq_s32_f32(position_floor);
        let idx1 = vaddq_s32(idx2, vdupq_n_s32(1));

        // Clamp indices to colormap bounds
        let zero = vdupq_n_s32(0);
        let max_idx = vsubq_s32(colormap_len, vdupq_n_s32(1));
        let idx2 = vminq_s32(vmaxq_s32(idx2, zero), max_idx);
        let idx1 = vminq_s32(idx1, colormap_len);

        // Compute the ratio
        let ratio = vsubq_f32(position, position_floor);

        // Masks for special cases
        let mask_zero = vceqq_f32(chunk_simd, vdupq_n_f32(0.0));
        let mask_max = vceqq_f32(chunk_simd, vdupq_n_f32(u16::MAX as f32));
        let mask_normal = vmvnq_u32(vorrq_u32(mask_zero, mask_max));

        // Prepare output arrays
        let mut out_r8g8b8 = [0u8; 12]; // 4 pixels x 3 color channels

        // Process each color channel
        for c in 0..3 {
            // Assuming COLORMAP_R, COLORMAP_G, COLORMAP_B are &[f32]
            let colormap = match c {
                0 => COLORMAP_R,
                1 => COLORMAP_G,
                _ => COLORMAP_B,
            };

            // Emulate gather operation for idx1 and idx2
            let idx1_array = [
                vgetq_lane_s32::<0>(idx1),
                vgetq_lane_s32::<1>(idx1),
                vgetq_lane_s32::<2>(idx1),
                vgetq_lane_s32::<3>(idx1),
            ];
            let idx2_array = [
                vgetq_lane_s32::<0>(idx2),
                vgetq_lane_s32::<1>(idx2),
                vgetq_lane_s32::<2>(idx2),
                vgetq_lane_s32::<3>(idx2),
            ];

            // Load colors for idx1 and idx2
            let mut color1 = vdupq_n_f32(u8::MAX as f32);
            if idx1_array[0] <= u8::MAX as i32 {
                color1 = vsetq_lane_f32::<0>(colormap[idx1_array[0] as usize], color1);
            }
            if idx1_array[1] <= u8::MAX as i32 {
                color1 = vsetq_lane_f32::<1>(colormap[idx1_array[1] as usize], color1);
            }
            if idx1_array[2] <= u8::MAX as i32 {
                color1 = vsetq_lane_f32::<2>(colormap[idx1_array[2] as usize], color1);
            }
            if idx1_array[3] <= u8::MAX as i32 {
                color1 = vsetq_lane_f32::<3>(colormap[idx1_array[3] as usize], color1);
            }

            let color2 = vsetq_lane_f32::<0>(colormap[idx2_array[0] as usize], vdupq_n_f32(0.0));
            let color2 = vsetq_lane_f32::<1>(colormap[idx2_array[1] as usize], color2);
            let color2 = vsetq_lane_f32::<2>(colormap[idx2_array[2] as usize], color2);
            let color2 = vsetq_lane_f32::<3>(colormap[idx2_array[3] as usize], color2);

            // Compute interpolated color
            let interpolated = vmlaq_f32(
                vmulq_f32(color1, ratio),
                color2,
                vsubq_f32(vdupq_n_f32(1.0), ratio),
            );

            // Apply masks
            let masked_color = vbslq_f32(mask_normal, interpolated, vdupq_n_f32(0.0));
            let masked_color = vbslq_f32(mask_max, vdupq_n_f32(255.0), masked_color);

            // Convert to u8 and store in the output array
            let color_u32 = vcvtq_u32_f32(vrndaq_f32(masked_color));
            let color_u16 = vmovn_u32(color_u32);
            let color_u8 = vmovn_u16(vcombine_u16(color_u16, vdup_n_u16(0)));

            // Store the color components
            vst1_lane_u8::<0>(&mut out_r8g8b8[c * 4] as _, color_u8);
            vst1_lane_u8::<1>(&mut out_r8g8b8[c * 4 + 1] as _, color_u8);
            vst1_lane_u8::<2>(&mut out_r8g8b8[c * 4 + 2] as _, color_u8);
            vst1_lane_u8::<3>(&mut out_r8g8b8[c * 4 + 3] as _, color_u8);
        }

        // Create an iterator over the output pixels
        (0..4).cartesian_product(0..4).map(move |(i, j)| {
            if j < 3 {
                out_r8g8b8[j * 4 + i]
            } else {
                u8::MAX
            }
        })
    }
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
pub unsafe fn map_grey_to_color_iter_neon(grey: &[u16]) -> impl Iterator<Item = u8> + use<'_> {
    unsafe {
        use std::arch::aarch64::*;

        let grey_to_pos_neon = vdupq_n_f32(GREY_TO_POS);
        let colormap_len_neon = vdupq_n_s32(COLORMAP_R.len() as i32);

        let grey_neon = grey.chunks_exact(4);
        let grey_remainder = grey_neon.remainder();
        let grey_fallback = grey_remainder;
        grey_neon
            .flat_map(move |chunk| {
                let mut chunk_iter = chunk.iter().map(|&x| x as f32);
                let chunk_f32 = Aligned::<A16, _>([(); 4].map(|_| chunk_iter.next().unwrap()));
                map_grey_to_color_neon(chunk_f32, grey_to_pos_neon, colormap_len_neon)
            })
            .chain(map_grey_to_color_iter_fallback(grey_fallback))
    }
}

pub fn map_grey_to_color_iter(grey: &[u16]) -> Box<dyn Iterator<Item = u8> + '_> {
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::is_x86_feature_detected;

        if is_x86_feature_detected!("avx2") {
            return unsafe { Box::new(map_grey_to_color_iter_avx2(grey)) };
        } else if is_x86_feature_detected!("sse4.1") {
            return unsafe { Box::new(map_grey_to_color_iter_sse41(grey)) };
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        use std::arch::is_aarch64_feature_detected;

        if is_aarch64_feature_detected!("neon") {
            return unsafe { Box::new(map_grey_to_color_iter_neon(grey)) };
        }
    }
    Box::new(map_grey_to_color_iter_fallback(grey))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::{Duration, Instant};

    use fast_image_resize::images::{TypedImage, TypedImageRef};
    use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer, pixels};
    use image::RgbImage;
    use ndarray::prelude::*;
    use ndarray_rand::{RandomExt, rand_distr::Uniform};

    #[test]
    fn show_colorbar() {
        let (width, height) = (50, 500);
        let colormap: Vec<pixels::U8x3> = multizip((COLORMAP_R, COLORMAP_G, COLORMAP_B))
            .rev()
            .map(|(r, g, b)| [r as u8, g as u8, b as u8])
            .map(pixels::U8x3::new)
            .collect();
        let src_image = TypedImageRef::new(1, colormap.len() as u32, colormap.as_slice()).unwrap();
        let mut dst_image = TypedImage::new(width, height);
        let options =
            ResizeOptions::new().resize_alg(ResizeAlg::Interpolation(FilterType::Bilinear));
        Resizer::new()
            .resize_typed(&src_image, &mut dst_image, &options)
            .unwrap();
        let dst_raw_vec = dst_image.pixels().iter().flat_map(|x| x.0).collect();
        RgbImage::from_raw(width, height, dst_raw_vec)
            .unwrap()
            .save("samples/colorbar.png")
            .unwrap();
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn grey_to_color_work_with_avx2() {
        use std::arch::is_x86_feature_detected;

        if !is_x86_feature_detected!("avx2") {
            return;
        }
        let mut sum_elapsed = Duration::ZERO;
        let mut sum_elapsed_avx2 = Duration::ZERO;
        for _ in 0..10 {
            let grey_arr = Array::random((149, 110), Uniform::new_inclusive(0, u16::MAX));
            let (grey, _) = grey_arr.into_raw_vec_and_offset();
            let grey_len = grey.len();
            let start_time = Instant::now();
            let rgba_avx2: Vec<_> = unsafe { map_grey_to_color_iter_avx2(&grey).collect() };
            sum_elapsed_avx2 += start_time.elapsed();
            let start_time = Instant::now();
            let rgba: Vec<_> = map_grey_to_color_iter_fallback(&grey).collect();
            sum_elapsed += start_time.elapsed();
            multizip((grey, rgba_avx2.chunks(4), rgba.chunks(4))).enumerate().for_each(|(i, (x, y_avx2, y))| {
                assert_eq!(
                    y_avx2, y,
                    "the difference between avx2 output {:?} and the answer {:?} is too large for the {}-th grey value {} (grey len: {})",
                    y_avx2, y, i, x, grey_len
                );
            });
        }
        println!(
            "AVX2 operations reduced {:.2} % of the elapsed duration.",
            100. - sum_elapsed_avx2.as_secs_f64() / sum_elapsed.as_secs_f64() * 100.
        );
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn grey_to_color_work_with_sse41() {
        use std::arch::is_x86_feature_detected;

        if !is_x86_feature_detected!("sse4.1") {
            return;
        }
        let mut sum_elapsed = Duration::ZERO;
        let mut sum_elapsed_sse41 = Duration::ZERO;
        for i in 0..10 {
            let grey_arr = Array::random((149, 110), Uniform::new_inclusive(0, u16::MAX));
            let (grey, _) = grey_arr.into_raw_vec_and_offset();
            let grey_len = grey.len();
            let start_time = Instant::now();
            let rgba_sse41: Vec<_> = unsafe { map_grey_to_color_iter_sse41(&grey).collect() };
            if i > 0 {
                sum_elapsed_sse41 += start_time.elapsed();
            }
            let start_time = Instant::now();
            let rgba: Vec<_> = map_grey_to_color_iter_fallback(&grey).collect();
            if i > 0 {
                sum_elapsed += start_time.elapsed();
            }
            multizip((grey, rgba_sse41.chunks(4), rgba.chunks(4))).enumerate().for_each(|(i, (x, y_avx2, y))| {
                assert_eq!(
                    y_avx2, y,
                    "the difference between sse4.1 output {:?} and the answer {:?} is too large for the {}-th grey value {} (grey len: {})",
                    y_avx2, y, i, x, grey_len
                );
            });
        }
        println!(
            "SSE4.1 operations reduced {:.2} % of the elapsed duration.",
            100. - sum_elapsed_sse41.as_secs_f64() / sum_elapsed.as_secs_f64() * 100.
        );
    }

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn grey_to_color_work_with_neon() {
        use std::arch::is_aarch64_feature_detected;

        if !is_aarch64_feature_detected!("neon") {
            return;
        }
        let mut sum_elapsed = Duration::ZERO;
        let mut sum_elapsed_neon = Duration::ZERO;
        for _ in 0..10 {
            let grey_arr = Array::random((149, 110), Uniform::new_inclusive(0, u16::MAX));
            let (grey, _) = grey_arr.into_raw_vec_and_offset();
            let grey_len = grey.len();
            let start_time = Instant::now();
            let rgba_neon: Vec<_> = unsafe { map_grey_to_color_iter_neon(&grey).collect() };
            sum_elapsed_neon += start_time.elapsed();
            let start_time = Instant::now();
            let rgba: Vec<_> = map_grey_to_color_iter_fallback(&grey).collect();
            sum_elapsed += start_time.elapsed();
            multizip((grey, rgba_neon.chunks(4), rgba.chunks(4))).enumerate().for_each(|(i, (x, y_neon, y))| {
                assert_eq!(
                    y_neon, y,
                    "the difference between neon output {:?} and the answer {:?} is too large for the {}-th grey value {} (grey len: {})",
                    y_neon, y, i, x, grey_len
                );
            });
        }
        println!(
            "neon operations reduced {:.2} % of the elapsed duration.",
            100. - sum_elapsed_neon.as_secs_f64() / sum_elapsed.as_secs_f64() * 100.
        );
    }
}
