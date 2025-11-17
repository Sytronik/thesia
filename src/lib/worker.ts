import { WasmAPI  } from "../api";
import type { IdChannel, WavInfo } from "../api/backend-wrapper";

type InitMessage = {
  type: "init";
  data: {
    idChStr: IdChannel;
    canvas: OffscreenCanvas;
    alpha?: boolean;
  };
};

type SetDevicePixelRatioMessage = {
  type: "setDevicePixelRatio";
  data: {
    devicePixelRatio: number;
  };
};

type SetWavMessage = {
  type: "setWav";
  data: {
    idChStr: IdChannel;
    wavInfo: WavInfo;
  };
};

type RemoveWavMessage = {
  type: "removeWav";
  data: {
    idChStr: IdChannel;
  };
};

type DrawWavMessage = {
  type: "drawWav";
  data: {
    idChStr: IdChannel;
    width: number;
    height: number;
    startSec: number;
    pxPerSec: number;
    ampRange: [number, number];
  };
};

type ClearWavMessage = {
  type: "clearWav";
  data: {
    idChStr: IdChannel;
    width: number;
    height: number;
  };
};

export type RendererWorkerMessage =
  | InitMessage
  | SetDevicePixelRatioMessage
  | SetWavMessage
  | RemoveWavMessage
  | DrawWavMessage
  | ClearWavMessage;

let initialized = false;
const canvases: Map<string, OffscreenCanvas> = new Map();
const ctxs: Map<string, OffscreenCanvasRenderingContext2D> = new Map();
const msgQueue: RendererWorkerMessage[] = [];

self.onmessage = (event: MessageEvent<RendererWorkerMessage>) => {
  const message = event.data;
  if (message.type === "init") {
    const { idChStr, canvas, alpha } = message.data;
    const ctx = canvas.getContext("2d", {
      alpha: alpha === undefined ? true : alpha,
      desynchronized: true,
    });
    if (!ctx) {
      console.error("failed to get 2d context for canvas", idChStr);
      return;
    }
    canvases.set(idChStr, canvas);
    ctxs.set(idChStr, ctx);
    return;
  }

  msgQueue.push(message);
  if (!initialized) return;

  while (msgQueue.length > 0) {
    const message = msgQueue.shift();
    if (!message) break;
    switch (message.type) {
      case "setDevicePixelRatio":
        WasmAPI.setDevicePixelRatio(message.data.devicePixelRatio);
        break;
      case "setWav": {
        const { idChStr, wavInfo } = message.data;
        const { wavArr, sr, isClipped } = wavInfo;
        const [wavWasmArr, view] = WasmAPI.createWasmFloat32Array(wavArr.length);
        view.set(wavArr);
        WasmAPI.setWav(idChStr, wavWasmArr, sr, isClipped);
        break;
      }
      case "removeWav":
        WasmAPI.removeWav(message.data.idChStr);
        break;
      case "drawWav": {
        const { idChStr, width, height, startSec, pxPerSec, ampRange } = message.data;
        const canvas = canvases.get(idChStr);
        const ctx = ctxs.get(idChStr);
        if (!canvas || !ctx) break;
        WasmAPI.drawWav(
          canvas,
          ctx,
          idChStr,
          width,
          height,
          startSec,
          pxPerSec,
          ampRange[0],
          ampRange[1]
        );
        break;
      }
      case "clearWav": {
        const { idChStr, width, height } = message.data;
        const canvas = canvases.get(idChStr);
        const ctx = ctxs.get(idChStr);
        if (!canvas || !ctx) break;
        WasmAPI.clearWav(canvas, ctx, width, height);
        break;
      }
      default: {
        console.error("unknown message type", message.type);
        break;
      }
    }
  }
};

await WasmAPI.initWasm();
await WasmAPI.initThreadPool(navigator.hardwareConcurrency ?? 1);
initialized = true;
