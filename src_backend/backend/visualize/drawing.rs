use std::cell::RefCell;
use std::ops::Neg;
// use std::time::Instant;

use fast_image_resize::images::{TypedImage, TypedImageRef};
use fast_image_resize::{pixels, FilterType, ImageView, ResizeAlg, ResizeOptions, Resizer};
use ndarray::prelude::*;
use rayon::prelude::*;
use tiny_skia::{
    FillRule, IntRect, Paint, PathBuilder, Pixmap, PixmapMut, PixmapPaint, PixmapRef, Transform,
};

use super::super::dynamics::{GuardClippingResult, MaxPeak};
use super::super::track::TrackList;
use super::super::utils::Pad;
use super::super::{IdChArr, IdChValueVec, TrackManager};
use super::colorize::*;
use super::drawing_wav::{draw_limiter_gain_to, draw_wav_to};
use super::img_slice::{ArrWithSliceInfo, CalcWidth, LeftWidth, OverviewHeights, PartGreyInfo};
use super::params::{DrawOptionForWav, DrawParams, ImageKind};

const OVERVIEW_MAX_CH: usize = 4;
const OVERVIEW_CH_GAP_HEIGHT: f32 = 1.;
const LIMITER_GAIN_HEIGHT_DENOM: usize = 5; // 1/5 of the height will be used for draw limiter gain

pub trait TrackDrawer {
    fn draw_entire_imgs(
        &self,
        tracklist: &TrackList,
        id_ch_tuples: &IdChArr,
        height: u32,
        px_per_sec: f64,
        kind: ImageKind,
    ) -> IdChValueVec<Array3<u8>>;

    fn draw_part_imgs(
        &self,
        tracklist: &TrackList,
        id_ch_tuples: &IdChArr,
        params: &DrawParams,
        fast_resize_vec: impl Into<Option<Vec<bool>>>,
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
        height: u32,
        px_per_sec: f64,
        kind: ImageKind,
    ) -> IdChValueVec<Array3<u8>> {
        // let start = Instant::now();
        let parallel = id_ch_tuples.len() < rayon::current_num_threads();
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
                let arr = match &kind {
                    ImageKind::Spec => {
                        let grey = if let Some(grey) = self.spec_greys.get(&(id, ch)) {
                            grey.view()
                        } else {
                            return out_for_not_exist();
                        };
                        let vec = resize_colorize_grey(grey.into(), width, height, false, parallel);
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
                            opt_for_wav,
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
        params: &DrawParams,
        fast_resize_vec: impl Into<Option<Vec<bool>>>,
    ) -> IdChValueVec<Vec<u8>> {
        // let start = Instant::now();
        let &DrawParams {
            start_sec,
            width,
            height,
            px_per_sec,
            ref opt_for_wav,
            blend,
        } = params;
        let fast_resize_vec = fast_resize_vec.into();
        let parallel = id_ch_tuples.len() < rayon::current_num_threads();
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
                    opt_for_wav,
                    blend,
                    fast_resize_vec.as_ref().map_or(false, |v| v[i]),
                    show_clipping,
                    parallel,
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
        .fill(u8::MAX);
}

pub fn blend_img_to(
    spec_background: &mut [u8],
    wav_img: &[u8],
    width: u32,
    height: u32,
    blend: f64,
    eff_l_w: impl Into<Option<LeftWidth>>,
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
    eff_l_w: impl Into<Option<LeftWidth>>,
) {
    // black
    if let Some((left, width)) = eff_l_w.into() {
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

fn resize_colorize_grey(
    grey: ArrWithSliceInfo<pixels::U16, Ix2>,
    width: u32,
    height: u32,
    fast_resize: bool,
    parallel: bool,
) -> Vec<u8> {
    thread_local! {
        static RESIZER: RefCell<Resizer> = RefCell::new(Resizer::new());
    }

    // let start = Instant::now();
    let (grey, trim_left, trim_width) = (grey.arr, grey.index, grey.length);
    let resized_buf = RESIZER.with_borrow_mut(|resizer| {
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

        let mut dst_buf = vec![0; width as usize * height as usize * 2];
        let mut dst_image =
            TypedImage::<pixels::U16>::from_buffer(width, height, &mut dst_buf).unwrap();
        resizer
            .resize_typed(&src_image, &mut dst_image, &resize_opt)
            .unwrap();
        dst_buf
    });
    let resized = unsafe {
        std::slice::from_raw_parts(resized_buf.as_ptr() as *const u16, resized_buf.len() / 2)
    };

    if parallel {
        resized
            .par_chunks(rayon::current_num_threads())
            .flat_map_iter(map_grey_to_color_iter)
            .collect()
    } else {
        map_grey_to_color_iter(resized).collect()
    }
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
    parallel: bool,
) -> Vec<u8> {
    // spec
    if spec_grey.length == 0 || wav.length == 0 {
        return vec![0u8; height as usize * width as usize * 4];
    }
    let mut result = if blend > 0. {
        resize_colorize_grey(spec_grey, width, height, fast_resize, parallel)
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
        blend_wav_img_to(&mut pixmap, wav_pixmap.as_ref(), blend, (0, width));
    }
    result
}
