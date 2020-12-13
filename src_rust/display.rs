use image::{
    imageops::{resize, FilterType},
    ImageBuffer, Luma, Rgb, RgbImage,
};
use ndarray::prelude::*;

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

#[allow(dead_code)]
fn colorbar(length: u32) -> RgbImage {
    let colormap: Vec<Rgb<u8>> = COLORMAP.iter().map(|&x| Rgb(x)).collect();
    let im = RgbImage::from_fn(50, colormap.len() as u32, |_, y| Rgb(COLORMAP[y as usize]));
    resize(&im, 50, length, FilterType::Triangle)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn colorbar_works() {
        let im = colorbar(500);
        im.save("colorbar.png").unwrap();
    }
}
