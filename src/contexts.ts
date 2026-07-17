import { createContext } from "react";
import type { BackendConstants } from "./api";

export const DevicePixelRatioContext = createContext(1);

export const BackendConstantsContext = createContext<BackendConstants>({
  PLAY_JUMP_SEC: 0,
  PLAY_BIG_JUMP_SEC: 0,
});
