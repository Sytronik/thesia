use std::cell::RefCell;
// use std::time::Instant;

use fast_image_resize::images::{TypedImage, TypedImageRef};
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer, pixels};
use ndarray::prelude::*;

#[allow(non_snake_case)]
pub fn convert_spec_to_img(
    spec: ArrayView2<f32>,
    i_freq_range: (usize, usize),
    dB_range: (f32, f32),
    colormap_length: Option<u32>,
) -> Array2<pixels::F32> {
    // spec: T x F
    // return: image with F x T
    let (i_freq_start, i_freq_end) = i_freq_range;
    let dB_span = dB_range.1 - dB_range.0;
    let width = spec.shape()[0];
    let height = i_freq_end - i_freq_start;
    Array2::from_shape_fn((height, width), |(i, j)| {
        let i_freq = i_freq_start + i;
        if i_freq < spec.raw_dim()[1] {
            let zero_to_one = (spec[[j, i_freq]] - dB_range.0) / dB_span;
            let eps_to_one = if let Some(colormap_length) = colormap_length {
                (zero_to_one * (colormap_length - 1) as f32 + 1.0) / colormap_length as f32
            } else {
                zero_to_one
            };
            pixels::F32::new(eps_to_one.clamp(0., 1.))
        } else {
            pixels::F32::new(0.)
        }
    })
}

pub fn resize(img: ArrayView2<pixels::F32>, width: u32, height: u32) -> Array2<pixels::F32> {
    thread_local! {
        static RESIZER: RefCell<Resizer> = RefCell::new(Resizer::new());
    }

    RESIZER.with_borrow_mut(|resizer| {
        let src_img = TypedImageRef::new(
            img.shape()[1] as u32,
            img.shape()[0] as u32,
            img.as_slice_memory_order().unwrap(),
        )
        .unwrap();
        let resize_opt =
            ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));

        let mut dst_buf = vec![pixels::F32::new(0.); width as usize * height as usize];
        let mut dst_img =
            TypedImage::<pixels::F32>::from_pixels_slice(width, height, &mut dst_buf).unwrap();
        resizer
            .resize_typed(&src_img, &mut dst_img, &resize_opt)
            .unwrap();
        Array2::from_shape_vec((height as usize, width as usize), dst_buf).unwrap()
    })
}
