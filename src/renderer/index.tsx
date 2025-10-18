import {createRoot} from "react-dom/client";
import {ipcRenderer} from "electron";
import {UserSettings} from "backend";
import App from "./App";
import BackendAPI, {WasmAPI, initWasm} from "./api";
import {setUserSetting} from "./lib/ipc-sender";
import {COLORMAP_RGBA8} from "./prototypes/constants/colors";

const container = document.getElementById("root") as HTMLElement;
const root = createRoot(container);

ipcRenderer.once("render-with-settings", async (_, settings) => {
  const canvas = document.createElement("canvas");
  const gl = canvas.getContext("webgl2");
  if (!gl) throw new Error("WebGL2 is not supported");
  const maxTextureSize = gl.getParameter(gl.MAX_TEXTURE_SIZE);
  gl.getExtension("WEBGL_lose_context")?.loseContext();

  // Initialize WASM module
  try {
    await initWasm();
    console.log("WASM module loaded successfully.");

    // Call a WASM function as a test
    WasmAPI.wasmGreet("Thesia");
  } catch (error) {
    console.error("Error occurred during WASM module initialization:", error);
  }

  const userSettings: UserSettings = {
    specSetting: settings.specSetting,
    blend: settings.blend,
    dBRange: settings.dBRange,
    commonGuardClipping: settings.commonGuardClipping,
    commonNormalize: settings.commonNormalize,
  };
  const userSettingsOrInitialValues = BackendAPI.init(userSettings, maxTextureSize);
  Object.entries(userSettingsOrInitialValues).forEach(([key, value]) =>
    setUserSetting(key as keyof UserSettings, value),
  );
  BackendAPI.setColormapLength(COLORMAP_RGBA8.length / 4);
  root.render(<App userSettings={userSettingsOrInitialValues} />);
});
