use image::{
    imageops::{resize, FilterType},
    ImageBuffer, Luma, Rgb, RgbImage, RgbaImage,
};
use ndarray::prelude::*;
use ndarray_stats::QuantileExt;

pub type GreyF32Image = ImageBuffer<Luma<f32>, Vec<f32>>;

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
pub const WAVECOLOR: [u8; 4] = [200, 21, 103, 255];

fn convert_grey_to_color(x: f32) -> Rgb<u8> {
    assert!(x >= 0.);
    let position = (COLORMAP.len() as f32) * x;
    let index = position.floor() as usize;
    if index >= COLORMAP.len() - 1 {
        Rgb(COLORMAP[COLORMAP.len() - 1])
    } else {
        let ratio = position - index as f32;
        let mut color = [0u8; 3];
        for (i, (&a, &b)) in COLORMAP[index]
            .iter()
            .zip(COLORMAP[index + 1].iter())
            .enumerate()
        {
            color[i] = (ratio * b as f32 + (1. - ratio) * a as f32).round() as u8;
        }
        Rgb(color)
    }
}

pub fn spec_to_grey(spec: ArrayView2<f32>, max: f32, min: f32) -> GreyF32Image {
    GreyF32Image::from_fn(spec.shape()[0] as u32, spec.shape()[1] as u32, |x, y| {
        let db = spec[[x as usize, spec.shape()[1] - y as usize - 1]];
        Luma([((db - min) / (max - min)).max(0.).min(1.)])
    })
}

pub fn grey_to_rgb(grey: &GreyF32Image, nwidth: u32, nheight: u32) -> RgbImage {
    let resized = resize(grey, nwidth, nheight, FilterType::Lanczos3);
    RgbImage::from_fn(nwidth, nheight, |x, y| {
        convert_grey_to_color(resized.get_pixel(x, y)[0])
    })
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
    #[test]
    fn show_colorbar() {
        let colormap: Vec<Rgb<u8>> = COLORMAP.iter().map(|&x| Rgb(x)).collect();
        let mut im = RgbImage::from_fn(50, colormap.len() as u32, |_, y| Rgb(COLORMAP[y as usize]));
        im = resize(&im, 50, 500, FilterType::Triangle);
        im.save("colorbar.png").unwrap();
    }
}
