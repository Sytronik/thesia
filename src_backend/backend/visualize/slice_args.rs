use super::super::spectrogram::SpecSetting;

/// Heights of the overview
/// height (total) = ch + gap + ... + ch
/// ch = gain + ch_wo_gain + gain
#[readonly::make]
pub struct OverviewHeights {
    pub ch: f64,
    pub gap: f64,
    pub gain: f64,
    pub ch_wo_gain: f64,
}

impl OverviewHeights {
    pub fn new(height: f64, gap: f64, n_ch: usize, gain_height_ratio: f64) -> Self {
        let height_without_gap = height - gap * ((n_ch - 1) as f64);
        let ch = height_without_gap / n_ch as f64;
        let gain = ch * gain_height_ratio;
        let ch_wo_gain = ch - 2. * gain;
        OverviewHeights {
            ch,
            gap,
            gain,
            ch_wo_gain,
        }
    }
}

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

pub struct WavSliceArgs {
    pub start_w_margin: usize,
    pub length_w_margin: usize,
    pub start_w_margin_f64: f64,
    pub drawing_sec: f64,
    pub pre_margin_sec: f64,
    pub post_margin_sec: f64,
    pub total_len: usize,
}

impl WavSliceArgs {
    pub fn new(
        sr: u32,
        sec_range: (f64, f64),
        px_per_samples: f64,
        wav_len: usize,
        margin_ratio: f64,
    ) -> Self {
        let px_per_sec = px_per_samples * sr as f64;
        let start_px_f64 = sec_range.0 * px_per_sec;
        let end_px_f64 = sec_range.1 * px_per_sec;
        let length_px_f64 = end_px_f64 - start_px_f64;
        let margin = (length_px_f64 * margin_ratio).round() as usize;
        let (start_px_w_margin, length_px_w_margin, pre_margin_px, post_margin_px) =
            add_pre_post_margin(
                start_px_f64,
                length_px_f64,
                (wav_len as f64 * px_per_samples) as usize,
                margin,
            );

        let (start_w_margin, length_w_margin) = (
            (start_px_w_margin as f64 / px_per_samples).round() as usize,
            (length_px_w_margin as f64 / px_per_samples).round() as usize,
        ); // this rounding makes pre_margin_sec and post_margin_sec not accurate, but it's okay for human eyes
        let start_w_margin_f64 = start_px_w_margin as f64 / px_per_samples;
        let drawing_sec = length_px_w_margin as f64 / px_per_sec;
        let (pre_margin_sec, post_margin_sec) =
            (pre_margin_px / px_per_sec, post_margin_px / px_per_sec);
        Self {
            start_w_margin,
            length_w_margin,
            start_w_margin_f64,
            drawing_sec,
            pre_margin_sec,
            post_margin_sec,
            total_len: length_px_w_margin,
        }
    }
}

pub struct WavDrawingInfoSliceArgs {
    pub start_w_margin: usize,
    pub length_w_margin: usize,
    pub drawing_sec: f64,
    pub pre_margin_sec: f64,
    pub post_margin_sec: f64,
}

impl WavDrawingInfoSliceArgs {
    pub fn new(cache_len: usize, sec_range: (f64, f64), track_sec: f64, margin_ratio: f64) -> Self {
        let cache_len_f64 = cache_len as f64;

        let i_start = sec_range.0 / track_sec * cache_len_f64;
        let i_end = sec_range.1 / track_sec * cache_len_f64;
        let length_f64 = i_end - i_start;
        let margin = (length_f64 * margin_ratio).round() as usize;
        let (start_w_margin, length_w_margin, pre_margin, post_margin) =
            add_pre_post_margin(i_start, length_f64, cache_len, margin);

        let len_to_sec = track_sec / cache_len_f64;
        let drawing_sec = length_w_margin as f64 * len_to_sec;
        let (pre_margin_sec, post_margin_sec) = (pre_margin * len_to_sec, post_margin * len_to_sec);
        Self {
            start_w_margin,
            length_w_margin,
            drawing_sec,
            pre_margin_sec,
            post_margin_sec,
        }
    }
}
