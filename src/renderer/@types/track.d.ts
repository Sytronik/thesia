// array of id & channel combination
type IdChArr = string[];

// written in snake case for compatibility with native api
type DrawOption = {
  px_per_sec: number;
  height: number;
};
type DrawOptionForWav = {
  min_amp: number;
  max_amp: number;
};
type SpecWavImage = [IdChArr, ArrayBuffer];

type TickPxPosition = number;
type TickLable = string;
type Markers = [TickPxPosition, TickLable][];
