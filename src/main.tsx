import ReactDOM from "react-dom/client";
// import {ipcRenderer} from "electron";
import {UserSettingsOptionals} from "src/api/backend-wrapper";
import BackendAPI from "./api";
import { setUserSetting } from "./lib/ipc-sender";
import { COLORMAP_RGBA8 } from "./prototypes/constants/colors";
import App from "./App";

const container = document.getElementById("root") as HTMLElement;
const root = ReactDOM.createRoot(container);

// ipcRenderer.once("render-with-settings", async (_, settings, tempDirectory) => {

const userSettings: UserSettingsOptionals = {
  // specSetting: settings.specSetting,
  // blend: settings.blend,
  // dBRange: settings.dBRange,
  // commonGuardClipping: settings.commonGuardClipping,
  // commonNormalize: settings.commonNormalize,
};
// const userSettingsOrInitialValues = BackendAPI.init(userSettings, maxTextureSize, tempDirectory);
const userSettingsOrInitialValues = await BackendAPI.init(userSettings);
// Object.entries(userSettingsOrInitialValues).forEach(([key, value]) =>
//   setUserSetting(key as keyof UserSettings, value),
// );
BackendAPI.setColormapLength(COLORMAP_RGBA8.length / 4);

root.render(
    <App userSettings={userSettingsOrInitialValues} />
);
// });
