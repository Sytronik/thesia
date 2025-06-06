use std::cell::RefCell;

use fast_image_resize::images::{TypedImage, TypedImageRef};
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer, pixels};
use ndarray::prelude::*;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use super::super::spectrogram::SpecSetting;

use super::slice_args::SpectrogramSliceArgs;

pub struct Mipmaps {
    mipmaps: Vec<Vec<Array2<pixels::F32>>>,
    max_size: u32,
}

impl Mipmaps {
    pub fn new(spec_img: Array2<pixels::F32>, max_size: u32) -> Self {
        let (orig_height, orig_width) = (spec_img.shape()[0], spec_img.shape()[1]);
        let mut sizes = vec![vec![(orig_width as u32, orig_height as u32)]];
        let mut skip = true; // skip the first (original) mipmap
        let mut height = orig_height as f64;
        loop {
            if !skip {
                sizes.push(vec![]);
            }
            let height_u32 = height.round() as u32;
            let mut width = orig_width as f64;
            loop {
                let width_u32 = width.round() as u32;
                if skip {
                    skip = false;
                } else {
                    let i = sizes.len() - 1;
                    sizes[i].push((width_u32, height_u32));
                }
                if (width_u32) == max_size {
                    break;
                }
                width /= 2.;
                if (width_u32) < max_size {
                    width = max_size as f64;
                }
            }
            if (height_u32) == max_size {
                break;
            }
            height /= 2.;
            if (height_u32) < max_size {
                height = max_size as f64;
            }
        }

        let mut mipmaps: Vec<Vec<_>> = sizes
            .into_par_iter()
            .map(|_sizes| {
                _sizes
                    .into_par_iter()
                    .map(|(width, height)| resize(spec_img.view(), width, height))
                    .collect()
            })
            .collect();
        mipmaps[0].insert(0, spec_img);
        Self { mipmaps, max_size }
    }

    pub fn get_sliced_mipmap(
        &self,
        track_sec: f64,
        sec_range: (f64, f64),
        spec_hz_range: (f32, f32),
        hz_range: (f32, f32),
        margin_px: usize,
        spec_setting: &SpecSetting,
    ) -> (SpectrogramSliceArgs, ArrayView2<pixels::F32>) {
        let max_size = self.max_size as usize;
        for spec_mipmap_along_widths in &self.mipmaps {
            for mipmap in spec_mipmap_along_widths {
                let args = SpectrogramSliceArgs::new(
                    mipmap.shape()[1],
                    mipmap.shape()[0],
                    track_sec,
                    sec_range,
                    spec_hz_range,
                    hz_range,
                    margin_px,
                    spec_setting,
                );
                if args.height > max_size {
                    break;
                }
                if args.width <= max_size {
                    let sliced_mipmap = mipmap.slice(s![
                        args.top..args.top + args.height,
                        args.left..args.left + args.width
                    ]);
                    return (args, sliced_mipmap);
                }
            }
        }
        panic!("No mipmap found!");
    }
}

fn resize(img: ArrayView2<pixels::F32>, width: u32, height: u32) -> Array2<pixels::F32> {
    thread_local! {
        static RESIZER: RefCell<Resizer> = RefCell::new(Resizer::new());
    }

    RESIZER.with_borrow_mut(|resizer| {
        let src_img = TypedImageRef::new(
            img.shape()[1] as u32,
            img.shape()[0] as u32,
            img.as_slice().unwrap(),
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
