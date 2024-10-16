import {createRoot} from "react-dom/client";
import {ipcRenderer} from "electron";
import {UserSettings} from "backend";
import App from "./App";
import BackendAPI from "./api";
import {setUserSetting} from "./lib/ipc-sender";

const container = document.getElementById("root") as HTMLElement;
const root = createRoot(container);

ipcRenderer.once("render-with-settings", (_, settings) => {
  const userSettingsOrInitialValues = BackendAPI.init(settings);
  Object.entries(userSettingsOrInitialValues).forEach(([key, value]) =>
    setUserSetting(key as keyof UserSettings, value),
  );
  root.render(<App userSettings={userSettingsOrInitialValues} />);
});
