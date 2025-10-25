use super::super::spectrogram::SpecSetting;

fn add_pre_post_margin(
    start: f64,
    length: f64,
    max_length: usize,
    margin: usize,
) -> (usize, usize, f64, f64) {
    let start_w_margin = start as isize - margin as isize;
    let len_w_margin = ((start + length).ceil() as isize + margin as isize - start_w_margin).max(0);

    let start_w_margin_clipped = start_w_margin.max(0) as usize;
    let len_w_margin_clipped =
        len_w_margin.min(max_length as isize - start_w_margin_clipped as isize) as usize;

    let pre_margin = start - start_w_margin_clipped as f64;
    let post_margin = len_w_margin_clipped as f64 - length;
    (
        start_w_margin_clipped,
        len_w_margin_clipped,
        pre_margin,
        post_margin,
    )
}

#[derive(Debug)]
#[readonly::make]
pub struct SpectrogramSliceArgs {
    pub px_per_sec: f64,
    pub left: usize,
    pub width: usize,
    pub top: usize,
    pub height: usize,
    pub left_margin: f64,
    pub right_margin: f64,
    pub top_margin: f64,
    pub bottom_margin: f64,
}

impl SpectrogramSliceArgs {
    pub fn new(
        n_frames: usize,
        n_freqs: usize,
        track_sec: f64,
        sec_range: (f64, f64),
        spec_hz_range: (f32, f32),
        hz_range: (f32, f32),
        margin_px: usize,
        spec_setting: &SpecSetting,
    ) -> Self {
        let px_per_sec = n_frames as f64 / track_sec;
        let left_f64 = sec_range.0 * px_per_sec;
        let width_f64 = ((sec_range.1 - sec_range.0) * px_per_sec).max(0.);

        let (left_w_margin_clipped, width_w_margin_clipped, left_margin, right_margin) =
            add_pre_post_margin(left_f64, width_f64, n_frames, margin_px);

        let (top_f64, height_f64) = {
            let top_f64 = spec_setting
                .freq_scale
                .hz_to_relative_freq(hz_range.0, spec_hz_range) as f64
                * n_freqs as f64;
            let bottom_f64 = spec_setting
                .freq_scale
                .hz_to_relative_freq(hz_range.1, spec_hz_range) as f64
                * n_freqs as f64;
            (top_f64, bottom_f64 - top_f64)
        };

        let (top_w_margin_clipped, height_w_margin_clipped, top_margin, bottom_margin) =
            add_pre_post_margin(top_f64, height_f64, n_freqs, margin_px);

        Self {
            px_per_sec,
            left: left_w_margin_clipped,
            width: width_w_margin_clipped,
            top: top_w_margin_clipped,
            height: height_w_margin_clipped,
            left_margin,
            right_margin,
            top_margin,
            bottom_margin,
        }
    }
}
