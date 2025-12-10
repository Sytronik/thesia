import RendererWorker from "./worker?worker";
import type { RendererWorkerMessage } from "./worker";

export const NUM_WORKERS = navigator.hardwareConcurrency ?? 1;
const workers = new Map<number, Worker>(
  Array.from({length: NUM_WORKERS}, (_, i) => [i, new RendererWorker()])
);

const setSpectrogramDoneListeners = new Map<number, Map<string, () => void>>();
const returnSpectrogramListeners = new Map<number, Map<string, (message: any) => void>>();

workers.forEach((worker, index) => {
  setSpectrogramDoneListeners.set(index, new Map());
  returnSpectrogramListeners.set(index, new Map());
  worker.onmessage = (event: MessageEvent<any>) => {
    const { type, data } = event.data;
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
  };
});

export const postMessageToWorker = (
  index: number,
  message: RendererWorkerMessage,
  transferList: Transferable[] = []
) => {
  workers.get(index)?.postMessage(message, transferList);
};

export const onSetSpectrogramDone = (
  index: number,
  idChStr: string,
  callback: () => void
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
  callback: (mipmap: Mipmap) => void
) => {
  if (!returnSpectrogramListeners.has(index)) {
    returnSpectrogramListeners.set(index, new Map());
  }
  returnSpectrogramListeners.get(index)?.set(idChStr, callback);
  return () => {
    returnSpectrogramListeners.get(index)?.delete(idChStr);
  };
};
