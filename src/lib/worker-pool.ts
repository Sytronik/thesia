import RendererWorker from "./worker?worker";
import type {
  RendererWorkerEventMessage,
  RendererWorkerMessage,
  OnReturnMipmapCallback,
  OnSetSpectrogramDoneCallback,
} from "./worker";

export const NUM_WORKERS = navigator.hardwareConcurrency ?? 1;
const setSpectrogramDoneListeners = new Map<number, Map<string, OnSetSpectrogramDoneCallback>>();
const returnSpectrogramListeners = new Map<number, Map<string, OnReturnMipmapCallback>>();

const workers = new Map<number, Worker>(
  Array.from({length: NUM_WORKERS}, (_, i) => {
    const worker = new RendererWorker();
    setSpectrogramDoneListeners.set(i, new Map());
    returnSpectrogramListeners.set(i, new Map());
    worker.onmessage = (event) => onmessage(i, event);
    return [i, worker];
  }),
);

export const postMessageToWorker = (
  index: number,
  message: RendererWorkerMessage,
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

function onmessage(index: number, event: MessageEvent<RendererWorkerEventMessage>) {
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
