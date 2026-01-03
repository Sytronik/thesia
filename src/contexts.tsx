import React, {createContext} from "react";
import {useDevicePixelRatio} from "use-device-pixel-ratio";
import {BackendConstants} from "./api";

export const DevicePixelRatioContext = createContext(1);

export function DevicePixelRatioProvider({children}: {children: React.ReactElement}) {
  const dpr = useDevicePixelRatio({round: false});

  return (
    <DevicePixelRatioContext.Provider value={dpr}>{children}</DevicePixelRatioContext.Provider>
  );
}

export const BackendConstantsContext = createContext<BackendConstants>({
  PLAY_JUMP_SEC: 0,
  PLAY_BIG_JUMP_SEC: 0,
});

export function BackendConstantsProvider({
  constants,
  children,
}: {
  constants: BackendConstants;
  children: React.ReactElement;
}) {
  return (
    <BackendConstantsContext.Provider value={constants}>
      {children}
    </BackendConstantsContext.Provider>
  );
}
