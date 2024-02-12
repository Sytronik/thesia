import React from "react";

function isApple() {
  const expression = /(Mac|iPhone|iPod|iPad)/i;
  return expression.test(navigator.platform);
}

export function isCommand(event: KeyboardEvent | React.KeyboardEvent) {
  // Returns true if Ctrl or cmd keys were pressed.
  if (isApple()) {
    return event.metaKey;
  }
  return event.ctrlKey; // Windows, Linux, UNIX
}

export function isCommandOnly(event: KeyboardEvent | React.KeyboardEvent) {
  // Returns true if Ctrl or cmd keys were pressed.
  if (isApple()) {
    return event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey;
  }
  // Windows, Linux, UNIX
  return event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey;
}
