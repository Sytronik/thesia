use wasm_bindgen::prelude::*;

// Import the `console.log` function from the browser's console
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// Define a macro to provide `println!(..)`-style syntax for `console.log` logging.
macro_rules! console_log {
    ( $( $t:tt )* ) => {
        log(&format!( $( $t )* ))
    }
}

#[wasm_bindgen]
pub fn greet(name: &str) {
    console_log!("Hello, {}!", name);
}

#[wasm_bindgen]
pub struct ThesiaRenderer {
    width: u32,
    height: u32,
}

#[wasm_bindgen]
impl ThesiaRenderer {
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32) -> ThesiaRenderer {
        ThesiaRenderer { width, height }
    }

    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn render(&self) {
        console_log!("Rendering at {}x{}", self.width, self.height);
    }
}
