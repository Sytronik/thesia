import init, {WavDrawingOptions, draw_wav, set_wav} from "thesia-wasm-renderer";

let wasmInitialized = false;

/**
 * Initializes the WASM module.
 * This function must be called once before using other WASM functions.
 */
export async function initWasm(): Promise<void> {
  if (!wasmInitialized) {
    await init();
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

export function wasmSetWav(
  idChStr: string,
  wav: Float32Array,
  sr: number,
  scale: number,
  devicePixelRatio: number,
): void {
  if (!wasmInitialized) {
    throw new Error("WASM module has not been initialized. Please call initWasm() first.");
  }
  set_wav(idChStr, wav, sr, scale, devicePixelRatio);
}

/**
 * Wav drawing options for WASM renderer
 */
export interface WasmWavDrawingOptions {
  startSec: number;
  pxPerSec: number;
  ampRange: [number, number];
  color: string;
  scale: number;
  devicePixelRatio: number;
  offsetY?: number;
  clipValues?: [number, number] | null;
  needBorderForEnvelope?: boolean;
  needBorderForLine?: boolean;
  doClear?: boolean;
}

/**
 * Wrapper for the WASM draw_wav function.
 */
export function wasmDrawWav(
  ctx: CanvasRenderingContext2D,
  idChStr: string,
  options: WasmWavDrawingOptions,
): void {
  if (!wasmInitialized) {
    throw new Error("WASM module has not been initialized. Please call initWasm() first.");
  }

  const wasmOptions = new WavDrawingOptions(
    options.startSec,
    options.pxPerSec,
    options.ampRange[0],
    options.ampRange[1],
    options.color,
    options.scale,
    options.devicePixelRatio,
  );

  if (options.offsetY !== undefined) {
    wasmOptions.offset_y = options.offsetY;
  }

  if (options.clipValues) {
    wasmOptions.clip_values = new Float32Array(options.clipValues);
  }

  if (options.needBorderForEnvelope !== undefined) {
    wasmOptions.need_border_for_envelope = options.needBorderForEnvelope;
  }

  if (options.needBorderForLine !== undefined) {
    wasmOptions.need_border_for_line = options.needBorderForLine;
  }

  if (options.doClear !== undefined) {
    wasmOptions.do_clear = options.doClear;
  }

  draw_wav(ctx, idChStr, wasmOptions);
}

/**
 * Note: ThesiaRenderer class is no longer available.
 * Use wasmDrawWav function instead for drawing waveforms.
 */

// Named exports
export {WavDrawingOptions, draw_wav};
export default {
  initWasm,
  isWasmInitialized,
  wasmDrawWav,
  wasmSetWav,
};
