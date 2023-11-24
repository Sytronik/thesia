use std::iter;
use std::mem::MaybeUninit;
// use std::time::Instant;

use cached::proc_macro::cached;
use napi_derive::napi;
use ndarray::prelude::*;
use ndarray_stats::QuantileExt;
use rayon::prelude::*;
use resize::{self, formats::Gray, Pixel::GrayF32, Resizer};
use rgb::FromSlice;
use serde::{Deserialize, Serialize};
use tiny_skia::{
    FillRule, IntRect, LineCap, Paint, PathBuilder, PixmapMut, PixmapPaint, PixmapRef, Rect,
    Stroke, Transform,
};

use super::img_slice::{ArrWithSliceInfo, CalcWidth, PartGreyInfo};
use super::resample::FftResampler;
use crate::backend::utils::Pad;
use crate::backend::{IdChArr, IdChMap, TrackManager};

pub type ResizeType = resize::Type;

const BLACK: [u8; 3] = [0; 3];
const WHITE: [u8; 3] = [255; 3];
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
pub const WAVECOLOR: [u8; 3] = [120, 150, 210];
pub const RESAMPLE_TAIL: usize = 500;
const THR_TOPBOTTOM_PERCENT: u32 = 70;
const OVERVIEW_CH_GAP_HEIGHT: f32 = 1.;

pub struct DprDependentConstants {
    thr_long_height: f32,
    topbottom_context_size: f32,
    wav_stroke_width: f32,
}

impl DprDependentConstants {
    fn calc(dpr: f32) -> Self {
        DprDependentConstants {
            thr_long_height: 2. * dpr,
            topbottom_context_size: 2. * dpr,
            wav_stroke_width: 1.75 * dpr,
        }
    }
}

#[napi(object)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DrawOption {
    pub px_per_sec: f64,
    pub height: u32,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub struct DrawOptionForWav {
    pub amp_range: (f32, f32),
    pub dpr: f32,
}

pub enum ImageKind {
    Spec,
    Wav(DrawOptionForWav),
}

pub trait TrackDrawer {
    fn draw_entire_imgs(
        &self,
        id_ch_tuples: &IdChArr,
        option: DrawOption,
        kind: ImageKind,
    ) -> IdChMap<Array3<u8>>;

    fn draw_part_imgs(
        &self,
        id_ch_tuples: &IdChArr,
        start_sec: f64,
        width: u32,
        option: DrawOption,
        opt_for_wav: DrawOptionForWav,
        blend: f64,
        fast_resize_vec: Option<Vec<bool>>,
    ) -> IdChMap<Vec<u8>>;

    fn draw_overview(&self, id: usize, width: u32, height: u32, dpr: f32) -> Vec<u8>;
}

impl TrackDrawer for TrackManager {
    fn draw_entire_imgs(
        &self,
        id_ch_tuples: &IdChArr,
        option: DrawOption,
        kind: ImageKind,
    ) -> IdChMap<Array3<u8>> {
        // let start = Instant::now();
        let DrawOption { px_per_sec, height } = option;
        let mut result = IdChMap::with_capacity(id_ch_tuples.len());
        result.par_extend(id_ch_tuples.par_iter().map(|&(id, ch)| {
            let out_for_not_exist = || ((id, ch), Array::zeros((0, 0, 0)));
            let track = if let Some(track) = self.track(id) {
                track
            } else {
                return out_for_not_exist();
            };
            let width = track.calc_width(px_per_sec);
            let shape = (height as usize, width as usize, 4);
            let arr = match kind {
                ImageKind::Spec => {
                    let grey = if let Some(grey) = self.spec_greys.get(&(id, ch)) {
                        grey.view()
                    } else {
                        return out_for_not_exist();
                    };
                    let vec = colorize_grey_with_size(
                        ArrWithSliceInfo::entire(grey),
                        width,
                        height,
                        false,
                    );
                    Array3::from_shape_vec(shape, vec).unwrap()
                }
                ImageKind::Wav(opt_for_wav) => {
                    let mut arr = Array3::zeros(shape);
                    draw_wav_to(
                        arr.as_slice_mut().unwrap(),
                        ArrWithSliceInfo::entire(track.channel(ch)),
                        width,
                        height,
                        &opt_for_wav,
                        None,
                    );
                    arr
                }
            };
            ((id, ch), arr)
        }));
        // println!("draw entire: {:?}", start.elapsed());
        result
    }

    /// Draw part of images. if blend < 0, draw waveform with transparent background
    fn draw_part_imgs(
        &self,
        id_ch_tuples: &IdChArr,
        start_sec: f64,
        width: u32,
        option: DrawOption,
        opt_for_wav: DrawOptionForWav,
        blend: f64,
        fast_resize_vec: Option<Vec<bool>>,
    ) -> IdChMap<Vec<u8>> {
        // let start = Instant::now();
        let DrawOption { px_per_sec, height } = option;
        let mut result = IdChMap::with_capacity(id_ch_tuples.len());
        let par_iter = id_ch_tuples.par_iter().enumerate().map(|(i, &(id, ch))| {
            let out_for_not_exist = || ((id, ch), Vec::new());
            let track = if let Some(track) = self.track(id) {
                track
            } else {
                return out_for_not_exist();
            };
            let spec_grey = if let Some(grey) = self.spec_greys.get(&(id, ch)) {
                grey
            } else {
                return out_for_not_exist();
            };
            let PartGreyInfo {
                i_w_and_width,
                start_sec_with_margin,
                width_with_margin,
            } = track.calc_part_grey_info(
                spec_grey.shape()[1] as u64,
                start_sec,
                width,
                px_per_sec,
            );

            let (pad_left, drawing_width_with_margin, pad_right) =
                track.decompose_width_of(start_sec_with_margin, width_with_margin, px_per_sec);
            if drawing_width_with_margin == 0 {
                return ((id, ch), vec![0u8; height as usize * width as usize * 4]);
            }

            let spec_grey_part = ArrWithSliceInfo::new(spec_grey.view(), i_w_and_width);
            let wav_part = ArrWithSliceInfo::new(
                track.channel(ch),
                track.calc_part_wav_info(start_sec_with_margin, width_with_margin, px_per_sec),
            );
            let vec = draw_blended_spec_wav(
                spec_grey_part,
                wav_part,
                drawing_width_with_margin,
                height,
                &opt_for_wav,
                blend,
                fast_resize_vec.as_ref().map_or(false, |v| v[i]),
            );
            let mut arr = Array3::from_shape_vec(
                (height as usize, drawing_width_with_margin as usize, 4),
                vec,
            )
            .unwrap();

            if width_with_margin != drawing_width_with_margin {
                arr = arr.pad(
                    (pad_left as usize, pad_right as usize),
                    Axis(1),
                    Default::default(),
                );
            }
            let margin_l = ((start_sec - start_sec_with_margin) * px_per_sec).round() as isize;
            arr.slice_collapse(s![.., margin_l..(margin_l + width as isize), ..]);
            let arr = if arr.is_standard_layout() {
                arr
            } else {
                arr.as_standard_layout().into_owned()
            };
            ((id, ch), arr.into_raw_vec())
        });
        result.par_extend(par_iter);

        // println!("draw: {:?}", start.elapsed());
        result
    }

    fn draw_overview(&self, id: usize, width: u32, height: u32, dpr: f32) -> Vec<u8> {
        let track = if let Some(track) = self.track(id) {
            track
        } else {
            return Vec::new();
        };
        let (pad_left, drawing_width, pad_right) =
            track.decompose_width_of(0., width, width as f64 / self.tracklist.max_sec);
        let (pad_left, drawing_width_usize, pad_right) = (
            pad_left as usize,
            drawing_width as usize,
            pad_right as usize,
        );
        let height = height as usize;
        let gap_h = (OVERVIEW_CH_GAP_HEIGHT * dpr).round() as usize;
        let height_without_gap = height - gap_h * (track.n_ch() - 1);
        let ch_h = height_without_gap / track.n_ch();
        let ch_h_u32 = ch_h as u32;
        let margin_top = height_without_gap % track.n_ch() / 2;
        let len_per_height = drawing_width_usize * 4; // RGBA
        let i_start = margin_top * len_per_height;
        let ch_vec_len = ch_h * len_per_height;
        let gap_vec_len = gap_h * len_per_height;
        let mut vec = vec![0u8; height * len_per_height];
        vec[i_start..]
            .par_chunks_mut(ch_vec_len + gap_vec_len)
            .enumerate()
            .for_each(|(ch, x)| {
                draw_wav_to(
                    &mut x[..ch_vec_len],
                    ArrWithSliceInfo::entire(track.channel(ch)),
                    drawing_width,
                    ch_h_u32,
                    &DrawOptionForWav {
                        amp_range: (-1., 1.),
                        dpr,
                    },
                    None,
                )
            });

        if width != drawing_width {
            let mut arr = Array3::from_shape_vec((height, drawing_width_usize, 4), vec).unwrap();
            arr = arr.pad((pad_left, pad_right), Axis(1), Default::default());
            arr.into_raw_vec()
        } else {
            vec
        }
    }
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

pub fn make_opaque(mut image: ArrayViewMut3<u8>, left: u32, width: u32) {
    image
        .slice_mut(s![.., left as isize..(left + width) as isize, 3])
        .mapv_inplace(|_| u8::MAX);
}

pub fn blend_img(
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

#[inline]
pub fn get_colormap_rgb() -> Vec<u8> {
    COLORMAP.iter().flat_map(|x| x.iter().cloned()).collect()
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

fn colorize_grey_with_size(
    grey: ArrWithSliceInfo<f32, Ix2>,
    width: u32,
    height: u32,
    fast_resize: bool,
) -> Vec<u8> {
    // let start = Instant::now();
    let (grey, trim_left, trim_width) = (grey.arr, grey.index, grey.length);
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
            grey.as_slice().unwrap()[trim_left..].as_gray(),
            grey.shape()[1],
            resized.as_gray_mut(),
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

fn draw_wav_directly(wav: &[f32], stroke_width: f32, pixmap: &mut PixmapMut, paint: &Paint) {
    let path = {
        let mut pb = PathBuilder::with_capacity(wav.len() + 1, wav.len() + 1);
        pb.move_to(0., wav[0]);
        for (i, &y) in wav.iter().enumerate().skip(1) {
            pb.line_to((i * pixmap.width() as usize) as f32 / wav.len() as f32, y);
        }
        if wav.len() == 1 {
            pb.line_to((pixmap.width().min(2) - 1) as f32, wav[0]);
        }
        pb.finish().unwrap()
    };

    let stroke = Stroke {
        width: stroke_width,
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
    let path = {
        let len = top_envlop.len() + btm_envlop.len() + 2;
        let mut pb = PathBuilder::with_capacity(len, len);
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

fn draw_wav_to(
    output: &mut [u8],
    wav: ArrWithSliceInfo<f32, Ix1>,
    width: u32,
    height: u32,
    opt_for_wav: &DrawOptionForWav,
    alpha: Option<u8>,
) {
    // let start = Instant::now();
    let &DrawOptionForWav { amp_range, dpr } = opt_for_wav;
    let DprDependentConstants {
        thr_long_height,
        topbottom_context_size,
        wav_stroke_width,
    } = DprDependentConstants::calc(dpr);
    let amp_to_px = |x: f32, clamp: bool| {
        let x = (amp_range.1 - x) * height as f32 / (amp_range.1 - amp_range.0);
        if clamp {
            x.clamp(0., height as f32)
        } else {
            x
        }
    };
    let samples_per_px = wav.length as f32 / width as f32;

    let alpha = alpha.unwrap_or(u8::MAX);
    let mut paint = Paint::default();
    let [r, g, b] = WAVECOLOR;
    paint.set_color_rgba8(r, g, b, alpha);
    paint.anti_alias = true;

    let mut out_arr =
        ArrayViewMut3::from_shape((height as usize, width as usize, 4), output).unwrap();
    let mut pixmap = PixmapMut::from_bytes(out_arr.as_slice_mut().unwrap(), width, height).unwrap();

    if amp_range.1 - amp_range.0 < 1e-16 {
        // over-zoomed
        let rect = Rect::from_xywh(0., 0., width as f32, height as f32).unwrap();
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    } else if samples_per_px < 2. {
        // upsampling
        let wav_tail = wav.as_sliced_with_tail(RESAMPLE_TAIL);
        let width_tail = (width as f32 * wav_tail.len() as f32 / wav.length as f32).round();
        let mut resampler = create_resampler(wav_tail.len(), width_tail as usize);
        let upsampled = resampler.resample(wav_tail).mapv(|x| amp_to_px(x, false));
        let wav_px = upsampled.slice_move(s![..width as usize]);
        draw_wav_directly(
            wav_px.as_slice().unwrap(),
            wav_stroke_width,
            &mut pixmap,
            &paint,
        );
    } else {
        let wav = wav.as_sliced();
        let half_context_size = topbottom_context_size / 2.;
        let mean_px = amp_to_px(wav.mean().unwrap_or(0.), true);
        let mut top_envlop = Vec::with_capacity(width as usize);
        let mut btm_envlop = Vec::with_capacity(width as usize);
        let mut n_mean_crossing = 0u32;
        for i_px in 0..width {
            let i_start = ((i_px as f32 - half_context_size) * samples_per_px)
                .round()
                .max(0.) as usize;
            let i_end = (((i_px as f32 + half_context_size) * samples_per_px).round() as usize)
                .min(wav.len());
            let wav_slice = wav.slice(s![i_start..i_end]);
            let top = amp_to_px(*wav_slice.max_skipnan(), false) - wav_stroke_width / 2.;
            let bottom = amp_to_px(*wav_slice.min_skipnan(), false) + wav_stroke_width / 2.;
            if top < mean_px + f32::EPSILON && bottom > mean_px - thr_long_height
                || top < mean_px + thr_long_height && bottom > mean_px - f32::EPSILON
            {
                n_mean_crossing += 1;
            }
            top_envlop.push(top);
            btm_envlop.push(bottom);
        }
        let thr_topbottom = width * THR_TOPBOTTOM_PERCENT / 100;
        if n_mean_crossing > thr_topbottom {
            draw_wav_topbottom(&top_envlop, &btm_envlop, &mut pixmap, &paint);
        } else {
            let wav_px = wav.map(|&x| amp_to_px(x, false));
            draw_wav_directly(
                wav_px.as_slice().unwrap(),
                wav_stroke_width,
                &mut pixmap,
                &paint,
            );
        }
    }

    // println!("drawing wav: {:?}", start.elapsed());
}

fn draw_blended_spec_wav(
    spec_grey: ArrWithSliceInfo<f32, Ix2>,
    wav: ArrWithSliceInfo<f32, Ix1>,
    width: u32,
    height: u32,
    opt_for_wav: &DrawOptionForWav,
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
        if (0.0..0.5).contains(&blend) {
            let rect = IntRect::from_xywh(0, 0, width, height).unwrap().to_rect();
            let mut paint = Paint::default();
            let alpha = (u8::MAX as f64 * (1. - 2. * blend)).round() as u8;
            paint.set_color_rgba8(0, 0, 0, alpha);
            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
        }

        let alpha = (u8::MAX as f64 * (2. - 2. * blend).min(1.)).round() as u8;
        // wave
        draw_wav_to(
            pixmap.data_mut(),
            wav,
            width,
            height,
            opt_for_wav,
            Some(alpha),
        );
    }
    result
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
            ResizeType::Triangle
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
}
