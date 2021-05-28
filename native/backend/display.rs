use std::iter;
use std::mem::MaybeUninit;
// use std::time::Instant;

use cached::proc_macro::cached;
use ndarray::prelude::*;
use ndarray_stats::QuantileExt;
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
pub const MAX_SIZE: u32 = 8192;

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
pub fn calc_effective_w(i_w: isize, width: usize, total_width: usize) -> Option<(usize, usize)> {
    if i_w >= total_width as isize {
        None
    } else if i_w < 0 {
        let i_right = width as isize + i_w;
        if i_right <= 0 {
            None
        } else {
            Some((0, (i_right as usize).min(total_width)))
        }
    } else {
        Some((i_w as usize, width.min(total_width - i_w as usize)))
    }
}

pub fn colorize_grey_with_size(
    grey: ArrayView2<f32>,
    width: u32,
    height: u32,
    fast_resize: bool,
    grey_left_width: Option<(usize, usize)>,
) -> Vec<u8> {
    // let start = Instant::now();
    let (trim_left, grey_width) = match grey_left_width {
        Some(x) => x,
        None => (0, grey.shape()[1]),
    };
    let mut resizer = create_resizer(
        grey_width,
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
            &mut resized[..].as_gray_mut(),
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
            pb.line_to(0., wav_avg[0]);
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
    wav: ArrayView1<f32>,
    width: u32,
    height: u32,
    amp_range: (f32, f32),
    alpha: Option<u8>,
) {
    // let start = Instant::now();
    let alpha = match alpha {
        Some(x) => x,
        None => u8::MAX,
    };
    let mut pixmap = PixmapMut::from_bytes(output, width, height).unwrap();

    let mut paint = Paint::default();
    let [r, g, b] = WAVECOLOR;
    paint.set_color_rgba8(r, g, b, alpha);
    paint.anti_alias = true;
    if amp_range.1 - amp_range.0 < 1e-16 {
        let rect = Rect::from_xywh(0., 0., width as f32, height as f32).unwrap();
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
        return;
    }

    let amp_to_height_px = |x: f32| {
        ((amp_range.1 - x) * height as f32 / (amp_range.1 - amp_range.0))
            .max(0.)
            .min(height as f32)
    };
    let samples_per_px = wav.len() as f32 / width as f32;

    // need upsampling
    if samples_per_px < 2. {
        let mut resampler = create_resampler(wav.len(), width as usize);
        let upsampled = resampler.resample(wav).mapv(amp_to_height_px);
        draw_wav_directly(upsampled.as_slice().unwrap(), &mut pixmap, &paint);
        return;
    }
    let mut top_envlop = Vec::<f32>::with_capacity(width as usize);
    let mut btm_envlop = Vec::<f32>::with_capacity(width as usize);
    let mut wav_avg = Vec::<f32>::with_capacity(width as usize);
    let mut n_conseq_long_h = 0usize;
    let mut max_n_conseq = 0usize;
    for i_px in 0..width {
        let i_start = ((i_px as f32 - 0.5) * samples_per_px).round().max(0.) as usize;
        let i_end = (((i_px as f32 + 0.5) * samples_per_px).round() as usize).min(wav.len());
        let wav_slice = wav.slice(s![i_start..i_end]);
        let mut top = amp_to_height_px(*wav_slice.max_skipnan());
        let mut bottom = amp_to_height_px(*wav_slice.min_skipnan());
        let avg = amp_to_height_px(wav_slice.mean().unwrap());
        let diff = THR_LONG_HEIGHT + top - bottom;
        if diff < 0. {
            n_conseq_long_h += 1;
        } else {
            max_n_conseq = max_n_conseq.max(n_conseq_long_h);
            n_conseq_long_h = 0;
            top -= diff / 2.;
            bottom += diff / 2.;
        }
        top_envlop.push(top);
        btm_envlop.push(bottom);
        wav_avg.push(avg);
    }
    max_n_conseq = max_n_conseq.max(n_conseq_long_h);
    if max_n_conseq > THR_N_CONSEQ_LONG_H {
        draw_wav_topbottom(&top_envlop[..], &btm_envlop[..], &mut pixmap, &paint);
    } else {
        draw_wav_directly(&wav_avg[..], &mut pixmap, &paint);
    }
    // println!("drawing wav: {:?}", start.elapsed());
}

pub fn draw_blended_spec_wav(
    entire_spec_grey: ArrayView2<f32>,
    wav_slice: ArrayView1<f32>,
    width: u32,
    height: u32,
    grey_left_width: Option<(usize, usize)>,
    amp_range: (f32, f32),
    fast_resize: bool,
    blend: f64,
) -> Vec<u8> {
    // spec
    let mut result = if blend > 0. {
        colorize_grey_with_size(
            entire_spec_grey,
            width,
            height,
            fast_resize,
            grey_left_width,
        )
    } else {
        vec![0u8; width as usize * height as usize * 4]
    };

    let mut pixmap = PixmapMut::from_bytes(&mut result[..], width, height).unwrap();

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
            wav_slice,
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
    eff_l_w: (u32, u32),
) -> Vec<u8> {
    assert!(0. < blend && blend < 1.);
    let mut result = spec_img.to_vec();
    let mut pixmap = PixmapMut::from_bytes(&mut result[..], width, height).unwrap();
    // black
    if blend < 0.5 && eff_l_w.1 > 0 {
        let (left, width) = eff_l_w;
        let rect = IntRect::from_xywh(left as i32, 0, width, height)
            .unwrap()
            .to_rect();
        let mut paint = Paint::default();
        paint.set_color_rgba8(0, 0, 0, (u8::MAX as f64 * (1. - 2. * blend)).round() as u8);
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
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

#[cached(size = 3)]
pub fn create_freq_axis(freq_scale: FreqScale, sr: u32, max_ticks: u32) -> Vec<(f64, f64)> {
    fn coarse_band(fine_band: f64) -> f64 {
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

    let mut result = Vec::with_capacity(max_ticks as usize);
    result.push((1., 0.));
    let max_freq = sr as f64 / 2.;

    if max_ticks >= 3 {
        match freq_scale {
            FreqScale::Mel if max_freq > 1000. => {
                let max_mel = mel::from_hz(max_freq);
                let mel_1k = mel::MIN_LOG_MEL as f64;
                let fine_band_mel = max_mel / (max_ticks as f64 - 1.);
                if max_ticks >= 4 && fine_band_mel <= mel_1k / 2. {
                    // divide [0, 1kHz] region
                    let fine_band = mel::to_hz(fine_band_mel);
                    let band = coarse_band(fine_band);
                    let mut freq = band;
                    let max_minus_band = 1000. - fine_band + 1.;
                    while freq < max_minus_band {
                        result.push((1. - mel::from_hz(freq) / max_mel, freq));
                        freq += band;
                    }
                }
                result.push((1. - mel_1k / max_mel, 1000.));
                if max_ticks >= 4 {
                    // divide [1kHz, max_freq] region
                    let ratio_step =
                        2u32.pow((fine_band_mel / mel::MEL_DIFF_2K_1K).ceil().max(1.) as u32);
                    let mut freq = ratio_step as f64 * 1000.;
                    let mut mel_f = mel::from_hz(freq);
                    let max_mel_minus_band = max_mel - fine_band_mel + 0.01;
                    while mel_f < max_mel_minus_band {
                        result.push((1. - mel_f / max_mel, freq));
                        freq *= ratio_step as f64;
                        mel_f = mel::from_hz(freq);
                    }
                }
            }
            _ => {
                let fine_band = max_freq / (max_ticks as f64 - 1.);
                let band = coarse_band(fine_band);
                let mut freq = band;
                while freq < max_freq - fine_band + 1. {
                    result.push((1. - freq / max_freq, freq));
                    freq += band;
                }
            }
        }
    }

    result.push((0., max_freq));
    result
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
        let assert_axis_eq = |a: &[(f64, f64)], b: &[(f64, f64)]| {
            a.into_iter()
                .flat_map(|x| vec![x.0, x.1].into_iter())
                .zip(b.into_iter().flat_map(|x| vec![x.0, x.1].into_iter()))
                .for_each(|(x, y)| assert_abs_diff_eq!(x, y));
        };
        assert_axis_eq(
            &create_freq_axis(FreqScale::Linear, 24000, 2),
            &vec![(1., 0.), (0., 12000.)],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Linear, 24000, 8),
            &vec![
                (1., 0.),
                (5. / 6., 2000.),
                (4. / 6., 4000.),
                (3. / 6., 6000.),
                (2. / 6., 8000.),
                (1. / 6., 10000.),
                (0., 12000.),
            ],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Linear, 24000, 24)[..3],
            &vec![(1., 0.), (11. / 12., 1000.), (10. / 12., 2000.)],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Linear, 24000, 25)[..3],
            &vec![(1., 0.), (23. / 24., 500.), (22. / 24., 1000.)],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Linear, 22050, 24)[20..],
            &vec![
                (1. - 10000. / 11025., 10000.),
                (1. - 10500. / 11025., 10500.),
                (0., 11025.),
            ],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Mel, 24000, 2),
            &vec![(1., 0.), (0., 12000.)],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Mel, 24000, 3),
            &vec![
                (1., 0.),
                (1. - mel::MIN_LOG_MEL as f64 / mel::from_hz(12000.), 1000.),
                (0., 12000.),
            ],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Mel, 3000, 4),
            &vec![
                (1., 0.),
                (1. - mel::from_hz(500.) / mel::from_hz(1500.), 500.),
                (1. - mel::MIN_LOG_MEL as f64 / mel::from_hz(1500.), 1000.),
                (0., 1500.),
            ],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Mel, 24000, 8),
            &vec![
                (1., 0.),
                (1. - mel::from_hz(500.) / mel::from_hz(12000.), 500.),
                (1. - mel::MIN_LOG_MEL as f64 / mel::from_hz(12000.), 1000.),
                (1. - mel::from_hz(2000.) / mel::from_hz(12000.), 2000.),
                (1. - mel::from_hz(4000.) / mel::from_hz(12000.), 4000.),
                (0., 12000.),
            ],
        );
        assert_axis_eq(
            &create_freq_axis(FreqScale::Mel, 96000, 6),
            &vec![
                (1., 0.),
                (1. - mel::MIN_LOG_MEL as f64 / mel::from_hz(48000.), 1000.),
                (1. - mel::from_hz(4000.) / mel::from_hz(48000.), 4000.),
                (1. - mel::from_hz(16000.) / mel::from_hz(48000.), 16000.),
                (0., 48000.),
            ],
        );
    }
}
