import React from "react";

function isApple() {
  const expression = /(Mac|iPhone|iPod|iPad)/i;
  return expression.test(navigator.platform);
}

export default function isCommand(event: KeyboardEvent | React.KeyboardEvent) {
  // Returns true if Ctrl or cmd keys were pressed.
  if (isApple()) {
    return event.metaKey;
  }
  return event.ctrlKey; // Windows, Linux, UNIX
}
