import init, {
  WasmFloat32Array,
  WasmU16Array,
  WasmU8Array,
  setSpectrogram as _setSpectrogram,
  getMipmap as _getMipmap,
  setDevicePixelRatio,
  setWav,
  drawWav,
  clearWav,
  drawOverview,
  removeWav,
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
  const view = new Float32Array(memory.buffer, wasmWav.ptr, wasmWav.length);
  return [wasmWav, view];
}

export function createWasmU16Array(length: number): [WasmU16Array, Uint16Array] {
  if (!wasmInitialized) {
    throw new Error("WASM module has not been initialized. Please call initWasm() first.");
  }

  const wasmSpec = new WasmU16Array(length);
  const view = new Uint16Array(memory.buffer, wasmSpec.ptr, wasmSpec.length);
  return [wasmSpec, view];
}

export function setSpectrogram(idChStr: string, arr: Uint16Array, width: number, height: number): void {
  const [wasmArr, view] = createWasmU16Array(arr.length);
  view.set(arr);
  _setSpectrogram(
    idChStr,
    wasmArr,
    width,
    height,
  );
}

export type Mipmap = {
  arr: Float32Array;
  width: number;
  height: number;
};

export function getMipmap(idChStr: string, width: number, height: number): Mipmap | null {
  if (!wasmInitialized) {
    throw new Error(
      "WASM module has not been initialized. Please call initWasm() first."
    );
  }

  const info = _getMipmap(idChStr, width, height);
  if (!info) return null;

  const view = new DataView(memory.buffer, info.ptr, info.length);
  const mipmapHeight = view.getUint32(0, true);
  const mipmapWidth = view.getUint32(4, true);
  const arr = new Float32Array(memory.buffer, info.ptr + 8, (info.length - 8) / 4);
  return {
    arr,
    width: mipmapWidth,
    height: mipmapHeight,
  };
}

// Named exports
export { WasmFloat32Array, WasmU16Array, WasmU8Array };
export default {
  initWasm,
  isWasmInitialized,
  createWasmFloat32Array,
  createWasmU16Array,
  setSpectrogram,
  getMipmap,
  setDevicePixelRatio,
  setWav,
  drawWav,
  clearWav,
  drawOverview,
  removeWav,
};
