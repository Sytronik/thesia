# Thesia: Multi-track Spectrogram / Waveform Viewer

This project is in a very early stage.

## Design Draft

![design_draft](https://github.com/Sytronik/thesia/assets/61383377/938e0425-999f-408c-ae16-82ddf207bc63)

## Build Setup

1. Install prerequisites
   - Common
     - [Rust](https://www.rust-lang.org/tools/install)
     - [node.js](https://nodejs.org/en/download/current) v16.20.2 ~ v21
       - The most recent version tested: v21.7.3
   - Windows
     - vcpkg & OpenbLAS
       ```powershell
       git clone https://github.com/microsoft/vcpkg
       .\vcpkg\bootstrap-vcpkg.bat
       .\vcpkg\vcpkg integrate install
       vcpkg install openblas --triplet x64-windows-static
       ```
   - Linux
     - ALSA
       - For Debian/Ubuntu: `sudo apt install libasound2-dev`
2. Install npm packages & build
   ```bash
   # clone thesia repo & cd to the directory
   npm install -g @napi-rs/cli
   npm run build:backend
   npm install
   ```

## Run in Dev Mode

```bash
npm run start
```

## Packaging into an executable binary

```bash
npm run package
```

The target binary is under `release/build/<os>/thesia.app`.

## Plan

- [ ] Re-ordering tracks
- [ ] Releasing v1.0
- [ ] Selecting audio output device (and exclusive mode)
- [ ] Region selection and loop playback 
- [ ] STFT parameters preset
- [ ] pitch / formant tracker
- [ ] Adaptive STFT (sth like iZotope RX Editor)
- [ ] Showing the average FFT magnitude
- [ ] Export as figures
