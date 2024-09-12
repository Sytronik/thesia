import {ipcRenderer} from "electron";

export function showElectronOpenDialog() {
  ipcRenderer.send("show-open-dialog");
}

export function showTrackContextMenu() {
  ipcRenderer.send("show-track-context-menu");
}

export function showAxisContextMenu(axisKind: AxisKind) {
  if (axisKind === "dBAxis") return;
  ipcRenderer.send("show-axis-context-menu", axisKind);
}

export function showElectronFileOpenErrorMsg(unsupportedPaths: string[], invalidPaths: string[]) {
  ipcRenderer.send("show-file-open-err-msg", unsupportedPaths, invalidPaths);
}
