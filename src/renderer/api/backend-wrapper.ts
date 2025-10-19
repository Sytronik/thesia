import backend, {
  Spectrogram,
  WavInfo as _WavInfo,
  WavDrawingInfo as _WavDrawingInfo,
} from "backend";
import {
  OVERVIEW_CH_GAP_HEIGHT,
  OVERVIEW_GAIN_HEIGHT_RATIO,
  OVERVIEW_LINE_WIDTH_FACTOR,
  WAV_TOPBOTTOM_CONTEXT_SIZE,
} from "renderer/prototypes/constants/tracks";

export {GuardClippingMode, FreqScale, SpecSetting, Spectrogram} from "backend";

// most api returns empty array for edge case
/* get each track file's information */
export function getChannelCounts(trackId: number): 1 | 2 {
  const ch = backend.getChannelCounts(trackId);
  if (!(ch === 1 || ch === 2)) console.error(`No. of channel ${ch} not supported!`);
  if (ch >= 1.5) return 2;
  return 1;
}

export type TickPxPosition = number;
export type TickLabel = string;
export type Markers = [TickPxPosition, TickLabel][];
export type MarkerDrawOption = {
  startSec?: number;
  endSec?: number;
  maxSec?: number;
  hzRange?: [number, number];
  maxTrackHz?: number;
  ampRange?: [number, number];
  mindB?: number;
  maxdB?: number;
};

/* draw tracks */
/* time axis */
export function getTimeAxisMarkers(
  subTickSec: number,
  subTickUnitCount: number,
  markerDrawOptions?: MarkerDrawOption,
): Markers {
  const {startSec, endSec, maxSec} = markerDrawOptions || {};

  if (startSec === undefined || endSec === undefined || maxSec === undefined) {
    console.error("no markerDrawOptions for time axis exist");
    return [];
  }
  return backend.getTimeAxisMarkers(startSec, endSec, subTickSec, subTickUnitCount, maxSec);
}

/* track axis */
export function getFreqAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  markerDrawOptions?: MarkerDrawOption,
): Markers {
  const {maxTrackHz, hzRange} = markerDrawOptions || {};

  if (maxTrackHz === undefined || hzRange === undefined) {
    console.error("no markerDrawOptions for freq axis exist");
    return [];
  }
  return backend.getFreqAxisMarkers(maxNumTicks, maxNumLabels, hzRange, maxTrackHz);
}

export function getAmpAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  markerDrawOptions?: MarkerDrawOption,
): Markers {
  const {ampRange} = markerDrawOptions || {};

  if (!ampRange) {
    console.error("no markerDrawOption for amp axis exist");
    return [];
  }

  return backend.getAmpAxisMarkers(maxNumTicks, maxNumLabels, ampRange);
}

/* dB Axis */

export function getdBAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  markerDrawOptions?: MarkerDrawOption,
): Markers {
  const {mindB, maxdB} = markerDrawOptions || {};

  if (mindB === undefined || maxdB === undefined) {
    console.error("no markerDrawOptions for dB axis exist");
    return [];
  }

  return backend.getdBAxisMarkers(maxNumTicks, maxNumLabels, mindB, maxdB);
}

// IdChannel is form of id#_ch#
export type IdChannel = string;
export type IdChArr = IdChannel[];
export type Spectrograms = {
  [key: IdChannel]: Spectrogram;
};

export type WavInfo = {
  wav: Float32Array;
  sr: number;
  isClipped: boolean;
};

function convertWavInfo(info: _WavInfo): WavInfo {
  return {wav: new Float32Array(info.wav), sr: info.sr, isClipped: info.isClipped};
}

export async function getWav(idChStr: string): Promise<WavInfo | null> {
  const info = await backend.getWav(idChStr);
  return convertWavInfo(info);
}

export type WavDrawingInfo = {
  line: Float32Array | null;
  topEnvelope: Float32Array | null;
  bottomEnvelope: Float32Array | null;
  startSec: number;
  pointsPerSec: number;
  preMargin: number;
  postMargin: number;
  clipValues: [number, number] | null;
};

function convertWavDrawingInfo(info: _WavDrawingInfo): WavDrawingInfo {
  let line = null;
  let topEnvelope = null;
  let bottomEnvelope = null;
  let clipValues: [number, number] | null = null;
  if (info.line) line = new Float32Array(info.line.buffer);
  if (info.topEnvelope) topEnvelope = new Float32Array(info.topEnvelope.buffer);
  if (info.bottomEnvelope) bottomEnvelope = new Float32Array(info.bottomEnvelope.buffer);
  if (info.clipValues) clipValues = [info.clipValues[0], info.clipValues[1]];

  return {...info, line, topEnvelope, bottomEnvelope, clipValues};
}

export async function getWavDrawingInfo(
  idChStr: string,
  secRange: [number, number],
  width: number,
  height: number,
  ampRange: [number, number],
  wavStrokeWidth: number,
  devicePixelRatio: number,
  marginRatio: number,
): Promise<WavDrawingInfo | null> {
  const info = await backend.getWavDrawingInfo(
    idChStr,
    secRange,
    width * devicePixelRatio,
    height * devicePixelRatio,
    ampRange,
    wavStrokeWidth * devicePixelRatio,
    WAV_TOPBOTTOM_CONTEXT_SIZE * devicePixelRatio,
    marginRatio,
  );
  if (!info) return null;
  return convertWavDrawingInfo(info);
}

export type OverviewDrawingInfo = {
  chDrawingInfos: WavDrawingInfo[];
  limiterGainTopInfo: WavDrawingInfo | null;
  limiterGainBottomInfo: WavDrawingInfo | null;
  scaledChHeight: number;
  scaledGapHeight: number;
  scaledLimiterGainHeight: number;
  scaledChWoGainHeight: number;
};

export async function getOverviewDrawingInfo(
  trackId: number,
  width: number,
  height: number,
  devicePixelRatio: number,
): Promise<OverviewDrawingInfo | null> {
  const info = await backend.getOverviewDrawingInfo(
    trackId,
    width * devicePixelRatio,
    height * devicePixelRatio,
    OVERVIEW_CH_GAP_HEIGHT * devicePixelRatio,
    OVERVIEW_GAIN_HEIGHT_RATIO,
    OVERVIEW_LINE_WIDTH_FACTOR * devicePixelRatio,
    WAV_TOPBOTTOM_CONTEXT_SIZE * devicePixelRatio,
  );
  if (!info) return null;
  return {
    chDrawingInfos: info.chDrawingInfos.map(convertWavDrawingInfo),
    limiterGainTopInfo: info.limiterGainTopInfo
      ? convertWavDrawingInfo(info.limiterGainTopInfo)
      : null,
    limiterGainBottomInfo: info.limiterGainBottomInfo
      ? convertWavDrawingInfo(info.limiterGainBottomInfo)
      : null,
    scaledChHeight: info.chHeight,
    scaledGapHeight: info.gapHeight,
    scaledLimiterGainHeight: info.limiterGainHeight,
    scaledChWoGainHeight: info.chWoGainHeight,
  };
}

export const NormalizeOnTypeValues = ["LUFS", "RMSdB", "PeakdB"] as const;
export type NormalizeOnType = (typeof NormalizeOnTypeValues)[number];
export type NormalizeTarget =
  | {type: "Off"}
  | {
      type: NormalizeOnType;
      target: number;
    };

export function getCommonNormalize(): NormalizeTarget {
  return backend.getCommonNormalize();
}

export async function setCommonNormalize(commonNormalize: NormalizeTarget): Promise<void> {
  return backend.setCommonNormalize(commonNormalize);
}

export function getGuardClipStats(trackId: number): [number, string][] {
  // [channel, stats]
  // if [[-1, stats]], then all channels have the same stats
  return backend.getGuardClipStats(trackId);
}

export type PlayerState = {
  err: string;
  isPlaying: boolean;
  positionSec: number;
};

export function getPlayerState(): PlayerState {
  return backend.getPlayerState();
}

export const {
  init,
  addTracks,
  reloadTracks,
  removeTracks,
  applyTrackListChanges,
  findIdByPath,
  getPath,
  getFileName,
  getLengthSec,
  getFormatInfo,
  getGlobalLUFS,
  getRMSdB,
  getMaxPeakdB,
  getLongestTrackLengthSec,
  freqPosToHz,
  freqHzToPos,
  secondsToLabel,
  timeLabelToSeconds,
  hzToLabel,
  freqLabelToHz,
  getMaxdB,
  getMindB,
  getMaxTrackHz,
  getSpectrogram,
  getdBRange,
  setdBRange,
  setColormapLength,
  getSampleRate,
  getSpecSetting,
  setSpecSetting,
  getCommonGuardClipping,
  setCommonGuardClipping,
  setVolumedB,
  setTrackPlayer,
  pausePlayer,
  resumePlayer,
  seekPlayer,
} = backend;
