use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmFloat32Array {
    ptr: *mut f32,
    len: usize,
    cap: usize,
}

#[wasm_bindgen]
impl WasmFloat32Array {
    /// Allocate an f32 buffer of length len in wasm memory (zero-initialized)
    #[wasm_bindgen(constructor)]
    pub fn new(len: usize) -> WasmFloat32Array {
        // Fix capacity to len so that reallocation doesn't change the pointer
        // Leak the Vec (release ownership) so we own the 'raw' memory
        let mut v: Vec<f32> = Vec::with_capacity(len);
        let ptr = v.as_mut_ptr();
        let cap = v.capacity();
        core::mem::forget(v);
        // Initialize to 0 if needed
        // unsafe {
        //     core::ptr::write_bytes(ptr.cast::<u8>(), 0, cap * core::mem::size_of::<f32>());
        // }
        WasmFloat32Array { ptr, len, cap }
    }

    /// Pointer (byte offset) for JS to create a view. u32 is sufficient for wasm32
    #[wasm_bindgen(getter)]
    pub fn ptr(&self) -> u32 {
        self.ptr as u32
    }

    /// Number of elements
    #[wasm_bindgen(getter)]
    pub fn length(&self) -> usize {
        self.len
    }

    #[inline(always)]
    unsafe fn forget(&mut self) {
        self.cap = 0;
    }
}

impl Drop for WasmFloat32Array {
    #[inline(always)]
    fn drop(&mut self) {
        if self.cap == 0 {
            return;
        }
        unsafe {
            let _ = Vec::from_raw_parts(self.ptr, self.len, self.cap);
            self.forget();
        }
    }
}

impl From<WasmFloat32Array> for Vec<f32> {
    #[inline(always)]
    fn from(mut value: WasmFloat32Array) -> Self {
        let v = unsafe { Vec::from_raw_parts(value.ptr, value.len, value.cap) };
        unsafe { value.forget() };
        v
    }
}
