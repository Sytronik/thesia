type Markers = import("../api").Markers;
type IdChannel = import("../api").IdChannel;
type IdChArr = IdChannel[];
type IdChMap = Map<number, IdChArr>;

type OptionalLensParams = {startSec?: number; pxPerSec?: number};

type SplitViewHandleElement = {
  getBoundingClientRect: () => DOMRect | null;
  scrollTo: (option: ScrollToOptions) => void;
  scrollTop: () => number;
};

type ImgCanvasHandleElement = {
  draw: (buf: Buffer) => void;
  updateLensParams: (params: OptionalLensParams) => void;
  getBoundingClientRect: () => DOMRect;
};

// Track Summary
type TrackSummaryData = {
  fileName: string;
  time: string;
  sampleFormat: string;
  sampleRate: string;
  globalLUFS: string;
};

// Axis Tick
type TickScaleTable = {
  [key: number]: number[];
};

type AxisCanvasHandleElement = {
  draw: (markersAndLength: [Markers, number], forced?: boolean) => void;
};

type OverviewHandleElement = {
  draw: (startSec: number, lensDurationSec: number, forced?: boolean) => Promise<void>;
};

type VScrollAnchorInfo = {imgIndex: number; cursorRatioOnImg: number; cursorOffset: number};

type FloatRangeInputElement = {
  setValue: (value: number) => void;
};

type TrackInfoElement = {
  getBoundingClientRect: () => DOMRect | null;
};
