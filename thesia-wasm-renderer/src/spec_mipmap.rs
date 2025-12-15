use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::u8;

use fast_image_resize::images::{TypedImage, TypedImageRef};
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer, pixels};
use ndarray::Array2;
use ndarray::prelude::*;
use parking_lot::RwLock;
use wasm_bindgen::prelude::*;

use crate::mem::WasmU8Array;
use crate::mem::WasmU16Array;

static SPEC_MIPMAPS: LazyLock<RwLock<HashMap<String, Mipmaps>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

#[wasm_bindgen(js_name = setSpectrogram)]
pub fn set_spectrogram(id_ch_str: &str, spectrogram: WasmU16Array, width: u32, height: u32) {
    let vec = spectrogram.into();
    let vec = unsafe { std::mem::transmute::<Vec<u16>, Vec<pixels::U16>>(vec) };
    let spec_img = Array2::from_shape_vec((height as usize, width as usize), vec).unwrap();
    SPEC_MIPMAPS
        .write()
        .insert(id_ch_str.into(), Mipmaps::new(spec_img));
}

#[wasm_bindgen(js_name = getMipmap)]
pub fn get_mipmap(id_ch_str: &str, width: u32, height: u32) -> Option<WasmU8Array> {
    SPEC_MIPMAPS
        .read()
        .get(id_ch_str)
        .map(|mipmaps| serialize_2d_array(mipmaps.get_mipmap(width, height).view()))
}

fn serialize_2d_array(array: ArrayView2<f32>) -> WasmU8Array {
    let mut buf: Vec<u8> =
        Vec::with_capacity(2 * size_of::<u32>() + array.len() * size_of::<f32>());
    buf.extend_from_slice(&array.shape()[0].to_le_bytes());
    buf.extend_from_slice(&array.shape()[1].to_le_bytes());
    buf.extend_from_slice(to_byte_slice(&array.as_slice().unwrap()));
    WasmU8Array::from_vec(buf)
}

struct Mipmaps {
    orig_img: Array2<pixels::U16>,
    // mipmaps: Vec<Vec<Array2<f32>>>,
}

impl Mipmaps {
    pub fn new(orig_img: Array2<pixels::U16>) -> Self {
        Self {
            orig_img,
            // mipmaps: vec![vec![]],
        }
    }

    pub fn get_mipmap(&self, width: u32, height: u32) -> Array2<f32> {
        let (orig_height, orig_width) = (self.orig_img.shape()[0], self.orig_img.shape()[1]);
        let mipmap: CowArray<pixels::U16, Ix2> =
            if width != orig_width as u32 || height != orig_height as u32 {
                let mipmap = resize(self.orig_img.view(), width, height);
                mipmap.into()
            } else {
                self.orig_img.view().into()
            };
        mipmap.mapv(u16_to_f32)
    }
}

fn resize(img: ArrayView2<pixels::U16>, width: u32, height: u32) -> Array2<pixels::U16> {
    static RESIZE_OPT: LazyLock<ResizeOptions> = LazyLock::new(|| {
        ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3))
    });
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

        let mut dst_buf = vec![pixels::U16::new(0); width as usize * height as usize];
        let mut dst_img =
            TypedImage::<pixels::U16>::from_pixels_slice(width, height, &mut dst_buf).unwrap();
        resizer
            .resize_typed(&src_img, &mut dst_img, &*RESIZE_OPT)
            .unwrap();
        Array2::from_shape_vec((height as usize, width as usize), dst_buf).unwrap()
    })
}

fn u16_to_f32(x: pixels::U16) -> f32 {
    (x.0 as f32) / u16::MAX as f32
}

fn to_byte_slice<'a>(floats: &'a [f32]) -> &'a [u8] {
    unsafe { std::slice::from_raw_parts(floats.as_ptr() as *const _, floats.len() * 4) }
}
