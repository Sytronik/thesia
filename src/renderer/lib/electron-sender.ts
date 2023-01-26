import {ipcRenderer} from "electron";
import {SUPPORTED_TYPES} from "renderer/prototypes/constants";

export function showElectronOpenDialog() {
  ipcRenderer.send("show-open-dialog", SUPPORTED_TYPES);
}

export function showElectronContextMenu(trackId: number) {
  ipcRenderer.send("show-track-context-menu", trackId);
}

export function showElectronFileOpenErrorMsg(unsupportedPaths: string[], invalidPaths: string[]) {
  ipcRenderer.send("show-file-open-err-msg", unsupportedPaths, invalidPaths, SUPPORTED_TYPES);
}
