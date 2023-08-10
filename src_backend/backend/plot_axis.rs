use std::iter;

use approx::abs_diff_ne;
use cached::proc_macro::cached;
use chrono::naive::NaiveTime;
use num_traits::Zero;

use super::spectrogram::FreqScale;
use super::{mel, TrackManager};

pub type PlotAxis = Vec<(i32, String)>;
pub type RelativeAxis = Vec<(f32, String)>;

const POSSIBLE_TEN_UNITS: [u32; 4] = [10, 20, 50, 100];

pub trait PlotAxisCreator {
    fn create_time_axis(
        &self,
        width: u32,
        start_sec: f64,
        px_per_sec: f64,
        tick_unit: f64,
        label_interval: u32,
    ) -> PlotAxis;

    fn create_freq_axis(&self, height: u32, max_num_ticks: u32, max_num_labels: u32) -> PlotAxis;

    fn create_db_axis(&self, height: u32, max_num_ticks: u32, max_num_labels: u32) -> PlotAxis;

    fn create_amp_axis(
        height: u32,
        max_num_ticks: u32,
        max_num_labels: u32,
        amp_range: (f32, f32),
    ) -> PlotAxis {
        calc_amp_axis(height, max_num_ticks, max_num_labels, amp_range)
    }
}

impl PlotAxisCreator for TrackManager {
    fn create_time_axis(
        &self,
        width: u32,
        start_sec: f64,
        px_per_sec: f64,
        tick_unit: f64,
        label_interval: u32,
    ) -> PlotAxis {
        create_time_axis(
            width,
            start_sec,
            px_per_sec,
            tick_unit,
            label_interval,
            self.tracklist.max_sec,
        )
    }

    fn create_freq_axis(&self, height: u32, max_num_ticks: u32, max_num_labels: u32) -> PlotAxis {
        create_freq_axis(
            self.setting.freq_scale,
            self.max_sr,
            max_num_ticks,
            max_num_labels,
        )
        .into_iter()
        .map(|(relative_y, s)| {
            let y = (relative_y * (height as f32)).round() as i32;
            (y, s)
        })
        .collect()
    }

    fn create_db_axis(&self, height: u32, max_num_ticks: u32, max_num_labels: u32) -> PlotAxis {
        calc_db_axis(
            height,
            max_num_ticks,
            max_num_labels,
            (self.min_db, self.max_db),
        )
    }
}

fn create_time_axis(
    width: u32,
    start_sec: f64,
    px_per_sec: f64,
    tick_unit: f64,
    label_interval: u32,
    max_sec: f64,
) -> PlotAxis {
    let first_unit = (start_sec / tick_unit).ceil() as u32;
    // The label just before start_sec (at negative coordinate) should be drawn.
    let first_unit = if first_unit > label_interval {
        first_unit - label_interval
    } else {
        0
    };
    let last_unit = ((start_sec + width as f64 / px_per_sec) / tick_unit).ceil() as u32;
    let label_unit = tick_unit * label_interval as f64;
    let (hms_format, hms_display) = if max_sec > 3599. {
        ("%H:%M:%S", "hh:mm:ss")
    } else if max_sec > 59. {
        ("%M:%S", "mm:ss")
    } else {
        ("%S", "ss")
    };
    let (milli_format, n_mod, milli_display);
    if label_unit > 0.999 {
        milli_format = "";
        milli_display = "";
        n_mod = 1;
    } else {
        milli_format = "%.3f";
        if label_unit > 0.099 {
            n_mod = 100;
            milli_display = ".x";
        } else if label_unit > 0.009 {
            n_mod = 10;
            milli_display = ".xx";
        } else {
            n_mod = 1;
            milli_display = ".xxx";
        };
    }

    let time_format = format!("{}{}", hms_format, milli_format);
    let elem_format_display = (i32::MIN, format!("{}{}", hms_display, milli_display));
    (first_unit..last_unit)
        .map(|unit| {
            let sec = unit as f64 * tick_unit;
            let x = (((sec - start_sec) * px_per_sec).round() as i32).min(width as i32);
            let s = if unit % label_interval == 0 {
                let sec_u32 = sec.floor() as u32;
                let nano = if milli_format.is_empty() {
                    0
                } else {
                    let milli = ((sec - sec_u32 as f64) * 1000.).round() as u32 / n_mod * n_mod;
                    milli * 1000_000
                };
                let mut result = NaiveTime::from_num_seconds_from_midnight_opt(sec_u32, nano)
                    .unwrap()
                    .format(&time_format)
                    .to_string();
                if time_format.starts_with("%S") && sec_u32 < 10 {
                    result = result.replacen("0", "", 1);
                }
                if milli_format.is_empty() {
                    result
                } else {
                    result.trim_end_matches("0").trim_end_matches(".").into()
                }
            } else {
                String::new()
            };
            (x, s)
        })
        .chain(iter::once(elem_format_display))
        .collect()
}

#[cached(size = 3)]
fn create_freq_axis(
    freq_scale: FreqScale,
    sr: u32,
    max_num_ticks: u32,
    max_num_labels: u32,
) -> RelativeAxis {
    // TODO: max_num_labels
    fn coarse_band(fine_band: f32) -> f32 {
        if fine_band <= 100. {
            100.
        } else if fine_band <= 200. {
            200.
        } else if fine_band <= 500. {
            500.
        } else {
            (fine_band / 1000.).ceil() * 1000.
        }
    }

    let mut result = Vec::with_capacity(max_num_ticks as usize);
    result.push((1., freq_to_str(0.)));
    let max_freq = sr as f32 / 2.;

    if max_num_ticks >= 3 {
        match freq_scale {
            FreqScale::Mel if max_freq > 1000. => {
                let max_mel = mel::from_hz(max_freq);
                let mel_1k = mel::MIN_LOG_MEL as f32;
                let fine_band_mel = max_mel / (max_num_ticks as f32 - 1.);
                if max_num_ticks >= 4 && fine_band_mel <= mel_1k / 2. {
                    // divide [0, 1kHz] region
                    let fine_band = mel::to_hz(fine_band_mel);
                    let band = coarse_band(fine_band);
                    let mut freq = band;
                    let max_minus_band = 1000. - fine_band + 1.;
                    while freq < max_minus_band {
                        result.push((1. - mel::from_hz(freq) / max_mel, freq_to_str(freq)));
                        freq += band;
                    }
                }
                result.push((1. - mel_1k / max_mel, freq_to_str(1000.)));
                if max_num_ticks >= 4 {
                    // divide [1kHz, max_freq] region
                    let ratio_step =
                        2u32.pow((fine_band_mel / mel::MEL_DIFF_2K_1K).ceil().max(1.) as u32);
                    let mut freq = ratio_step as f32 * 1000.;
                    let mut mel_f = mel::from_hz(freq);
                    let max_mel_minus_band = max_mel - fine_band_mel + 0.01;
                    while mel_f < max_mel_minus_band {
                        result.push((1. - mel_f / max_mel, freq_to_str(freq)));
                        freq *= ratio_step as f32;
                        mel_f = mel::from_hz(freq);
                    }
                }
            }
            _ => {
                let fine_band = max_freq / (max_num_ticks as f32 - 1.);
                let band = coarse_band(fine_band);
                let mut freq = band;
                while freq < max_freq - fine_band + 1. {
                    result.push((1. - freq / max_freq, freq_to_str(freq)));
                    freq += band;
                }
            }
        }
    }

    result.push((0., freq_to_str(max_freq)));
    result
}

fn calc_amp_axis(
    height: u32,
    max_num_ticks: u32,
    max_num_labels: u32,
    amp_range: (f32, f32),
) -> PlotAxis {
    assert!(amp_range.1 > amp_range.0);
    assert!(max_num_ticks >= 3);
    if abs_diff_ne!(amp_range.0, -amp_range.1) {
        unimplemented!()
    }
    if max_num_ticks % 2 != 1 {
        unimplemented!()
    }
    let n_ticks_half = (max_num_ticks - 1) / 2;
    let half_axis = calc_linear_axis(0., amp_range.1, n_ticks_half + 1); // amp_range.1 ~ 0
    let half_len = half_axis.len();
    let half_axis: Vec<_> =
        omit_labels_from_linear_axis(half_axis.into_iter().rev(), half_len, max_num_labels)
            .collect(); // 0 ~ amp_range.1
    let positive_half_axis = half_axis.iter().rev().map(|(y_ratio, s)| {
        let y = (height as f32 * y_ratio / 2.).round() as i32;
        (y, s.clone())
    }); // 0 ~ amp_range.1
    let negative_half_axis = half_axis.iter().skip(1).map(|(y_ratio, s)| {
        let y = (height as f32 * (1. - y_ratio / 2.)).round() as i32;
        let s = if s.is_empty() {
            String::new()
        } else {
            format!("-{}", s)
        };
        (y, s)
    }); // 0 ~ amp_range.0

    positive_half_axis.chain(negative_half_axis).collect()
}

fn calc_db_axis(
    height: u32,
    max_num_ticks: u32,
    max_num_labels: u32,
    db_range: (f32, f32),
) -> PlotAxis {
    assert!(db_range.1 > db_range.0);
    assert!(max_num_ticks >= 2);
    let axis = calc_linear_axis(db_range.0, db_range.1, max_num_ticks);
    let len = axis.len();
    omit_labels_from_linear_axis(
        axis.into_iter()
            .map(|(y_ratio, s)| ((height as f32 * y_ratio).round() as i32, s)),
        len,
        max_num_labels,
    )
    .collect()
}

fn calc_linear_axis(min: f32, max: f32, max_num_ticks: u32) -> RelativeAxis {
    if max_num_ticks == 2 {
        return vec![
            (0., format_ticklabel(max, None)),
            (1., format_ticklabel(min, None)),
        ];
    }
    let raw_unit = (max - min) / (max_num_ticks - 1) as f32;
    let mut unit_exponent = raw_unit.log10().floor() as i32;
    let (ten_unit, unit, min_i, max_i) = POSSIBLE_TEN_UNITS
        .iter()
        .find_map(|&x| {
            let unit = x as f32 * 10f32.powi(unit_exponent - 1);
            let min_i = (min / unit).ceil() as i32;
            let max_i = (max / unit).floor() as i32;
            if max_i + 1 - min_i <= max_num_ticks as i32 {
                Some((x, unit, min_i, max_i))
            } else {
                None
            }
        })
        .unwrap();
    if ten_unit == 100 {
        unit_exponent += 1;
    }
    (min_i..=max_i)
        .rev()
        .map(|i| {
            let value = i as f32 * unit;
            let y_ratio = (max - value) / (max - min);
            (y_ratio, format_ticklabel(value, Some(unit_exponent)))
        })
        .collect()
}

fn omit_labels_from_linear_axis<Y>(
    iter: impl DoubleEndedIterator<Item = (Y, String)> + ExactSizeIterator,
    len: usize,
    max_num_labels: u32,
) -> impl DoubleEndedIterator<Item = (Y, String)> + ExactSizeIterator {
    let n_mod = (len as f32 / max_num_labels as f32).ceil() as usize;
    iter.enumerate().map(move |(i, (y, s))| -> (Y, String) {
        if i % n_mod == 0 && (len - 1 - i) >= n_mod || i == len - 1 {
            (y, s)
        } else {
            (y, String::new())
        }
    })
}

fn freq_to_str(freq: f32) -> String {
    let freq = freq.round().max(0.);
    let freq_int = freq as usize;
    if freq_int >= 1000 {
        if freq_int % 1000 == 0 {
            format!("{}k", freq_int / 1000)
        } else if freq_int % 100 == 0 {
            format!("{:.1}k", freq / 1000.)
        } else if freq_int % 10 == 0 {
            format!("{:.2}k", freq / 1000.)
        } else {
            format!("{:.3}k", freq / 1000.)
        }
    } else {
        format!("{}", freq_int)
    }
}

fn format_ticklabel(value: f32, unit_exponent: Option<i32>) -> String {
    if value.is_zero() {
        return String::from("0");
    }
    let exponent = value.abs().log10().floor() as i32;
    match unit_exponent {
        Some(unit_exponent) => {
            let rounded = (value * 10f32.powi(-unit_exponent)).round() * 10f32.powi(unit_exponent);
            let n_effs = (exponent - unit_exponent).max(0) as usize;
            if exponent <= -3 || exponent > 3 && unit_exponent > 0 {
                format!("{:.*e}", n_effs, rounded)
            } else {
                format!("{:.*}", (-unit_exponent).max(0) as usize, rounded)
            }
        }
        None => {
            if exponent <= -3 || exponent > 3 {
                format!("{:e}", value)
            } else {
                format!("{}", value)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use approx::assert_abs_diff_eq;

    #[test]
    fn freq_axis_works() {
        let assert_axis_eq = |a: &[(f32, String)], b: &[(f32, &str)]| {
            a.into_iter()
                .zip(b.into_iter())
                .for_each(|((y0, s0), (y1, s1))| {
                    assert_abs_diff_eq!(*y0, *y1);
                    assert_eq!(s0, s1);
                });
        };
        assert_axis_eq(
            &create_freq_axis(FreqScale::Linear, 24000, 2, 2),
            &vec![(1., "0"), (0., "12k")],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Linear, 24000, 8, 8),
            &vec![
                (1., "0"),
                (5. / 6., "2k"),
                (4. / 6., "4k"),
                (3. / 6., "6k"),
                (2. / 6., "8k"),
                (1. / 6., "10k"),
                (0., "12k"),
            ],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Linear, 24000, 24, 24)[..3],
            &vec![(1., "0"), (11. / 12., "1k"), (10. / 12., "2k")],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Linear, 24000, 25, 25)[..3],
            &vec![(1., "0"), (23. / 24., "500"), (22. / 24., "1k")],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Linear, 22050, 24, 24)[20..],
            &vec![
                (1. - 10000. / 11025., "10k"),
                (1. - 10500. / 11025., "10.5k"),
                (0., "11.025k"),
            ],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Mel, 24000, 2, 2),
            &vec![(1., "0"), (0., "12k")],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Mel, 24000, 3, 3),
            &vec![
                (1., "0"),
                (1. - mel::MIN_LOG_MEL as f32 / mel::from_hz(12000.), "1k"),
                (0., "12k"),
            ],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Mel, 3000, 4, 4),
            &vec![
                (1., "0"),
                (1. - mel::from_hz(500.) / mel::from_hz(1500.), "500"),
                (1. - mel::MIN_LOG_MEL as f32 / mel::from_hz(1500.), "1k"),
                (0., "1.5k"),
            ],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Mel, 24000, 8, 8),
            &vec![
                (1., "0"),
                (1. - mel::from_hz(500.) / mel::from_hz(12000.), "500"),
                (1. - mel::MIN_LOG_MEL as f32 / mel::from_hz(12000.), "1k"),
                (1. - mel::from_hz(2000.) / mel::from_hz(12000.), "2k"),
                (1. - mel::from_hz(4000.) / mel::from_hz(12000.), "4k"),
                (0., "12k"),
            ],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Mel, 96000, 6, 6),
            &vec![
                (1., "0"),
                (1. - mel::MIN_LOG_MEL as f32 / mel::from_hz(48000.), "1k"),
                (1. - mel::from_hz(4000.) / mel::from_hz(48000.), "4k"),
                (1. - mel::from_hz(16000.) / mel::from_hz(48000.), "16k"),
                (0., "48k"),
            ],
        );
    }

    #[test]
    fn db_axis_works() {
        let assert_axis_eq = |a: &[(i32, String)], b: &[(i32, &str)]| {
            a.into_iter()
                .zip(b.into_iter())
                .for_each(|((y0, s0), (y1, s1))| {
                    assert_eq!(y0, y1);
                    assert_eq!(s0, s1);
                });
        };
        assert_axis_eq(
            &calc_db_axis(100, 2, 2, (-100., 0.)),
            &vec![(0i32, "0"), (100, "-100")],
        );
        assert_axis_eq(
            &calc_db_axis(12, 3, 3, (-12., 0.)),
            &vec![(0i32, "0"), (5, "-5"), (10, "-10")],
        );
        assert_axis_eq(
            &calc_db_axis(90, 3, 3, (-2., -1.1)),
            &vec![(40i32, "-1.5"), (90, "-2.0")],
        );
    }
}
