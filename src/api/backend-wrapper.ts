import {createWasmFloat32Array, WasmFloat32Array} from "./wasm-wrapper";

// Import types from local types file (works even when backend type definitions are not available)
import type { Spectrogram, PlayerState } from "./backend-types";
// Note: GuardClippingMode and FreqScale types are exported from backend-types
// We don't import them here to avoid conflicts with enum values

import * as backend from "./backend-mock";

// Re-export types from local types file
// Note: GuardClippingMode and FreqScale types are available from backend-types
// but we don't re-export them here to avoid conflicts with enum values
export type {
  SpecSetting,
  Spectrogram,
  WavMetadata,
  AudioFormatInfo,
  UserSettings,
  UserSettingsOptionals,
  PlayerState,
} from "./backend-types";

export { GuardClippingMode, FreqScale } from "./backend-types";
// Re-export enum values (needed for runtime usage)
// Use backend's enum values if available, otherwise use mock's
// Note: Enum types can be imported directly from "./backend-types" if needed

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
  wav: WasmFloat32Array;
  sr: number;
  isClipped: boolean;
};

export function getWav(idChStr: string): WavInfo | null {
  const metadata = backend.getWavMetadata(idChStr);
  if (!metadata) return null;

  const {length, sr, isClipped} = metadata;
  const [wav, view] = createWasmFloat32Array(length);
  backend.assignWavTo(view, idChStr);
  return {wav, sr, isClipped};
}

export function getLimiterGainSeq(trackId: number): WasmFloat32Array | null {
  const length = backend.getLimiterGainLength(trackId);
  if (length === 0) return null;
  const [wav, view] = createWasmFloat32Array(length);
  backend.assignLimiterGainTo(view, trackId);
  return wav;
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
