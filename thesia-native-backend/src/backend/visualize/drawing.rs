use fast_image_resize::pixels;
use ndarray::prelude::*;

#[allow(non_snake_case)]
pub fn convert_spectrogram_to_img(
    spec: ArrayView2<f32>,
    i_freq_range: (usize, usize),
    dB_range: (f32, f32),
    colormap_length: Option<u32>,
) -> Array2<pixels::U16> {
    // spec: T x F
    // return: image with F x T
    let (i_freq_start, i_freq_end) = i_freq_range;
    let dB_span = dB_range.1 - dB_range.0;
    let width = spec.shape()[0];
    let height = i_freq_end - i_freq_start;
    let min_value =
        colormap_length.map_or(1, |l| ((u16::MAX as f64 / l as f64).round() as u16).max(1));
    let u16_span = (u16::MAX - min_value) as f32;
    Array2::from_shape_fn((height, width), |(i, j)| {
        let i_freq = i_freq_start + i;
        if i_freq < spec.raw_dim()[1] {
            let zero_to_one = (spec[[j, i_freq]] - dB_range.0) / dB_span;
            let u16_min_to_max = zero_to_one * u16_span + min_value as f32;
            pixels::U16::new(u16_min_to_max.round().clamp(0., u16::MAX as f32) as u16)
        } else {
            pixels::U16::new(0)
        }
    })
}
