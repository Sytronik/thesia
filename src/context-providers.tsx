import type { ReactElement } from "react";
import { useDevicePixelRatio } from "use-device-pixel-ratio";
import type { BackendConstants } from "./api";
import { BackendConstantsContext, DevicePixelRatioContext } from "./contexts";

export function DevicePixelRatioProvider({ children }: { children: ReactElement }) {
  const dpr = useDevicePixelRatio({ round: false });

  return (
    <DevicePixelRatioContext.Provider value={dpr}>{children}</DevicePixelRatioContext.Provider>
  );
}

export function BackendConstantsProvider({
  constants,
  children,
}: {
  constants: BackendConstants;
  children: ReactElement;
}) {
  return (
    <BackendConstantsContext.Provider value={constants}>
      {children}
    </BackendConstantsContext.Provider>
  );
}
