// Mock for WASM module
const MockWasmFloat32Array = jest.fn().mockImplementation((length) => ({
  ptr: 0,
  length,
  free: jest.fn(),
}));

export const WasmFloat32Array = MockWasmFloat32Array;

export async function initWasm() {
  return Promise.resolve();
}

export function isWasmInitialized() {
  return true;
}

export function createWasmFloat32Array(length) {
  const wasmWav = new MockWasmFloat32Array(length);
  const view = new Float32Array(length);
  return [wasmWav, view];
}

export const setDevicePixelRatio = jest.fn();
export const setWav = jest.fn();
export const drawWav = jest.fn();
export const clearWav = jest.fn();
export const drawOverview = jest.fn();
export const removeWav = jest.fn();

export default {
  initWasm,
  isWasmInitialized,
  createWasmFloat32Array,
  setDevicePixelRatio,
  setWav,
  drawWav,
  clearWav,
  drawOverview,
  removeWav,
};
