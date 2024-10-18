use std::ops::Neg;
// use std::time::Instant;

use fast_image_resize::images::{TypedImage, TypedImageRef};
use fast_image_resize::pixels;
use fast_image_resize::{FilterType, ImageView, ResizeAlg, ResizeOptions, Resizer};
use napi_derive::napi;
use ndarray::prelude::*;
use rayon::prelude::*;
use tiny_skia::{
    FillRule, IntRect, Paint, PathBuilder, Pixmap, PixmapMut, PixmapPaint, PixmapRef, Transform,
};

use super::drawing_wav::{draw_limiter_gain_to, draw_wav_to, DrawOptionForWav};
use super::img_slice::{ArrWithSliceInfo, CalcWidth, LeftWidth, OverviewHeights, PartGreyInfo};
use crate::backend::dynamics::{GuardClippingResult, MaxPeak};
use crate::backend::track::TrackList;
use crate::backend::utils::Pad;
use crate::backend::{IdChArr, IdChValueVec, TrackManager};

const BLACK: [u8; 3] = [000; 3];
const WHITE: [u8; 3] = [255; 3];

#[rustfmt::skip]
pub const COLORMAP: [[u8; 3]; 256] = [
    [0, 0, 4], [1, 0, 5], [1, 1, 6], [1, 1, 8], [2, 1, 10], [2, 2, 12], [2, 2, 14], [3, 2, 16],
    [4, 3, 18], [4, 3, 21], [5, 4, 23], [6, 4, 25], [7, 5, 27], [8, 6, 29], [9, 6, 32], [10, 7, 34],
    [11, 7, 36], [12, 8, 38], [13, 8, 41], [14, 9, 43], [16, 9, 45], [17, 10, 48], [18, 10, 50], [20, 11, 53],
    [21, 11, 55], [22, 11, 58], [24, 12, 60], [25, 12, 62], [27, 12, 65], [28, 12, 67], [30, 12, 70], [31, 12, 72],
    [33, 12, 74], [35, 12, 77], [36, 12, 79], [38, 12, 81], [40, 11, 83], [42, 11, 85], [43, 11, 87], [45, 11, 89],
    [47, 10, 91], [49, 10, 93], [51, 10, 94], [52, 10, 96], [54, 9, 97], [56, 9, 98], [58, 9, 99], [59, 9, 100],
    [61, 9, 101], [63, 9, 102], [64, 10, 103], [66, 10, 104], [68, 10, 105], [69, 10, 105], [71, 11, 106], [73, 11, 107],
    [74, 12, 107], [76, 12, 108], [78, 13, 108], [79, 13, 108], [81, 14, 109], [83, 14, 109], [84, 15, 109], [86, 15, 110],
    [87, 16, 110], [89, 17, 110], [91, 17, 110], [92, 18, 110], [94, 18, 111], [95, 19, 111], [97, 20, 111], [99, 20, 111],
    [100, 21, 111], [102, 21, 111], [103, 22, 111], [105, 23, 111], [107, 23, 111], [108, 24, 111], [110, 24, 111], [111, 25, 111],
    [113, 25, 110], [115, 26, 110], [116, 27, 110], [118, 27, 110], [119, 28, 110], [121, 28, 110], [123, 29, 109], [124, 29, 109],
    [126, 30, 109], [127, 31, 109], [129, 31, 108], [130, 32, 108], [132, 32, 108], [134, 33, 107], [135, 33, 107], [137, 34, 107],
    [138, 34, 106], [140, 35, 106], [142, 36, 105], [143, 36, 105], [145, 37, 105], [146, 37, 104], [148, 38, 104], [150, 38, 103],
    [151, 39, 102], [153, 40, 102], [154, 40, 101], [156, 41, 101], [158, 41, 100], [159, 42, 100], [161, 43, 99], [162, 43, 98],
    [164, 44, 98], [165, 45, 97], [167, 45, 96], [169, 46, 95], [170, 46, 95], [172, 47, 94], [173, 48, 93], [175, 49, 92],
    [176, 49, 92], [178, 50, 91], [179, 51, 90], [181, 51, 89], [182, 52, 88], [184, 53, 87], [185, 54, 86], [187, 54, 85],
    [188, 55, 85], [190, 56, 84], [191, 57, 83], [193, 58, 82], [194, 59, 81], [196, 60, 80], [197, 60, 79], [198, 61, 78],
    [200, 62, 77], [201, 63, 76], [203, 64, 75], [204, 65, 74], [205, 66, 72], [207, 67, 71], [208, 68, 70], [209, 69, 69],
    [211, 70, 68], [212, 72, 67], [213, 73, 66], [214, 74, 65], [216, 75, 64], [217, 76, 62], [218, 77, 61], [219, 79, 60],
    [220, 80, 59], [221, 81, 58], [223, 82, 57], [224, 84, 56], [225, 85, 54], [226, 86, 53], [227, 88, 52], [228, 89, 51],
    [229, 90, 50], [230, 92, 48], [231, 93, 47], [232, 95, 46], [233, 96, 45], [234, 98, 43], [235, 99, 42], [235, 101, 41],
    [236, 102, 40], [237, 104, 38], [238, 105, 37], [239, 107, 36], [240, 109, 35], [240, 110, 33], [241, 112, 32], [242, 113, 31],
    [242, 115, 30], [243, 117, 28], [244, 118, 27], [244, 120, 26], [245, 122, 24], [246, 123, 23], [246, 125, 22], [247, 127, 20],
    [247, 129, 19], [248, 130, 18], [248, 132, 16], [249, 134, 15], [249, 136, 14], [249, 137, 12], [250, 139, 11], [250, 141, 10],
    [250, 143, 9], [251, 145, 8], [251, 146, 7], [251, 148, 7], [252, 150, 6], [252, 152, 6], [252, 154, 6], [252, 156, 6],
    [252, 158, 7], [253, 160, 7], [253, 161, 8], [253, 163, 9], [253, 165, 10], [253, 167, 12], [253, 169, 13], [253, 171, 15],
    [253, 173, 17], [253, 175, 19], [253, 177, 20], [253, 179, 22], [253, 181, 24], [252, 183, 27], [252, 185, 29], [252, 186, 31],
    [252, 188, 33], [252, 190, 35], [251, 192, 38], [251, 194, 40], [251, 196, 43], [251, 198, 45], [250, 200, 48], [250, 202, 50],
    [250, 204, 53], [249, 206, 56], [249, 208, 58], [248, 210, 61], [248, 212, 64], [247, 214, 67], [247, 216, 70], [246, 218, 73],
    [246, 220, 76], [245, 222, 80], [245, 224, 83], [244, 226, 86], [244, 228, 90], [244, 229, 94], [243, 231, 97], [243, 233, 101],
    [243, 235, 105], [242, 237, 109], [242, 238, 113], [242, 240, 117], [242, 241, 122], [243, 243, 126], [243, 244, 130], [244, 246, 134],
    [244, 247, 138], [245, 249, 142], [246, 250, 146], [247, 251, 150], [249, 252, 154], [250, 253, 158], [251, 254, 162], [253, 255, 165],
];

const OVERVIEW_MAX_CH: usize = 4;
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
        tracklist: &TrackList,
        id_ch_tuples: &IdChArr,
        option: DrawOption,
        kind: ImageKind,
    ) -> IdChValueVec<Array3<u8>>;

    fn draw_part_imgs(
        &self,
        tracklist: &TrackList,
        id_ch_tuples: &IdChArr,
        start_sec: f64,
        width: u32,
        option: DrawOption,
        opt_for_wav: DrawOptionForWav,
        blend: f64,
        fast_resize_vec: Option<Vec<bool>>,
    ) -> IdChValueVec<Vec<u8>>;

    fn draw_overview(
        &self,
        tracklist: &TrackList,
        id: usize,
        width: u32,
        height: u32,
        dpr: f32,
    ) -> Vec<u8>;
}

impl TrackDrawer for TrackManager {
    fn draw_entire_imgs(
        &self,
        tracklist: &TrackList,
        id_ch_tuples: &IdChArr,
        option: DrawOption,
        kind: ImageKind,
    ) -> IdChValueVec<Array3<u8>> {
        // let start = Instant::now();
        let DrawOption { px_per_sec, height } = option;
        id_ch_tuples
            .par_iter()
            .map(|&(id, ch)| {
                let out_for_not_exist = || ((id, ch), Array::zeros((0, 0, 0)));
                let track = if let Some(track) = tracklist.get(id) {
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
            })
            .collect()
        // println!("draw entire: {:?}", start.elapsed());
    }

    /// Draw part of images. if blend < 0, draw waveform with transparent background
    fn draw_part_imgs(
        &self,
        tracklist: &TrackList,
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
        id_ch_tuples
            .par_iter()
            .enumerate()
            .map(|(i, &(id, ch))| {
                let out_for_not_exist = || ((id, ch), Vec::new());
                let track = if let Some(track) = tracklist.get(id) {
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
                let (vec, _) = arr
                    .as_standard_layout()
                    .to_owned()
                    .into_raw_vec_and_offset();
                ((id, ch), vec)
            })
            .collect()
        // println!("draw: {:?}", start.elapsed());
    }

    fn draw_overview(
        &self,
        tracklist: &TrackList,
        id: usize,
        width: u32,
        height: u32,
        dpr: f32,
    ) -> Vec<u8> {
        let track = if let Some(track) = tracklist.get(id) {
            track
        } else {
            return Vec::new();
        };
        let (pad_left, drawing_width, pad_right) =
            track.decompose_width_of(0., width, width as f64 / tracklist.max_sec);
        let (pad_left, drawing_width_usize, pad_right) = (
            pad_left as usize,
            drawing_width as usize,
            pad_right as usize,
        );
        let n_ch = track.n_ch().min(OVERVIEW_MAX_CH);
        let heights = OverviewHeights::new(height, n_ch, OVERVIEW_CH_GAP_HEIGHT, dpr);
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
            .into_par_iter()
            .enumerate()
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
                        let (gain_h, wav_h) = draw_gain_heights;
                        draw_wav(gain_h, wav_h);
                        if ch > 0 {
                            return;
                        }
                        let gain_seq = gain_seq.slice(s![0, ..]);
                        let neg_gain_seq = gain_seq.neg();
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
                        draw_gain(0, gain_seq, (0.5, 1.), true);
                        draw_gain(gain_h + wav_h, neg_gain_seq.view(), (-1., -0.5), false);
                    }
                    _ => {
                        draw_wav(0, heights.ch);
                    }
                }
            });

        if draw_gain_heights != Default::default() {
            let (gain_h, wav_h) = draw_gain_heights;
            let gain_upper = arr
                .slice(s![heights.margin.., .., ..])
                .slice(s![..gain_h, .., ..])
                .to_owned();
            let gain_lower = arr
                .slice(s![heights.margin.., .., ..])
                .slice(s![(gain_h + wav_h)..heights.ch, .., ..])
                .to_owned();

            arr.slice_mut(s![heights.margin.., .., ..])
                .axis_chunks_iter_mut(Axis(0), heights.ch_and_gap())
                .into_par_iter()
                .enumerate()
                .filter(|(ch, _)| *ch > 0)
                .for_each(|(_, mut arr_ch)| {
                    arr_ch.slice_mut(s![..gain_h, .., ..]).assign(&gain_upper);
                    arr_ch
                        .slice_mut(s![(gain_h + wav_h)..heights.ch, .., ..])
                        .assign(&gain_lower);
                });
        }
        if width != drawing_width {
            arr = arr.pad((pad_left, pad_right), Axis(1), Default::default());
        }
        arr.into_raw_vec_and_offset().0
    }
}

#[inline]
pub fn get_colormap_rgb() -> Vec<u8> {
    COLORMAP
        .iter()
        .chain(Some(&WHITE))
        .flat_map(|x| x.iter().copied())
        .collect()
}

#[allow(non_snake_case)]
pub fn convert_spec_to_grey(
    spec: ArrayView2<f32>,
    i_freq_range: (usize, usize),
    dB_range: (f32, f32),
) -> Array2<pixels::U16> {
    // spec: T x F
    // return: grey image with F(inverted) x T
    let (i_freq_start, i_freq_end) = i_freq_range;
    let dB_span = dB_range.1 - dB_range.0;
    let width = spec.shape()[0];
    let height = i_freq_end - i_freq_start;
    Array2::from_shape_fn((height, width), |(i, j)| {
        let i_freq = i_freq_start + height - 1 - i;
        if i_freq < spec.raw_dim()[1] {
            pixels::U16::new(
                (((spec[[j, i_freq]] - dB_range.0) / dB_span).mul_add((u16::MAX - 1) as f32, 1.))
                    .clamp(1., u16::MAX as f32)
                    .round() as u16,
            )
        } else {
            pixels::U16::new(0)
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
    debug_assert!(0. < blend && blend < 1.);
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
            let alpha = (u8::MAX as f64 * (blend.mul_add(-2., 1.))).round() as u8;
            paint.set_color_rgba8(0, 0, 0, alpha);
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
        opacity: blend.mul_add(-2., 2.).min(1.) as f32,
        ..Default::default()
    };
    pixmap.draw_pixmap(0, 0, wav_pixmap, &paint, Transform::identity(), None);
}

#[inline]
fn interpolate<const L: usize>(color1: &[u8; L], color2: &[u8; L], ratio: f32) -> [u8; L] {
    let mut iter = color1.iter().zip(color2).map(|(&a, &b)| {
        (ratio.mul_add(a as f32, (b as f32).mul_add(-ratio, b as f32))).round() as u8
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
    grey: ArrWithSliceInfo<pixels::U16, Ix2>,
    width: u32,
    height: u32,
    fast_resize: bool,
) -> Vec<u8> {
    // let start = Instant::now();
    let (grey, trim_left, trim_width) = (grey.arr, grey.index, grey.length);
    let resized = {
        let mut resizer = Resizer::new();
        let src_image = TypedImageRef::new(
            grey.shape()[1] as u32,
            grey.shape()[0] as u32,
            grey.as_slice().unwrap(),
        )
        .unwrap();
        let resize_opt = ResizeOptions::new()
            .crop(
                trim_left as f64,
                0.,
                trim_width as f64,
                src_image.height() as f64,
            )
            .resize_alg(ResizeAlg::Convolution(if fast_resize {
                FilterType::Bilinear
            } else {
                FilterType::Lanczos3
            }));

        let mut dst_image = TypedImage::<pixels::U16>::new(width, height);
        resizer
            .resize_typed(&src_image, &mut dst_image, Some(&resize_opt))
            .unwrap();
        dst_image
    };

    resized
        .pixels()
        .iter()
        .flat_map(|x| map_grey_to_color(x.0).into_iter().chain(Some(u8::MAX)))
        .collect()
    // println!("drawing spec: {:?}", start.elapsed());
}

/// blend can be < 0 for not drawing spec
fn draw_blended_spec_wav(
    spec_grey: ArrWithSliceInfo<pixels::U16, Ix2>,
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

    #[test]
    fn show_colorbar() {
        let (width, height) = (50, 500);
        let colormap: Vec<pixels::U8x3> = COLORMAP
            .iter()
            .rev()
            .copied()
            .map(pixels::U8x3::new)
            .collect();
        let src_image = TypedImageRef::new(1, colormap.len() as u32, colormap.as_slice()).unwrap();
        let mut dst_image = TypedImage::new(width, height);
        let options =
            ResizeOptions::new().resize_alg(ResizeAlg::Interpolation(FilterType::Bilinear));
        Resizer::new()
            .resize_typed(&src_image, &mut dst_image, &options)
            .unwrap();
        let dst_raw_vec = dst_image
            .pixels()
            .into_iter()
            .map(|x| x.0)
            .flatten()
            .collect();
        RgbImage::from_raw(width, height, dst_raw_vec)
            .unwrap()
            .save("samples/colorbar.png")
            .unwrap();
    }
}
