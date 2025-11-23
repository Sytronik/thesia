import RendererWorker from "./worker?worker";
import type { RendererWorkerMessage } from "./worker";

export const NUM_WORKERS = navigator.hardwareConcurrency ?? 1;
const workers = new Map<number, Worker>(
  Array.from({length: NUM_WORKERS}, (_, i) => [i, new RendererWorker()])
);
const returnSpectrogramListeners = new Map<number, Map<string, (message: any) => void>>();

workers.forEach((worker, index) => {
  returnSpectrogramListeners.set(index, new Map());
  worker.onmessage = (event: MessageEvent<any>) => {
    const { type, data } = event.data;
    if (type === "returnMipmap") {
      const listeners = returnSpectrogramListeners.get(index);
      if (listeners) {
        listeners.get(data.idChStr)?.(data.mipmap);
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
