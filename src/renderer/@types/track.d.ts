// IdChannel is form of id#_ch#
type IdChannel = string;
type IdChArr = IdChannel[];

// written in snake case for compatibility with native api
type DrawOptionForWav = {
  min_amp: number;
  max_amp: number;
};

type MarkerDrawOption = {
  startSec?: number;
  pxPerSec?: number;
  drawOptionForWav?: DrawOptionForWav;
};

type SpecWavImages = {
  [key: IdChannel]: ArrayBuffer;
};

type ImgCanvasHandleElement = {
  draw: (buf: ArrayBuffer) => void;
};

// Track Summary
type TrackSummary = {
  fileName: string;
  time: string;
  sampleFormat: string;
  sampleRate: string;
};

// Axis Tick
type TickPxPosition = number;
type TickLable = string;
type Markers = [TickPxPosition, TickLable][];

type TickScaleTable = {
  [key: number]: number[];
};

type AxisCanvasHandleElement = {
  draw: (markers: Markers) => void;
};
