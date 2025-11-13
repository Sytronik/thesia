import { invoke } from "@tauri-apps/api/core";
import {createWasmFloat32Array, WasmFloat32Array} from "./wasm-wrapper";

export async function getChannelCounts(trackId: number): Promise<1 | 2> {
  const ch = await invoke<number>("get_channel_counts", {trackId});
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

export interface AudioFormatInfo {
  name: string
  sampleRate: number
  bitDepth: string
  bitrate: string
}

export type FreqScale = 'Linear' | 'Mel';

export type GuardClippingMode = 'Clip' | 'ReduceGlobalLevel' | 'Limiter';

export interface PlayerState {
  isPlaying: boolean
  positionSec: number
  err: string
}

export interface SpecSetting {
  winMillisec: number
  tOverlap: number
  fOverlap: number
  freqScale: FreqScale
}

export interface Spectrogram {
  buf: Float32Array
  width: number
  height: number
  startSec: number
  pxPerSec: number
  leftMargin: number
  rightMargin: number
  topMargin: number
  bottomMargin: number
  isLowQuality: boolean
}

export interface UserSettings {
  specSetting: SpecSetting
  blend: number
  dBRange: number
  commonGuardClipping: GuardClippingMode
  commonNormalize: any
}

export interface UserSettingsOptionals {
  specSetting?: SpecSetting
  blend?: number
  dBRange?: number
  commonGuardClipping?: GuardClippingMode
  commonNormalize?: any
}

/* draw tracks */
/* time axis */
export async function getTimeAxisMarkers(
  subTickSec: number,
  subTickUnitCount: number,
  markerDrawOptions?: MarkerDrawOption,
): Promise<Markers> {
  const {startSec, endSec, maxSec} = markerDrawOptions || {};

  if (startSec === undefined || endSec === undefined || maxSec === undefined) {
    console.error("no markerDrawOptions for time axis exist");
    return [];
  }
  return invoke<Markers>(
    "get_time_axis_markers",
    {
      startSec,
      endSec,
      tickUnit: subTickSec,
      labelInterval: subTickUnitCount,
      maxSec,
    },
  );
}

/* track axis */
export async function getFreqAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  markerDrawOptions?: MarkerDrawOption,
): Promise<Markers> {
  const {maxTrackHz, hzRange} = markerDrawOptions || {};

  if (maxTrackHz === undefined || hzRange === undefined) {
    console.error("no markerDrawOptions for freq axis exist");
    return [];
  }
  return invoke<Markers>(
    "get_freq_axis_markers",
    { maxNumTicks, maxNumLabels, hzRange, maxTrackHz },
  );
}

export async function getAmpAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  markerDrawOptions?: MarkerDrawOption,
): Promise<Markers> {
  const {ampRange} = markerDrawOptions || {};

  if (!ampRange) {
    console.error("no markerDrawOption for amp axis exist");
    return [];
  }

  return invoke<Markers>(
    "get_amp_axis_markers",
    { maxNumTicks, maxNumLabels, ampRange },
  );
}

/* dB Axis */

export async function getdBAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  markerDrawOptions?: MarkerDrawOption,
): Promise<Markers> {
  const {mindB, maxdB} = markerDrawOptions || {};

  if (mindB === undefined || maxdB === undefined) {
    console.error("no markerDrawOptions for dB axis exist");
    return [];
  }

  return invoke<Markers>(
    "get_dB_axis_markers",
    { maxNumTicks, maxNumLabels, minDB: mindB, maxDB: maxdB },
  );
}

// IdChannel is form of id#_ch#
export type IdChannel = string;
export type IdChArr = IdChannel[];
export type Spectrograms = {
  [key: IdChannel]: Spectrogram;
};

export type WavInfo = {
  wavWasmArr: WasmFloat32Array;
  sr: number;
  isClipped: boolean;
};

export async function getWav(idChStr: string): Promise<WavInfo | null> {
  const wavInfo = await invoke<any | null>("get_wav", {idChStr});
  if (!wavInfo) return null;

  const {wav, sr, isClipped} = wavInfo;
  const [wavWasmArr, view] = createWasmFloat32Array(wav.length);
  view.set(wav);
  return {wavWasmArr, sr, isClipped};
}

export async function getLimiterGainSeq(trackId: number): Promise<WasmFloat32Array | null> {
  const gainSeq = await invoke<number[] | null>("get_limiter_gain", {trackId});
  if (gainSeq === null) return null;
  const [gainSeqWasmArr, view] = createWasmFloat32Array(gainSeq.length);
  view.set(gainSeq);
  return gainSeqWasmArr;
}

export const NormalizeOnTypeValues = ["LUFS", "RMSdB", "PeakdB"] as const;
export type NormalizeOnType = (typeof NormalizeOnTypeValues)[number];
export type NormalizeTarget =
  | {type: "Off"}
  | {
      type: NormalizeOnType;
      target: number;
    };

export async function getCommonNormalize(): Promise<NormalizeTarget> {
  const commonNormalize = await invoke<NormalizeTarget>("get_common_normalize");
  return commonNormalize;
}

export async function setCommonNormalize(commonNormalize: NormalizeTarget): Promise<void> {
  return invoke<void>("set_common_normalize", {commonNormalize});
}

export async function getGuardClipStats(trackId: number): Promise<[number, string][]> {
  // [channel, stats]
  // if [[-1, stats]], then all channels have the same stats
  return invoke<[number, string][]>("get_guard_clip_stats", {trackId});
}

export async function getPlayerState(): Promise<PlayerState> {
  return invoke<PlayerState>("get_player_state");
}

export async function init(
  userSettings: UserSettingsOptionals,
  maxTextureSize: number,
  tempDirectory: string,
): Promise<UserSettings> {
  return invoke<UserSettings>("init", {
    userSettings,
    maxSpectrogramSize: maxTextureSize,
    tmpDirPath: tempDirectory,
  });
}

export async function addTracks(trackIds: number[], paths: string[]): Promise<number[]> {
  return invoke<number[]>("add_tracks", {trackIds, paths});
}

export async function reloadTracks(trackIds: number[]): Promise<number[]> {
  return invoke<number[]>("reload_tracks", {trackIds});
}

export async function removeTracks(trackIds: number[]): Promise<void> {
  return invoke<void>("remove_tracks", {trackIds});
}

export async function applyTrackListChanges(): Promise<string[]> {
  return invoke<string[]>("apply_track_list_changes");
}

export async function getdBRange(): Promise<number> {
  return invoke<number>("get_dB_range");
}

export async function setdBRange(dBRange: number): Promise<void> {
  return invoke<void>("set_dB_range", {dBRange});
}

export async function setColormapLength(colormapLength: number): Promise<void> {
  return invoke<void>("set_colormap_length", {colormapLength});
}

export async function getSpecSetting(): Promise<SpecSetting> {
  return invoke<SpecSetting>("get_spec_setting");
}

export async function setSpecSetting(specSetting: SpecSetting): Promise<void> {
  return invoke<void>("set_spec_setting", {specSetting});
}

export async function getCommonGuardClipping(): Promise<GuardClippingMode> {
  return invoke<GuardClippingMode>("get_common_guard_clipping");
}

export async function setCommonGuardClipping(commonGuardClipping: GuardClippingMode): Promise<void> {
  return invoke<void>("set_common_guard_clipping", {commonGuardClipping});
}

export async function getSpectrogram(
  idChStr: string,
  secRange: [number, number],
  hzRange: [number, number],
  marginPx: number,
): Promise<Spectrogram | null> {
  const out = await invoke<any>(
    "get_spectrogram",
    { idChStr, secRange, hzRange, marginPx },
  );
  if (!out) return null;
  out.buf = new Float32Array(out.buf);
  return out;
}

export async function findIdByPath(path: string): Promise<number> {
  return invoke<number>("find_id_by_path", {path});
}

export async function freqPosToHz(
  y: number,
  height: number,
  hzRange: [number, number],
): Promise<number> {
  return invoke<number>("freq_pos_to_hz", {y, height, hzRange});
}

export async function freqHzToPos(
  hz: number,
  height: number,
  hzRange: [number, number],
): Promise<number> {
  return invoke<number>("freq_hz_to_pos", {hz, height, hzRange});
}

export async function secondsToLabel(sec: number): Promise<string> {
  return invoke<string>("seconds_to_label", {sec});
}

export async function timeLabelToSeconds(label: string): Promise<number> {
  return invoke<number>("time_label_to_seconds", {label});
}

export async function hzToLabel(hz: number): Promise<string> {
  return invoke<string>("hz_to_label", {hz});
}

export async function freqLabelToHz(label: string): Promise<number> {
  return invoke<number>("freq_label_to_hz", {label});
}

export async function getMaxdB(): Promise<number> {
  return invoke<number>("get_max_dB");
}

export async function getMindB(): Promise<number> {
  return invoke<number>("get_min_dB");
}

export async function getMaxTrackHz(): Promise<number> {
  return invoke<number>("get_max_track_hz");
}

export async function getLongestTrackLengthSec(): Promise<number> {
  return invoke<number>("get_longest_track_length_sec");
}

export async function getLengthSec(trackId: number): Promise<number> {
  return invoke<number>("get_length_sec", {trackId});
}

export async function getSampleRate(trackId: number): Promise<number> {
  return invoke<number>("get_sample_rate", {trackId});
}

export async function getFormatInfo(trackId: number): Promise<AudioFormatInfo> {
  const out = await invoke<any>("get_format_info", {trackId});
  return {
    name: out.name,
    sampleRate: out.sr,
    bitDepth: out.bitDepth,
    bitrate: out.bitrate,
  };
}

export async function getGlobalLUFS(trackId: number): Promise<number> {
  return invoke<number>("get_global_lufs", {trackId});
}

export async function getRMSdB(trackId: number): Promise<number> {
  return invoke<number>("get_rms_dB", {trackId});
}

export async function getMaxPeakdB(trackId: number): Promise<number> {
  return invoke<number>("get_max_peak_dB", {trackId});
}

export async function getPath(trackId: number): Promise<string> {
  return invoke<string>("get_path", {trackId});
}

export async function getFileName(trackId: number): Promise<string> {
  return invoke<string>("get_file_name", {trackId});
}

export async function setVolumedB(volumedB: number): Promise<void> {
  return invoke<void>("set_volume_dB", {volumeDB: volumedB});
}

export async function setTrackPlayer(trackId: number, sec?: number): Promise<void> {
  return invoke<void>("set_track_player", {trackId, sec});
}

export async function seekPlayer(sec: number): Promise<void> {
  return invoke<void>("seek_player", {sec});
}

export async function pausePlayer(): Promise<void> {
  return invoke<void>("pause_player");
}

export async function resumePlayer(): Promise<void> {
  return invoke<void>("resume_player");
}
