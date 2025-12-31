import ReactDOM from "react-dom/client";
import BackendAPI, {WasmAPI} from "./api";
import {COLORMAP_RGBA8} from "./prototypes/constants/colors";
import App from "./App";
import {BackendConstantsProvider} from "./contexts";
import {isWindows} from "./utils/osSpecifics";

// Initialize WASM module
try {
  await WasmAPI.initWasm();
} catch (error) {
  console.error("Error occurred during WASM module initialization:", error);
}

// Add platform class to body for platform-specific styling
if (isWindows()) {
  document.body.classList.add("platform-windows");
} else {
  document.body.classList.add("platform-non-windows");
}

const container = document.getElementById("root") as HTMLElement;
const root = ReactDOM.createRoot(container);

const {constants, userSettings} = await BackendAPI.init(COLORMAP_RGBA8.length / 4);

root.render(
  <BackendConstantsProvider constants={constants}>
    <App userSettings={userSettings} />
  </BackendConstantsProvider>,
);
