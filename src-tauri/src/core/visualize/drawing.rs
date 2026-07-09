use ndarray::prelude::*;

#[allow(non_snake_case)]
pub fn convert_spectrogram_to_img(
    spec: &ArrayRef2<f32>,
    i_freq_range: (usize, usize),
    dB_range: (f32, f32),
    colormap_length: Option<u32>,
) -> Array2<u16> {
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
            u16_min_to_max.round().clamp(0., u16::MAX as f32) as u16
        } else {
            0
        }
    })
}

#[cfg(test)]
mod tests {
    use ndarray::array;

    use super::*;

    #[test]
    #[allow(non_snake_case)]
    fn spectrogram_to_img_transposes_and_clamps_dB_values() {
        let spec = array![[-100.0f32, -50.0, 0.0], [100.0, -200.0, -25.0]];
        let img = convert_spectrogram_to_img(&spec.view(), (0, 4), (-100.0, 0.0), Some(4));

        assert_eq!(img.shape(), &[4, 2]);
        assert_eq!(img[[0, 0]], 16_384);
        assert_eq!(img[[0, 1]], u16::MAX);
        assert_eq!(img[[1, 0]], 40_960);
        assert_eq!(img[[1, 1]], 0);
        assert_eq!(img[[2, 0]], u16::MAX);
        assert_eq!(img[[2, 1]], 53_247);
        assert_eq!(img[[3, 0]], 0);
        assert_eq!(img[[3, 1]], 0);
    }
}
