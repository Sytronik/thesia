import ReactDOM from "react-dom/client";
import BackendAPI, {WasmAPI} from "./api";
import {COLORMAP_RGBA8} from "./prototypes/constants/colors";
import App from "./App";
import {BackendConstantsProvider} from "./contexts";

// Initialize WASM module
try {
  await WasmAPI.initWasm();
} catch (error) {
  console.error("Error occurred during WASM module initialization:", error);
}

const container = document.getElementById("root") as HTMLElement;
const root = ReactDOM.createRoot(container);

const {constants, userSettings} = await BackendAPI.init(COLORMAP_RGBA8.length / 4);

root.render(
  <BackendConstantsProvider constants={constants}>
    <App userSettings={userSettings} />
  </BackendConstantsProvider>,
);
