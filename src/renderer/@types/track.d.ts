type SpecSetting = import("../api").SpecSetting;
type GuardClippingMode = import("../api").GuardClippingMode;
type NormalizeTarget = import("../api").NormalizeTarget;
type Markers = import("../api").Markers;
type IdChannel = import("../api").IdChannel;
type IdChArr = import("../api").IdChArr;
type IdChMap = Map<number, IdChArr>;
type Spectrogram = import("../api").Spectrogram;
type Spectrograms = import("../api").Spectrograms;
type WavDrawingInfo = import("../api").WavDrawingInfo;

type MouseOrKeyboardEvent = MouseEvent | KeyboardEvent | React.MouseEvent | React.KeyboardEvent;

type OptionalLensParams = {startSec?: number; pxPerSec?: number};

// Track Summary
type TrackSummaryData = {
  fileName: string;
  time: string;
  formatName: string;
  bitDepth: string;
  bitrate: string;
  sampleRate: string;
  globalLUFS: string;
  guardClipStats: Record<string, string>;
};

// Axis Tick
type TickScaleTable = {
  [key: number]: [number, number];
};

type VScrollAnchorInfo = {imgIndex: number; cursorRatioOnImg: number; cursorOffset: number};

type AxisKind = "timeRuler" | "ampAxis" | "freqAxis" | "dBAxis";

type SplitViewHandleElement = {
  getBoundingClientRect: () => DOMRect | null;
  scrollTo: (option: ScrollToOptions) => void;
  scrollTop: () => number;
};

type ImgCanvasHandleElement = {
  getBoundingClientRect: () => DOMRect;
};

type AxisCanvasHandleElement = {
  getBoundingClientRect: () => DOMRect | null;
};

type LocatorHandleElement = {
  enableInteraction: () => void;
  disableInteraction: () => void;
  draw: () => void;
};

type FloatRangeInputElement = {
  setValue: (value: number) => void;
};

type FloatingUserInputElement = {
  setValue: (value: string) => void;
  isEditing: () => boolean;
};

type TrackInfoElement = {
  getBoundingClientRect: () => DOMRect | null;
  scrollIntoView: (alignToTop: boolean) => void;
};
