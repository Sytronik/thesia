import RendererWorker from "./worker?worker";

export const NUM_WORKERS = navigator.hardwareConcurrency ?? 1;
const workers = new Map<number, Worker>(
  Array.from({length: NUM_WORKERS}, (_, i) => [i, new RendererWorker()])
);

export const postMessageToWorker = (
    index: number, 
    message: any, 
    transferList: Transferable[] = []
) => {
    workers.get(index)?.postMessage(message, transferList);
}
