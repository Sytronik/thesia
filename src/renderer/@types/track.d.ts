type SpecSetting = import("../api").SpecSetting;
type GuardClippingMode = import("../api").GuardClippingMode;
type NormalizeTarget = import("../api").NormalizeTarget;
type Markers = import("../api").Markers;
type IdChannel = import("../api").IdChannel;
type IdChArr = import("../api").IdChArr;
type IdChMap = Map<number, IdChArr>;
type Spectrogram = import("../api").Spectrogram;
type Spectrograms = import("../api").Spectrograms;

type MouseOrKeyboardEvent = MouseEvent | KeyboardEvent | React.MouseEvent | React.KeyboardEvent;

type OptionalLensParams = {startSec?: number; pxPerSec?: number};

type SplitViewHandleElement = {
  getBoundingClientRect: () => DOMRect | null;
  scrollTo: (option: ScrollToOptions) => void;
  scrollTop: () => number;
};

type ImgCanvasHandleElement = {
  getBoundingClientRect: () => DOMRect;
};

type LocatorHandleElement = {
  enableInteraction: () => void;
  disableInteraction: () => void;
  draw: () => void;
};

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
  [key: number]: number[];
};

type AxisCanvasHandleElement = {
  draw: (markersAndLength: [Markers, number], forced?: boolean) => void;
  getBoundingClientRect: () => DOMRect | null;
};

type OverviewHandleElement = {
  draw: (startSec: number, lensDurationSec: number, forced?: boolean) => Promise<void>;
};

type VScrollAnchorInfo = {imgIndex: number; cursorRatioOnImg: number; cursorOffset: number};

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

type AxisKind = "timeRuler" | "ampAxis" | "freqAxis" | "dBAxis";
