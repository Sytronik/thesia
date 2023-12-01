use std::num::NonZeroU32;
use std::ops::Neg;
// use std::time::Instant;

use fast_image_resize::pixels::U16;
use fast_image_resize::{CropBox, FilterType, ImageView, ImageViewMut, ResizeAlg, Resizer};
use napi_derive::napi;
use ndarray::prelude::*;
use rayon::prelude::*;
use tiny_skia::{
    FillRule, IntRect, Paint, PathBuilder, Pixmap, PixmapMut, PixmapPaint, PixmapRef, Transform,
};

use super::drawing_wav::{draw_limiter_gain_to, draw_wav_to, DrawOptionForWav};
use super::img_slice::{ArrWithSliceInfo, CalcWidth, LeftWidth, OverviewHeights, PartGreyInfo};
use crate::backend::dynamics::{GuardClippingResult, MaxPeak};
use crate::backend::utils::Pad;
use crate::backend::{IdChArr, IdChValueVec, TrackManager};

const BLACK: [u8; 3] = [000; 3];
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
const OVERVIEW_CH_GAP_HEIGHT: f32 = 1.;
const LIMITER_GAIN_HEIGHT_DENOM: usize = 5; // 1/5 of the height will be used for draw limiter gain

#[napi(object)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DrawOption {
    pub px_per_sec: f64,
    pub height: u32,
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
    ) -> IdChValueVec<Array3<u8>>;

    fn draw_part_imgs(
        &self,
        id_ch_tuples: &IdChArr,
        start_sec: f64,
        width: u32,
        option: DrawOption,
        opt_for_wav: DrawOptionForWav,
        blend: f64,
        fast_resize_vec: Option<Vec<bool>>,
    ) -> IdChValueVec<Vec<u8>>;

    fn draw_overview(&self, id: usize, width: u32, height: u32, dpr: f32) -> Vec<u8>;
}

impl TrackDrawer for TrackManager {
    fn draw_entire_imgs(
        &self,
        id_ch_tuples: &IdChArr,
        option: DrawOption,
        kind: ImageKind,
    ) -> IdChValueVec<Array3<u8>> {
        // let start = Instant::now();
        let DrawOption { px_per_sec, height } = option;
        let mut result = Vec::with_capacity(id_ch_tuples.len());
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
                    let vec = colorize_resize_grey(grey.into(), width, height, false);
                    Array3::from_shape_vec(shape, vec).unwrap()
                }
                ImageKind::Wav(opt_for_wav) => {
                    let mut arr = Array3::zeros(shape);
                    let (wav, show_clipping) = track.channel_for_drawing(ch);
                    draw_wav_to(
                        arr.as_slice_mut().unwrap(),
                        wav.into(),
                        width,
                        height,
                        &opt_for_wav,
                        show_clipping,
                        true,
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
    ) -> IdChValueVec<Vec<u8>> {
        // let start = Instant::now();
        let DrawOption { px_per_sec, height } = option;
        let mut result = Vec::with_capacity(id_ch_tuples.len());
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
            let (wav, show_clipping) = track.channel_for_drawing(ch);
            let wav_part = ArrWithSliceInfo::new(
                wav,
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
                show_clipping,
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
        let heights = OverviewHeights::new(height, track.n_ch(), OVERVIEW_CH_GAP_HEIGHT, dpr);
        let (clipped_peak, draw_gain_heights) = match track.guard_clip_result() {
            GuardClippingResult::WavBeforeClip(before_clip) => {
                (before_clip.max_peak(), Default::default())
            }
            GuardClippingResult::GainSequence(gain_seq) if gain_seq.iter().any(|&x| x < 1.) => {
                (1., heights.decompose_by_gain(LIMITER_GAIN_HEIGHT_DENOM))
            }
            _ => (1., Default::default()),
        };

        let mut arr = Array3::zeros((heights.total, drawing_width_usize, 4));
        arr.slice_mut(s![heights.margin.., .., ..])
            .axis_chunks_iter_mut(Axis(0), heights.ch_and_gap())
            .enumerate()
            .par_bridge()
            .for_each(|(ch, mut arr_ch)| {
                let mut draw_wav = |i_h, h| {
                    draw_wav_to(
                        arr_ch
                            .slice_mut(s![i_h..(i_h + h), .., ..])
                            .as_slice_mut()
                            .unwrap(),
                        track.channel(ch).into(),
                        drawing_width,
                        h as u32,
                        &DrawOptionForWav::with_dpr(dpr),
                        false,
                        false,
                    )
                };
                match track.guard_clip_result() {
                    GuardClippingResult::WavBeforeClip(before_clip) if clipped_peak > 1. => {
                        draw_wav_to(
                            arr_ch
                                .slice_mut(s![..heights.ch, .., ..])
                                .as_slice_mut()
                                .unwrap(),
                            before_clip.slice(s![ch, ..]).into(),
                            drawing_width,
                            heights.ch as u32,
                            &DrawOptionForWav {
                                amp_range: (-clipped_peak, clipped_peak),
                                dpr,
                            },
                            true,
                            false,
                        )
                    }
                    GuardClippingResult::GainSequence(gain_seq)
                        if draw_gain_heights != Default::default() =>
                    {
                        let gain_seq_ch = gain_seq.slice(s![ch, ..]);
                        let neg_gain_seq_ch = gain_seq_ch.neg();
                        let (gain_h, wav_h) = draw_gain_heights;
                        draw_wav(gain_h, wav_h);
                        let mut draw_gain = |i_h, gain: ArrayView1<f32>, amp_range, draw_bottom| {
                            draw_limiter_gain_to(
                                arr_ch
                                    .slice_mut(s![i_h..(i_h + gain_h), .., ..])
                                    .as_slice_mut()
                                    .unwrap(),
                                gain,
                                drawing_width,
                                gain_h as u32,
                                &DrawOptionForWav { amp_range, dpr },
                                draw_bottom,
                            );
                        };
                        draw_gain(0, gain_seq_ch, (0.5, 1.), true);
                        draw_gain(gain_h + wav_h, neg_gain_seq_ch.view(), (-1., -0.5), false);
                    }
                    _ => {
                        draw_wav(0, heights.ch);
                    }
                }
            });

        if width != drawing_width {
            arr = arr.pad((pad_left, pad_right), Axis(1), Default::default());
        }
        arr.into_raw_vec()
    }
}

#[inline]
pub fn get_colormap_rgb() -> Vec<u8> {
    COLORMAP
        .iter()
        .chain(Some(&WHITE))
        .flat_map(|x| x.iter().cloned())
        .collect()
}

pub fn convert_spec_to_grey(
    spec: ArrayView2<f32>,
    up_ratio: f32,
    max: f32,
    min: f32,
) -> Array2<U16> {
    // spec: T x F
    // return: grey image with F(inverted) x T
    let width = spec.shape()[0];
    let height = (spec.shape()[1] as f32 * up_ratio).round() as usize;
    Array2::from_shape_fn((height, width), |(i, j)| {
        if height - 1 - i < spec.raw_dim()[1] {
            U16::new(
                ((spec[[j, height - 1 - i]] - min) * (u16::MAX - 1) as f32 / (max - min) + 1.)
                    .clamp(1., u16::MAX as f32)
                    .round() as u16,
            )
        } else {
            U16::new(0)
        }
    })
}

pub fn make_opaque(mut image: ArrayViewMut3<u8>, left: u32, width: u32) {
    image
        .slice_mut(s![.., left as isize..(left + width) as isize, 3])
        .mapv_inplace(|_| u8::MAX);
}

pub fn blend_img_to(
    spec_background: &mut [u8],
    wav_img: &[u8],
    width: u32,
    height: u32,
    blend: f64,
    eff_l_w: Option<LeftWidth>,
) {
    assert!(0. < blend && blend < 1.);
    let mut pixmap = PixmapMut::from_bytes(spec_background, width, height).unwrap();

    let wav_pixmap = PixmapRef::from_bytes(wav_img, width, height).unwrap();
    blend_wav_img_to(&mut pixmap, wav_pixmap, blend, eff_l_w);
}

fn blend_wav_img_to(
    pixmap: &mut PixmapMut,
    wav_pixmap: PixmapRef,
    blend: f64,
    eff_l_w: Option<LeftWidth>,
) {
    // black
    if let Some((left, width)) = eff_l_w {
        if (0.0..0.5).contains(&blend) && width > 0 {
            let rect = IntRect::from_xywh(left as i32, 0, width, pixmap.height())
                .unwrap()
                .to_rect();
            let path = PathBuilder::from_rect(rect);
            let mut paint = Paint::default();
            paint.set_color_rgba8(0, 0, 0, (u8::MAX as f64 * (1. - 2. * blend)).round() as u8);
            pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
    let paint = PixmapPaint {
        opacity: (2. - 2. * blend).min(1.) as f32,
        ..Default::default()
    };
    pixmap.draw_pixmap(0, 0, wav_pixmap, &paint, Transform::identity(), None);
}

#[inline]
fn interpolate<const L: usize>(color1: &[u8; L], color2: &[u8; L], ratio: f32) -> [u8; L] {
    let mut iter = color1
        .iter()
        .zip(color2)
        .map(|(&a, &b)| (ratio * a as f32 + (1. - ratio) * b as f32).round() as u8);
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
    let position = (x - 1) as f32 * COLORMAP.len() as f32 / (u16::MAX - 1) as f32;
    let index = position.floor() as usize;
    let rgb1 = if index >= COLORMAP.len() - 1 {
        &WHITE
    } else {
        &COLORMAP[index + 1]
    };
    interpolate(rgb1, &COLORMAP[index], position - index as f32)
}

fn colorize_resize_grey(
    grey: ArrWithSliceInfo<U16, Ix2>,
    width: u32,
    height: u32,
    fast_resize: bool,
) -> Vec<u8> {
    // let start = Instant::now();
    let (grey, trim_left, trim_width) = (grey.arr, grey.index, grey.length);
    let resized = {
        let mut src_image = ImageView::from_pixels(
            NonZeroU32::new(grey.shape()[1] as u32).unwrap(),
            NonZeroU32::new(grey.shape()[0] as u32).unwrap(),
            grey.as_slice().unwrap(),
        )
        .unwrap();
        src_image
            .set_crop_box(CropBox {
                left: trim_left as u32,
                top: 0,
                width: NonZeroU32::new(trim_width as u32).unwrap(),
                height: src_image.height(),
            })
            .unwrap();
        let mut resizer = Resizer::new(ResizeAlg::Convolution(if fast_resize {
            FilterType::Bilinear
        } else {
            FilterType::Lanczos3
        }));

        let mut dst_vec = vec![U16::new(0); width as usize * height as usize];
        let dst_image = ImageViewMut::from_pixels(
            NonZeroU32::new(width).unwrap(),
            NonZeroU32::new(height).unwrap(),
            &mut dst_vec,
        )
        .unwrap();

        resizer
            .resize(&src_image.into(), &mut dst_image.into())
            .unwrap();
        dst_vec
    };

    resized
        .into_iter()
        .flat_map(|x| map_grey_to_color(x.0).into_iter().chain(Some(u8::MAX)))
        .collect()
    // println!("drawing spec: {:?}", start.elapsed());
}

/// blend can be < 0 for not drawing spec
fn draw_blended_spec_wav(
    spec_grey: ArrWithSliceInfo<U16, Ix2>,
    wav: ArrWithSliceInfo<f32, Ix1>,
    width: u32,
    height: u32,
    opt_for_wav: &DrawOptionForWav,
    blend: f64,
    fast_resize: bool,
    show_clipping: bool,
) -> Vec<u8> {
    // spec
    if spec_grey.length == 0 || wav.length == 0 {
        return vec![0u8; height as usize * width as usize * 4];
    }
    let mut result = if blend > 0. {
        colorize_resize_grey(spec_grey, width, height, fast_resize)
    } else {
        vec![0u8; height as usize * width as usize * 4]
    };

    let mut pixmap = PixmapMut::from_bytes(&mut result, width, height).unwrap();

    if blend < 1. {
        // wave
        let mut wav_pixmap = Pixmap::new(width, height).unwrap();
        draw_wav_to(
            wav_pixmap.data_mut(),
            wav,
            width,
            height,
            opt_for_wav,
            show_clipping,
            blend != 0.,
        );
        blend_wav_img_to(&mut pixmap, wav_pixmap.as_ref(), blend, Some((0, width)));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::RgbImage;
    use resize::Pixel::RGB8;
    use rgb::FromSlice;

    #[test]
    fn show_colorbar() {
        let (width, height) = (50, 500);
        let colormap: Vec<u8> = COLORMAP.iter().rev().flatten().cloned().collect();
        let mut imvec = vec![0u8; width * height * 3];
        let mut resizer = resize::new(1, 10, width, height, RGB8, resize::Type::Triangle).unwrap();
        resizer
            .resize(&colormap.as_rgb(), imvec.as_rgb_mut())
            .unwrap();

        RgbImage::from_raw(width as u32, height as u32, imvec)
            .unwrap()
            .save("samples/colorbar.png")
            .unwrap();
    }
}
