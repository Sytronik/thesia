# Thesia: Multi-track Spectrogram / Waveform Viewer

This project is in a beta stage.

## Design Draft

![design_draft](https://github.com/Sytronik/thesia/assets/61383377/938e0425-999f-408c-ae16-82ddf207bc63)

## Build Setup

1. Install prerequisites
   - Common
     - [Rust](https://www.rust-lang.org/tools/install)
     - [node.js](https://nodejs.org/en/download/current) >= v22
       - The most recent version tested: v24.12.0
   - macOS
     - Xcode
       - `xcode-select --install` to install Command Line Tools for Xcode
   - Windows
    - Webview2
      - need to install only on Windows < 10
      - see [Tauri's prerequisites](https://v2.tauri.app/ko/start/prerequisites/#webview2-%EC%84%A4%EC%B9%98)
     - vcpkg & OpenBLAS
       ```powershell
       git clone https://github.com/microsoft/vcpkg
       .\vcpkg\bootstrap-vcpkg.bat
       .\vcpkg\vcpkg integrate install
       # add vcpkg directory to PATH and restart the terminal, then run:
       vcpkg install openblas:x64-windows-static openblas:x64-windows-static-md
       ```
   - Linux
     - ALSA
       - For Debian/Ubuntu: `sudo apt install libasound2-dev`
2. Install npm packages & build
   ```bash
   # clone thesia repo & cd to the directory
   npm install -g wasm-pack
   npm run build:wasm
   npm install
   ```

## Run in Dev Mode

```bash
# if you need the WASM build for debugging, run `npm run build:wasm.debug` first
npm run tauri dev
```

### DevTools

You can use the system webview's developer tools in dev mode.

#### React DevTools (for frontend)
```bash
npm install -g react-devtools
npx react-devtools
# run thesia in dev mode
```

#### Tauri DevTools (for backend)
When you run `npm run tauri dev`, the address of the Tauri devtools is displayed in the terminal (not the web console).

## Packaging into an executable binary

```bash
npm run tauri build [--debug]
```

The target binary is under `target/release/bundle/<os>/thesia.app`.

## Plan

- [ ] Releasing v1.0
- [ ] Selecting audio output device (and exclusive mode)
- [ ] Region selection and loop playback 
- [ ] STFT parameters preset
- [ ] pitch / formant tracker
- [ ] Adaptive STFT (sth like iZotope RX Editor)
- [ ] Showing the average FFT magnitude
- [ ] Save normalized audio
- [ ] Export as figures
