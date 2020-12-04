use image::{imageops, ImageBuffer, Luma};
use image::{Rgb, RgbImage};
use ndarray::prelude::*;
use rustfft::num_traits::Float;
use std::convert::TryInto;
use std::ops::*;

const COLORMAP: [[u8; 3]; 10] = [
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
        let color_vec: Vec<u8> = COLORMAP[index]
            .iter()
            .zip(COLORMAP[index + 1].iter())
            .map(|(&a, &b)| (ratio * b as f32 + (1. - ratio) * a as f32).round() as u8)
            .collect();
        Rgb(color_vec[..].try_into().unwrap())
    }
}

pub fn spec_to_image<A: Float + Sub + Div>(
    spec_db: &Array2<A>,
    nwidth: u32,
    nheight: u32,
) -> RgbImage {
    // let im = RgbImage::from_fn(
    //     spec_db.shape()[0] as u32, spec_db.shape()[1] as u32,
    //     |x, y| {
    //         convert_db_to_color(spec_db[[y as usize, x as usize]].to_f32().unwrap())
    //     }
    // );
    // imageops::resize(&im, nwidth, nheight, imageops::FilterType::Lanczos3)
    let min_db = -100f32;
    let max_db = 20f32;
    let im = ImageBuffer::<Luma<f32>, Vec<f32>>::from_fn(
        spec_db.shape()[0] as u32,
        spec_db.shape()[1] as u32,
        |x, y| {
            Luma([(spec_db[[x as usize, spec_db.shape()[1] - y as usize - 1]]
                .to_f32()
                .unwrap()
                - min_db)
                / (max_db - min_db)])
        },
    );
    let im = imageops::resize(&im, nwidth, nheight, imageops::FilterType::Lanczos3);
    RgbImage::from_fn(nwidth, nheight, |x, y| {
        convert_grey_to_color(im.get_pixel(x, y)[0])
    })
}

pub fn colorbar(length: u32) -> RgbImage {
    let colormap: Vec<Rgb<u8>> = COLORMAP.iter().map(|&x| Rgb(x)).collect();
    let im = RgbImage::from_fn(50, colormap.len() as u32, |_, y| Rgb(COLORMAP[y as usize]));
    imageops::resize(&im, 50, length, imageops::FilterType::Triangle)
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
