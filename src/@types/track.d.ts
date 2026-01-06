type SpecSetting = import("../api").SpecSetting;
type GuardClippingMode = import("../api").GuardClippingMode;
type NormalizeTarget = import("../api").NormalizeTarget;
type Markers = import("../api").Markers;
type IdChannel = import("../api").IdChannel;
type IdChArr = import("../api").IdChArr;
type IdChMap = Map<number, IdChArr>;
type Mipmap = import("../api/wasm-wrapper").Mipmap;
type WavMetadata = import("../api").WavMetadata;

type MouseOrKeyboardEvent = MouseEvent | KeyboardEvent | React.MouseEvent | React.KeyboardEvent;

type OptionalLensParams = { startSec?: number; pxPerSec?: number };

// Track Summary
type TrackSummaryData = {
  fileName: string;
  time: string;
  formatName: string;
  bitDepth: string;
  bitrate: string;
  sampleRate: string;
  globalLUFS: string;
  guardClipStats: [number, string][];
};

// Axis Tick
type TickScaleTable = {
  [key: number]: [number, number];
};

type MarkerPosition = {
  MAJOR_TICK_POS: number;
  MINOR_TICK_POS: number;
  LABEL_POS: number; // distance from the axis line to the label
  LABEL_ADJUSTMENT: number; // distance from the marker position to the label
};

type VScrollAnchorInfo = {
  clientY: number;
  imgIndex: number;
  cursorRatioOnImg: number;
  cursorOffset: number;
};

type SplitViewHandleElement = {
  getBoundingClientRect: () => DOMRect | null;
  scrollTo: (option: ScrollToOptions) => void;
  scrollTop: () => number;
  hasScrollBar: () => boolean;
};

type ImgCanvasHandleElement = {
  getBoundingClientRect: () => DOMRect;
};

type AxisCanvasHandleElement = {
  getBoundingClientRect: () => DOMRect | null;
};

type LocatorHandleElement = {
  isOnLocator: (clientX: number) => boolean;
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
