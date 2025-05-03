import {createRoot} from "react-dom/client";
import {ipcRenderer} from "electron";
import {UserSettings} from "backend";
import App from "./App";
import BackendAPI from "./api";
import {setUserSetting} from "./lib/ipc-sender";
import {COLORMAP_RGBA8} from "./prototypes/constants/colors";

const container = document.getElementById("root") as HTMLElement;
const root = createRoot(container);

ipcRenderer.once("render-with-settings", (_, settings) => {
  const canvas = document.createElement("canvas");
  const gl = canvas.getContext("webgl2");
  if (!gl) {
    throw new Error("WebGL2 is not supported");
  }
  const userSettingsOrInitialValues = BackendAPI.init(
    settings,
    gl.getParameter(gl.MAX_TEXTURE_SIZE),
  );
  Object.entries(userSettingsOrInitialValues).forEach(([key, value]) =>
    setUserSetting(key as keyof UserSettings, value),
  );
  BackendAPI.setColormapLength(COLORMAP_RGBA8.length);
  root.render(<App userSettings={userSettingsOrInitialValues} />);
});
