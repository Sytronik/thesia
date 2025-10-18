import init, {greet, ThesiaRenderer} from "thesia-wasm-renderer";

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

/**
 * Wrapper for the WASM greet function.
 */
export function wasmGreet(name: string): void {
  if (!wasmInitialized) {
    throw new Error("WASM module has not been initialized. Please call initWasm() first.");
  }
  greet(name);
}

/**
 * Wrapper class for the Thesia WASM renderer.
 */
export class WasmRenderer {
  private renderer: ThesiaRenderer;

  constructor(width: number, height: number) {
    if (!wasmInitialized) {
      throw new Error("WASM module has not been initialized. Please call initWasm() first.");
    }
    this.renderer = new ThesiaRenderer(width, height);
  }

  get width(): number {
    return this.renderer.width;
  }

  get height(): number {
    return this.renderer.height;
  }

  render(): void {
    this.renderer.render();
  }

  free(): void {
    this.renderer.free();
  }
}

// Default export
export {ThesiaRenderer};
export default {
  initWasm,
  isWasmInitialized,
  wasmGreet,
  WasmRenderer,
};
