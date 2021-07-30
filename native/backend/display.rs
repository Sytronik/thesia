use std::iter;
use std::mem::MaybeUninit;
use std::time::Duration;
// use std::time::Instant;

use approx::abs_diff_ne;
use cached::proc_macro::cached;
use hhmmss::Hhmmss;
use ndarray::Slice;
use ndarray::{prelude::*, Data};
use ndarray_stats::QuantileExt;
use num_traits::Zero;
use resize::{self, formats::Gray, Pixel::GrayF32, Resizer};
use rgb::FromSlice;
use tiny_skia::{
    FillRule, IntRect, LineCap, Paint, PathBuilder, PixmapMut, PixmapPaint, PixmapRef, Rect,
    Stroke, Transform,
};

use super::mel;
use super::resample::FftResampler;
use super::stft::FreqScale;

pub type ResizeType = resize::Type;
pub type PlotAxis = Vec<(u32, String)>;
pub type RelativeAxis = Vec<(f32, String)>;

const BLACK: [u8; 3] = [0; 3];
const WHITE: [u8; 3] = [255; 3];
const THR_LONG_HEIGHT: f32 = 2.;
const THR_N_CONSEQ_LONG_H: usize = 5;
const WAV_STROKE_WIDTH: f32 = 1.75;
pub const COLORMAP: [[u8; 3]; 10] = [
    [0, 0, 4],
    [27, 12, 65],
    [74, 12, 107],
    [120, 28, 109],
    [165, 44, 96],
    [207, 68, 70],
    [237, 105, 37],
    [251, 155, 6],
    [247, 209, 61],
    [252, 255, 164],
];
pub const WAVECOLOR: [u8; 3] = [200, 21, 103];
pub const MAX_SIZE: u32 = 8192; // tiny-skia max size
pub const LARGE_WIDTH_SPLIT_HOP: usize = 7680;
pub const LARGE_WIDTH_OVERLAP_HALF: usize = (MAX_SIZE as usize - LARGE_WIDTH_SPLIT_HOP) / 2;
pub const RESAMPLE_TAIL: usize = 500;

const POSSIBLE_TEN_UNITS: [u32; 4] = [10, 20, 50, 100];

pub struct ArrWithSliceInfo<'a, A, D: Dimension> {
    arr: ArrayView<'a, A, D>,
    index: usize,
    length: usize,
}

impl<'a, A, D: Dimension> ArrWithSliceInfo<'a, A, D> {
    pub fn from(arr: ArrayView<'a, A, D>, (index, length): (isize, usize)) -> Self {
        let (index, length) =
            calc_effective_slice(index, length, arr.shape()[arr.ndim() - 1]).unwrap_or((0, 0));
        ArrWithSliceInfo { arr, index, length }
    }

    pub fn from_ref<S>(arr: &'a ArrayBase<S, D>, (index, length): (isize, usize)) -> Self
    where
        S: Data<Elem = A>,
    {
        let (index, length) =
            calc_effective_slice(index, length, arr.shape()[arr.ndim() - 1]).unwrap_or((0, 0));
        ArrWithSliceInfo {
            arr: arr.view(),
            index,
            length,
        }
    }

    pub fn entire<S>(arr: &'a ArrayBase<S, D>) -> Self
    where
        S: Data<Elem = A>,
    {
        ArrWithSliceInfo {
            arr: arr.view(),
            index: 0,
            length: arr.shape()[arr.ndim() - 1],
        }
    }

    pub fn get_sliced(&self) -> ArrayView<A, D> {
        self.arr.slice_axis(
            Axis(self.arr.ndim() - 1),
            Slice::new(
                self.index as isize,
                Some((self.index + self.length) as isize),
                1,
            ),
        )
    }

    pub fn get_sliced_with_tail(&self, tail: usize) -> ArrayView<A, D> {
        let end = (self.index + self.length + tail).min(self.arr.shape()[self.arr.ndim() - 1]);
        self.arr.slice_axis(
            Axis(self.arr.ndim() - 1),
            Slice::new(self.index as isize, Some(end as isize), 1),
        )
    }
}

#[inline]
fn interpolate(rgba1: &[u8], rgba2: &[u8], ratio: f32) -> Vec<u8> {
    rgba1
        .iter()
        .zip(rgba2)
        .map(|(&a, &b)| (ratio * a as f32 + (1. - ratio) * b as f32).round() as u8)
        .collect()
}

fn convert_grey_to_rgb(x: f32) -> Vec<u8> {
    if x < 0. {
        return BLACK.to_vec();
    }
    if x >= 1. {
        return WHITE.to_vec();
    }
    let position = x * COLORMAP.len() as f32;
    let index = position.floor() as usize;
    let rgba1 = if index >= COLORMAP.len() - 1 {
        &WHITE
    } else {
        &COLORMAP[index + 1]
    };
    interpolate(rgba1, &COLORMAP[index], position - index as f32)
}

pub fn convert_spec_to_grey(
    spec: ArrayView2<f32>,
    up_ratio: f32,
    max: f32,
    min: f32,
) -> Array2<f32> {
    // spec: T x F
    // return: grey image with F(inverted) x T
    let width = spec.shape()[0];
    let height = (spec.shape()[1] as f32 * up_ratio).round() as usize;
    let mut grey = Array2::uninit((height, width));
    for ((i, j), x) in grey.indexed_iter_mut() {
        if height - 1 - i < spec.raw_dim()[1] {
            *x = MaybeUninit::new((spec[[j, height - 1 - i]] - min) / (max - min));
        } else {
            *x = MaybeUninit::new(0.);
        }
    }
    unsafe { grey.assume_init() }
}

#[inline]
pub fn calc_effective_slice(
    index: isize,
    length: usize,
    total_length: usize,
) -> Option<(usize, usize)> {
    if index >= total_length as isize {
        None
    } else if index < 0 {
        let i_right = length as isize + index;
        if i_right <= 0 {
            None
        } else {
            Some((0, (i_right as usize).min(total_length)))
        }
    } else {
        Some((index as usize, length.min(total_length - index as usize)))
    }
}

pub fn colorize_grey_with_size(
    grey: ArrWithSliceInfo<f32, Ix2>,
    width: u32,
    height: u32,
    fast_resize: bool,
) -> Vec<u8> {
    // let start = Instant::now();
    let ArrWithSliceInfo {
        arr: grey,
        index: trim_left,
        length: trim_width,
    } = grey;
    let mut resizer = create_resizer(
        trim_width,
        grey.shape()[0],
        width as usize,
        height as usize,
        fast_resize,
    );
    let mut resized = vec![0f32; width as usize * height as usize];
    resizer
        .resize_stride(
            &grey.as_slice().unwrap()[trim_left..].as_gray(),
            grey.shape()[1],
            &mut resized.as_gray_mut(),
        )
        .unwrap();
    resized
        .into_iter()
        .flat_map(|x| {
            convert_grey_to_rgb(x)
                .into_iter()
                .chain(iter::once(u8::MAX))
        })
        .collect()
    // println!("drawing spec: {:?}", start.elapsed());
}

fn draw_wav_directly(wav_avg: &[f32], pixmap: &mut PixmapMut, paint: &Paint) {
    // println!("avg rendering. short height ratio: {}", n_short_height as f32 / width as f32);
    let path = {
        let mut pb = PathBuilder::new();
        pb.move_to(0., wav_avg[0]);
        for (x, &y) in wav_avg.iter().enumerate().skip(1) {
            pb.line_to(x as f32, y);
        }
        if wav_avg.len() == 1 {
            pb.line_to(0.999, wav_avg[0]);
        }
        pb.finish().unwrap()
    };

    let stroke = Stroke {
        width: WAV_STROKE_WIDTH,
        line_cap: LineCap::Round,
        ..Default::default()
    };
    pixmap.stroke_path(&path, paint, &stroke, Transform::identity(), None);
}

fn draw_wav_topbottom(
    top_envlop: &[f32],
    btm_envlop: &[f32],
    pixmap: &mut PixmapMut,
    paint: &Paint,
) {
    // println!("top-bottom rendering. short height ratio: {}", n_short_height as f32 / width as f32);
    let path = {
        let mut pb = PathBuilder::new();
        pb.move_to(0., top_envlop[0]);
        for (x, &y) in top_envlop.iter().enumerate().skip(1) {
            pb.line_to(x as f32, y);
        }
        for (x, &y) in btm_envlop.iter().enumerate().rev() {
            pb.line_to(x as f32, y);
        }
        pb.close();
        pb.finish().unwrap()
    };

    pixmap.fill_path(&path, paint, FillRule::Winding, Transform::identity(), None);
}

pub fn draw_wav_to(
    output: &mut [u8],
    wav: ArrWithSliceInfo<f32, Ix1>,
    width: u32,
    height: u32,
    amp_range: (f32, f32),
    alpha: Option<u8>,
) {
    // let start = Instant::now();
    let amp_to_px = |x: f32, clamp: bool| {
        let x = (amp_range.1 - x) * height as f32 / (amp_range.1 - amp_range.0);
        if clamp {
            x.clamp(0., height as f32)
        } else {
            x
        }
    };
    let samples_per_px = wav.length as f32 / width as f32;
    let over_zoomed = amp_range.1 - amp_range.0 < 1e-16;
    let need_upsampling = !over_zoomed && samples_per_px < 2.;
    let wav: CowArray<f32, Ix1> = if need_upsampling {
        let wav_tail = wav.get_sliced_with_tail(RESAMPLE_TAIL);
        let width_tail = (width as f32 * wav_tail.len() as f32 / wav.length as f32).round();
        let mut resampler = create_resampler(wav_tail.len(), width_tail as usize);
        let upsampled = resampler.resample(wav_tail).mapv(|x| amp_to_px(x, false));
        upsampled.slice_move(s![..width as usize]).into()
    } else {
        wav.get_sliced().into()
    };

    let alpha = alpha.unwrap_or(u8::MAX);
    let mut paint = Paint::default();
    let [r, g, b] = WAVECOLOR;
    paint.set_color_rgba8(r, g, b, alpha);
    paint.anti_alias = true;

    let num_split = if width < MAX_SIZE {
        1
    } else {
        ((width - MAX_SIZE) as f32 / LARGE_WIDTH_SPLIT_HOP as f32).ceil() as usize + 1
    };

    let mut out_arr =
        ArrayViewMut3::from_shape((height as usize, width as usize, 4), output).unwrap();
    for i in 0..num_split {
        let width_part = if i < num_split - 1 {
            MAX_SIZE
        } else {
            width - (i * LARGE_WIDTH_SPLIT_HOP) as u32
        };
        let i_wav =
            ((wav.len() * i * LARGE_WIDTH_SPLIT_HOP) as f32 / width as f32).round() as usize;
        let wavlen_part = (wav.len() as f32 * width_part as f32 / width as f32).round() as usize;
        let wav_part = wav.slice(s![i_wav..(i_wav + wavlen_part).min(wav.len())]);

        let mut owned_part = if num_split > 1 {
            Some(vec![0u8; (height * width_part * 4) as usize])
        } else {
            None
        };
        let out_part = owned_part
            .as_mut()
            .map_or_else(|| out_arr.as_slice_mut().unwrap(), |x| &mut x[..]);
        let mut pixmap = PixmapMut::from_bytes(out_part, width_part, height).unwrap();

        if over_zoomed {
            let rect = Rect::from_xywh(0., 0., width_part as f32, height as f32).unwrap();
            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
        } else if need_upsampling {
            draw_wav_directly(wav_part.as_slice().unwrap(), &mut pixmap, &paint);
        } else {
            let mut wav_slices = Vec::with_capacity(width_part as usize);
            let mut top_envlop = Vec::with_capacity(width_part as usize);
            let mut btm_envlop = Vec::with_capacity(width_part as usize);
            let mut n_conseq_long_h = 0usize;
            let mut max_n_conseq = 0usize;
            for i_px in 0..width_part {
                let i_start = ((i_px as f32 - 0.5) * samples_per_px).round().max(0.) as usize;
                let i_end =
                    (((i_px as f32 + 0.5) * samples_per_px).round() as usize).min(wav_part.len());
                let wav_slice = wav_part.slice(s![i_start..i_end]);
                let mut top = amp_to_px(*wav_slice.max_skipnan(), true);
                let mut bottom = amp_to_px(*wav_slice.min_skipnan(), true);
                let diff = THR_LONG_HEIGHT + top - bottom;
                if diff < 0. {
                    n_conseq_long_h += 1;
                } else {
                    max_n_conseq = max_n_conseq.max(n_conseq_long_h);
                    n_conseq_long_h = 0;
                    top -= diff / 2.;
                    bottom += diff / 2.;
                }
                wav_slices.push(wav_slice);
                top_envlop.push(top);
                btm_envlop.push(bottom);
            }
            max_n_conseq = max_n_conseq.max(n_conseq_long_h);
            if max_n_conseq > THR_N_CONSEQ_LONG_H {
                draw_wav_topbottom(&top_envlop, &btm_envlop, &mut pixmap, &paint);
            } else {
                let wav_avg: Vec<f32> = wav_slices
                    .into_iter()
                    .map(|wav_slice| amp_to_px(wav_slice.mean().unwrap(), false))
                    .collect();
                draw_wav_directly(&wav_avg, &mut pixmap, &paint);
            }
        }
        if let Some(part) = owned_part {
            let part =
                Array::from_shape_vec((height as usize, part.len() / height as usize / 4, 4), part)
                    .unwrap();
            let out_left = if i == 0 {
                0
            } else {
                i * LARGE_WIDTH_SPLIT_HOP + LARGE_WIDTH_OVERLAP_HALF
            };
            let out_right = if i < num_split - 1 {
                (i + 1) * LARGE_WIDTH_SPLIT_HOP + LARGE_WIDTH_OVERLAP_HALF
            } else {
                i * LARGE_WIDTH_SPLIT_HOP + width_part as usize
            };
            let part_left = if i == 0 { 0 } else { LARGE_WIDTH_OVERLAP_HALF };
            let part_right = part_left + out_right - out_left;
            out_arr
                .slice_mut(s![.., out_left..out_right, ..])
                .assign(&part.slice(s![.., part_left..part_right, ..]));
        }
    }
    // println!("drawing wav: {:?}", start.elapsed());
}

pub fn draw_blended_spec_wav(
    spec_grey: ArrWithSliceInfo<f32, Ix2>,
    wav: ArrWithSliceInfo<f32, Ix1>,
    width: u32,
    height: u32,
    amp_range: (f32, f32),
    blend: f64,
    fast_resize: bool,
) -> Vec<u8> {
    // spec
    if spec_grey.length == 0 || wav.length == 0 {
        return vec![0u8; height as usize * width as usize * 4];
    }
    let mut result = if blend > 0. {
        colorize_grey_with_size(spec_grey, width, height, fast_resize)
    } else {
        vec![0u8; height as usize * width as usize * 4]
    };

    let mut pixmap = PixmapMut::from_bytes(&mut result, width, height).unwrap();

    if blend < 1. {
        // black
        if blend < 0.5 {
            let rect = IntRect::from_xywh(0, 0, width, height).unwrap().to_rect();
            let mut paint = Paint::default();
            paint.set_color_rgba8(0, 0, 0, (u8::MAX as f64 * (1. - 2. * blend)).round() as u8);
            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
        }

        let alpha = (u8::MAX as f64 * (2. - 2. * blend).min(1.)).round() as u8;
        // wave
        draw_wav_to(
            pixmap.data_mut(),
            wav,
            width,
            height,
            amp_range,
            Some(alpha),
        );
    }
    result
}

pub fn blend(
    spec_img: &[u8],
    wav_img: &[u8],
    width: u32,
    height: u32,
    blend: f64,
    eff_l_w: Option<(u32, u32)>,
) -> Vec<u8> {
    assert!(0. < blend && blend < 1.);
    let mut result = spec_img.to_vec();
    let mut pixmap = PixmapMut::from_bytes(&mut result, width, height).unwrap();
    // black
    if let Some((left, width)) = eff_l_w {
        if blend < 0.5 && width > 0 {
            let rect = IntRect::from_xywh(left as i32, 0, width, height)
                .unwrap()
                .to_rect();
            let mut paint = Paint::default();
            paint.set_color_rgba8(0, 0, 0, (u8::MAX as f64 * (1. - 2. * blend)).round() as u8);
            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
        }
    }
    {
        let paint = PixmapPaint {
            opacity: (2. - 2. * blend).min(1.) as f32,
            ..Default::default()
        };
        pixmap.draw_pixmap(
            0,
            0,
            PixmapRef::from_bytes(wav_img, width, height).unwrap(),
            &paint,
            Transform::identity(),
            None,
        );
    }
    result
}

pub fn create_time_axis(
    width: u32,
    start_sec: f64,
    px_per_sec: f64,
    tick_unit: f64,
    label_interval: u32,
) -> PlotAxis {
    let first_unit = (start_sec / tick_unit).ceil() as u32;
    let last_unit = ((start_sec + width as f64 / px_per_sec) / tick_unit).ceil() as u32;
    (first_unit..last_unit)
        .map(|unit| {
            let sec = unit as f64 * tick_unit;
            let x = (((sec - start_sec) * px_per_sec).round().max(0.) as u32).min(width);
            let s = if unit % label_interval == 0 {
                let millis = (sec * 1000.).round() as u64;
                let dur = Duration::from_millis(millis);
                if millis % 1000 == 0 {
                    dur.hhmmss()
                } else {
                    dur.hhmmssxxx()
                }
            } else {
                String::new()
            };
            (x, s)
        })
        .collect()
}

#[cached(size = 3)]
pub fn create_freq_axis(
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

pub fn create_amp_axis(
    height: u32,
    max_num_ticks: u32,
    max_num_labels: u32,
    amp_range: (f32, f32),
) -> PlotAxis {
    // TODO: max_num_labels
    assert!(amp_range.1 > amp_range.0);
    assert!(max_num_ticks >= 3);
    if abs_diff_ne!(amp_range.0, -amp_range.1) {
        unimplemented!()
    }
    if max_num_ticks % 2 != 1 {
        unimplemented!()
    }
    let n_ticks_half = (max_num_ticks - 1) / 2;
    let half_axis = create_linear_axis(0., amp_range.1, n_ticks_half + 1);
    let positive_half_axis = half_axis.iter().map(|(y_ratio, s)| {
        let y = (height as f32 * y_ratio / 2.).round() as u32;
        (y.max(0), s.clone())
    });
    let negative_half_axis =
        half_axis
            .iter()
            .take(n_ticks_half as usize)
            .rev()
            .map(|(y_ratio, s)| {
                let y = (height as f32 * (1. - y_ratio / 2.)).round() as u32;
                (y.min(height), format!("-{}", s))
            });

    positive_half_axis.chain(negative_half_axis).collect()
}

pub fn create_db_axis(
    height: u32,
    max_num_ticks: u32,
    max_num_labels: u32,
    db_range: (f32, f32),
) -> PlotAxis {
    // TODO: max_num_labels
    assert!(db_range.1 > db_range.0);
    assert!(max_num_ticks >= 2);
    create_linear_axis(db_range.0, db_range.1, max_num_ticks)
        .into_iter()
        .map(|(y_ratio, s)| {
            let y = (height as f32 * y_ratio).round() as u32;
            (y.clamp(0, height), s)
        })
        .collect()
}

#[inline]
pub fn get_colormap_rgba() -> Vec<u8> {
    COLORMAP
        .iter()
        .flat_map(|x| x.iter().cloned().chain(iter::once(u8::MAX)))
        .collect()
}

#[cached(size = 64)]
fn create_resizer(
    src_width: usize,
    src_height: usize,
    dest_width: usize,
    dest_height: usize,
    fast_resize: bool,
) -> Resizer<Gray<f32, f32>> {
    resize::new(
        src_width,
        src_height,
        dest_width,
        dest_height,
        GrayF32,
        if fast_resize {
            ResizeType::Point
        } else {
            ResizeType::Lanczos3
        },
    )
    .unwrap()
}

#[cached(size = 64)]
fn create_resampler(input_size: usize, output_size: usize) -> FftResampler<f32> {
    FftResampler::new(input_size, output_size)
}

fn create_linear_axis(min: f32, max: f32, max_num_ticks: u32) -> RelativeAxis {
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
    use image::RgbImage;
    use resize::Pixel::RGB8;

    #[test]
    fn show_colorbar() {
        let (width, height) = (50, 500);
        let colormap: Vec<u8> = COLORMAP.iter().rev().flatten().cloned().collect();
        let mut imvec = vec![0u8; width * height * 3];
        let mut resizer = resize::new(1, 10, width, height, RGB8, ResizeType::Triangle).unwrap();
        resizer
            .resize(&colormap.as_rgb(), imvec.as_rgb_mut())
            .unwrap();

        RgbImage::from_raw(width as u32, height as u32, imvec)
            .unwrap()
            .save("samples/colorbar.png")
            .unwrap();
    }

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
        let assert_axis_eq = |a: &[(u32, String)], b: &[(u32, &str)]| {
            a.into_iter()
                .zip(b.into_iter())
                .for_each(|((y0, s0), (y1, s1))| {
                    assert_eq!(y0, y1);
                    assert_eq!(s0, s1);
                });
        };
        assert_axis_eq(
            &create_db_axis(100, 2, 2, (-100., 0.)),
            &vec![(0, "0"), (100, "-100")],
        );
        assert_axis_eq(
            &create_db_axis(12, 3, 3, (-12., 0.)),
            &vec![(0, "0"), (5, "-5"), (10, "-10")],
        );
        assert_axis_eq(
            &create_db_axis(90, 3, 3, (-2., -1.1)),
            &vec![(40, "-1.5"), (90, "-2.0")],
        );
    }
}
