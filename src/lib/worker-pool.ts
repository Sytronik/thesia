import WasmWorker from "./worker?worker";
import type {
  WasmWorkerEventMessage,
  WasmWorkerMessage,
  OnReturnMipmapCallback,
  OnSetSpectrogramDoneCallback,
} from "./worker";

export const NUM_WORKERS = navigator.hardwareConcurrency ?? 1;
const setSpectrogramDoneListeners = new Map<number, Map<string, OnSetSpectrogramDoneCallback>>();
const returnSpectrogramListeners = new Map<number, Map<string, OnReturnMipmapCallback>>();

let workers: Map<number, Worker>;
initializeWorkerPool();

export function initializeWorkerPool() {
  workers = new Map<number, Worker>(
    Array.from({length: NUM_WORKERS}, (_, i) => {
      const worker = new WasmWorker();
      if (!setSpectrogramDoneListeners.has(i)) setSpectrogramDoneListeners.set(i, new Map());
      if (!returnSpectrogramListeners.has(i)) returnSpectrogramListeners.set(i, new Map());
      worker.onmessage = (event) => onmessage(i, event);
      return [i, worker];
    }),
  );
}

export const postMessageToWorker = (
  index: number,
  message: WasmWorkerMessage,
  transferList: Transferable[] = [],
) => {
  workers.get(index)?.postMessage(message, transferList);
};

export const onSetSpectrogramDone = (
  index: number,
  idChStr: string,
  callback: OnSetSpectrogramDoneCallback,
) => {
  if (!setSpectrogramDoneListeners.has(index)) {
    setSpectrogramDoneListeners.set(index, new Map());
  }
  setSpectrogramDoneListeners.get(index)?.set(idChStr, callback);
  return () => {
    setSpectrogramDoneListeners.get(index)?.delete(idChStr);
  };
};

export const onReturnMipmap = (
  index: number,
  idChStr: string,
  callback: OnReturnMipmapCallback,
) => {
  if (!returnSpectrogramListeners.has(index)) {
    returnSpectrogramListeners.set(index, new Map());
  }
  returnSpectrogramListeners.get(index)?.set(idChStr, callback);
  return () => {
    returnSpectrogramListeners.get(index)?.delete(idChStr);
  };
};

function onmessage(index: number, event: MessageEvent<WasmWorkerEventMessage>) {
  const {type, data} = event.data;
  switch (type) {
    case "setSpectrogramDone": {
      const listeners = setSpectrogramDoneListeners.get(index);
      if (listeners) {
        listeners.get(data.idChStr)?.();
      }
      break;
    }
    case "returnMipmap": {
      const listeners = returnSpectrogramListeners.get(index);
      if (listeners) {
        listeners.get(data.idChStr)?.(data.mipmap);
      }
      break;
    }
    default: {
      console.error("unknown message type", type);
      break;
    }
  }
}
