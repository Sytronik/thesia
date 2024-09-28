import {createRoot} from "react-dom/client";
import {ipcRenderer} from "electron";
import App from "./App";
import BackendAPI from "./api";
import {setUserSetting} from "./lib/ipc-sender";

const container = document.getElementById("root") as HTMLElement;
const root = createRoot(container);
BackendAPI.init();

async function getUserOrInitialValue<K extends keyof UserSettings>(
  settings: UserSettingsOptionals,
  key: K,
  getFromBackend: () => NonNullable<UserSettings[K]> | Promise<NonNullable<UserSettings[K]>>,
  setToBackend: (v: NonNullable<UserSettings[K]>) => void | Promise<void>,
): Promise<NonNullable<UserSettings[K]>> {
  const value = settings[key];
  if (value !== undefined) {
    await setToBackend(value);
    return value;
  }
  const valueFromBackend = await getFromBackend();
  setUserSetting(key, valueFromBackend);
  return valueFromBackend;
}

ipcRenderer.once("render-with-settings", async (_, settings) => {
  const specSetting = await getUserOrInitialValue(
    settings,
    "specSetting",
    BackendAPI.getSpecSetting,
    BackendAPI.setSpecSetting,
  );
  const blend = settings.blend !== undefined ? settings.blend : 0.5;
  const dBRange = await getUserOrInitialValue(
    settings,
    "dBRange",
    BackendAPI.getdBRange,
    BackendAPI.setdBRange,
  );
  const commonGuardClipping = await getUserOrInitialValue(
    settings,
    "commonGuardClipping",
    BackendAPI.getCommonGuardClipping,
    BackendAPI.setCommonGuardClipping,
  );
  const commonNormalize = await getUserOrInitialValue(
    settings,
    "commonNormalize",
    BackendAPI.getCommonNormalize,
    BackendAPI.setCommonNormalize,
  );

  root.render(
    <App userSettings={{specSetting, blend, dBRange, commonGuardClipping, commonNormalize}} />,
  );
});
