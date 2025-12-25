import ReactDOM from "react-dom/client";
import BackendAPI, {WasmAPI} from "./api";
import {COLORMAP_RGBA8} from "./prototypes/constants/colors";
import App from "./App";

// Initialize WASM module
try {
  await WasmAPI.initWasm();
} catch (error) {
  console.error("Error occurred during WASM module initialization:", error);
}

const container = document.getElementById("root") as HTMLElement;
const root = ReactDOM.createRoot(container);

const userSettings = await BackendAPI.init(COLORMAP_RGBA8.length / 4);

root.render(<App userSettings={userSettings} />);
