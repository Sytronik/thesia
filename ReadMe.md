# Multi-track Spectrogram / Waveform Viewer

This project is in a very early stage.

## Usage

It needs python>=3.8 because of `:=` operator.
```
./main.py [-p PORT(default: 8080)]
```
And open an web browser: http://0.0.0.0:8080/

## Plan

- [ ] Incrementally adding / removing audio files (Currently uploading files overrides the entire file list.)
- [ ] floating tools: dB colorbar, waveform zoom in/out slider, track height control slider
- [ ] selecting spectrogram mode / waveform mode
- [ ] performance improvement (by not using Dash/Plotly)
- [ ] Audio Player
- [ ] Adaptive STFT (sth like iZotope RX Editor)