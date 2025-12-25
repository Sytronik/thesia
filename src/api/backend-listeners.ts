import {listen, UnlistenFn} from "@tauri-apps/api/event";
import {AxisKind} from "./backend-wrapper";

export async function listenOpenFiles(
  handler: (files: string[]) => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen("open-files", (event) => {
    handler(event.payload as string[]);
  });
}

export async function listenMenuOpenAudioTracks(handler: () => Promise<void>): Promise<UnlistenFn> {
  return listen("open-audio-tracks", handler);
}

export async function listenMenuEditDelete(
  handler: () => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen("edit-delete", handler);
}

export async function listenMenuEditSelectAll(
  handler: () => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen("edit-select-all", handler);
}

export async function listenEditMenuEvents() {
  const promiseUnlistenDelete = listenMenuEditDelete(() => {
    const activeElement = document.activeElement;
    if (activeElement instanceof HTMLInputElement || activeElement instanceof HTMLTextAreaElement) {
      if (activeElement.selectionStart === null || activeElement.selectionEnd === null) return;
      const text = activeElement.value;
      activeElement.value =
        text.slice(0, activeElement.selectionStart) + text.slice(activeElement.selectionEnd);
    }
  });
  const promiseUnlistenSelectAll = listenMenuEditSelectAll(() => {
    const activeElement = document.activeElement;
    if (activeElement instanceof HTMLInputElement || activeElement instanceof HTMLTextAreaElement) {
      activeElement.select();
    }
  });
  return Promise.all([promiseUnlistenDelete, promiseUnlistenSelectAll]);
}

export async function listenFreqZoomIn(handler: () => void | Promise<void>): Promise<UnlistenFn> {
  return listen("freq-zoom-in", handler);
}

export async function listenFreqZoomOut(handler: () => void | Promise<void>): Promise<UnlistenFn> {
  return listen("freq-zoom-out", handler);
}

export async function listenTimeZoomIn(handler: () => void | Promise<void>): Promise<UnlistenFn> {
  return listen("time-zoom-in", handler);
}

export async function listenTimeZoomOut(handler: () => void | Promise<void>): Promise<UnlistenFn> {
  return listen("time-zoom-out", handler);
}

export async function listenMenuRemoveSelectedTracks(
  handler: () => Promise<void>,
): Promise<UnlistenFn> {
  return listen("remove-selected-tracks", handler);
}

export async function listenMenuSelectAllTracks(
  handler: () => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen("select-all-tracks", handler);
}

export async function listenTogglePlay(handler: () => void | Promise<void>): Promise<UnlistenFn> {
  return listen("toggle-play", handler);
}

export type JumpPlayerMode = "fast-forward" | "rewind" | "fast-forward-big" | "rewind-big";

export async function listenJumpPlayer(
  handler: (mode: JumpPlayerMode) => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen<JumpPlayerMode>("jump-player", (event) => {
    handler(event.payload);
  });
}

export async function listenRewindToFront(
  handler: () => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen("rewind-to-front", handler);
}

export async function listenMenuEditAmpRange(
  id: number,
  handler: () => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen(`edit-amp-range-${id}`, handler);
}

export async function listenMenuEditFreqUpperLimit(
  id: number,
  handler: () => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen(`edit-freq-upper-limit-${id}`, handler);
}

export async function listenMenuEditFreqLowerLimit(
  id: number,
  handler: () => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen(`edit-freq-lower-limit-${id}`, handler);
}

export async function listenMenuResetAxisRange(
  handlers: Map<AxisKind, () => void | Promise<void>>,
): Promise<UnlistenFn[]> {
  return Promise.all(
    Array.from(handlers.entries()).map(([axisKind, handler]) =>
      listen(`reset-axis-range-${axisKind}`, handler),
    ),
  );
}
