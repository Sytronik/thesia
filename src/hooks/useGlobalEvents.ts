import {useEffect} from "react";
import BackendAPI from "../api";

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

export function addGlobalEventListeners() {
  document.body.addEventListener("contextmenu", showEditContextMenuIfEditableNode);
  document.body.addEventListener("focusin", changeEditMenuForFocusIn);
  document.body.addEventListener("focusout", changeEditMenuForFocusOut);
}

export function removeGlobalEventListeners() {
  document.body.removeEventListener("contextmenu", showEditContextMenuIfEditableNode);
  document.body.removeEventListener("focusin", changeEditMenuForFocusIn);
  document.body.removeEventListener("focusout", changeEditMenuForFocusOut);
}

export function useGlobalEvents() {
  useEffect(() => {
    addGlobalEventListeners();
    return () => {
      removeGlobalEventListeners();
    };
  }, []);
}
