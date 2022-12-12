// id & channel combination
type IdChannel = string;
type IdChArr = IdChannel[];

// written in snake case for compatibility with native api
type DrawOption = {
  px_per_sec: number;
  height: number;
};
type DrawOptionForWav = {
  min_amp: number;
  max_amp: number;
};
type SpecWavImages = {
  [key: string]: ArrayBuffer;
};

type TickPxPosition = number;
type TickLable = string;
type Markers = [TickPxPosition, TickLable][];
