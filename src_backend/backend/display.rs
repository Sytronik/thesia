use std::iter;
use std::mem::MaybeUninit;
// use std::time::Instant;

use cached::proc_macro::cached;
use ndarray::Slice;
use ndarray::{prelude::*, Data};
use ndarray_stats::QuantileExt;
use rayon::prelude::*;
use resize::{self, formats::Gray, Pixel::GrayF32, Resizer};
use rgb::FromSlice;
use tiny_skia::{
    FillRule, IntRect, LineCap, Paint, PathBuilder, PixmapMut, PixmapPaint, PixmapRef, Rect,
    Stroke, Transform,
};

use super::resample::FftResampler;
use super::utils::{Pad, PadMode};
use super::{IdChArr, IdChMap, TrackManager};

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
pub const RESAMPLE_TAIL: usize = 500;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DrawOption {
    pub px_per_sec: f64,
    pub height: u32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DrawOptionForWav {
    pub amp_range: (f32, f32),
}

pub enum ImageKind {
    Spec,
    Wav(DrawOptionForWav),
}

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

    fn draw_overview(&self, id: usize, width: u32, height: u32) -> Vec<u8>;
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
            let track = self.tracks[id].as_ref().unwrap();
            let width = track.calc_width(px_per_sec);
            let arr = match kind {
                ImageKind::Spec => {
                    let grey = self.spec_greys.get(&(id, ch)).unwrap().view();
                    let vec = colorize_grey_with_size(
                        ArrWithSliceInfo::entire(&grey),
                        width,
                        height,
                        false,
                    );
                    Array3::from_shape_vec((height as usize, width as usize, 4), vec).unwrap()
                }
                ImageKind::Wav(option_for_wav) => {
                    let mut arr = Array3::zeros((height as usize, width as usize, 4));
                    draw_wav_to(
                        arr.as_slice_mut().unwrap(),
                        ArrWithSliceInfo::entire(&track.get_wav(ch)),
                        width,
                        height,
                        option_for_wav.amp_range,
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
            let track = self.tracks[id].as_ref().unwrap();
            let spec_grey = self.spec_greys.get(&(id, ch)).unwrap();
            let (pad_left, drawing_width, pad_right) =
                track.decompose_width_of(start_sec, width, px_per_sec);

            if drawing_width == 0 {
                return ((id, ch), vec![0u8; height as usize * width as usize * 4]);
            }
            let spec_grey_part = ArrWithSliceInfo::from_ref(
                &spec_grey,
                track.calc_part_grey_info(
                    spec_grey.shape()[1] as u64,
                    start_sec,
                    width,
                    px_per_sec,
                ),
            );
            let wav_part = ArrWithSliceInfo::from(
                track.get_wav(ch),
                track.calc_part_wav_info(start_sec, width, px_per_sec),
            );
            let vec = draw_blended_spec_wav(
                spec_grey_part,
                wav_part,
                drawing_width,
                height,
                opt_for_wav.amp_range,
                blend,
                fast_resize_vec.as_ref().map_or(false, |v| v[i]),
            );
            let mut arr =
                Array3::from_shape_vec((height as usize, drawing_width as usize, 4), vec).unwrap();

            if width != drawing_width {
                arr = arr.pad(
                    (pad_left as usize, pad_right as usize),
                    Axis(1),
                    PadMode::Constant(0),
                );
            }
            ((id, ch), arr.into_raw_vec())
        });
        result.par_extend(par_iter);

        // println!("draw: {:?}", start.elapsed());
        result
    }

    fn draw_overview(&self, id: usize, width: u32, height: u32) -> Vec<u8> {
        let track = self.tracks[id].as_ref().unwrap();
        let ch_h = height / track.n_ch() as u32;
        let i_start = (height % track.n_ch() as u32 / 2 * width * 4) as usize;
        let i_end = i_start + (track.n_ch() as u32 * ch_h * width * 4) as usize;
        let mut result = vec![0u8; width as usize * height as usize * 4];
        result[i_start..i_end]
            .par_chunks_exact_mut(ch_h as usize * width as usize * 4)
            .enumerate()
            .for_each(|(ch, x)| {
                draw_wav_to(
                    x,
                    ArrWithSliceInfo::entire(&track.get_wav(ch)),
                    width,
                    ch_h,
                    (-1., 1.),
                    None,
                )
            });
        result
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

fn draw_wav_to(
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

    let mut out_arr =
        ArrayViewMut3::from_shape((height as usize, width as usize, 4), output).unwrap();
    let mut pixmap = PixmapMut::from_bytes(out_arr.as_slice_mut().unwrap(), width, height).unwrap();

    if over_zoomed {
        let rect = Rect::from_xywh(0., 0., width as f32, height as f32).unwrap();
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    } else if need_upsampling {
        draw_wav_directly(wav.as_slice().unwrap(), &mut pixmap, &paint);
    } else {
        let mut wav_slices = Vec::with_capacity(width as usize);
        let mut top_envlop = Vec::with_capacity(width as usize);
        let mut btm_envlop = Vec::with_capacity(width as usize);
        let mut n_conseq_long_h = 0usize;
        let mut max_n_conseq = 0usize;
        for i_px in 0..width {
            let i_start = ((i_px as f32 - 0.5) * samples_per_px).round().max(0.) as usize;
            let i_end = (((i_px as f32 + 0.5) * samples_per_px).round() as usize).min(wav.len());
            let wav_slice = wav.slice(s![i_start..i_end]);
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

    // println!("drawing wav: {:?}", start.elapsed());
}

fn draw_blended_spec_wav(
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
