use std::cell::RefCell;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fs, mem};

use fast_image_resize::images::{TypedImage, TypedImageRef};
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer, pixels};
use memmap2::Mmap;
use ndarray::{SliceArg, prelude::*};
use ndarray_npy::{ViewNpyExt, write_npy};
use parking_lot::RwLock;
use temp_dir::TempDir;

use super::super::spectrogram::SpecSetting;

use super::slice_args::SpectrogramSliceArgs;

fn f32_to_u16(x: pixels::F32) -> u16 {
    (x.0 * u16::MAX as f32).round().clamp(0., u16::MAX as f32) as u16
}

fn u16_to_f32(x: u16) -> f32 {
    (x as f32) / u16::MAX as f32
}

#[derive(Clone, Copy)]
enum FileStatus {
    NoFile,
    Creating,
    Exists,
}

struct Mipmap {
    width: u32,
    height: u32,
    path: PathBuf,
    status: FileStatus,
}

impl Mipmap {
    fn new(width: u32, height: u32, dir: &Path) -> Self {
        let path = dir.join(format!("{}_{}.npy", width, height));
        Self {
            width,
            height,
            path,
            status: FileStatus::NoFile,
        }
    }

    fn read<I: SliceArg<Ix2, OutDim = Ix2>>(&self, slice: I) -> Array2<f32> {
        let file = File::open(&self.path).unwrap();
        let mmap = unsafe { Mmap::map(&file).unwrap() };
        let view = ArrayView2::<u16>::view_npy(&mmap).unwrap();
        view.slice(slice).mapv(u16_to_f32)
    }

    fn write(&mut self, img: ArrayView2<pixels::F32>) {
        write_npy(&self.path, &img.mapv(f32_to_u16)).unwrap();
        self.status = FileStatus::Exists;
    }

    fn has_file(&self) -> bool {
        matches!(self.status, FileStatus::Exists)
    }
}

pub struct Mipmaps {
    orig_img: Arc<Array2<pixels::F32>>,
    mipmaps: Arc<RwLock<Vec<Vec<Mipmap>>>>,
    max_size: u32,
    _tmp_dir: TempDir,
}

impl Mipmaps {
    pub fn new(spec_img: Array2<pixels::F32>, max_size: u32) -> Self {
        let _tmp_dir = TempDir::with_prefix("mipmaps").unwrap();
        let (orig_height, orig_width) = (spec_img.shape()[0], spec_img.shape()[1]);
        let mut mipmaps = vec![vec![Mipmap::new(
            orig_width as u32,
            orig_height as u32,
            &_tmp_dir.path(),
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
                    mipmaps[i].push(Mipmap::new(width_u32, height_u32, &_tmp_dir.path()));
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
            let last = mipmaps[i_last].last_mut().unwrap();
            let img = resize(spec_img.view(), last.width, last.height);
            last.write(img.view());
        }

        Self {
            orig_img: Arc::new(spec_img),
            mipmaps: Arc::new(RwLock::new(mipmaps)),
            max_size,
            _tmp_dir,
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
    ) -> (SpectrogramSliceArgs, Array2<f32>, bool) {
        let max_size = self.max_size as usize;
        let mut out_idx_img_args = None; // Some((i_h, i_w, args))
        let mut need_to_create = None; // Some((i_h, i_w, is_creating))
        for (i_h, spec_mipmap_along_widths) in self.mipmaps.read().iter().enumerate() {
            for (i_w, mipmap) in spec_mipmap_along_widths.iter().enumerate() {
                let args = SpectrogramSliceArgs::new(
                    mipmap.width as usize,
                    mipmap.height as usize,
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
                    if i_h == 0 && i_w == 0 || mipmap.has_file() {
                        let slice = s![
                            args.top..args.top + args.height,
                            args.left..args.left + args.width
                        ];
                        let sliced_img = if i_h == 0 && i_w == 0 {
                            let pixels = self.orig_img.slice(slice).to_owned();
                            unsafe { mem::transmute::<_, Array2<f32>>(pixels) }
                        } else if mipmap.has_file() {
                            mipmap.read(slice)
                        } else {
                            unreachable!();
                        };
                        out_idx_img_args = Some((i_h, i_w, sliced_img, args));
                        break;
                    } else if need_to_create.is_none() {
                        need_to_create = Some((i_h, i_w, mipmap.status));
                    }
                }
            }
            if out_idx_img_args.is_some() {
                break;
            }
        }
        if let Some((i_h_out, i_w_out, sliced_img, args)) = out_idx_img_args {
            // prune mipmaps
            {
                let mut mipmaps = self.mipmaps.write();
                for i_h in 0..mipmaps.len() {
                    for i_w in 0..mipmaps[i_h].len() {
                        if i_h == i_h_out && i_w == i_w_out {
                            continue;
                        }
                        if (i_h, i_w) == (mipmaps.len() - 1, mipmaps[i_h].len() - 1) {
                            continue;
                        }
                        if !mipmaps[i_h][i_w].has_file() {
                            continue;
                        }
                        if let Err(err) = fs::remove_file(&mipmaps[i_h][i_w].path) {
                            match err.kind() {
                                std::io::ErrorKind::NotFound => (),
                                _ => log::error!("Failed to remove mipmap: {}", err),
                            }
                        }
                        mipmaps[i_h][i_w].status = FileStatus::NoFile;
                    }
                }
            }
            if let Some((i_h, i_w, status)) = need_to_create {
                if matches!(status, FileStatus::NoFile) {
                    let (width, height) = {
                        let mut mipmaps = self.mipmaps.write();
                        mipmaps[i_h][i_w].status = FileStatus::Creating;
                        (mipmaps[i_h][i_w].width, mipmaps[i_h][i_w].height)
                    };
                    if (width, height) != (0, 0) {
                        let orig_img_clone = Arc::clone(&self.orig_img);
                        let mipmaps_clone = Arc::clone(&self.mipmaps);
                        rayon::spawn(move || {
                            let resized_img = resize(orig_img_clone.view(), width, height);
                            let mut mipmaps = mipmaps_clone.write();
                            mipmaps[i_h][i_w].write(resized_img.view());
                        });
                    }
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
