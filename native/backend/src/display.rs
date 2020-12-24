use std::error::Error as stdError;
use std::time::Instant;

use image::{
    imageops::{resize, FilterType},
    DynamicImage, ImageBuffer, Luma, Pixel, Rgba, RgbaImage,
};
use imageproc::{
    drawing::{draw_antialiased_line_segment_mut, draw_filled_rect_mut, Blend, Canvas},
    pixelops::interpolate,
    rect::Rect,
};
use ndarray::prelude::*;
use ndarray_stats::QuantileExt;

pub type GreyF32Image = ImageBuffer<Luma<f32>, Vec<f32>>;

pub const COLORMAP: [[u8; 4]; 10] = [
    [0, 0, 4, 255],
    [27, 12, 65, 255],
    [74, 12, 107, 255],
    [120, 28, 109, 255],
    [165, 44, 96, 255],
    [207, 68, 70, 255],
    [237, 105, 37, 255],
    [251, 155, 6, 255],
    [247, 209, 61, 255],
    [252, 255, 164, 255],
];
pub const WAVECOLOR: [u8; 4] = [200, 21, 103, 255];

fn convert_grey_to_color_raw(x: f32) -> [u8; 4] {
    assert!(x >= 0.);
    let position = (COLORMAP.len() as f32) * x;
    let index = position.floor() as usize;
    if index >= COLORMAP.len() - 1 {
        COLORMAP[COLORMAP.len() - 1]
    } else {
        let ratio = position - index as f32;
        let mut color = [255u8; 4];
        for i in (0..3).into_iter() {
            color[i] = (ratio * COLORMAP[index + 1][i] as f32
                + (1. - ratio) * COLORMAP[index][i] as f32)
                .round() as u8;
        }
        color
    }
}

fn convert_grey_to_color(x: &Luma<f32>) -> Rgba<u8> {
    assert!(x.0[0] >= 0.);
    let position = (COLORMAP.len() as f32) * x.0[0];
    let index = position.floor() as usize;
    if index >= COLORMAP.len() - 1 {
        Rgba(COLORMAP[COLORMAP.len() - 1])
    } else {
        let ratio = position - index as f32;
        interpolate(Rgba(COLORMAP[index + 1]), Rgba(COLORMAP[index]), ratio)
    }
}

pub fn spec_to_grey(spec: ArrayView2<f32>, up_ratio: f32, max: f32, min: f32) -> GreyF32Image {
    let height = (spec.shape()[1] as f32 * up_ratio).round() as u32;
    GreyF32Image::from_fn(spec.shape()[0] as u32, height, |x, y| {
        if y >= height - spec.shape()[1] as u32 {
            let db = spec[[x as usize, (height - 1 - y) as usize]];
            Luma([((db - min) / (max - min)).max(0.).min(1.)])
        } else {
            Luma([0.])
        }
    })
}

pub fn blend_spec_wav(
    output: &mut [u8],
    spec_grey: &GreyF32Image,
    wav: ArrayView1<f32>,
    width: u32,
    height: u32,
    blend: f64,
) -> Result<(), Box<dyn stdError>> {
    let mut output = Blend(ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, output).unwrap());

    if blend > 0. {
        // spec
        let resized = resize(spec_grey, width, height, FilterType::Lanczos3);
        resized.enumerate_pixels().for_each(|(x, y, p)| {
            output.draw_pixel(x, y, convert_grey_to_color(p));
        });
    }

    if blend < 1. {
        if blend < 0.5 {
            // black
            let black_blend = Rgba([0, 0, 0, (255. * (1. - 2. * blend)) as u8]);
            draw_filled_rect_mut(
                &mut output,
                Rect::at(0, 0).of_size(width, height),
                black_blend,
            );
        }

        // wave
        draw_wav(&mut output, wav, (255. * (2. - 2. * blend).min(1.)) as u8);
    }

    Ok(())
}

pub fn grey_to_rgb(output: &mut [u8], grey: &GreyF32Image, width: u32, height: u32) {
    let start = Instant::now();
    let resized = resize(grey, width, height, FilterType::Lanczos3);
    println!("resizing: {:?}", start.elapsed());
    let start = Instant::now();
    let im = resized
        .into_raw()
        .into_iter()
        .zip(output.chunks_exact_mut(4))
        .for_each(|(x, y)| {
            let [r, g, b, a] = convert_grey_to_color_raw(x);
            y[0] = r;
            y[1] = g;
            y[2] = b;
            y[3] = a;
        });
    println!("Applying colormap: {:?}", start.elapsed());
    im
}

fn draw_wav(output: &mut Blend<ImageBuffer<Rgba<u8>, &mut [u8]>>, wav: ArrayView1<f32>, alpha: u8) {
    let width = output.width();
    let height = output.height();
    let mut im = DynamicImage::new_rgba8(width, height);
    let amp_range = (-1., 1.);
    let amp_to_height_px =
        |x: f32| ((amp_range.1 - x) * height as f32 / (amp_range.1 - amp_range.0)).round() as isize;
    let samples_per_px = wav.len() as f32 / width as f32;
    for i_px in (0..width as i32).into_iter() {
        let i_start = ((i_px as f32 - 0.5) * samples_per_px).round().max(0.) as usize;
        let i_end = (((i_px as f32 + 0.5) * samples_per_px).round() as usize).min(wav.len());
        let wav_slice = wav.slice(s![i_start..i_end]);
        let max = *wav_slice.max().unwrap();
        let min = *wav_slice.min().unwrap();
        let mut top = amp_to_height_px(max);
        let mut bottom = amp_to_height_px(min);
        // if bottom - top < 3 {
        //     let pad_bottom = ((3 - bottom + top) as f32 / 2.).ceil() as isize;
        //     let pad_top = ((3 - bottom + top) as f32 / 2.).floor() as isize;
        //     top -= pad_top;
        //     bottom += pad_bottom;
        // }
        let top = top.max(0) as i32;
        let bottom = bottom.min(output.height() as isize) as i32;
        // draw_line_segment_mut(
        //     output,
        //     (i_px as f32, top as f32),
        //     (i_px as f32, bottom as f32),
        //     Rgba(WAVECOLOR).map_with_alpha(|x| x, |_| alpha),
        //     // |a, b, w| interpolate(a, b, w),
        // );
        draw_antialiased_line_segment_mut(
            &mut im,
            (i_px, top),
            (i_px, bottom),
            Rgba(WAVECOLOR).map_with_alpha(|x| x, |_| alpha),
            |a, b, w| interpolate(a, b, w),
        )
    }
    for x in (0..width).into_iter() {
        for y in (0..height).into_iter() {
            output.draw_pixel(x, y, im.get_pixel(x, y));
        }
    }
}

pub fn wav_to_image(
    wav: ArrayView1<f32>,
    nwidth: u32,
    nheight: u32,
    amp_range: (f32, f32),
) -> RgbaImage {
    // let nwidth = nwidth * 2;
    // let nheight = nheight * 2;
    let amp_to_height_px = |x: f32| {
        ((amp_range.1 - x) * nheight as f32 / (amp_range.1 - amp_range.0)).round() as isize
    };
    let samples_per_px = wav.len() as f32 / nwidth as f32;
    let mut arr = Array3::<u8>::zeros((nheight as usize, nwidth as usize, 4));
    let wav = if samples_per_px < 1. {
        let factor = (1. / samples_per_px).ceil() as usize;
        let mut new_wav = Array1::<f32>::zeros(factor as usize * wav.len());
        new_wav.indexed_iter_mut().for_each(|(i, x)| {
            let b = if i / factor + 1 < wav.len() {
                wav[i / factor + 1]
            } else {
                0.
            };
            *x = b * ((i % factor) as f32 / factor as f32)
                + wav[i / factor] * (1. - (i % factor) as f32 / factor as f32);
        });
        CowArray::from(new_wav)
    } else {
        CowArray::from(wav)
    };
    for i_px in (0..nwidth as i32).into_iter() {
        let i_start = ((i_px as f32 - 1.5) * samples_per_px).round().max(0.) as usize;
        let i_end = (((i_px as f32 + 1.5) * samples_per_px).round() as usize).min(wav.len());
        let wav_slice = wav.slice(s![i_start..i_end]);
        let max = *wav_slice.max().unwrap();
        let min = *wav_slice.min().unwrap();
        let mut top = amp_to_height_px(max);
        let mut bottom = amp_to_height_px(min);
        if bottom - top < 3 {
            let pad_bottom = ((3 - bottom + top) as f32 / 2.).ceil() as isize;
            let pad_top = ((3 - bottom + top) as f32 / 2.).floor() as isize;
            top -= pad_top;
            bottom += pad_bottom;
        }
        let top = top.max(0) as usize;
        let bottom = bottom.min(nheight as isize) as usize;
        arr.slice_mut(s![top..bottom + 1, i_px as usize, ..])
            .indexed_iter_mut()
            .for_each(|((_, j), x)| *x = WAVECOLOR[j]);
    }
    let im = RgbaImage::from_raw(nwidth, nheight, arr.into_raw_vec()).unwrap();
    im
    // resize(&im, nwidth/2, nheight/2, FilterType::Triangle)
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::Rgba;

    #[test]
    fn show_colorbar() {
        let colormap: Vec<Rgba<u8>> = COLORMAP.iter().map(|&x| Rgba(x)).collect();
        let mut im =
            RgbaImage::from_fn(50, colormap.len() as u32, |_, y| Rgba(COLORMAP[y as usize]));
        im = resize(&im, 50, 500, FilterType::Triangle);
        im.save("../../samples/colorbar.png").unwrap();
    }
}
