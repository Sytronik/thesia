import BackendAPI, {UserSettings} from "src/api";
// import {ipcRenderer} from "electron";

export function notifyAppRendered() {
  // ipcRenderer.send("app-rendered");
}

export function setUserSetting<K extends keyof UserSettings>(
  key: K,
  value: NonNullable<UserSettings[K]>,
) {
  // ipcRenderer.send("set-setting", key, value);
}

function callDifferentFuncIfEditableNode(
  node: HTMLElement | null,
  funcForEditable: () => void,
  funcForNonEditable?: () => void,
) {
  let ancestor = node;
  let isEditable = false;
  while (ancestor) {
    if (ancestor.nodeName.match(/^(input|textarea)$/i) || ancestor.isContentEditable) {
      funcForEditable();
      isEditable = true;
      break;
    }
    if (ancestor.parentNode === null) break;
    ancestor = ancestor.parentNode as HTMLElement;
  }
  if (!isEditable && funcForNonEditable) funcForNonEditable();
}

export function showEditContextMenuIfEditableNode(e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  callDifferentFuncIfEditableNode(e.target as HTMLElement | null, BackendAPI.showEditContextMenu);
}

export function changeEditMenuForFocusIn(e: FocusEvent) {
  callDifferentFuncIfEditableNode(
    e.target as HTMLElement | null,
    BackendAPI.enableEditMenu,
    BackendAPI.disableEditMenu,
  );
}

export function changeEditMenuForFocusOut(e: FocusEvent) {
  callDifferentFuncIfEditableNode(
    e.relatedTarget as HTMLElement | null,
    BackendAPI.enableEditMenu,
    BackendAPI.disableEditMenu,
  );
}

export function addGlobalFocusInListener() {
  document.body.addEventListener("focusin", changeEditMenuForFocusIn);
}
export function removeGlobalFocusInListener() {
  document.body.removeEventListener("focusin", changeEditMenuForFocusIn);
}

export function addGlobalFocusOutListener() {
  document.body.addEventListener("focusout", changeEditMenuForFocusOut);
}
export function removeGlobalFocusOutListener() {
  document.body.removeEventListener("focusout", changeEditMenuForFocusOut);
}
