use wasm_bindgen::prelude::*;

mod mem;
mod overview;
mod simd;
mod wav;

// Import the `console.log` function from the browser's console
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub(crate) fn log(s: &str);
}

// Define a macro to provide `println!(..)`-style syntax for `console.log` logging.
#[allow(unused_macros)]
#[macro_export]
macro_rules! console_log {
    ( $( $t:tt )* ) => {
        crate::log(&format!( $( $t )* ))
    }
}
