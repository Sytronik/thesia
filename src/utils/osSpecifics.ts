import { Hotkey } from "react-hotkeys-hook/packages/react-hotkeys-hook/dist/types";
import { platform } from "@tauri-apps/plugin-os";

export function isApple() {
  const currentPlatform = platform();
  return currentPlatform === "macos" || currentPlatform === "ios";
}

export function isWindows() {
  const currentPlatform = platform();
  return currentPlatform === "windows";
}

export function isCommand(event: MouseOrKeyboardEvent) {
  // Returns true if Ctrl or cmd keys were pressed.
  if (isApple()) {
    return event.metaKey;
  }
  return event.ctrlKey; // Windows, Linux, UNIX
}

export function isCommandOnly(event: MouseOrKeyboardEvent) {
  // Returns true if Ctrl or cmd keys were pressed.
  if (isApple()) {
    return event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey;
  }
  // Windows, Linux, UNIX
  return event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey;
}

export function isHotkeyCommandOnly(hotkey: Hotkey) {
  // Returns true if Ctrl or cmd keys were pressed.
  if (isApple()) {
    return hotkey.meta && !hotkey.ctrl && !hotkey.shift && !hotkey.alt;
  }
  // Windows, Linux, UNIX
  return hotkey.ctrl && !hotkey.meta && !hotkey.shift && !hotkey.alt;
}
