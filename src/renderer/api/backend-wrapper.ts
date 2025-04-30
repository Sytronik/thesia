import backend from "backend";

export {GuardClippingMode, FreqScale, SpecSetting} from "backend";

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
  const {maxTrackHz} = markerDrawOptions || {};

  if (maxTrackHz === undefined) {
    console.error("no markerDrawOptions for freq axis exist");
    return [];
  }
  return backend.getFreqAxisMarkers(maxNumTicks, maxNumLabels, maxTrackHz);
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
export type SpecWavImages = {
  [key: IdChannel]: {
    buf: Buffer;
    width: number;
    height: number;
  };
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
  init,
  addTracks,
  reloadTracks,
  removeTracks,
  applyTrackListChanges,
  setImageState,
  findIdByPath,
  getPath,
  getFileName,
  getLengthSec,
  getFormatInfo,
  getGlobalLUFS,
  getRMSdB,
  getMaxPeakdB,
  getLongestTrackLengthSec,
  freqPosToHzOnCurrentRange,
  freqPosToHz,
  freqHzToPos,
  secondsToLabel,
  timeLabelToSeconds,
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
