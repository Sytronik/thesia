# Thesia: Multi-track Spectrogram / Waveform Viewer

This project is in a very early stage.

## Design Draft

![design_draft](https://user-images.githubusercontent.com/61383377/102886103-d806b200-4497-11eb-91b2-2e752df089e5.png)

## Run in Dev Mode

```
yarn global add @napi-rs/cli
yarn build:backend
yarn install
yarn start
```

## packaging into an executable binary
```
yarn global add @napi-rs/cli
yarn build:backend
yarn install
yarn package
```

## Plan

- [x] dB colorbar
- [x] Hi-DPI display support
- [ ] time / frequency info on mouse hover
- [ ] waveform amplitude zoom in/out slider
- [ ] selecting spectrogram mode / waveform mode
- [ ] configurable STFT parameters (preset?)
- [ ] Audio Player
- [ ] peak / RMS / LUFS / LKFS level calculator
- [ ] volume normalization
- [ ] pitch / formant tracker
- [ ] Adaptive STFT (sth like iZotope RX Editor)
