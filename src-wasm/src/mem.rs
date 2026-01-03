use wasm_bindgen::prelude::*;

macro_rules! def_wasm_array {
    ($name:ident, $type:ty) => {
        #[wasm_bindgen]
        pub struct $name {
            ptr: *mut $type,
            len: usize,
            cap: usize,
        }

        #[wasm_bindgen]
        impl $name {
            /// Allocate an f32 buffer of length len in wasm memory (zero-initialized)
            #[wasm_bindgen(constructor)]
            pub fn new(len: usize) -> $name {
                // Fix capacity to len so that reallocation doesn't change the pointer
                // Leak the Vec (release ownership) so we own the 'raw' memory
                let mut v: Vec<$type> = Vec::with_capacity(len);
                let ptr = v.as_mut_ptr();
                let cap = v.capacity();
                core::mem::forget(v);
                // Initialize to 0 if needed
                // unsafe {
                //     core::ptr::write_bytes(ptr.cast::<u8>(), 0, cap * core::mem::size_of::<f32>());
                // }
                $name { ptr, len, cap }
            }

            #[allow(dead_code)]
            pub(crate) fn from_vec(mut vec: Vec<$type>) -> $name {
                let len = vec.len();
                let cap = vec.capacity();
                let ptr = vec.as_mut_ptr();
                core::mem::forget(vec);
                $name { ptr, len, cap }
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

        impl Drop for $name {
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

        impl From<$name> for Vec<$type> {
            #[inline(always)]
            fn from(mut value: $name) -> Self {
                let v = unsafe { Vec::from_raw_parts(value.ptr, value.len, value.cap) };
                unsafe { value.forget() };
                v
            }
        }
    };
}

def_wasm_array!(WasmFloat32Array, f32);
def_wasm_array!(WasmU16Array, u16);
def_wasm_array!(WasmU8Array, u8);
