use image::{imageops, ImageBuffer, Luma};
use image::{Rgb, RgbImage};
use ndarray::prelude::*;

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
        let mut color = [0u8; 3];
        for (i, c) in color.iter_mut().enumerate() {
            *c = (ratio * COLORMAP[index + 1][i] as f32 + (1. - ratio) * COLORMAP[index][i] as f32)
                .round() as u8;
        }
        Rgb(color)
    }
}

pub fn spec_to_image(spec_db: ArrayView2<f32>, nwidth: u32, nheight: u32) -> RgbImage {
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
            Luma([
                (spec_db[[x as usize, spec_db.shape()[1] - y as usize - 1]] - min_db)
                    / (max_db - min_db),
            ])
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
