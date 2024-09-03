import {ipcRenderer} from "electron";
import {SUPPORTED_TYPES} from "renderer/prototypes/constants/tracks";

export function showElectronOpenDialog() {
  ipcRenderer.send("show-open-dialog", SUPPORTED_TYPES);
}

export function showTrackContextMenu(trackId: number) {
  ipcRenderer.send("show-track-context-menu", trackId);
}

export function showAxisContextMenu(axisKind: AxisKind) {
  if (axisKind === "dBAxis") return;
  ipcRenderer.send("show-axis-context-menu", axisKind);
}

export function showElectronFileOpenErrorMsg(unsupportedPaths: string[], invalidPaths: string[]) {
  ipcRenderer.send("show-file-open-err-msg", unsupportedPaths, invalidPaths, SUPPORTED_TYPES);
}
