import {createRoot} from "react-dom/client";
import {ipcRenderer} from "electron";
import App from "./App";

const container = document.getElementById("root") as HTMLElement;
const root = createRoot(container);

ipcRenderer.once("render-with-settings", (_, settings) => {
  root.render(<App userSettings={settings} />);
});
