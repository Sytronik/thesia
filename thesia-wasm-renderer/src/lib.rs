use wasm_bindgen::prelude::*;

mod simd;
mod wav;

pub use crate::wav::{
    WavDrawingOptions, draw_wav, get_wav_img_scale, set_device_pixel_ratio, set_wav,
};

// Import the `console.log` function from the browser's console
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// Define a macro to provide `println!(..)`-style syntax for `console.log` logging.
#[allow(unused_macros)]
macro_rules! console_log {
    ( $( $t:tt )* ) => {
        log(&format!( $( $t )* ))
    }
}
