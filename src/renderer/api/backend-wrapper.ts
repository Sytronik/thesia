import backend from "backend";

export {GuardClippingMode, FreqScale, SpecSetting} from "backend";

backend.init();

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
  ampRange?: [number, number];
};

/* draw tracks */
/* time axis */
export async function getTimeAxisMarkers(
  subTickSec: number,
  subTickUnitCount: number,
  markerDrawOptions: MarkerDrawOption,
): Promise<Markers> {
  const {startSec, endSec} = markerDrawOptions || {};

  if (startSec === undefined || endSec === undefined) {
    console.error("no start sec of px per sec value exist");
    return [];
  }
  return backend.getTimeAxisMarkers(startSec, endSec, subTickSec, subTickUnitCount);
}

/* track axis */
export async function getFreqAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
): Promise<Markers> {
  return backend.getFreqAxisMarkers(maxNumTicks, maxNumLabels);
}

export async function getAmpAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  markerDrawOptions: MarkerDrawOption,
): Promise<Markers> {
  const {ampRange} = markerDrawOptions || {};

  if (!ampRange) {
    console.error("no draw option for wav exist");
    return [];
  }

  return backend.getAmpAxisMarkers(maxNumTicks, maxNumLabels, ampRange);
}

/* dB Axis */

export async function getdBAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
): Promise<Markers> {
  return backend.getdBAxisMarkers(maxNumTicks, maxNumLabels);
}

// IdChannel is form of id#_ch#
export type IdChannel = string;
export type SpecWavImages = {
  [key: IdChannel]: Buffer;
};

/* images */
export function getImages(): SpecWavImages {
  return backend.getImages();
}

// written in snake case for compatibility with native api
export type DrawOptionForWav = {
  amp_range: [number, number];
  dpr: number;
};

export async function setImageState(
  idChArr: string[],
  startSec: number,
  width: number,
  height: number,
  pxPerSec: number,
  drawOptionForWav: DrawOptionForWav,
  blend: number,
) {
  return backend.setImageState(
    idChArr,
    startSec,
    width,
    {pxPerSec, height},
    drawOptionForWav,
    blend,
  );
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

export type PlayerState = {
  err: string;
  isPlaying: boolean;
  positionSec: number;
};

export function getPlayerState(): PlayerState {
  return backend.getPlayerState();
}

export const {
  addTracks,
  reloadTracks,
  removeTracks,
  applyTrackListChanges,
  findIdByPath,
  getPath,
  getFileName,
  getLengthSec,
  getSampleRate,
  getSampleFormat,
  getGlobalLUFS,
  getRMSdB,
  getMaxPeakdB,
  getLongestTrackLengthSec,
  freqPosToHzOnCurrentRange,
  freqPosToHz,
  freqHzToPos,
  secondsToLabel,
  hzToLabel,
  freqLabelToHz,
  getMaxdB,
  getMindB,
  getMaxTrackHz,
  getColorMap,
  getOverview,
  getdBRange,
  setdBRange,
  getHzRange,
  setHzRange,
  getSpecSetting,
  setSpecSetting,
  getCommonGuardClipping,
  setCommonGuardClipping,
  getGuardClipStats,
  setVolumedB,
  setTrackPlayer,
  pausePlayer,
  resumePlayer,
  seekPlayer,
} = backend;
