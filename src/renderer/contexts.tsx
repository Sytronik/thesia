import React, {createContext} from "react";
import {useDevicePixelRatio} from "use-device-pixel-ratio";

export const DevicePixelRatioContext = createContext(1);

export function DevicePixelRatioProvider({children}: {children: React.ReactElement}) {
  const dpr = useDevicePixelRatio(); // TODO: no round?
  return (
    <DevicePixelRatioContext.Provider value={dpr}>{children}</DevicePixelRatioContext.Provider>
  );
}
