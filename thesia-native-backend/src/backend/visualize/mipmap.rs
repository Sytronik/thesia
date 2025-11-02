use std::cell::RefCell;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{io, mem};

use fast_image_resize::images::{TypedImage, TypedImageRef};
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer, pixels};
use memmap2::Mmap;
use ndarray::{SliceArg, prelude::*};
use ndarray_npy::{ViewNpyExt, WriteNpyError, write_npy};
use parking_lot::RwLock;
use tempfile::TempDir;

use super::super::spectrogram::SpecSetting;

use super::slice_args::SpectrogramSliceArgs;

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

    fn read<I: SliceArg<Ix2, OutDim = Ix2>>(&self, slice: I) -> io::Result<Array2<f32>> {
        let file = File::open(&self.path)?;
        let mmap = unsafe { Mmap::map(&file).unwrap() };
        let view = ArrayView2::<u16>::view_npy(&mmap).unwrap();
        Ok(view.slice(slice).mapv(u16_to_f32))
    }

    fn write(&mut self, img: ArrayView2<pixels::U16>) -> io::Result<()> {
        let arr = unsafe { mem::transmute::<ArrayView2<pixels::U16>, ArrayView2<u16>>(img) };
        match write_npy(&self.path, &arr) {
            Ok(_) => {
                self.status = FileStatus::Exists;
                Ok(())
            }
            Err(err) => {
                self.status = FileStatus::NoFile;
                match err {
                    WriteNpyError::Io(err) => Err(err),
                    _ => panic!("Failed to write mipmap: {:?}", err),
                }
            }
        }
    }

    fn move_to(&mut self, dir: &Path) -> bool {
        let new_path = dir.join(self.path.file_name().unwrap());
        if self.has_file() && fs::copy(&self.path, &new_path).is_err() {
            self.status = FileStatus::NoFile;
        }
        self.path = new_path;
        self.has_file()
    }

    fn remove(&mut self) -> io::Result<()> {
        if self.has_file() {
            self.status = FileStatus::NoFile;
            fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    fn has_file(&self) -> bool {
        matches!(self.status, FileStatus::Exists)
    }
}

pub struct Mipmaps {
    orig_img: Arc<Array2<pixels::U16>>,
    mipmaps: Arc<RwLock<Vec<Vec<Mipmap>>>>,
    max_size: u32,
    _tmp_dir: TempDir,
}

impl Mipmaps {
    pub fn new(spec_img: Array2<pixels::U16>, max_size: u32, dir: &Path) -> io::Result<Self> {
        let _tmp_dir = TempDir::new_in(dir)?;
        let (orig_height, orig_width) = (spec_img.shape()[0], spec_img.shape()[1]);
        let mut mipmaps = vec![vec![Mipmap::new(
            orig_width as u32,
            orig_height as u32,
            _tmp_dir.path(),
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
                    mipmaps[i].push(Mipmap::new(width_u32, height_u32, _tmp_dir.path()));
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

        let _self = Self {
            orig_img: Arc::new(spec_img),
            mipmaps: Arc::new(RwLock::new(mipmaps)),
            max_size,
            _tmp_dir,
        };
        _self.ensure_last_mipmap_exists()?;
        Ok(_self)
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
        for (i_h, mipmaps_along_width) in self.mipmaps.read().iter().enumerate() {
            for (i_w, mipmap) in mipmaps_along_width.iter().enumerate() {
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
                        let sliced_arr = if i_h == 0 && i_w == 0 {
                            let pixels = self.orig_img.slice(slice);
                            let arr = unsafe {
                                mem::transmute::<ArrayView2<pixels::U16>, ArrayView2<u16>>(pixels)
                            };
                            arr.mapv(u16_to_f32)
                        } else if mipmap.has_file() {
                            mipmap.read(slice).unwrap()
                        } else {
                            unreachable!();
                        };
                        out_idx_img_args = Some((i_h, i_w, sliced_arr, args));
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
                        if let Err(err) = mipmaps[i_h][i_w].remove() {
                            log::error!("Failed to remove mipmap: {:?}", err);
                        }
                    }
                }
            }
            if let Some((i_h, i_w, status)) = need_to_create
                && matches!(status, FileStatus::NoFile)
            {
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
                        if let Err(err) = mipmaps[i_h][i_w].write(resized_img.view()) {
                            log::error!("Failed to write mipmap: {:?}", err);
                        }
                    });
                }
            }

            (args, sliced_img, need_to_create.is_some())
        } else {
            panic!("No mipmap found!");
        }
    }

    pub fn move_to(&mut self, dir: &Path) -> io::Result<()> {
        let _tmp_dir = TempDir::new_in(dir)?;
        for mipmaps_along_width in self.mipmaps.write().iter_mut() {
            for mipmap in mipmaps_along_width.iter_mut() {
                mipmap.move_to(_tmp_dir.path());
            }
        }
        self.ensure_last_mipmap_exists()?;
        self._tmp_dir = _tmp_dir;
        Ok(())
    }

    fn ensure_last_mipmap_exists(&self) -> io::Result<()> {
        let mut mipmaps = self.mipmaps.write();
        if mipmaps.len() > 1 || mipmaps[0].len() > 1 {
            let i_last = mipmaps.len() - 1;
            let last = mipmaps[i_last].last_mut().unwrap();
            if last.has_file() {
                return Ok(());
            }
            let img = resize(self.orig_img.view(), last.width, last.height);
            last.write(img.view())?;
        }
        Ok(())
    }
}

fn resize(img: ArrayView2<pixels::U16>, width: u32, height: u32) -> Array2<pixels::U16> {
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

        let mut dst_buf = vec![pixels::U16::new(0); width as usize * height as usize];
        let mut dst_img =
            TypedImage::<pixels::U16>::from_pixels_slice(width, height, &mut dst_buf).unwrap();
        resizer
            .resize_typed(&src_img, &mut dst_img, &resize_opt)
            .unwrap();
        Array2::from_shape_vec((height as usize, width as usize), dst_buf).unwrap()
    })
}
