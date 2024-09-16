import {ipcRenderer} from "electron";

export function notifyAppRendered() {
  ipcRenderer.send("app-rendered");
}

export function setUserSetting<K extends keyof UserSettings>(
  key: K,
  value: NonNullable<UserSettings[K]>,
) {
  ipcRenderer.send("set-setting", key, value);
}

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

export function showEditContextMenu() {
  ipcRenderer.send("show-edit-context-menu");
}

export function showElectronFileOpenErrorMsg(unsupportedPaths: string[], invalidPaths: string[]) {
  ipcRenderer.send("show-file-open-err-msg", unsupportedPaths, invalidPaths);
}

export function enableEditMenu() {
  ipcRenderer.send("enable-edit-menu");
}

export function disableEditMenu() {
  ipcRenderer.send("disable-edit-menu");
}

export function showPlayOrPauseMenu(isPlaying: boolean) {
  if (isPlaying) ipcRenderer.send("show-pause-menu");
  else ipcRenderer.send("show-play-menu");
}

export function enableTogglePlayMenu() {
  ipcRenderer.send("enable-toggle-play-menu");
}

export function disableTogglePlayMenu() {
  ipcRenderer.send("disable-toggle-play-menu");
}

export function changeMenuDepsOnTrackExistence(trackExists: boolean) {
  if (trackExists) {
    ipcRenderer.send("enable-axis-zoom-menu");
    ipcRenderer.send("enable-remove-track-menu");
    ipcRenderer.send("enable-play-menu");
  } else {
    ipcRenderer.send("disable-axis-zoom-menu");
    ipcRenderer.send("disable-remove-track-menu");
    ipcRenderer.send("disable-play-menu");
  }
}
