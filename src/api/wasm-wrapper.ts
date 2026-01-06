import init, {
  WasmFloat32Array,
  WasmU16Array,
  WasmU8Array,
  setSpectrogram as _setSpectrogram,
  removeSpectrogram,
  getMipmap as _getMipmap,
  setDevicePixelRatio,
  setWav,
  drawWav,
  clearWav,
  drawOverview,
  removeWav,
  calcTimeAxisMarkers as _calcTimeAxisMarkers,
  calcFreqAxisMarkers as _calcFreqAxisMarkers,
  calcAmpAxisMarkers as _calcAmpAxisMarkers,
  calcDbAxisMarkers as _calcDbAxisMarkers,
  secondsToLabel,
  timeLabelToSeconds,
  hzToLabel,
  freqLabelToHz,
  freqPosToHz,
  freqHzToPos,
} from "thesia-wasm-module";
import { FreqScale } from "./backend-wrapper";

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

export function setSpectrogram(
  idChStr: string,
  arr: Uint16Array,
  width: number,
  height: number,
): void {
  const [wasmArr, view] = createWasmU16Array(arr.length);
  view.set(arr);
  _setSpectrogram(idChStr, wasmArr, width, height);
}

export type Mipmap = {
  arr: Float32Array;
  width: number;
  height: number;
};

export function getMipmap(idChStr: string, width: number, height: number): Mipmap | null {
  if (!wasmInitialized) {
    throw new Error("WASM module has not been initialized. Please call initWasm() first.");
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

export type TickPxPosition = number;
export type TickLabel = string;
export type Markers = [TickPxPosition, TickLabel][];
export type MarkerDrawOption = {
  startSec?: number;
  endSec?: number;
  maxSec?: number;
  freqScale?: FreqScale;
  hzRange?: [number, number];
  maxTrackHz?: number;
  ampRange?: [number, number];
  mindB?: number;
  maxdB?: number;
};

export function calcTimeAxisMarkers(
  subTickSec: number,
  subTickUnitCount: number,
  markerDrawOptions?: MarkerDrawOption,
): Markers {
  const { startSec, endSec, maxSec } = markerDrawOptions || {};

  if (startSec === undefined || endSec === undefined || maxSec === undefined) {
    console.error("no markerDrawOptions for time axis exist");
    return [];
  }
  return _calcTimeAxisMarkers(startSec, endSec, subTickSec, subTickUnitCount, maxSec);
}

/* track axis */
export function calcFreqAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  markerDrawOptions?: MarkerDrawOption,
): Markers {
  const { maxTrackHz, freqScale, hzRange } = markerDrawOptions || {};

  if (maxTrackHz === undefined || freqScale === undefined || hzRange === undefined) {
    console.error("no markerDrawOptions for freq axis exist");
    return [];
  }
  return _calcFreqAxisMarkers(
    hzRange[0],
    hzRange[1],
    freqScale,
    maxNumTicks,
    maxNumLabels,
    maxTrackHz,
  );
}

export function calcAmpAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  markerDrawOptions?: MarkerDrawOption,
): Markers {
  const { ampRange } = markerDrawOptions || {};

  if (!ampRange) {
    console.error("no markerDrawOption for amp axis exist");
    return [];
  }

  return _calcAmpAxisMarkers(maxNumTicks, maxNumLabels, ampRange[0], ampRange[1]);
}

export function calcDbAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  markerDrawOptions?: MarkerDrawOption,
): Markers {
  const { mindB, maxdB } = markerDrawOptions || {};

  if (mindB === undefined || maxdB === undefined) {
    console.error("no markerDrawOptions for dB axis exist");
    return [];
  }

  return _calcDbAxisMarkers(maxNumTicks, maxNumLabels, mindB, maxdB);
}

// Named exports
export { WasmFloat32Array, WasmU16Array, WasmU8Array };
export default {
  initWasm,
  isWasmInitialized,
  createWasmFloat32Array,
  createWasmU16Array,
  setSpectrogram,
  removeSpectrogram,
  getMipmap,
  setDevicePixelRatio,
  setWav,
  drawWav,
  clearWav,
  drawOverview,
  removeWav,
  calcTimeAxisMarkers,
  calcFreqAxisMarkers,
  calcAmpAxisMarkers,
  calcDbAxisMarkers,
  secondsToLabel,
  timeLabelToSeconds,
  hzToLabel,
  freqLabelToHz,
  freqPosToHz,
  freqHzToPos,
};
