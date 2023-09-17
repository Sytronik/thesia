// IdChannel is form of id#_ch#
type IdChannel = string;
type IdChArr = IdChannel[];
type IdChMap = Map<number, IdChArr>;

// written in snake case for compatibility with native api
type DrawOptionForWav = {
  amp_range: [number, number];
};

type MarkerDrawOption = {
  startSec?: number;
  pxPerSec?: number;
  drawOptionForWav?: DrawOptionForWav;
};

type SpecWavImages = {
  [key: IdChannel]: Buffer;
};

type SplitViewHandleElement = {
  getBoundingClientY: () => number;
  scrollTo: (option: ScrollToOptions) => void;
};

type ImgCanvasHandleElement = {
  draw: (buf: Buffer) => void;
  updateLensParams: (params: OptionalLensParams) => void;
  getBoundingClientRect: () => DOMRect;
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

type OverviewHandleElement = {
  draw: (startSec: number, lensDurationSec: number) => void;
};

type OptionalLensParams = {startSec?: number; pxPerSec?: number};

type VScrollAnchorInfo = {imgIndex: number; cursorRatioOnImg: number; cursorOffset: number};

type FreqScale = "Mel" | "Linear";

type SpecSetting = {
  win_ms: number;
  t_overlap: number;
  f_overlap: number;
  freq_scale: FreqScale;
};
