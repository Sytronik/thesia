import React, {createContext, useEffect} from "react";
import {useDevicePixelRatio} from "use-device-pixel-ratio";
import {WasmAPI} from "./api";

export const DevicePixelRatioContext = createContext(1);

export function DevicePixelRatioProvider({children}: {children: React.ReactElement}) {
  const dpr = useDevicePixelRatio({round: false});

  useEffect(() => {
    WasmAPI.setDevicePixelRatio(dpr);
  }, [dpr]);

  return (
    <DevicePixelRatioContext.Provider value={dpr}>{children}</DevicePixelRatioContext.Provider>
  );
}
