use std::cell::RefCell;
use std::sync::Arc;

use fast_image_resize::images::{TypedImage, TypedImageRef};
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer, pixels};
use ndarray::prelude::*;
use parking_lot::RwLock;

use super::super::spectrogram::SpecSetting;

use super::slice_args::SpectrogramSliceArgs;

enum Mipmap {
    WidthHeight(u32, u32),
    Img(Array2<pixels::F32>),
}

impl Mipmap {
    fn is_img(&self) -> bool {
        matches!(self, Mipmap::Img(_))
    }

    fn get_width_height(&self) -> (u32, u32) {
        match self {
            Mipmap::WidthHeight(width, height) => (*width, *height),
            Mipmap::Img(img) => (img.shape()[1] as u32, img.shape()[0] as u32),
        }
    }
}

pub struct Mipmaps {
    orig_img: Arc<Array2<pixels::F32>>,
    mipmaps: Arc<RwLock<Vec<Vec<(Mipmap, bool)>>>>,
    max_size: u32,
}

impl Mipmaps {
    pub fn new(spec_img: Array2<pixels::F32>, max_size: u32) -> Self {
        let (orig_height, orig_width) = (spec_img.shape()[0], spec_img.shape()[1]);
        let mut mipmaps = vec![vec![(
            Mipmap::WidthHeight(orig_width as u32, orig_height as u32),
            false,
        )]];
        let mut skip = true; // skip the first (original) mipmap
        let mut height = orig_height as f64;
        loop {
            if !skip {
                mipmaps.push(vec![]);
            }
            let height_u32 = height.round() as u32;
            let mut width = orig_width as f64;
            loop {
                let width_u32 = width.round() as u32;
                if skip {
                    skip = false;
                } else {
                    let i = mipmaps.len() - 1;
                    mipmaps[i].push((Mipmap::WidthHeight(width_u32, height_u32), false));
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
        if mipmaps.len() > 1 || mipmaps[0].len() > 1 {
            let i_last = mipmaps.len() - 1;
            let last = mipmaps[i_last].pop().unwrap();
            if let (Mipmap::WidthHeight(width, height), _) = last {
                mipmaps[i_last].push((Mipmap::Img(resize(spec_img.view(), width, height)), false));
            }
        }

        Self {
            orig_img: Arc::new(spec_img),
            mipmaps: Arc::new(RwLock::new(mipmaps)),
            max_size,
        }
    }

    pub fn get_sliced_mipmap(
        &self,
        track_sec: f64,
        sec_range: (f64, f64),
        spec_hz_range: (f32, f32),
        hz_range: (f32, f32),
        margin_px: usize,
        spec_setting: &SpecSetting,
    ) -> (SpectrogramSliceArgs, Array2<pixels::F32>, bool) {
        let max_size = self.max_size as usize;
        let mut out_idx_img_args = None; // Some((i_h, i_w, args))
        let mut need_to_create = None; // Some((i_h, i_w, is_creating))
        for (i_h, spec_mipmap_along_widths) in self.mipmaps.read().iter().enumerate() {
            for (i_w, (mipmap, is_creating)) in spec_mipmap_along_widths.iter().enumerate() {
                let (n_frames, n_freqs) = match mipmap {
                    Mipmap::WidthHeight(width, height) => (*width as usize, *height as usize),
                    Mipmap::Img(img) => (img.shape()[1], img.shape()[0]),
                };
                let args = SpectrogramSliceArgs::new(
                    n_frames,
                    n_freqs,
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
                    if i_h == 0 && i_w == 0 || mipmap.is_img() {
                        let img = if i_h == 0 && i_w == 0 {
                            self.orig_img.view()
                        } else if let Mipmap::Img(img) = &mipmap {
                            img.view()
                        } else {
                            unreachable!();
                        };
                        let sliced_img = img
                            .slice(s![
                                args.top..args.top + args.height,
                                args.left..args.left + args.width
                            ])
                            .to_owned();
                        out_idx_img_args = Some((i_h, i_w, sliced_img, args));
                        break;
                    } else if need_to_create.is_none() {
                        need_to_create = Some((i_h, i_w, *is_creating));
                    }
                }
            }
            if out_idx_img_args.is_some() {
                break;
            }
        }
        if let Some((i_h_out, i_w_out, sliced_img, args)) = out_idx_img_args {
            log::info!("return_idx: {}, {}", i_h_out, i_w_out);
            // prune mipmaps
            {
                let mut mipmaps = self.mipmaps.write();
                for i_h in 0..mipmaps.len() {
                    for i_w in 0..mipmaps[i_h].len() {
                        if (i_h_out.max(1) - 1..=i_h_out + 1).contains(&i_h)
                            && (i_w_out.max(1) - 1..=i_w_out + 1).contains(&i_w)
                        {
                            continue;
                        }
                        if (i_h, i_w) == (mipmaps.len() - 1, mipmaps[i_h].len() - 1) {
                            continue;
                        }
                        if !mipmaps[i_h][i_w].0.is_img() || mipmaps[i_h][i_w].1 {
                            continue;
                        }
                        let (w, h) = mipmaps[i_h][i_w].0.get_width_height();
                        mipmaps[i_h][i_w] = (Mipmap::WidthHeight(w, h), false);
                        log::info!("pruned mipmap: {}, {}", i_h, i_w);
                    }
                }
            }
            if let Some((i_h, i_w, is_creating)) = need_to_create {
                if !is_creating {
                    log::info!("creating mipmap: {}, {}", i_h, i_w);
                    let (width, height) = {
                        let mut mipmaps = self.mipmaps.write();
                        mipmaps[i_h][i_w].1 = true; // mark as creating
                        if let (Mipmap::WidthHeight(width, height), _) = &mipmaps[i_h][i_w] {
                            (*width, *height)
                        } else {
                            unreachable!();
                        }
                    };
                    let orig_img_clone = Arc::clone(&self.orig_img);
                    let mipmaps_clone = Arc::clone(&self.mipmaps);
                    rayon::spawn(move || {
                        let resized_img = resize(orig_img_clone.view(), width, height);
                        let mut mipmaps = mipmaps_clone.write();
                        mipmaps[i_h][i_w] = (Mipmap::Img(resized_img), false); // mark as created
                        log::info!("created mipmap: {}, {}", i_h, i_w);
                    });
                }
            }

            (args, sliced_img, need_to_create.is_some())
        } else {
            panic!("No mipmap found!");
        }
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
