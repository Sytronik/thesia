use fast_image_resize::pixels;
use ndarray::prelude::*;

use super::super::spectrogram::SpecSetting;

use super::drawing::resize;
use super::img_slice::SpectrogramSliceArgs;

pub struct Mipmaps {
    mipmaps: Vec<Vec<Array2<pixels::F32>>>,
    max_size: u32,
}

impl Mipmaps {
    pub fn new(spec_img: Array2<pixels::F32>, max_size: u32) -> Self {
        let (orig_height, orig_width) = (spec_img.shape()[0], spec_img.shape()[1]);
        let mut mipmaps = vec![vec![spec_img]];
        let mut skip = true; // skip the first (original) mipmap
        let mut height = orig_height as f64;
        loop {
            let mut width = orig_width as f64;
            if !skip {
                mipmaps.push(vec![]);
            }
            loop {
                if skip {
                    skip = false;
                } else {
                    let resized = resize(
                        mipmaps[0][0].view(),
                        width.round() as u32,
                        height.round() as u32,
                    );
                    let i = mipmaps.len() - 1;
                    mipmaps[i].push(resized);
                }
                if (width.round() as u32) == max_size {
                    break;
                }
                width /= 2.;
                if (width.round() as u32) < max_size {
                    width = max_size as f64;
                }
            }
            if (height.round() as u32) == max_size {
                break;
            }
            height /= 2.;
            if (height.round() as u32) < max_size {
                height = max_size as f64;
            }
        }
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
