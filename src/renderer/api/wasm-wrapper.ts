import init, {
  drawWav,
  setWav,
  getWavImgScale,
  setDevicePixelRatio,
  WasmFloat32Array,
} from "thesia-wasm-renderer";

let wasmInitialized = false;
let memory: WebAssembly.Memory;

/**
 * Initializes the WASM module.
 * This function must be called once before using other WASM functions.
 */
export async function initWasm(): Promise<void> {
  if (!wasmInitialized) {
    const wasm = await init();
    memory = wasm.memory;

    wasmInitialized = true;
    console.log("WASM module has been initialized.");
  }
}

/**
 * Checks if the WASM module has been initialized.
 */
export function isWasmInitialized(): boolean {
  return wasmInitialized;
}

export function createWasmFloat32Array(length: number): [WasmFloat32Array, Float32Array] {
  if (!wasmInitialized) {
    throw new Error("WASM module has not been initialized. Please call initWasm() first.");
  }

  const wasmWav = new WasmFloat32Array(length);
  let view = new Float32Array(memory.buffer, wasmWav.ptr, wasmWav.length);
  return [wasmWav, view];
}

// Named exports
export {WasmFloat32Array};
export default {
  initWasm,
  isWasmInitialized,
  drawWav,
  setWav,
  getWavImgScale,
  setDevicePixelRatio,
  createWasmFloat32Array,
};
