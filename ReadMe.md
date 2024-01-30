# Thesia: Multi-track Spectrogram / Waveform Viewer

This project is in a very early stage.

## Design Draft

![design_draft](https://github.com/Sytronik/thesia/assets/61383377/938e0425-999f-408c-ae16-82ddf207bc63)

## Setup

### macOS, Linux

1. Install [Rust](https://www.rust-lang.org/tools/install)
2. Install node.js v16.20.2 ~ v21 (The most recent version tested: v21.6.1)
   - Example using nvm on macOS
      ```
      brew install nvm
      nvm install 21.6.1
      nvm use 21.6.1
      ```

3. Install napi-rs/cli and dependencies
   ```
   npm install -g @napi-rs/cli
   npm run build:backend
   npm install
   ```

### Windows

1. Install [Rust](https://www.rust-lang.org/tools/install)
2. Install vcpkg and openblas

   ```
   git clone https://github.com/microsoft/vcpkg
   .\vcpkg\bootstrap-vcpkg.bat
   .\vcpkg\vcpkg integrate install
   vcpkg install openblas --triplet x64-windows-static
   ```

3. Install nvm-windows (using a GUI installer)
2. Install node.js v16.20.2 ~ v21 (The most recent version tested: v21.6.1)
   ```
   nvm install 21.6.1
   nvm use 21.6.1
   ```

5. Install napi-rs/cli and dependencies

   ```
   npm install -g @napi-rs/cli
   npm run build:backend
   npm install
   ```

## Run in Dev Mode

```
npm run start
```

## packaging into an executable binary

```
npm run package
```

## Plan

- [x] dB colorbar
- [x] Hi-DPI display support
- [x] time / frequency info on mouse hover
- [x] waveform amplitude zoom in/out slider
- [x] selecting spectrogram mode / waveform mode
- [x] configurable STFT parameters
- [x] peak / RMS / LUFS / LKFS level calculator
- [x] volume normalization
- [ ] Audio Player
- [ ] STFT parameters preset
- [ ] pitch / formant tracker
- [ ] Adaptive STFT (sth like iZotope RX Editor)
