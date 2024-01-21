# Thesia: Multi-track Spectrogram / Waveform Viewer

This project is in a very early stage.

## Design Draft

![design_draft](https://user-images.githubusercontent.com/61383377/102886103-d806b200-4497-11eb-91b2-2e752df089e5.png)

## Setup

### macOS, Linux

1. Install [Rust](https://www.rust-lang.org/tools/install)
2. Install node.js v16 and yarn
   ```
   brew install nvm
   nvm install 16
   nvm use 16
   npm install --global yarn
   ```
3. Install napi-rs/cli and dependencies
   ```
   yarn global add @napi-rs/cli
   yarn build:backend
   yarn install
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
4. Install node.js v16 and yarn
   ```
   nvm install 16
   nvm use 16
   npm install --global yarn
   ```
5. Install napi-rs/cli and dependencies
   ```
   yarn global add @napi-rs/cli
   yarn build:backend
   yarn install
   ```

## Run in Dev Mode

```
yarn start
```

## packaging into an executable binary

```
yarn package
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
